//! Platform-agnostic core for Close My Lid.
//!
//! This crate owns everything that is not a user interface: the hold state
//! machine, its on-disk persistence, battery safety, agent-session detection,
//! and the per-OS backends that actually keep the machine awake with the lid
//! shut.
//!
//! Nothing here may depend on a UI framework. `lid-cli` and `lid-gui` are both
//! thin shells over this crate, which keeps the UI choice reversible.
//!
//! Two modules are the entry points for anything configurable or fallible:
//! [`config`] holds every tunable value and well-known path, and [`error`]
//! holds every error, each carrying the attempted action and an actionable
//! hint.

pub mod agents;
mod atomic;
pub mod battery;
pub mod config;
pub mod duration;
pub mod error;
#[cfg(target_os = "macos")]
pub mod heartbeat;
#[cfg(target_os = "macos")]
pub mod launchd;
pub mod lock;
#[cfg(target_os = "macos")]
pub mod notify;
pub mod power;
#[cfg(target_os = "macos")]
pub mod prefs;
pub mod session;
pub mod state;
pub mod store;
#[cfg(target_os = "macos")]
pub mod sudoers;
#[cfg(target_os = "macos")]
pub mod updates;

pub use agents::{AgentHarness, RunningProcess, session_counts, sessions_now};
pub use battery::{BatterySafetyPolicy, BatteryStatus};
pub use config::{APP_ID, APP_NAME, VERSION};
pub use duration::SessionDuration;
pub use error::{LidError, Result};
#[cfg(target_os = "macos")]
pub use heartbeat::{HoldHeartbeat, HoldHeartbeatStore, WatchdogPolicy, run_once as watchdog_once};
#[cfg(target_os = "macos")]
pub use launchd::WATCHDOG_ARG;
pub use lock::HoldLock;
#[cfg(target_os = "macos")]
pub use notify::{SessionNotificationPlan, plan as notification_plan};
pub use power::{LidPowerBackend, backend, backend_readonly};
pub use session::SleepSessionController;
pub use state::SleepControlState;
pub use store::SleepSessionStore;
