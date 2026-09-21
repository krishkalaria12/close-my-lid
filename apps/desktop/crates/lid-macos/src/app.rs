//! The application object: the status item, the timers, and the wiring that
//! connects the panel's controls to [`Controller`].
//!
//! Carried over from the Swift app's `AppDelegate` + `StatusMenuController`.
//! Everything here runs on the main thread. The two things that must not —
//! reading the system hold, which can fall back to a forked `pmset`, and
//! fetching the release feed — go through GCD: a shared global queue does the
//! work and the answer is posted back through [`on_main`]. A dispatch hop
//! costs nothing next to the thread-per-reconciliation this used to spawn,
//! forever, for as long as the app runs.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use block2::RcBlock;
use dispatch2::{DispatchQoS, DispatchQueue, GlobalQueueIdentifier};
use lidcore::{LidError, SessionDuration, VERSION};
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate, NSEventMask, NSImage,
    NSStatusBar, NSStatusItem, NSVariableStatusItemLength, NSWorkspace,
};
use objc2_foundation::{
    MainThreadMarker, NSNotification, NSObject, NSObjectProtocol, NSRunLoop, NSRunLoopCommonModes,
    NSString, NSTimer,
};
use tracing::{debug, warn};

use crate::config;
use crate::controller::{self, Controller};
use crate::panel::Panel;
use crate::settings::SettingsWindow;
use crate::ui::ActionTarget;

// The running app, reachable from the main thread. A menu bar app has exactly
// one, it lives for the whole process, and blocks posted back from worker
// threads need to find it without carrying a non-`Send` pointer across the
// boundary.
thread_local! {
    static CURRENT: RefCell<Option<Rc<App>>> = const { RefCell::new(None) };
}

pub struct App {
    pub controller: RefCell<Controller>,
    status_item: Retained<NSStatusItem>,
    panel: RefCell<Option<Panel>>,
    settings: RefCell<Option<SettingsWindow>>,
    /// Fires at a timed session's exact end, so a hold is released on the
    /// second rather than up to a reconciliation interval late.
    expiry_timer: RefCell<Option<Retained<NSTimer>>>,
    /// Kept alive: AppKit does not retain a control's target.
    targets: RefCell<Vec<Retained<ActionTarget>>>,
    /// Long-lived timers and notification observers, released on quit.
    keepalive: RefCell<Vec<Retained<NSObject>>>,
    mtm: MainThreadMarker,
}

define_class!(
    // SAFETY:
    // - NSObject has no subclassing requirements.
    // - Delegate does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "CMLAppDelegate"]
    #[ivars = ()]
    struct Delegate;

    unsafe impl NSObjectProtocol for Delegate {}

    unsafe impl NSApplicationDelegate for Delegate {
        // Releasing here rather than in a `Drop` because AppKit tears the
        // process down without unwinding: the hold is a global `pmset`
        // setting and must not outlive the app that took it.
        #[unsafe(method(applicationWillTerminate:))]
        fn will_terminate(&self, _notification: &NSNotification) {
            with_app(|app| {
                if let Err(error) = app.controller.borrow_mut().stop_if_holding() {
                    warn!(%error, "could not release the hold while quitting");
                }
            });
        }
    }
);

/// Runs the menu bar app. Returns when the user quits.
pub fn run() {
    let mtm = MainThreadMarker::new().expect("the app must start on the main thread");
    let ns_app = NSApplication::sharedApplication(mtm);
    // Accessory: a menu bar app has no Dock icon and no menu bar of its own.
    ns_app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

    let controller = match Controller::new() {
        Ok(controller) => controller,
        Err(error) => {
            crate::alert::fatal(mtm, &error);
            return;
        }
    };

    let app = Rc::new(App {
        controller: RefCell::new(controller),
        status_item: make_status_item(mtm),
        panel: RefCell::new(None),
        settings: RefCell::new(None),
        expiry_timer: RefCell::new(None),
        targets: RefCell::new(Vec::new()),
        keepalive: RefCell::new(Vec::new()),
        mtm,
    });
    CURRENT.with(|current| *current.borrow_mut() = Some(app.clone()));

    app.wire_status_item();
    app.observe_sleep_and_wake();
    app.schedule_timers();
    app.reconcile();

    let delegate = Delegate::alloc(mtm).set_ivars(());
    let delegate: Retained<Delegate> = unsafe { msg_send![super(delegate), init] };
    ns_app.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));

    ns_app.run();
}

