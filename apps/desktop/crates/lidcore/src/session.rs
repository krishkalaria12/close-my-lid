//! The hold state machine: what the UI and CLI both drive.
//!
//! Carried over from the Swift app's `SleepSessionController`. Every state
//! change writes through to the store, so a restart can tell whether a hold
//! was meant to be running — which is what lets a stranded hold be released
//! by whatever runs next, app or watchdog.

use chrono::{DateTime, Utc};
use tracing::{debug, info, warn};

use crate::battery::{self, BatterySafetyPolicy, BatteryStatus};
use crate::duration::SessionDuration;
use crate::error::Result;
use crate::lock::HoldLock;
use crate::power::{LidPowerBackend, backend};
use crate::state::SleepControlState;
use crate::store::SleepSessionStore;

pub struct SleepSessionController {
    power: Box<dyn LidPowerBackend>,
    store: SleepSessionStore,
    battery_policy: BatterySafetyPolicy,
    state: SleepControlState,
    /// Consecutive reconciliations that saw the system reporting no hold while
    /// the stored session is still active. A single disagreeing reading can be
    /// a stale view right after a hold was applied; confirmation across two
    /// polls is required before discarding a session the app believes is live.
    external_disable_observations: u8,
}

impl SleepSessionController {
    /// Builds a controller for the current OS and adopts any saved state.
    pub fn new() -> Result<Self> {
        Ok(Self::with_parts(backend()?, SleepSessionStore::new()?))
    }

    pub fn with_parts(power: Box<dyn LidPowerBackend>, store: SleepSessionStore) -> Self {
        let state = store.load();
        Self {
            power,
            store,
            battery_policy: BatterySafetyPolicy::default(),
            state,
            external_disable_observations: 0,
        }
    }

    pub fn state(&self) -> SleepControlState {
        self.state
    }

