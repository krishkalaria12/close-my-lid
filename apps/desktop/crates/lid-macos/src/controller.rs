//! Everything the menu bar app does that is not drawing.
//!
//! Carried over from the Swift app's `StatusMenuController`, minus its AppKit
//! half. Keeping the reconciliation, heartbeat, watchdog and notification
//! decisions here means `app.rs` is only wiring: timers and observers call
//! into this, and the panel reads back out of it.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use lidcore::heartbeat::{self, HoldHeartbeat, HoldHeartbeatStore};
use lidcore::power::macos::PmsetLidGuard;
use lidcore::updates::UpdateInfo;
use lidcore::{
    AgentHarness, BatterySafetyPolicy, BatteryStatus, LidError, Result, SessionDuration,
    SleepControlState, SleepSessionController, battery, launchd, notify, prefs, sudoers,
};
use tracing::{debug, warn};

use crate::notifications::Notifier;

pub struct Controller {
    session: SleepSessionController,
    heartbeat: Option<HoldHeartbeatStore>,
    notifier: Notifier,
    battery_policy: BatterySafetyPolicy,

    /// Tracks the active→inactive edge, so pending notifications are cancelled
    /// exactly once when a hold ends rather than on every reconciliation pass.
    was_active: bool,

    /// A hold was running when the machine went to sleep, so the `pmset`
    /// setting has to be re-checked — and usually re-applied — on wake.
    wake_restore_pending: bool,
    /// Set only once the machine is actually awake again: reading power
    /// settings while it is on its way down answers for the wrong moment.
    wake_restore_ready: bool,

    /// Refreshed only while the panel is open; see [`Self::refresh_readouts`].
    battery: Option<BatteryStatus>,
    agents: HashMap<AgentHarness, usize>,
    update: Option<UpdateInfo>,
}

impl Controller {
    pub fn new() -> Result<Self> {
        let session = SleepSessionController::new()?;
        let was_active = session.state().is_active();

        Ok(Self {
            session,
            // A hold that cannot record its heartbeat is still a hold, so a
            // missing store degrades the watchdog rather than the app.
            heartbeat: HoldHeartbeatStore::new()
                .inspect_err(|error| warn!(%error, "no heartbeat store; the watchdog is blind"))
                .ok(),
            notifier: Notifier::new(),
            battery_policy: BatterySafetyPolicy::default(),
            was_active,
            wake_restore_pending: false,
            wake_restore_ready: false,
            battery: None,
            agents: HashMap::new(),
            update: None,
        })
    }

    // MARK: readouts the panel draws

    pub fn state(&self) -> SleepControlState {
        self.session.state()
    }

    pub fn battery(&self) -> Option<BatteryStatus> {
        self.battery
    }

    pub fn battery_policy(&self) -> BatterySafetyPolicy {
        self.battery_policy
    }

    pub fn agent_sessions(&self, harness: AgentHarness) -> usize {
        self.agents.get(&harness).copied().unwrap_or(0)
    }

    pub fn available_update(&self) -> Option<&UpdateInfo> {
        self.update.as_ref()
    }

    pub fn set_available_update(&mut self, update: Option<UpdateInfo>) {
        self.update = update;
    }

    /// Re-reads the battery and the agent process table.
    ///
    /// Called only when the panel is about to be shown and while it is open,
    /// so the process scan never runs for a UI nobody is looking at.
    pub fn refresh_readouts(&mut self) {
        self.battery = battery::read();
        self.agents = lidcore::sessions_now();
    }

    // MARK: starting and stopping

    /// The main ON/OFF toggle. Turning it on re-applies the duration the user
    /// last picked rather than always starting an unlimited hold.
    pub fn set_holding(&mut self, holding: bool) -> Result<()> {
        if holding {
            self.start(prefs::load())
        } else {
            self.stop()
        }
    }

    pub fn start(&mut self, duration: SessionDuration) -> Result<()> {
        self.session.start(duration)?;

        if let Err(error) = prefs::save(duration) {
            // The hold is running; only the toggle's memory of it is lost.
            warn!(%error, "could not remember the chosen duration");
        }
        self.record_heartbeat();

        // Planned from the session's own start, not from before the call that
        // took the hold. Applying `pmset` can sit behind an administrator
        // prompt for as long as the user takes to type a password, and a plan
        // built on the earlier instant would announce the session's end that
        // much before it actually ended.
        let started_at = self.session.state().started_at().unwrap_or_else(Utc::now);
        self.notifier.apply(&notify::plan(duration, started_at));

        self.was_active = true;
        self.install_watchdog_if_granted();
        Ok(())
    }

    pub fn stop(&mut self) -> Result<()> {
        self.session.stop()?;
        self.clear_heartbeat();
        self.notifier.cancel_pending();
        self.was_active = false;
        Ok(())
    }

    // MARK: reconciliation

    /// True while a hold is waiting to be reasserted after a wake.
    ///
    /// During that window the `pmset` read is not trusted: the setting can
    /// come back off across a sleep/wake cycle even though the user's session
    /// is still running, and treating that as "the user turned it off" would
    /// discard a live session.
    pub fn is_awaiting_wake_restore(&self) -> bool {
        self.wake_restore_pending
    }

