//! Close My Lid for Windows and Linux.
//!
//! A desktop app over the same core as the macOS menu bar app: start and stop
//! a hold, watch the battery and the running agents, and change settings.
//! Built on gpui-kit — Zed's GPUI plus the
//! gpui-component library — which is pinned exactly because gpui is pre-1.0
//! and breaks between minor releases; see `apps/desktop/README.md`.
//!
//! macOS has its own AppKit app in the `lid-macos` crate. This one also builds
//! and runs there, which is only for working on the interface from a Mac.
//!
//! Hides the console window on Windows release builds.
#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod agent_list;
mod config;
mod error;
mod icons;
mod overview;
mod prefs;
mod preview;
mod settings;
mod shell;
mod state;
mod system;
mod tasks;
mod theme;
mod updates;
mod widgets;

use gpui_kit::component::{Root, TitleBar};
use gpui_kit::{
    App, AppContext, Bounds, KeyBinding, Size, Window, WindowBounds, WindowOptions, px, size,
};
use lidcore::APP_ID;

use crate::shell::{KEY_CONTEXT, Quit, Shell, ShowAgents, ShowOverview, ShowSettings, ToggleHold};
use crate::state::AppState;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into()),
        )
        .init();

    // Held until the process exits; see `InstanceGuard`.
    let Some(_instance) = system::claim_single_instance() else {
        system::notify("Close My Lid is already running.");
        return;
    };

    let minimized = std::env::args().any(|arg| arg == config::MINIMIZED_ARG);

    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx: &mut App| {
            gpui_kit::init(cx);
            bind_keys(cx);

            let state = cx.new(|_| AppState::new());

            // The last line of defence: whatever ends the app — the window's
            // close button, the OS asking at sign-out — releases the hold
            // first. Quitting from the app does this itself; releasing twice
            // is a no-op.
            let on_quit = state.clone();
            cx.on_app_quit(move |cx| {
                on_quit.update(cx, |state, _| state.release_for_quit());
                async {}
            })
            .detach();

            open_window(state, minimized, cx);
        });
}

fn bind_keys(cx: &mut App) {
    let context = Some(KEY_CONTEXT);
    cx.bind_keys([
        // `secondary` is Ctrl on Windows and Linux, Command on macOS.
        KeyBinding::new("secondary-q", Quit, context),
        KeyBinding::new("secondary-enter", ToggleHold, context),
        KeyBinding::new("secondary-1", ShowOverview, context),
        KeyBinding::new("secondary-2", ShowAgents, context),
        KeyBinding::new("secondary-,", ShowSettings, context),
    ]);
}

fn open_window(state: gpui_kit::Entity<AppState>, minimized: bool, cx: &mut App) {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(window_bounds(cx))),
        window_min_size: Some(size(
            px(config::WINDOW_MIN_WIDTH),
            px(config::WINDOW_MIN_HEIGHT),
        )),
        app_id: Some(APP_ID.to_string()),
        ..TitleBar::window_options()
    };

    let opened = cx.open_window(options, |window: &mut Window, cx| {
        // gpui-component starts in its light theme; follow the system instead,
        // now and whenever it changes.
        theme::sync(window, cx);
        window.observe_window_appearance(theme::sync).detach();

        // The close button quits, and quitting releases the hold: an app
        // that held the lid with no window left to show it would be a hold
        // nobody could see or stop.
        let on_close = state.clone();
        window.on_window_should_close(cx, move |_, cx| {
            tasks::quit(&on_close, cx);
            true
        });

        let scene = preview::scene();
        if let Some(duration) = scene.hold {
            state.update(cx, |state, _| state.select_and_start(duration));
        }
        let shell = cx.new(|cx| {
            let mut shell = Shell::new(state.clone(), window, cx);
            if let Some(page) = scene.page {
                shell.show(page, cx);
            }
            shell
        });
        cx.new(|cx| Root::new(shell, window, cx))
    });

    match opened {
        Ok(handle) => {
            let handle: gpui_kit::AnyWindowHandle = handle.into();
            if minimized {
                let _ = handle.update(cx, |_, window, _| window.minimize_window());
            }
            tasks::start(&state, handle, cx);
        }
        Err(error) => {
            let error = crate::error::GuiError::Window {
                detail: error.to_string(),
            };
            tracing::error!(%error, "could not open the window");
            system::notify(&error.headline());
            cx.quit();
        }
    }
}

/// The designed size, centred, shrunk to fit a display too small for it — the
/// pages scroll rather than running off the screen.
fn window_bounds(cx: &App) -> Bounds<gpui_kit::Pixels> {
    let wanted: Size<gpui_kit::Pixels> = size(px(config::WINDOW_WIDTH), px(config::WINDOW_HEIGHT));
    let fitted = match cx.primary_display() {
        Some(display) => {
            let room = display.bounds().size
                - size(
                    px(config::DISPLAY_MARGIN * 2.0),
                    px(config::DISPLAY_MARGIN * 2.0),
                );
            size(wanted.width.min(room.width), wanted.height.min(room.height))
        }
        None => wanted,
    };
    Bounds::centered(None, fitted, cx)
}
