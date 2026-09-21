//! The hold state machine: what the UI and CLI both drive.
//!
//! Ported from the Swift `SleepSessionController`, minus the macOS-only
//! wake reconciliation. Every state change writes through to the store, so a
//! restart can tell whether a hold was meant to be running.

use chrono::Utc;
use tracing::{debug, info, warn};

use crate::battery::{self, BatterySafetyPolicy};
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

        Ok(false)
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

        let held = self.power.is_held().unwrap_or(false);

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
            // State claims a session the OS is not honouring.
            (true, false) => {
                warn!("saved session is no longer held by the system; clearing it");
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
}