    pub fn describe_backend(&self) -> &'static str {
        self.power.describe()
    }

    /// Whether a hold is genuinely in force right now, as opposed to merely
    /// recorded in the state file.
    ///
    /// Two sources, because the platforms disagree about where the truth
    /// lives. On Windows the change is global and persistent, so the OS is
    /// authoritative. On Linux the inhibitor belongs to a process and dies
    /// with it, so the live owner of the pid lock is authoritative — this
    /// backend's own descriptor says nothing about a hold another process
    /// took.
    pub fn is_really_held(&self) -> bool {
        self.power.is_held().unwrap_or(false) || HoldLock::owner().is_some()
    }

    pub fn start(&mut self, duration: SessionDuration) -> Result<()> {
        let now = Utc::now();
        // Acquire before recording, so a refused hold never leaves state
        // claiming a session that is not running.
        self.power.acquire()?;
        self.state = SleepControlState::Active {
            started_at: now,
            ends_at: duration.end_at(now),
        };
        self.store.save(&self.state)?;
        info!(duration = duration.label(), "hold started");
        Ok(())
    }

    pub fn stop(&mut self) -> Result<()> {
        self.power.release()?;
        self.state = SleepControlState::Inactive;
        self.store.save(&self.state)?;
        info!("hold stopped");
        Ok(())
    }

    /// One pass of the supervision loop: expiry and the battery safety release.
    ///
    /// Returns `true` if the hold was released.
    pub fn tick(&mut self) -> Result<bool> {
        if !self.state.is_active() {
            return Ok(false);
        }

        if self.state.has_expired(Utc::now()) {
            info!("session reached its end");
            self.stop()?;
            return Ok(true);
        }

        if let Some(status) = battery::read()
            && self.battery_policy.should_release(status)
        {
            warn!(
                percentage = status.percentage,
                "releasing the hold to protect the battery"
            );
            self.stop()?;
            return Ok(true);
        }

        // The OS hold can disappear under us (Windows: user changed the lid
        // action in Control Panel mid-hold). Log it so diagnostics explain
        // why a supposedly active session is not actually holding.
        match self.power.is_held() {
            Ok(false) => {
                debug!("session is recorded as active but the OS is not holding");
            }
            Err(error) => {
                debug!(%error, "could not verify the OS hold during supervision");
            }
            Ok(true) => {}
        }

        Ok(false)
    }

    /// Releases a timed hold whose end has passed. Returns true when a hold was
    /// released. Carried over from the Swift app's `stopIfExpired`, used by the macOS
    /// reconciliation pass.
    pub fn stop_if_expired(&mut self, now: DateTime<Utc>) -> Result<bool> {
        if !self.state.has_expired(now) {
            return Ok(false);
        }
        self.stop()?;
        Ok(true)
    }

    /// Reconciles the stored session against the OS-reported hold, for
    /// backends whose setting persists outside the process (macOS `pmset`).
    ///
    /// Carried over from the Swift app's `syncWithSystem`, including the two-strike rule:
    /// a hold the app believes is live is only discarded after two consecutive
    /// polls disagree, so a stale read right after applying cannot strand a
    /// session the user just started.
    pub fn sync_with_system(&mut self, held: bool, now: DateTime<Utc>) -> Result<()> {
        if self.state.has_expired(now) {
            self.stop()?;
            return Ok(());
        }

        match (self.state.is_active(), held) {
            (false, true) => {
                self.external_disable_observations = 0;
                self.state = self.adopt_external_hold(now);
                self.store.save(&self.state)?;
            }
            (true, false) => {
                self.external_disable_observations += 1;
                if self.external_disable_observations >= 2 {
                    self.external_disable_observations = 0;
                    self.clear_stored_session()?;
                }
            }
            (true, true) | (false, false) => {
                self.external_disable_observations = 0;
            }
        }
        Ok(())
    }

    /// The session to adopt when the OS reports a hold this controller did not
    /// take.
    ///
    /// The state file is checked first, because on macOS another process can
    /// legitimately own the hold: `close-my-lid enable --for 30m` applies the
    /// persistent `pmset` setting, records its deadline and exits. Inventing
    /// an indefinite session here would overwrite that deadline and quietly
    /// turn a timed hold into one that never ends. Anything else — a hold
    /// taken with `pmset` by hand, say — really is indefinite.
    fn adopt_external_hold(&self, now: DateTime<Utc>) -> SleepControlState {
        let stored = self.store.load();
        if stored.is_active() && !stored.has_expired(now) {
            debug!("adopting the session another process recorded");
            return stored;
        }
        SleepControlState::Active {
            started_at: now,
            ends_at: None,
        }
    }

    /// Reasserts a saved hold after macOS wakes. Power settings can be reset
    /// during a sleep/wake cycle even though the user's session is still
    /// active. Carried over from the Swift app's `restoreAfterWake`.
    pub fn restore_after_wake(&mut self, held: bool, now: DateTime<Utc>) -> Result<()> {
        // A disagreement seen before the machine slept says nothing about the
        // setting it came back with; start the two-strike count over.
        self.external_disable_observations = 0;

        if !self.state.is_active() {
            return Ok(());
        }

        if self.state.has_expired(now) {
            self.stop()?;
            return Ok(());
        }

        if !held {
            self.power.acquire()?;
        }
        Ok(())
    }

    /// Releases an active hold when the battery has drained to an unsafe level
    /// on battery power. Returns `true` when the hold was released. Ported
    /// from the Swift app's `stopIfBatteryLow`.
    pub fn stop_if_battery_low(&mut self, status: BatteryStatus) -> Result<bool> {
        if !self.state.is_active() {
            return Ok(false);
        }
        if !self.battery_policy.should_release(status) {
            return Ok(false);
        }
        self.stop()?;
        Ok(true)
    }

    /// Forgets the stored session without touching the OS. Used when
    /// reconciliation concludes the system hold went away on its own.
    pub fn clear_stored_session(&mut self) -> Result<()> {
        self.state = SleepControlState::Inactive;
        self.store.save(&self.state)?;
        Ok(())
    }

    /// Reconciles saved state against the OS at launch.
    ///
    /// Covers two cases: a previous run that died holding the lid (Windows can
    /// strand its power-scheme edit; Linux cannot), and a hold whose deadline
    /// passed while nothing was running.
    pub fn reconcile_at_launch(&mut self) -> Result<()> {
        // Another live process may legitimately own the hold; clearing its
        // state or releasing on its behalf would be wrong.
        if let Some(pid) = HoldLock::owner()
            && pid != std::process::id()
        {
            debug!(pid, "another process owns the hold; leaving it alone");
            return Ok(());
        }

        let held = match self.power.is_held() {
            Ok(held) => held,
            Err(error) => {
                debug!(%error, "could not query the OS hold; assuming not held");
                false
            }
        };

        match (self.state.is_active(), held) {
            // Saved session already over, but the OS is still holding.
            (true, true) if self.state.has_expired(Utc::now()) => {
                info!("clearing a hold whose session had already expired");
                self.stop()?;
            }
            // Stranded hold with no session behind it.
            (false, true) => {
                warn!("found a lid hold with no session; releasing it");
                self.power.release()?;
            }
            // State claims a session the OS is not honouring. On Windows the
            // previous scheme values may still be stranded in the non-active
            // scheme (user switched plans mid-hold), so release first to
            // restore them rather than just forgetting the state.
            (true, false) => {
                warn!("saved session is no longer held by the system; clearing it");
                // Best-effort: on Linux this is a no-op; on Windows it
                // restores a stranded non-active scheme. Never fail
                // reconciliation because of it.
                if let Err(error) = self.power.release() {
                    debug!(%error, "best-effort release during reconciliation failed");
                }
                self.state = SleepControlState::Inactive;
                self.store.save(&self.state)?;
            }
            _ => {}
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::LidError;
    use chrono::Duration;

    /// Records calls so the state machine can be tested without an OS.
    #[derive(Default)]
    struct FakeBackend {
        held: bool,
        fail_acquire: bool,
    }

    impl LidPowerBackend for FakeBackend {
        fn acquire(&mut self) -> Result<()> {
            if self.fail_acquire {
                return Err(LidError::denied("start a test hold", "refused"));
            }
            self.held = true;
            Ok(())
        }
        fn release(&mut self) -> Result<()> {
            self.held = false;
            Ok(())
        }
        fn is_held(&self) -> Result<bool> {
            Ok(self.held)
        }
        fn describe(&self) -> &'static str {
            "fake"
        }
    }

    fn controller(name: &str) -> SleepSessionController {
        let store = SleepSessionStore::at(std::env::temp_dir().join(name));
        let _ = store.clear();
        SleepSessionController::with_parts(Box::new(FakeBackend::default()), store)
    }

    #[test]
    fn starting_and_stopping_moves_state() {
        let mut controller = controller("cml-test-start.json");
        controller.start(SessionDuration::ONE_HOUR).unwrap();
        assert!(controller.state().is_active());

        controller.stop().unwrap();
        assert!(!controller.state().is_active());
    }

    #[test]
    fn a_refused_hold_leaves_state_inactive() {
        let store = SleepSessionStore::at(std::env::temp_dir().join("cml-test-refused.json"));
        let _ = store.clear();
        let backend = FakeBackend {
            held: false,
            fail_acquire: true,
        };
        let mut controller = SleepSessionController::with_parts(Box::new(backend), store);

        assert!(controller.start(SessionDuration::ONE_HOUR).is_err());
        assert!(
            !controller.state().is_active(),
            "a hold the OS refused must not be recorded as running"
        );
    }

    #[test]
    fn tick_is_a_no_op_while_a_session_is_running() {
        let mut controller = controller("cml-test-tick.json");
        controller.start(SessionDuration::ONE_HOUR).unwrap();
        assert!(!controller.tick().unwrap());
        assert!(controller.state().is_active());
    }
    #[test]
    fn launch_reconciliation_releases_a_stranded_hold() {
        let store = SleepSessionStore::at(std::env::temp_dir().join("cml-test-stranded.json"));
        let _ = store.clear();
        // OS is holding, but no session was saved.
        let backend = FakeBackend {
            held: true,
            fail_acquire: false,
        };
        let mut controller = SleepSessionController::with_parts(Box::new(backend), store);

        controller.reconcile_at_launch().unwrap();
        assert!(!controller.power.is_held().unwrap());
    }

    #[test]
    fn stop_if_expired_releases_only_elapsed_sessions() {
        let mut controller = controller("cml-test-expiry.json");
        controller.start(SessionDuration::ONE_HOUR).unwrap();
        assert!(!controller.stop_if_expired(Utc::now()).unwrap());
        assert!(controller.state().is_active());

        let past_end = Utc::now() + Duration::hours(2);
        assert!(controller.stop_if_expired(past_end).unwrap());
        assert!(!controller.state().is_active());

        // Inactive sessions are a no-op, not an error.
        assert!(!controller.stop_if_expired(past_end).unwrap());
    }

    #[test]
    fn adopting_an_external_hold_records_an_untimed_session() {
        let mut controller = controller("cml-test-adopt.json");
        controller.sync_with_system(true, Utc::now()).unwrap();
        assert!(controller.state().is_active());
        assert_eq!(controller.state().ends_at(), None);
    }

    #[test]
    fn adopting_a_hold_another_process_took_keeps_its_deadline() {
        // `close-my-lid enable --for 30m` applies the persistent macOS setting,
        // records its deadline and exits. A running app must adopt that
        // session rather than overwrite it with an indefinite one.
        let path = std::env::temp_dir().join("cml-test-adopt-timed.json");
        let now = Utc::now();
        let ends_at = now + Duration::minutes(30);
        SleepSessionStore::at(&path)
            .save(&SleepControlState::Active {
                started_at: now,
                ends_at: Some(ends_at),
            })
            .unwrap();

        let mut controller = SleepSessionController::with_parts(
            Box::new(FakeBackend::default()),
            SleepSessionStore::at(&path),
        );
        // The app started before that hold existed, so its own state is stale.
        controller.state = SleepControlState::Inactive;

        controller.sync_with_system(true, now).unwrap();
        assert_eq!(controller.state().ends_at(), Some(ends_at));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn an_expired_stored_session_is_not_adopted() {
        let path = std::env::temp_dir().join("cml-test-adopt-expired.json");
        let now = Utc::now();
        SleepSessionStore::at(&path)
            .save(&SleepControlState::Active {
                started_at: now - Duration::hours(2),
                ends_at: Some(now - Duration::hours(1)),
            })
            .unwrap();

        let mut controller = SleepSessionController::with_parts(
            Box::new(FakeBackend::default()),
            SleepSessionStore::at(&path),
        );
        controller.state = SleepControlState::Inactive;

        controller.sync_with_system(true, now).unwrap();
        assert_eq!(
            controller.state().ends_at(),
            None,
            "a finished session is not a deadline"
        );
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn a_wake_clears_a_strike_from_before_the_sleep() {
        let now = Utc::now();
        let mut controller = controller("cml-test-wake-strike.json");
        controller.start(SessionDuration::ONE_HOUR).unwrap();

        // One disagreement, then a sleep/wake cycle, then another. The pair
        // must not add up: they are readings of two different power states.
        controller.sync_with_system(false, now).unwrap();
        controller.restore_after_wake(true, now).unwrap();
        controller.sync_with_system(false, now).unwrap();
        assert!(controller.state().is_active());
    }

    #[test]
    fn a_single_missing_hold_read_does_not_clear_the_session() {
        let now = Utc::now();
        let mut controller = controller("cml-test-strike1.json");
        controller.start(SessionDuration::ONE_HOUR).unwrap();

        controller.sync_with_system(false, now).unwrap();
        assert!(
            controller.state().is_active(),
            "one stale read must not discard a live session"
        );

        controller.sync_with_system(false, now).unwrap();
        assert!(
            !controller.state().is_active(),
            "two consecutive misses confirm the hold is gone"
        );
    }

    #[test]
    fn an_agreeing_read_resets_the_strike_count() {
        let now = Utc::now();
        let mut controller = controller("cml-test-strikereset.json");
        controller.start(SessionDuration::ONE_HOUR).unwrap();

        controller.sync_with_system(false, now).unwrap();
        controller.sync_with_system(true, now).unwrap();
        controller.sync_with_system(false, now).unwrap();
        assert!(
            controller.state().is_active(),
            "the intervening agreement must reset the count"
        );
    }

    #[test]
    fn restore_after_wake_reapplies_a_missing_hold() {
        let mut controller = controller("cml-test-wake.json");
        controller.start(SessionDuration::ONE_HOUR).unwrap();
        // Simulate the wake reset: the OS no longer holds.
        controller.power.release().unwrap();

        controller.restore_after_wake(false, Utc::now()).unwrap();
        assert!(controller.power.is_held().unwrap());
        assert!(controller.state().is_active());
    }

    #[test]
    fn restore_after_wake_ends_an_expired_session() {
        let mut controller = controller("cml-test-wake-expired.json");
        controller.start(SessionDuration::ONE_HOUR).unwrap();

        controller
            .restore_after_wake(true, Utc::now() + Duration::hours(2))
            .unwrap();
        assert!(!controller.state().is_active());
    }

    #[test]
    fn battery_safety_releases_only_unsafe_holds() {
        use crate::battery::BatteryStatus;
        let mut controller = controller("cml-test-battery.json");

        // Inactive sessions are untouched.
        assert!(
            !controller
                .stop_if_battery_low(BatteryStatus {
                    percentage: 1,
                    is_charging: false
                })
                .unwrap()
        );

        controller.start(SessionDuration::ONE_HOUR).unwrap();
        // Healthy or charging batteries keep the hold.
        assert!(
            !controller
                .stop_if_battery_low(BatteryStatus {
                    percentage: 80,
                    is_charging: false
                })
                .unwrap()
        );
        assert!(
            !controller
                .stop_if_battery_low(BatteryStatus {
                    percentage: 1,
                    is_charging: true
                })
                .unwrap()
        );
        assert!(controller.state().is_active());

        // Drained and unplugged releases it.
        assert!(
            controller
                .stop_if_battery_low(BatteryStatus {
                    percentage: 5,
                    is_charging: false
                })
                .unwrap()
        );
        assert!(!controller.state().is_active());
    }
}
