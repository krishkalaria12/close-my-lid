//! App state: the hold controller plus everything the window renders.
//!
//! One entity, so the supervision loop, the readout refresh and the view can
//! all touch it, and so the view re-renders whenever any of them changes it.
//! Nothing here knows about gpui beyond being held in an entity; the decisions
//! — what a toggle does, which notification is due — are plain methods.

use std::collections::HashMap;
use std::time::Duration;

use chrono::{DateTime, Utc};
use lidcore::notify::{self, ScheduledNotification};
use lidcore::updates::UpdateInfo;
use lidcore::{
    AgentHarness, BatterySafetyPolicy, BatteryStatus, HoldLock, SessionDuration, SleepControlState,
    SleepSessionController,
};
use tracing::{error, warn};

use crate::config::SUPERVISION_INTERVAL;
use crate::error::{GuiError, Result};
use crate::prefs::{self, GuiPrefs};
use crate::preview;
use crate::system;

/// Where the update check stands, for the footer row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateStatus {
    /// Nothing checked yet this run.
    Unchecked,
    Checking,
    /// Checked by hand and found nothing; shown briefly, then back to
    /// `Unchecked` so the row reads as a button again.
    Current,
    Available(UpdateInfo),
}

/// Why a supervision pass ended the hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Release {
    Expired,
    Battery,
}

pub struct AppState {
    controller: Option<SleepSessionController>,
    /// Held while this app owns the hold, so the app and the CLI cannot take
    /// overlapping holds of the same global setting.
    lock: Option<HoldLock>,
    /// Why the backend could not be built, if it could not be. Held as a typed
    /// error so the panel can show its hint, not just a message.
    pub startup_error: Option<GuiError>,
    /// The last thing that went wrong, until the user dismisses it or the next
    /// action succeeds. Shown in the window rather than only logged: a
    /// release build has no console, and a click that silently does nothing
    /// is the worst possible answer to a refusal.
    pub banner: Option<GuiError>,

    pub battery: Option<BatteryStatus>,
    pub battery_policy: BatterySafetyPolicy,
    pub agents: HashMap<AgentHarness, usize>,
    pub update: UpdateStatus,

    /// The duration Start applies, and so the one the picker shows. Shared
    /// with the macOS app through `lidcore::prefs`.
    pub selected: SessionDuration,
    pub prefs: GuiPrefs,
    pub launch_at_login: bool,

    /// The "ending soon" and "ended" messages of the running hold, delivered
    /// by the supervision loop when their time comes.
    pending: Vec<ScheduledNotification>,
}

impl AppState {
    pub fn new() -> Self {
        let built = if preview::ACTIVE {
            Ok(preview::controller())
        } else {
            SleepSessionController::new()
        };
        let (controller, startup_error) = match built {
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

        Self {
            controller,
            lock: None,
            startup_error,
            banner: None,
            // Read before the first frame, so the battery section is there
            // from the start rather than pushing everything down a moment
            // later. One IOKit or sysfs read; the process walk is the slow
            // part, and that stays on the background executor.
            battery: lidcore::battery::read(),
            battery_policy: BatterySafetyPolicy::default(),
            agents: HashMap::new(),
            update: UpdateStatus::Unchecked,
            selected: lidcore::prefs::load(),
            prefs: prefs::load(),
            launch_at_login: system::launches_at_login(),
            pending: Vec::new(),
        }
    }

    // MARK: readouts

    pub fn state(&self) -> SleepControlState {
        self.controller
            .as_ref()
            .map(|controller| controller.state())
            .unwrap_or(SleepControlState::Inactive)
    }

    pub fn is_active(&self) -> bool {
        self.state().is_active()
    }

    pub fn can_hold(&self) -> bool {
        self.controller.is_some()
    }

    /// How the lid is being held on this system, for the settings page.
    pub fn mechanism(&self) -> Option<&'static str> {
        self.controller
            .as_ref()
            .map(|controller| controller.describe_backend())
    }

    pub fn sessions(&self, harness: AgentHarness) -> usize {
        self.agents.get(&harness).copied().unwrap_or(0)
    }

    pub fn apply_readouts(
        &mut self,
        battery: Option<BatteryStatus>,
        agents: HashMap<AgentHarness, usize>,
    ) {
        self.battery = battery;
        self.agents = agents;
    }

    // MARK: starting and stopping

    /// Start or stop. Starting applies the duration the user last picked
    /// rather than always an unlimited hold.
    pub fn toggle(&mut self) {
        let result = if self.is_active() {
            self.stop()
        } else {
            self.start(self.selected)
        };
        self.settle(result);
    }

    /// The duration picker. While idle it only chooses what Start will do;
    /// while a hold runs it restarts the hold with the new length, measured
    /// from now — which is what someone choosing "1 hour" means.
    pub fn select(&mut self, duration: SessionDuration) {
        if self.is_active() {
            let result = self.start(duration);
            self.settle(result);
            return;
        }
        self.selected = duration;
        self.remember(duration);
    }

