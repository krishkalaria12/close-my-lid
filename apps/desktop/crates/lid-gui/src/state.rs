//! UI-facing state: the controller plus whatever the panel renders.
//!
//! Lives in its own entity so the supervision loop and the panel can both
//! touch it, and so the panel re-renders when either changes it.

use std::collections::HashMap;

use lidcore::{
    AgentHarness, BatteryStatus, HoldLock, SessionDuration, SleepControlState,
    SleepSessionController, battery,
};
use tracing::{error, warn};

use crate::error::{GuiError, Result};

pub struct AppState {
    controller: Option<SleepSessionController>,
    /// Held while this app owns the hold, so the tray app and the CLI cannot
    /// take overlapping holds of the same global setting.
    lock: Option<HoldLock>,
    /// Why the backend could not be built, if it could not be. Held as a typed
    /// error so the panel can show its hint, not just a message.
    pub startup_error: Option<GuiError>,
    pub battery: Option<BatteryStatus>,
    pub agents: HashMap<AgentHarness, usize>,
}

impl AppState {
    pub fn new() -> Self {
        let (controller, startup_error) = match SleepSessionController::new() {
            Ok(mut controller) => {
                // Undo a hold a previous run left behind before showing any UI.
                if let Err(error) = controller.reconcile_at_launch() {
                    warn!(%error, "launch reconciliation failed");
                }
                (Some(controller), None)
            }
            Err(source) => {
                error!(%source, "no lid backend available");
                (None, Some(GuiError::NoBackend { source }))
            }
        };

        let mut state = Self {
            controller,
            lock: None,
            startup_error,
            battery: None,
            agents: HashMap::new(),
        };
        state.refresh_readouts();
        state
    }

    pub fn state(&self) -> SleepControlState {
        self.controller
            .as_ref()
            .map(|controller| controller.state())
            .unwrap_or(SleepControlState::Inactive)
    }

    pub fn is_active(&self) -> bool {
        self.state().is_active()
    }

    /// Re-reads battery and agent counts. Called when the panel opens rather
    /// than on a timer, so the process scan never runs for a hidden UI — the
    /// same optimisation the macOS panel makes.
    pub fn refresh_readouts(&mut self) {
        self.battery = battery::read();
        self.agents = lidcore::sessions_now();
    }

    pub fn start(&mut self, duration: SessionDuration) -> Result<()> {
        // Take the lock before touching the system: if the CLI is already
        // holding, this fails with a message naming that process rather than
        // quietly taking a second, conflicting hold of the same setting.
        if self.lock.is_none() {
            self.lock = Some(HoldLock::acquire()?);
        }

        if let Err(error) = self.controller_mut()?.start(duration) {
            self.lock = None;
            return Err(error.into());
        }
        Ok(())
    }

    pub fn stop(&mut self) -> Result<()> {
        self.controller_mut()?.stop()?;
        // Released only after the system change succeeded, so a failed stop
        // does not advertise the hold as available.
        self.lock = None;
        Ok(())
    }

    /// The controller, or an error explaining why there isn't one.
    fn controller_mut(&mut self) -> Result<&mut SleepSessionController> {
        self.controller.as_mut().ok_or_else(|| GuiError::NoBackend {
            source: lidcore::LidError::UnsupportedPlatform {
                os: std::env::consts::OS,
            },
        })
    }

    /// One supervision pass. Returns true if the hold was released.
    pub fn tick(&mut self) -> bool {
        let Some(controller) = self.controller.as_mut() else {
            return false;
        };
        match controller.tick() {
            Ok(released) => {
                if released {
                    self.lock = None;
                }
                released
            }
            Err(error) => {
                error!(%error, "supervision tick failed");
                false
            }
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