/// Runs `body` with the running app, if there is one.
pub fn with_app(body: impl FnOnce(&Rc<App>)) {
    let app = CURRENT.with(|current| current.borrow().clone());
    if let Some(app) = app {
        body(&app);
    }
}

impl App {
    // MARK: status item

    fn wire_status_item(self: &Rc<Self>) {
        let Some(button) = self.status_item.button(self.mtm) else {
            return;
        };

        let app = self.clone();
        let target = ActionTarget::new(move || app.toggle_panel());
        unsafe {
            button.setTarget(Some(&target));
            button.setAction(Some(ActionTarget::selector()));
        }
        // Right-click opens the panel too: there is no separate context menu,
        // so an unhandled right-click would look like the app was dead.
        button.sendActionOn(NSEventMask::LeftMouseUp | NSEventMask::RightMouseUp);
        self.targets.borrow_mut().push(target);
    }

    // MARK: actions the panel calls

    pub fn toggle_panel(self: &Rc<Self>) {
        if self.panel.borrow().as_ref().is_some_and(Panel::is_visible) {
            self.close_panel();
        } else {
            self.open_panel();
        }
    }

    fn open_panel(self: &Rc<Self>) {
        // Scan for agents only when the panel is about to appear, so the
        // process walk never runs for a UI nobody can see.
        self.controller.borrow_mut().refresh_readouts();

        if self.panel.borrow().is_none() {
            let panel = Panel::new(self.mtm, self);
            *self.panel.borrow_mut() = Some(panel);
        }
        let Some(button) = self.status_item.button(self.mtm) else {
            return;
        };

        let panel = self.panel.borrow();
        if let Some(panel) = panel.as_ref() {
            panel.refresh(self);
            panel.show(&button, self.mtm, self);
            debug!(height = panel.height(), "panel shown");
        }
    }

    pub fn close_panel(self: &Rc<Self>) {
        if let Some(panel) = self.panel.borrow().as_ref() {
            panel.close();
        }
    }

    /// Re-reads the panel's readouts. Called by its own ticker while open.
    pub fn refresh_panel_readouts(self: &Rc<Self>) {
        self.controller.borrow_mut().refresh_readouts();
        self.refresh_panel();
    }

    fn refresh_panel(self: &Rc<Self>) {
        if let Some(panel) = self.panel.borrow().as_ref() {
            panel.refresh(self);
        }
    }

    pub fn toggle_holding(self: &Rc<Self>) {
        let holding = self.controller.borrow().state().is_active();
        let result = self.controller.borrow_mut().set_holding(!holding);
        self.finish_session_change(result);
    }

    pub fn start_hold(self: &Rc<Self>, duration: SessionDuration) {
        let result = self.controller.borrow_mut().start(duration);
        self.finish_session_change(result);
    }

    /// The tail every start/stop shares: report a refusal, re-arm the expiry
    /// timer, and redraw.
    fn finish_session_change(self: &Rc<Self>, result: Result<(), LidError>) {
        if let Err(error) = result {
            crate::alert::show(self.mtm, &error);
        }
        self.arm_expiry_timer();
        self.refresh_panel();
    }

    /// Runs `body` with the Settings window, if it has been opened.
    pub fn with_settings(self: &Rc<Self>, body: impl FnOnce(&SettingsWindow)) {
        if let Some(settings) = self.settings.borrow().as_ref() {
            body(settings);
        }
    }

    pub fn open_settings(self: &Rc<Self>) {
        if self.settings.borrow().is_none() {
            *self.settings.borrow_mut() = Some(SettingsWindow::new(self.mtm, self));
        }
        if let Some(settings) = self.settings.borrow().as_ref() {
            settings.show(self.mtm);
        }
    }

    /// Opens the release page for an available update, or checks now if none
    /// is known yet. Downloading and installing stay a deliberate user action.
    pub fn open_updates(self: &Rc<Self>) {
        let known = self
            .controller
            .borrow()
            .available_update()
            .map(|update| update.url.clone());

        match known {
            Some(url) => {
                self.close_panel();
                open_release_url(&url);
            }
            None => self.check_for_updates(),
        }
    }

    pub fn quit(self: &Rc<Self>) {
        // The release happens in `applicationWillTerminate:`, which also
        // covers a quit that did not come through here.
        NSApplication::sharedApplication(self.mtm).terminate(None);
    }