    pub fn is_ready_to_restore(&self) -> bool {
        self.wake_restore_ready
    }

    /// One reconciliation pass, given what the OS reports.
    ///
    /// The `pmset -g` read is done by the caller off the main thread — it
    /// spawns a process — and only the answer is handed back here.
    pub fn apply_system_read(&mut self, held: bool) {
        if let Err(error) = self.session.sync_with_system(held, Utc::now()) {
            // A cancelled admin prompt during an expiry release is not a
            // failure: leave the hold in place and retry next pass.
            if matches!(error, LidError::ElevationCancelled) {
                debug!("administrator prompt dismissed; retrying on the next pass");
            } else {
                warn!(%error, "reconciliation failed; releasing if expired");
                let _ = self.session.stop_if_expired(Utc::now());
            }
        }

        self.after_reconciliation();
    }

    /// The pass taken when the OS could not be asked at all.
    pub fn apply_failed_read(&mut self) {
        let _ = self.session.stop_if_expired(Utc::now());
        self.after_reconciliation();
    }

    /// Re-applies a hold the machine may have dropped while it was asleep.
    pub fn restore_after_wake(&mut self, held: bool) {
        match self.session.restore_after_wake(held, Utc::now()) {
            Ok(()) => {
                self.wake_restore_pending = false;
                self.wake_restore_ready = false;
            }
            Err(error) => {
                warn!(%error, "could not reassert the hold after wake");
                let _ = self.session.stop_if_expired(Utc::now());
                // Only give up once there is no session left to restore;
                // otherwise the next pass tries again.
                if !self.session.state().is_active() {
                    self.wake_restore_pending = false;
                    self.wake_restore_ready = false;
                }
            }
        }

        self.after_reconciliation();
    }

    pub fn system_will_sleep(&mut self) {
        self.wake_restore_pending = self.session.state().is_active();
        self.wake_restore_ready = false;
    }

    pub fn system_did_wake(&mut self) {
        self.wake_restore_pending = self.session.state().is_active();
        self.wake_restore_ready = self.wake_restore_pending;
        self.enforce_battery_safety();
    }

    /// The tail every reconciliation path shares.
    fn after_reconciliation(&mut self) {
        self.enforce_battery_safety();
        self.record_heartbeat();
        self.install_watchdog_if_granted();
        self.note_session_transition();
    }

    /// Releases the hold when the battery has drained to an unsafe level on
    /// battery power — holding the lid open on a draining battery is how you
    /// come back to a dead laptop.
    fn enforce_battery_safety(&mut self) {
        let Some(status) = battery::read() else {
            return;
        };
        self.battery = Some(status);
        match self.session.stop_if_battery_low(status) {
            Ok(true) => {
                self.clear_heartbeat();
                self.notifier.cancel_pending();
                self.notifier.deliver_now(
                    lidcore::APP_NAME,
                    "Battery is low, so normal closed-lid sleep is back on.",
                );
                self.was_active = false;
            }
            Ok(false) => {}
            Err(error) => warn!(%error, "could not release the hold for low battery"),
        }
    }

    /// A timed session's "ended" message is delivered by the system at its own
    /// fire date. When reconciliation sees the hold turn off early, whatever
    /// is still pending is no longer true.
    fn note_session_transition(&mut self) {
        let is_active = self.session.state().is_active();
        if self.was_active && !is_active {
            self.notifier.cancel_pending();
        }
        self.was_active = is_active;
    }

    // MARK: heartbeat

    fn record_heartbeat(&self) {
        let Some(store) = &self.heartbeat else {
            return;
        };
        match self.session.state() {
            SleepControlState::Active { ends_at, .. } => {
                // Supervised: this app is alive and refreshes the record on
                // every pass, so the watchdog may judge it by liveness.
                let beat = HoldHeartbeat::supervised(ends_at, heartbeat::now_utc());
                if let Err(error) = store.write(&beat) {
                    warn!(%error, "could not write the hold heartbeat");
                }
            }
            SleepControlState::Inactive => store.remove(),
        }
    }

    fn clear_heartbeat(&self) {
        if let Some(store) = &self.heartbeat {
            store.remove();
        }
    }

    // MARK: watchdog

    /// Registers the dead-man LaunchAgent once the passwordless grant exists.
    /// Without the grant the agent could not release anything anyway, and
    /// installing it early would only leave a job that fails every minute.
    pub fn install_watchdog_if_granted(&self) {
        if !sudoers::is_installed() || launchd::is_installed() {
            return;
        }
        if let Err(error) = launchd::install() {
            warn!(%error, "could not register the watchdog agent");
        }
    }

    /// When a timed hold is due to end, so the app can release it on the
    /// second instead of up to a poll interval late.
    pub fn ends_at(&self) -> Option<DateTime<Utc>> {
        self.session.state().ends_at()
    }
}

/// Asks the OS whether closed-lid sleep is currently held.
///
/// Spawns `pmset -g`, so it is run on a worker thread and the answer posted
/// back to the main queue; see `app::schedule_system_read`.
pub fn read_system_hold() -> Result<bool> {
    use lidcore::LidPowerBackend;

    PmsetLidGuard::new().is_held()
}
