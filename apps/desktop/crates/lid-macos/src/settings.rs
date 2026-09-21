//! The Settings window.
//!
//! Carried over from the Swift app's `SettingsWindowController`. Three
//! controls, which is all the app has ever needed: launch at login, the one-
//! time administrator grant, and a shortcut into the system's own Battery
//! settings.

use std::rc::Rc;

use lidcore::power::macos::PmsetLidGuard;
use lidcore::{LidError, VERSION, launchd, sudoers};
use objc2::rc::Retained;
use objc2::{MainThreadOnly, msg_send};
use objc2_app_kit::{
    NSApplication, NSBackingStoreType, NSBezelStyle, NSButton, NSColor, NSControlStateValue,
    NSTextField, NSView, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize, NSString};
use tracing::warn;

use crate::app::{App, open_url};
use crate::config;
use crate::login_item;
use crate::ui::{ActionTarget, Weight, container, label, set_frame, set_text, system_font};

const WIDTH: f64 = 340.0;
const PAD: f64 = 20.0;
const CONTENT: f64 = WIDTH - PAD * 2.0;
const ROW_GAP: f64 = 14.0;
const LINE: f64 = 18.0;

pub struct SettingsWindow {
    window: Retained<NSWindow>,
    launch_toggle: Retained<NSButton>,
    grant_status: Retained<NSTextField>,
    grant_button: Retained<NSButton>,
    error: Retained<NSTextField>,
    /// Owned, never read: AppKit keeps a control's `target` without retaining
    /// it, so dropping these would leave the buttons pointing at freed memory.
    #[allow(dead_code)]
    targets: Vec<Retained<ActionTarget>>,
}

impl SettingsWindow {
    pub fn new(mtm: MainThreadMarker, app: &Rc<App>) -> Self {
        let content = container(
            mtm,
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(WIDTH, 0.0)),
        );
        let mut targets = Vec::new();

        let launch_toggle = checkbox(mtm, "Launch at Login");
        let toggle_target = ActionTarget::new(move || {
            crate::app::with_app(|app| app.toggle_launch_at_login());
        });
        wire(&launch_toggle, &toggle_target);
        targets.push(toggle_target);
        content.addSubview(&launch_toggle);

        let grant_heading = label(
            mtm,
            "Administrator Access",
            &system_font(13.0, Weight::Semibold),
            &NSColor::labelColor(),
        );
        content.addSubview(&grant_heading);

        let grant_status = wrapping_label(mtm, "");
        content.addSubview(&grant_status);

        let grant_button = push_button(mtm, "Grant Once…");
        let grant_target = ActionTarget::new(move || {
            crate::app::with_app(|app| app.toggle_admin_grant());
        });
        wire(&grant_button, &grant_target);
        targets.push(grant_target);
        content.addSubview(&grant_button);

        let battery_button = push_button(mtm, "Open Battery Settings…");
        let battery_target = ActionTarget::new(move || open_url(config::BATTERY_SETTINGS_URL));
        wire(&battery_button, &battery_target);
        targets.push(battery_target);
        content.addSubview(&battery_button);

        let error = wrapping_label(mtm, "");
        error.setTextColor(Some(&NSColor::systemRedColor()));
        error.setHidden(true);
        content.addSubview(&error);

        let version = label(
            mtm,
            &format!("{} {VERSION}", lidcore::APP_NAME),
            &system_font(11.0, Weight::Regular),
            &NSColor::secondaryLabelColor(),
        );
        content.addSubview(&version);

        let height = place(
            &[
                &launch_toggle,
                &grant_heading,
                &grant_status,
                &grant_button,
                &battery_button,
                &error,
                &version,
            ],
            &content,
        );