    // MARK: reconciliation

    fn schedule_timers(self: &Rc<Self>) {
        let app = self.clone();
        let reconcile = repeating_timer(config::RECONCILE_INTERVAL, move || app.reconcile());

        let app = self.clone();
        let first_check = one_shot_timer(config::FIRST_UPDATE_CHECK_DELAY, move || {
            app.check_for_updates()
        });

        let app = self.clone();
        let checks = repeating_timer(config::UPDATE_CHECK_INTERVAL, move || {
            app.check_for_updates()
        });

        let mut keepalive = self.keepalive.borrow_mut();
        for timer in [reconcile, first_check, checks] {
            keepalive.push(unsafe { Retained::cast_unchecked(timer) });
        }
    }

    /// A hold taken before the machine slept has to be reasserted on wake:
    /// power settings can come back off across a sleep/wake cycle even though
    /// the user's session is still running.
    fn observe_sleep_and_wake(self: &Rc<Self>) {
        let center = NSWorkspace::sharedWorkspace().notificationCenter();
        let mut keepalive = self.keepalive.borrow_mut();

        for (name, wake) in [
            (
                unsafe { objc2_app_kit::NSWorkspaceWillSleepNotification },
                false,
            ),
            (
                unsafe { objc2_app_kit::NSWorkspaceDidWakeNotification },
                true,
            ),
        ] {
            let block = RcBlock::new(move |_notification: std::ptr::NonNull<NSNotification>| {
                with_app(|app| {
                    if wake {
                        app.controller.borrow_mut().system_did_wake();
                        app.reconcile();
                    } else {
                        app.controller.borrow_mut().system_will_sleep();
                    }
                });
            });
            let observer = unsafe {
                center.addObserverForName_object_queue_usingBlock(Some(name), None, None, &block)
            };
            keepalive.push(unsafe { Retained::cast_unchecked(observer) });
        }
    }

    /// One reconciliation pass.
    ///
    /// The `pmset -g` read spawns a process, so it runs on a worker thread and
    /// the answer is posted back here — the main thread never blocks on it.
    fn reconcile(self: &Rc<Self>) {
        // While a wake restore is outstanding the system's answer is not
        // trusted; the hold is reasserted instead of reconciled against.
        let awaiting = {
            let controller = self.controller.borrow();
            controller.is_awaiting_wake_restore() && controller.is_ready_to_restore()
        };

        schedule_system_read(move |held| {
            with_app(|app| {
                {
                    let mut controller = app.controller.borrow_mut();
                    match (awaiting, held) {
                        (true, Some(held)) => controller.restore_after_wake(held),
                        (false, Some(held)) => controller.apply_system_read(held),
                        (_, None) => controller.apply_failed_read(),
                    }
                }
                app.arm_expiry_timer();
                app.refresh_panel();
            });
        });
    }

    /// Fires a one-shot timer at the session's end, so a timed hold is
    /// released on time instead of whenever the next poll happens to land.
    ///
    /// Once that end has passed the timer becomes a *retry*, and retries back
    /// off to the ordinary reconciliation interval. Without that, a release
    /// that keeps failing — the passwordless grant is missing and the user
    /// dismisses the administrator prompt — re-arms at the overshoot and asks
    /// again half a second later, producing an unclosable loop of password
    /// dialogs rather than one refusal and a retry on the next pass.
    fn arm_expiry_timer(self: &Rc<Self>) {
        if let Some(timer) = self.expiry_timer.borrow_mut().take() {
            timer.invalidate();
        }

        let Some(ends_at) = self.controller.borrow().ends_at() else {
            return;
        };

        let delay = match (ends_at - chrono::Utc::now()).to_std() {
            Ok(remaining) => remaining + config::EXPIRY_OVERSHOOT,
            // Already past: the hold should be gone, and is not.
            Err(_) => config::RECONCILE_INTERVAL,
        };

        let app = self.clone();
        let timer = one_shot_timer(delay, move || app.reconcile());
        *self.expiry_timer.borrow_mut() = Some(timer);
    }

    // MARK: updates