    fn start(&mut self, duration: SessionDuration) -> Result<()> {
        // Take the lock before touching the system: if the CLI is already
        // holding, this fails with a message naming that process rather than
        // quietly taking a second, conflicting hold of the same setting.
        if self.lock.is_none() && !preview::ACTIVE {
            self.lock = Some(HoldLock::acquire()?);
        }

        let controller = self.controller_mut()?;
        if let Err(error) = controller.start(duration) {
            self.lock = None;
            return Err(error.into());
        }
        let started_at = controller.state().started_at().unwrap_or_else(Utc::now);

        self.selected = duration;
        self.remember(duration);

        // Planned from the session's own start, so "ending soon" lands five
        // minutes before the real end.
        let plan = notify::plan(duration, started_at);
        self.announce(&plan.start_body);
        self.pending = [plan.ending_soon, plan.ended]
            .into_iter()
            .flatten()
            .collect();
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        self.controller_mut()?.stop()?;
        // Released only after the system change succeeded, so a failed stop
        // does not advertise the hold as available.
        self.lock = None;
        // Stopped by hand: "ending soon" and "ended" are no longer true.
        self.pending.clear();
        Ok(())
    }

    /// Starts a hold of the given length, for the preview's scripted scenes.
    pub fn select_and_start(&mut self, duration: SessionDuration) {
        let result = self.start(duration);
        self.settle(result);
    }

    /// Saves the chosen duration, shared with the macOS app's toggle through
    /// `lidcore::prefs`. A preview build leaves the file alone: the installed
    /// macOS app reads it.
    fn remember(&self, duration: SessionDuration) {
        if !preview::ACTIVE
            && let Err(error) = lidcore::prefs::save(duration)
        {
            // Only the choice is lost; the hold itself is unaffected.
            warn!(%error, "could not remember the chosen duration");
        }
    }

    /// Releases the hold on the way out, if there is one. Quitting must never
    /// leave the lid held — on Windows the power-scheme change would otherwise
    /// outlive the process.
    pub fn release_for_quit(&mut self) {
        if !self.is_active() {
            return;
        }
        if let Err(error) = self.stop() {
            error!(%error, hint = error.hint(), "could not release the hold on quit");
        }
    }

    /// Clears the banner on success, raises it on failure.
    fn settle(&mut self, result: Result<()>) {
        match result {
            Ok(()) => self.banner = None,
            Err(error) => {
                error!(%error, hint = error.hint(), "hold change failed");
                self.banner = Some(error);
            }
        }
    }

    pub fn show_error(&mut self, error: GuiError) {
        warn!(%error, "showing an error");
        self.banner = Some(error);
    }

    pub fn dismiss_banner(&mut self) {
        self.banner = None;
    }

    /// The controller, or an error explaining why there isn't one.
    fn controller_mut(&mut self) -> Result<&mut SleepSessionController> {
        self.controller.as_mut().ok_or(GuiError::NoBackend {
            source: lidcore::LidError::UnsupportedPlatform {
                os: std::env::consts::OS,
            },
        })
    }

    // MARK: supervision

    /// One supervision pass: delivers notifications that have come due, then
    /// lets the controller end an expired hold or one draining the battery.
    pub fn tick(&mut self, now: DateTime<Utc>) -> Option<Release> {
        self.deliver_due(now);

        let controller = self.controller.as_mut()?;
        let expired = controller.state().has_expired(now);
        match controller.tick() {
            Ok(true) => {
                self.lock = None;
                if expired {
                    // Everything still pending was due by the end anyway,
                    // "ended" included.
                    self.deliver_due(DateTime::<Utc>::MAX_UTC);
                    Some(Release::Expired)
                } else {
                    self.pending.clear();
                    self.announce(&format!(
                        "Battery is low, so normal closed-lid sleep is back on at {}%.",
                        self.battery_policy.threshold
                    ));
                    Some(Release::Battery)
                }
            }
            Ok(false) => None,
            Err(error) => {
                error!(%error, "supervision tick failed");
                None
            }
        }
    }

    /// How long the supervision loop may sleep: its usual interval, or less
    /// when the hold ends or a notification is due sooner, so both happen on
    /// the second rather than up to an interval late.
    pub fn next_wake(&self, now: DateTime<Utc>) -> Duration {
        let soonest = self
            .state()
            .ends_at()
            .into_iter()
            .chain(self.pending.iter().map(|note| note.fire_at))
            .min();
        match soonest {
            Some(at) => (at - now)
                .to_std()
                .unwrap_or(Duration::ZERO)
                .clamp(Duration::from_millis(250), SUPERVISION_INTERVAL),
            None => SUPERVISION_INTERVAL,
        }
    }

    fn deliver_due(&mut self, now: DateTime<Utc>) {
        let (due, later): (Vec<_>, Vec<_>) = std::mem::take(&mut self.pending)
            .into_iter()
            .partition(|note| note.fire_at <= now);
        self.pending = later;
        for note in due {
            self.announce(&note.body);
        }
    }

    fn announce(&self, body: &str) {
        if self.prefs.notifications {
            system::notify(body);
        }
    }

    // MARK: settings

    pub fn set_notifications(&mut self, enabled: bool) {
        self.prefs.notifications = enabled;
        if let Err(error) = prefs::save(self.prefs) {
            self.show_error(error);
        }
    }

    pub fn set_launch_at_login(&mut self, enabled: bool) {
        match system::set_launch_at_login(enabled) {
            Ok(()) => self.launch_at_login = enabled,
            Err(error) => self.show_error(error),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
