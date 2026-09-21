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
//! macOS is deliberately not implemented here — it is served by the shipping
//! Swift app in `apps/macos`. [`power::backend`] returns
//! [`LidError::UnsupportedPlatform`] there so this crate still builds on a Mac
//! for development.

pub mod battery;
pub mod duration;
pub mod error;
pub mod power;
pub mod session;
pub mod state;
pub mod store;

pub use battery::{BatterySafetyPolicy, BatteryStatus};
pub use duration::SessionDuration;
pub use error::LidError;
pub use power::{LidPowerBackend, backend};
pub use session::SleepSessionController;
pub use state::SleepControlState;
pub use store::SleepSessionStore;

/// Reverse-DNS identifier, used for the single-instance lock, the config
/// directory and the systemd user unit name.
pub const APP_ID: &str = "com.krishkalaria.close-my-lid";

/// Human-facing app name.
pub const APP_NAME: &str = "Close My Lid";

/// Kept in sync with the root `package.json` and the Swift `CommandLineInterface`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