        let window: Retained<NSWindow> = unsafe {
            msg_send![
                NSWindow::alloc(mtm),
                initWithContentRect: NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(WIDTH, height)),
                styleMask: NSWindowStyleMask::Titled | NSWindowStyleMask::Closable,
                backing: NSBackingStoreType::Buffered,
                defer: true,
            ]
        };
        window.setTitle(&NSString::from_str("Close My Lid Settings"));
        // SAFETY: a closed Settings window is reopened rather than rebuilt, so
        // this `Retained` has to keep owning it after the close button is hit.
        unsafe { window.setReleasedWhenClosed(false) };
        window.setContentView(Some(&content));
        window.center();

        let _ = app;
        let settings = Self {
            window,
            launch_toggle,
            grant_status,
            grant_button,
            error,
            targets,
        };
        settings.refresh();
        settings
    }

    pub fn show(&self, mtm: MainThreadMarker) {
        self.refresh();
        // An accessory app has to activate explicitly, or its window opens
        // behind whatever the user was working in.
        NSApplication::sharedApplication(mtm).activate();
        self.window.makeKeyAndOrderFront(None);
    }

    /// Re-reads both settings from the system rather than trusting what the
    /// controls were last set to — either can change outside this window.
    pub fn refresh(&self) {
        let enabled = login_item::is_enabled();
        self.launch_toggle
            .setState(NSControlStateValue::from(isize::from(enabled)));

        let granted = sudoers::is_installed();
        set_text(
            &self.grant_status,
            if granted {
                "Granted. Holds start, end, and restore without asking for your password."
            } else {
                "Not granted yet. Every hold change asks for your password."
            },
        );
        self.grant_button.setTitle(&NSString::from_str(if granted {
            "Remove…"
        } else {
            "Grant Once…"
        }));
    }

    pub fn report(&self, error: Option<&LidError>) {
        match error {
            Some(error) => {
                set_text(&self.error, &message_for(error));
                self.error.setHidden(false);
            }
            None => self.error.setHidden(true),
        }
    }
}

impl App {
    /// Registers or unregisters the login item, reverting the control if the
    /// system refuses.
    pub fn toggle_launch_at_login(self: &Rc<Self>) {
        let enable = !login_item::is_enabled();
        let result = login_item::set_enabled(enable);
        self.report_settings(result.err().as_ref());
        self.refresh_settings();
    }

    /// Installs or removes the passwordless sudoers grant, and keeps the
    /// watchdog agent in step: without the grant it could not release
    /// anything, so it is registered alongside and removed with it.
    pub fn toggle_admin_grant(self: &Rc<Self>) {
        let installed = sudoers::is_installed();
        let result = if installed {
            PmsetLidGuard::remove_passwordless_grant().inspect(|()| launchd::uninstall())
        } else {
            PmsetLidGuard::install_passwordless_grant().inspect(|()| {
                if let Err(error) = launchd::install() {
                    warn!(%error, "grant installed but the watchdog agent was not");
                }
            })
        };

        self.report_settings(result.err().as_ref());
        self.refresh_settings();
    }

    fn refresh_settings(self: &Rc<Self>) {
        self.with_settings(|settings| settings.refresh());
    }

    fn report_settings(self: &Rc<Self>, error: Option<&LidError>) {
        self.with_settings(|settings| settings.report(error));
    }
}

/// The user-facing wording for a settings failure.
fn message_for(error: &LidError) -> String {
    match error {
        LidError::ElevationCancelled => "Administrator approval cancelled.".to_string(),
        other => other.to_string(),
    }
}

/// Stacks the rows top-down and returns the window's content height.
fn place(rows: &[&NSView], content: &NSView) -> f64 {
    let heights: Vec<f64> = rows
        .iter()
        .map(|row| {
            let natural = row.fittingSize().height;
            if natural > 0.0 { natural } else { LINE }
        })
        .collect();

    let total =
        PAD * 2.0 + heights.iter().sum::<f64>() + ROW_GAP * (rows.len() as f64 - 1.0).max(0.0);

    let mut y = total - PAD;
    for (row, height) in rows.iter().zip(&heights) {
        y -= height;
        set_frame(row, PAD, y, CONTENT, *height);
        y -= ROW_GAP;
    }

    set_frame(content, 0.0, 0.0, WIDTH, total);
    total
}

fn checkbox(mtm: MainThreadMarker, title: &str) -> Retained<NSButton> {
    unsafe {
        NSButton::checkboxWithTitle_target_action(&NSString::from_str(title), None, None, mtm)
    }
}

fn push_button(mtm: MainThreadMarker, title: &str) -> Retained<NSButton> {
    let button = unsafe {
        NSButton::buttonWithTitle_target_action(&NSString::from_str(title), None, None, mtm)
    };
    button.setBezelStyle(NSBezelStyle::Push);
    button
}

/// Explanatory text that may run to two lines, unlike the panel's labels.
fn wrapping_label(mtm: MainThreadMarker, text: &str) -> Retained<NSTextField> {
    let field = label(
        mtm,
        text,
        &system_font(11.0, Weight::Regular),
        &NSColor::secondaryLabelColor(),
    );
    field.setLineBreakMode(objc2_app_kit::NSLineBreakMode::ByWordWrapping);
    field.setPreferredMaxLayoutWidth(CONTENT);
    field
}

fn wire(control: &NSButton, target: &ActionTarget) {
    unsafe {
        control.setTarget(Some(target));
        control.setAction(Some(ActionTarget::selector()));
    }
}
