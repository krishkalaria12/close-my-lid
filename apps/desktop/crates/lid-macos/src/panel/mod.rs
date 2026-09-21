//! The menu bar panel: the app's whole interface.
//!
//! Carried over from the Swift app's `MenuBarPanelController` +
//! `MenuPanelView` pair. The views are built once and then only re-read on
//! refresh, so showing the panel is a layout pass and a few string comparisons
//! rather than a rebuild.
//!
//! The window is a borderless, non-activating `NSPanel` at the status-bar
//! level, which is what lets it appear under the menu bar icon without taking
//! focus away from whatever the user is doing.

mod compose;
mod layout;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use lidcore::{AgentHarness, SessionDuration, SleepControlState};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{DefinedClass, MainThreadOnly, Message, define_class, msg_send};
use objc2_app_kit::{
    NSBackingStoreType, NSColor, NSControlStateValue, NSEvent, NSEventMask, NSEventModifierFlags,
    NSPanel, NSScreen, NSStatusBarButton, NSSwitch, NSTextField, NSView,
    NSVisualEffectBlendingMode, NSVisualEffectMaterial, NSVisualEffectState, NSVisualEffectView,
    NSWindow, NSWindowCollectionBehavior, NSWindowDelegate, NSWindowLevel, NSWindowStyleMask,
};
use objc2_foundation::{
    MainThreadMarker, NSNotification, NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize,
};

use crate::app::App;
use crate::icons;
use crate::ui::{
    ActionTarget, BadgeView, BarView, ClickStyle, ClickView, Weight, container, label,
    monospaced_digit_font, place_trailing, rgb, separator_color, set_frame, set_text, system_font,
};

use layout::PanelHeights;
pub use layout::{ANCHOR_GAP, SCREEN_MARGIN, WIDTH};

/// Escape's virtual key code.
const KEY_ESCAPE: u16 = 53;

/// One agent's row: the parts that change when the scan comes back.
struct AgentRow {
    row: Retained<NSView>,
    detail: Retained<NSTextField>,
    dot: Retained<NSView>,
    name: Retained<NSTextField>,
}

/// One footer row: a title on the left and a shortcut or status on the right.
struct FooterRow {
    view: Retained<ClickView>,
    title: Retained<NSTextField>,
    trailing: Retained<NSTextField>,
}

/// Views the refresh pass writes to. Everything else is placed once and never
/// touched again.
struct Views {
    content: Retained<NSVisualEffectView>,
    header: Retained<NSView>,
    status: Retained<NSTextField>,
    toggle: Retained<NSSwitch>,

    battery_section: Retained<NSView>,
    battery_bar: Retained<BarView>,
    battery_level: Retained<NSTextField>,
    battery_caption: Retained<NSTextField>,

    agents_section: Retained<NSView>,
    agents_summary: Retained<NSTextField>,
    agent_rows: Vec<AgentRow>,

    hold_section: Retained<NSView>,
    pills: Vec<Retained<ClickView>>,

    footer_section: Retained<NSView>,
    updates_row: FooterRow,
    settings_row: FooterRow,
    quit_row: FooterRow,

    separators: Vec<Retained<NSView>>,

    /// Owned, never read: AppKit keeps a control's `target` without retaining
    /// it, so dropping these would leave the toggle pointing at freed memory.
    #[allow(dead_code)]
    targets: Vec<Retained<ActionTarget>>,
}

pub struct Panel {
    window: Retained<NSPanel>,
    views: Views,
    /// Which layout is currently applied, so the frames are only recomputed
    /// when the machine gains or loses a battery — that is, effectively never.
    laid_out_with_battery: Cell<bool>,
    monitors: RefCell<Vec<Retained<AnyObject>>>,
    /// Refreshes the readouts while the panel is on screen.
    ticker: RefCell<Option<Retained<objc2_foundation::NSTimer>>>,
    status_button: RefCell<Option<Retained<NSStatusBarButton>>>,
    delegate: RefCell<Option<Retained<PanelDelegate>>>,
}

define_class!(
    // SAFETY:
    // - NSPanel's only subclassing requirement is main-thread use.
    // - KeyablePanel does not implement Drop.
    #[unsafe(super(NSPanel, NSWindow))]
    #[thread_kind = MainThreadOnly]
    #[name = "CMLKeyablePanel"]
    struct KeyablePanel;

    impl KeyablePanel {
        // A borderless window refuses key status by default, which would stop
        // the panel from ever seeing a keystroke.
        #[unsafe(method(canBecomeKeyWindow))]
        fn can_become_key(&self) -> bool {
            true
        }
    }
);

