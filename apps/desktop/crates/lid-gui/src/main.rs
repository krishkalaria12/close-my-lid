//! Close My Lid tray app for Windows.
//!
//! Built on adabraka-gpui, a fork of Zed's GPUI that adds the tray,
//! notification and daemon-mode APIs GPUI itself does not provide. The version
//! is pinned exactly because gpui is pre-1.0 and breaks between minor
//! releases; see `apps/desktop/README.md`.
//!
//! macOS has its own AppKit app in the `lid-macos` crate; both are shells over
//! the same `lidcore`.
//!
//! Hides the console window on Windows release builds.
#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

// The tray app is Windows-only. Linux has no tray to anchor to — the CLI is
// the product there — and macOS has `lid-macos`. `Cargo.toml` intentionally
// provides no gpui dependency for either, so fail here with a clear message
// instead of a wall of missing-crate errors.
#[cfg(not(target_os = "windows"))]
compile_error!(
    "lid-gui is Windows-only; use `lid-macos` on macOS and the `close-my-lid` CLI on Linux"
);

mod anchor;
mod config;
mod error;
mod panel;
mod state;
mod theme;

use gpui::single_instance::{SingleInstance, send_activate_to_existing};
use gpui::{
    App, AppContext, Application, Bounds, TitlebarOptions, TrayIconEvent, TrayMenuItem, Window,
    WindowBackgroundAppearance, WindowBounds, WindowKind, WindowOptions, px, size,
};
use lidcore::{APP_ID, APP_NAME, SessionDuration};

use crate::config::action;
use crate::panel::Panel;
use crate::state::AppState;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into()),
        )
        .init();

    // A second hold would fight the first over the same global power setting,
    // so only one instance may run.
    let _instance = match SingleInstance::acquire(APP_ID) {
        Ok(instance) => instance,
        Err(_) => {
            let _ = send_activate_to_existing(APP_ID);
            return;
        }
    };

    Application::new().run(|cx: &mut App| {
        // The app is a tray app: it must outlive its windows.
        cx.set_keep_alive_without_windows(true);

        let state = cx.new(|_| AppState::new());

        setup_tray(cx);
        wire_tray_actions(state.clone(), cx);
        start_supervisor(state.clone(), cx);
    });
}

fn setup_tray(cx: &mut App) {
    cx.set_tray_tooltip(APP_NAME);

    // On Windows the icon lands in the taskbar overflow by default, so most
    // users will not see it until they pin it. Worth surfacing in onboarding.
    // TODO: ship 16x16 light and dark .ico assets and pass them here; unlike
    // macOS template images, Windows does not auto-invert for the taskbar theme.
    cx.set_tray_icon(None);

    // Clicking the icon should open the panel rather than the context menu.
    // Not yet implemented on the Windows backend in 0.5.1, so the menu below
    // remains the reliable path.
    cx.set_tray_panel_mode(true);

    let mut items = vec![
        TrayMenuItem::Action {
            label: "Open Panel".into(),
            id: action::PANEL.into(),
        },
        TrayMenuItem::Separator,
    ];
    for duration in SessionDuration::PRESETS {
        items.push(TrayMenuItem::Action {
            label: format!("Hold for {}", duration.label()).into(),
            id: format!("{}{}", action::HOLD_PREFIX, duration.label()).into(),
        });
    }
    items.push(TrayMenuItem::Separator);
    items.push(TrayMenuItem::Action {
        label: "Stop Holding".into(),
        id: action::STOP.into(),
    });
    items.push(TrayMenuItem::Separator);
    items.push(TrayMenuItem::Action {
        label: "Quit".into(),
        id: action::QUIT.into(),
    });

    cx.set_tray_menu(items);
}

fn wire_tray_actions(state: gpui::Entity<AppState>, cx: &mut App) {
    let panel_state = state.clone();
    cx.on_tray_icon_event(move |event, cx| {
        if matches!(event, TrayIconEvent::LeftClick) {
            open_panel(panel_state.clone(), cx);
        }
    });

    cx.on_tray_menu_action(move |id, cx| match id.as_ref() {
        action::PANEL => open_panel(state.clone(), cx),
        action::STOP => {
            let headline: Option<String> = state.update(cx, |state, cx| {
                if let Err(error) = state.stop() {
                    tracing::error!(%error, hint = error.hint(), "could not stop the hold");
                    cx.notify();
                    return error.deserves_notification().then(|| error.headline());
                }
                cx.notify();
                None
            });
            // Release builds hide the console, so refusals must surface as a
            // notification — otherwise the click silently does nothing.
            if let Some(headline) = headline {
                let _ = cx.show_notification(APP_NAME, &headline);
            }
        }
        action::QUIT => {
            // Release before exiting; on Windows the power-scheme edit would
            // otherwise outlive the process.
            state.update(cx, |state, _| {
                let _ = state.stop();
            });
            cx.quit();
        }
        other => {
            let Some(label) = other.strip_prefix(action::HOLD_PREFIX) else {
                return;
            };
            let Some(duration) = SessionDuration::PRESETS
                .into_iter()
                .find(|preset| preset.label() == label)
            else {
                return;
            };
            let headline: Option<String> = state.update(cx, |state, cx| {
                if let Err(error) = state.start(duration) {
                    tracing::error!(%error, hint = error.hint(), "could not start the hold");
                    cx.notify();
                    return error.deserves_notification().then(|| error.headline());
                }
                cx.notify();
                None
            });
            if let Some(headline) = headline {
                let _ = cx.show_notification(APP_NAME, &headline);
            }
        }
    });
}

fn open_panel(state: gpui::Entity<AppState>, cx: &mut App) {
    // Scan for agents only when the panel is about to be shown, so the process
    // walk never runs for a UI nobody can see.
    state.update(cx, |state, cx| {
        state.refresh_readouts();
        cx.notify();
    });

    let bounds: Bounds<_> =
        anchor::panel_bounds(size(px(config::PANEL_WIDTH), px(config::PANEL_HEIGHT)), cx);

    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        kind: WindowKind::PopUp,
        titlebar: None::<TitlebarOptions>,
        focus: true,
        show: true,
        is_movable: false,
        window_background: WindowBackgroundAppearance::Transparent,
        ..Default::default()
    };

    if let Err(error) = cx.open_window(options, |_window: &mut Window, cx| {
        cx.new(|cx| Panel::new(state.clone(), cx))
    }) {
        let error = crate::error::GuiError::Window {
            detail: error.to_string(),
        };
        tracing::error!(%error, "could not open the panel");
    }
}

/// Drives expiry and the battery safety release regardless of whether any
/// window is open — the hold must end on time with the panel closed.
fn start_supervisor(state: gpui::Entity<AppState>, cx: &mut App) {
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor()
                .timer(config::SUPERVISION_INTERVAL)
                .await;

            let released = state.update(cx, |state, cx| {
                let released = state.tick();
                if released {
                    cx.notify();
                }
                released
            });

            match released {
                Ok(true) => {
                    let _ = cx.update(|cx| {
                        let _ =
                            cx.show_notification(APP_NAME, "Session ended; normal sleep restored.");
                    });
                }
                Ok(false) => {}
                // The entity is gone, which means the app is shutting down.
                Err(_) => break,
            }
        }
    })
    .detach();
}