    /// Checks the release feed and records the answer.
    ///
    /// `NSURLSession` is already asynchronous, so this needs no worker of its
    /// own; only the answer comes back to the main queue.
    fn check_for_updates(self: &Rc<Self>) {
        crate::updates::check(|found| {
            on_main(move || {
                with_app(|app| {
                    match &found {
                        Some(update) => {
                            debug!(version = %update.version, current = VERSION, "update available");
                        }
                        // Logged too: "nothing happened" and "the check never
                        // finished" look identical otherwise, and this is the
                        // one path that talks to the network.
                        None => debug!(current = VERSION, "no newer release on the feed"),
                    }
                    app.controller
                        .borrow_mut()
                        .set_available_update(found.clone());
                    app.refresh_panel();
                });
            });
        });
    }
}

fn make_status_item(mtm: MainThreadMarker) -> Retained<NSStatusItem> {
    let item = NSStatusBar::systemStatusBar().statusItemWithLength(NSVariableStatusItemLength);
    if let Some(button) = item.button(mtm) {
        let symbol = NSString::from_str(config::STATUS_ITEM_SYMBOL);
        let description = NSString::from_str(lidcore::APP_NAME);
        if let Some(image) =
            NSImage::imageWithSystemSymbolName_accessibilityDescription(&symbol, Some(&description))
        {
            // A template image is tinted by AppKit, so one asset is correct in
            // both the light and dark menu bar.
            image.setTemplate(true);
            button.setImage(Some(&image));
        }
        button.setToolTip(Some(&description));
    }
    item
}

/// Reads the system hold off the main thread and delivers the answer on it.
/// `None` means the read itself failed.
///
/// The read is usually a single IOKit call, but it falls back to forking
/// `pmset -g`, and the main thread must not be the one waiting on that.
fn schedule_system_read(deliver: impl FnOnce(Option<bool>) + Send + 'static) {
    background(move || {
        let held = controller::read_system_hold()
            .inspect_err(|error| debug!(%error, "could not read the closed-lid sleep setting"))
            .ok();
        on_main(move || deliver(held));
    });
}

/// Runs `body` on a shared global queue.
///
/// Utility: none of this is work the user is waiting on, and the class tells
/// the scheduler it may be coalesced with other background work rather than
/// waking a core on its own.
pub fn background(body: impl FnOnce() + Send + 'static) {
    DispatchQueue::global_queue(GlobalQueueIdentifier::QualityOfService(
        DispatchQoS::Utility,
    ))
    .exec_async(body);
}

/// Runs `body` on the main thread.
pub fn on_main(body: impl FnOnce() + Send + 'static) {
    DispatchQueue::main().exec_async(body);
}

/// A repeating timer registered in the common run loop modes, so it keeps
/// firing while the user is tracking a menu or dragging.
pub fn repeating_timer(interval: Duration, body: impl Fn() + 'static) -> Retained<NSTimer> {
    scheduled(interval, true, body)
}

fn one_shot_timer(delay: Duration, body: impl Fn() + 'static) -> Retained<NSTimer> {
    scheduled(delay, false, body)
}

fn scheduled(interval: Duration, repeats: bool, body: impl Fn() + 'static) -> Retained<NSTimer> {
    let block = RcBlock::new(move |_timer: std::ptr::NonNull<NSTimer>| body());
    let timer = unsafe {
        NSTimer::timerWithTimeInterval_repeats_block(
            // A zero interval would busy-loop; the shortest useful delay is
            // one run loop pass.
            interval.as_secs_f64().max(0.001),
            repeats,
            &block,
        )
    };
    // Slack lets the scheduler coalesce this with other wake-ups instead of
    // waking the CPU on its own for it.
    timer.setTolerance(config::TIMER_TOLERANCE.as_secs_f64());
    unsafe { NSRunLoop::mainRunLoop().addTimer_forMode(&timer, NSRunLoopCommonModes) };
    timer
}

pub fn open_url(url: &str) {
    let Some(url) = objc2_foundation::NSURL::URLWithString(&NSString::from_str(url)) else {
        warn!(url, "could not parse a URL to open");
        return;
    };
    NSWorkspace::sharedWorkspace().openURL(&url);
}

/// Opens a release page, and only a release page.
///
/// The URL came off a network feed that nothing signs any more, and this is
/// the point where it would reach the user's browser. `lidcore` already
/// refuses to record anything else, so a rejection here means the two checks
/// have drifted apart — which is worth a log line rather than a silent pass.
fn open_release_url(url: &str) {
    if !lidcore::updates::is_release_url(url) {
        warn!(
            url,
            "refusing to open an update URL outside the project releases"
        );
        return;
    }
    open_url(url);
}