pub struct DelegateIvars {
    app: RefCell<Option<Rc<App>>>,
}

define_class!(
    // SAFETY:
    // - NSObject has no subclassing requirements.
    // - PanelDelegate does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "CMLPanelDelegate"]
    #[ivars = DelegateIvars]
    pub struct PanelDelegate;

    unsafe impl NSObjectProtocol for PanelDelegate {}

    unsafe impl NSWindowDelegate for PanelDelegate {
        // Clicking anywhere else dismisses the panel, like a menu.
        #[unsafe(method(windowDidResignKey:))]
        fn window_did_resign_key(&self, _notification: &NSNotification) {
            let app = self.ivars().app.borrow().clone();
            if let Some(app) = app {
                app.close_panel();
            }
        }
    }
);

impl Panel {
    /// Builds the window and every view in it. Called once, lazily, the first
    /// time the user opens the panel.
    pub fn new(mtm: MainThreadMarker, app: &Rc<App>) -> Self {
        let window: Retained<KeyablePanel> = unsafe {
            msg_send![
                KeyablePanel::alloc(mtm),
                initWithContentRect: NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(WIDTH, 100.0)),
                styleMask: NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel,
                backing: NSBackingStoreType::Buffered,
                defer: true,
            ]
        };
        let window: Retained<NSPanel> = unsafe { Retained::cast_unchecked(window) };

