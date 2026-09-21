//! Platform-agnostic core for Close My Lid.
//!
//! This crate owns everything that is not a user interface: the hold state
//! machine, its on-disk persistence, battery safety, agent-session detection,
//! and the per-OS backends that actually keep the machine awake with the lid
//! shut.
//!
//! Nothing here may depend on a UI framework. `lid-cli` (Linux's primary
//! surface) and `lid-gui` (the Windows tray app) are both thin shells over
//! this crate, which keeps the UI choice reversible.
//!
//! Two modules are the entry points for anything configurable or fallible:
//! [`config`] holds every tunable value and well-known path, and [`error`]
//! holds every error, each carrying the attempted action and an actionable
//! hint.
//!
//! macOS is deliberately not implemented here — it is served by the shipping
//! Swift app in `apps/macos`. [`power::backend`] returns
//! [`LidError::UnsupportedPlatform`] there so this crate still builds on a Mac
//! for development.

pub mod agents;
pub mod battery;
pub mod config;
pub mod duration;
pub mod error;
pub mod lock;
pub mod power;
pub mod session;
pub mod state;
pub mod store;

pub use agents::{AgentHarness, RunningProcess, session_counts, sessions_now};
pub use battery::{BatterySafetyPolicy, BatteryStatus};
pub use config::{APP_ID, APP_NAME, VERSION};
pub use duration::SessionDuration;
pub use error::{LidError, Result};
pub use lock::HoldLock;
pub use power::{LidPowerBackend, backend, backend_readonly};
pub use session::SleepSessionController;
pub use state::SleepControlState;
pub use store::SleepSessionStore;
