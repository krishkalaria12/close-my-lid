//! UI-facing state: the controller plus whatever the panel renders.
//!
//! Lives in its own entity so the supervision loop and the panel can both
//! touch it, and so the panel re-renders when either changes it.

use std::collections::HashMap;

use lidcore::{
    AgentHarness, BatteryStatus, SessionDuration, SleepControlState, SleepSessionController,
    battery,
};
use tracing::{error, warn};

pub struct AppState {
    controller: Option<SleepSessionController>,
    /// Why the backend could not be built, if it could not be.
    pub startup_error: Option<String>,
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
            Err(error) => {
                error!(%error, "no lid backend available");
                (None, Some(error.to_string()))
            }
        };

        let mut state = Self {
            controller,
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

    pub fn start(&mut self, duration: SessionDuration) -> Result<(), String> {
        let controller = self.controller.as_mut().ok_or("no lid backend")?;
        controller
            .start(duration)
            .map_err(|error| error.to_string())
    }

    pub fn stop(&mut self) -> Result<(), String> {
        let controller = self.controller.as_mut().ok_or("no lid backend")?;
        controller.stop().map_err(|error| error.to_string())
    }

    /// One supervision pass. Returns true if the hold was released.
    pub fn tick(&mut self) -> bool {
        let Some(controller) = self.controller.as_mut() else {
            return false;
        };
        match controller.tick() {
            Ok(released) => released,
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