        window.setOpaque(false);
        window.setBackgroundColor(Some(&NSColor::clearColor()));
        window.setHasShadow(true);
        // Above the menu bar's own windows, so the panel is never clipped.
        window.setLevel(NSWindowLevel::from(objc2_app_kit::NSStatusWindowLevel));
        window.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::FullScreenAuxiliary,
        );
        window.setMovable(false);
        window.setHidesOnDeactivate(false);

        let views = compose::build_views(mtm, app);
        window.setContentView(Some(&views.content));

        let delegate = PanelDelegate::new(mtm, app.clone());
        window.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));

        let panel = Self {
            window,
            views,
            laid_out_with_battery: Cell::new(false),
            monitors: RefCell::new(Vec::new()),
            ticker: RefCell::new(None),
            status_button: RefCell::new(None),
            delegate: RefCell::new(Some(delegate)),
        };
        // Start from the layout the machine actually needs, so the first
        // appearance is already the right height. The controller was refreshed
        // just before this, so its reading is current — taking another one
        // here would be a second IOKit query for an answer already in hand.
        panel.apply_layout(app.controller.borrow().battery().is_some());
        panel
    }

    pub fn is_visible(&self) -> bool {
        self.window.isVisible()
    }

    /// The panel's current height. Reported in debug logs, where it is the
    /// quickest confirmation that the layout ran and produced something
    /// sensible.
    pub fn height(&self) -> f64 {
        self.window.frame().size.height
    }

    /// Positions the panel under the status item and shows it.
    pub fn show(&self, button: &NSStatusBarButton, mtm: MainThreadMarker, app: &Rc<App>) {
        *self.status_button.borrow_mut() = Some(button.retain());

        let size = self.window.frame().size;
        let origin = anchor_origin(button, size, mtm);
        self.window
            .setFrame_display(NSRect::new(origin, size), true);

        self.window.orderFrontRegardless();
        self.window.makeKeyWindow();
        // The shadow is cached against the previous frame; without this a
        // resized panel draws last appearance's shadow.
        self.window.invalidateShadow();
        button.highlight(true);

        self.install_monitors(mtm, app);
        self.start_ticker(app);
    }

    pub fn close(&self) {
        self.remove_monitors();
        self.stop_ticker();
        if let Some(button) = self.status_button.borrow().as_ref() {
            button.highlight(false);
        }
        self.window.orderOut(None);
    }

    /// Re-reads everything the panel shows from the controller.
    pub fn refresh(&self, app: &Rc<App>) {
        let controller = app.controller.borrow();
        let now = chrono::Utc::now();

        set_text(&self.views.status, &controller.state().summary(now));
        let active = controller.state().is_active();
        let wanted = if active {
            NSControlStateValue::from(1isize)
        } else {
            NSControlStateValue::from(0isize)
        };
        if self.views.toggle.state() != wanted {
            self.views.toggle.setState(wanted);
        }

        let battery = controller.battery();
        if self.laid_out_with_battery.get() != battery.is_some() {
            drop(controller);
            self.apply_layout(battery.is_some());
            self.reposition();
            return self.refresh(app);
        }

        if let Some(battery) = battery {
            let policy = controller.battery_policy();
            let low = policy.should_release(battery);
            self.views
                .battery_bar
                .set_level(f64::from(battery.percentage) / 100.0, low);
            set_text(
                &self.views.battery_level,
                &format!("{}% left", battery.percentage),
            );

            let caption = if battery.is_charging {
                "charging".to_string()
            } else if low {
                "stopping to protect battery".to_string()
            } else {
                format!("stops at {}%", policy.threshold)
            };
            set_text(&self.views.battery_caption, &caption);
            self.views.battery_caption.setTextColor(Some(&*if low {
                NSColor::systemRedColor()
            } else {
                NSColor::secondaryLabelColor()
            }));
            place_trailing(
                &self.views.battery_caption,
                layout::CONTENT,
                0.0,
                layout::BODY_LINE,
            );
        }

        let mut working = 0;
        for (harness, row) in AgentHarness::ALL.iter().zip(&self.views.agent_rows) {
            let count = controller.agent_sessions(*harness);
            let busy = count > 0;
            working += usize::from(busy);

            set_text(&row.detail, &session_detail(count));
            row.detail.setTextColor(Some(&*if busy {
                NSColor::secondaryLabelColor()
            } else {
                NSColor::tertiaryLabelColor()
            }));
            row.dot.setHidden(!busy);
            // An idle agent is dimmed rather than hidden, so the list does not
            // reflow every time a session starts or stops.
            row.row.setAlphaValue(if busy { 1.0 } else { 0.55 });
            row.name.setTextColor(Some(&NSColor::labelColor()));

            let dot_edge = if busy { layout::DOT + 6.0 } else { 0.0 };
            place_trailing(
                &row.detail,
                layout::CONTENT - dot_edge,
                (layout::AGENT_ROW - layout::BODY_LINE) / 2.0,
                layout::BODY_LINE,
            );
        }
        let summary = if working == 0 {
            "all idle".to_string()
        } else {
            format!("{working} working")
        };
        set_text(&self.views.agents_summary, &summary);
        place_trailing(
            &self.views.agents_summary,
            layout::CONTENT,
            0.0,
            layout::SECTION_LINE,
        );

        match controller.available_update() {
            Some(update) => {
                set_text(&self.views.updates_row.title, "Update Available");
                set_text(
                    &self.views.updates_row.trailing,
                    &format!("{} →", update.version),
                );
                self.views
                    .updates_row
                    .trailing
                    .setTextColor(Some(&NSColor::controlAccentColor()));
            }
            None => {
                set_text(&self.views.updates_row.title, "Check for Updates…");
                set_text(&self.views.updates_row.trailing, "");
            }
        }
        // Re-placed rather than just re-written: the trailing label is
        // right-aligned at its natural width, so its frame moves whenever the
        // text changes length.
        compose::place_footer_text(&self.views.updates_row);
    }

    /// Applies the frames for the given battery presence and resizes the
    /// window to match.
    fn apply_layout(&self, has_battery: bool) {
        let height = compose::place(&self.views, has_battery);
        self.laid_out_with_battery.set(has_battery);
        let frame = self.window.frame();
        self.window.setFrame_display(
            NSRect::new(frame.origin, NSSize::new(layout::WIDTH, height)),
            self.window.isVisible(),
        );
    }

    /// Re-anchors a panel that changed height while it was open.
    fn reposition(&self) {
        let button = self.status_button.borrow();
        let Some(button) = button.as_ref() else {
            return;
        };
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let size = self.window.frame().size;
        self.window
            .setFrame_display(NSRect::new(anchor_origin(button, size, mtm), size), true);
        self.window.invalidateShadow();
    }

    // MARK: event monitors

    /// A click outside dismisses the panel; Escape and the menu shortcuts work
    /// while it is key.
    fn install_monitors(&self, mtm: MainThreadMarker, app: &Rc<App>) {
        self.remove_monitors();
        let mut monitors = self.monitors.borrow_mut();

        let dismiss = app.clone();
        let outside = block2::RcBlock::new(move |_event: std::ptr::NonNull<NSEvent>| {
            dismiss.close_panel();
        });
        if let Some(monitor) = NSEvent::addGlobalMonitorForEventsMatchingMask_handler(
            NSEventMask::LeftMouseDown | NSEventMask::RightMouseDown,
            &outside,
        ) {
            monitors.push(monitor);
        }

        let keys = app.clone();
        let handler =
            block2::RcBlock::new(move |event: std::ptr::NonNull<NSEvent>| -> *mut NSEvent {
                // SAFETY: AppKit hands the monitor a live event for the duration
                // of the call.
                let event = unsafe { event.as_ref() };
                let Some(action) = key_action(event) else {
                    // Not ours: hand the event back for normal delivery.
                    return Retained::into_raw(event.retain());
                };
                match action {
                    KeyAction::Dismiss => keys.close_panel(),
                    KeyAction::Quit => keys.quit(),
                    KeyAction::Settings => {
                        keys.close_panel();
                        keys.open_settings();
                    }
                }
                std::ptr::null_mut()
            });
        // SAFETY: the handler returns either the event it was given or null,
        // which is exactly the contract for a local monitor.
        if let Some(monitor) = unsafe {
            NSEvent::addLocalMonitorForEventsMatchingMask_handler(NSEventMask::KeyDown, &handler)
        } {
            monitors.push(monitor);
        }

        let _ = mtm;
    }

    fn remove_monitors(&self) {
        for monitor in self.monitors.borrow_mut().drain(..) {
            unsafe { NSEvent::removeMonitor(&monitor) };
        }
    }

    /// Battery and agent counts refresh only while the panel is on screen, so
    /// the process scan never runs for a UI nobody can see.
    fn start_ticker(&self, app: &Rc<App>) {
        self.stop_ticker();
        let app = app.clone();
        let timer = crate::app::repeating_timer(crate::config::PANEL_REFRESH_INTERVAL, move || {
            app.refresh_panel_readouts();
        });
        *self.ticker.borrow_mut() = Some(timer);
    }

    fn stop_ticker(&self) {
        if let Some(timer) = self.ticker.borrow_mut().take() {
            timer.invalidate();
        }
    }
}

impl Drop for Panel {
    fn drop(&mut self) {
        self.remove_monitors();
        self.stop_ticker();
        // Break the delegate's reference back to the app.
        if let Some(delegate) = self.delegate.borrow_mut().take() {
            *delegate.ivars().app.borrow_mut() = None;
        }
        self.window.setDelegate(None);
    }
}

impl PanelDelegate {
    fn new(mtm: MainThreadMarker, app: Rc<App>) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(DelegateIvars {
            app: RefCell::new(Some(app)),
        });
        unsafe { msg_send![super(this), init] }
    }
}

enum KeyAction {
    Dismiss,
    Quit,
    Settings,
}

/// The keystrokes the panel claims. Pure so the mapping is testable.
fn key_action(event: &NSEvent) -> Option<KeyAction> {
    if event.keyCode() == KEY_ESCAPE {
        return Some(KeyAction::Dismiss);
    }
    if !event
        .modifierFlags()
        .contains(NSEventModifierFlags::Command)
    {
        return None;
    }
    match event.charactersIgnoringModifiers()?.to_string().as_str() {
        "q" => Some(KeyAction::Quit),
        "," => Some(KeyAction::Settings),
        _ => None,
    }
}

fn session_detail(count: usize) -> String {
    match count {
        0 => "idle".to_string(),
        1 => "1 session".to_string(),
        many => format!("{many} sessions"),
    }
}

/// Where the panel's bottom-left corner goes: centred under the status item,
/// then nudged back inside the screen's visible area.
fn anchor_origin(button: &NSStatusBarButton, size: NSSize, mtm: MainThreadMarker) -> NSPoint {
    let Some(window) = button.window() else {
        return NSPoint::new(0.0, 0.0);
    };
    let in_window = button.convertRect_toView(button.bounds(), None);
    let frame = window.convertRectToScreen(in_window);

    let mut origin = NSPoint::new(
        frame.mid().x - size.width / 2.0,
        frame.min().y - size.height - ANCHOR_GAP,
    );

    if let Some(screen) = window.screen().or_else(|| NSScreen::mainScreen(mtm)) {
        let visible = screen.visibleFrame();
        let (min, max) = (visible.min(), visible.max());
        origin.x = origin
            .x
            .clamp(min.x + SCREEN_MARGIN, max.x - size.width - SCREEN_MARGIN);
        origin.y = origin.y.max(min.y + SCREEN_MARGIN);
    }
    origin
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_counts_read_as_english() {
        assert_eq!(session_detail(0), "idle");
        assert_eq!(session_detail(1), "1 session");
        assert_eq!(session_detail(4), "4 sessions");
    }
}
