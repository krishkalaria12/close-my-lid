//! Fallback so the workspace still builds on platforms with no backend —
//! chiefly macOS, where the shipping Swift app in `apps/macos` owns this job.
//!
//! This keeps `lidcore` and `lid-cli` compilable and testable on a Mac, which
//! is where development happens.

use crate::error::{LidError, Result};
use crate::power::LidPowerBackend;

pub struct UnsupportedBackend;

impl LidPowerBackend for UnsupportedBackend {
    fn acquire(&mut self) -> Result<()> {
        Err(LidError::UnsupportedPlatform(std::env::consts::OS))
    }

    fn release(&mut self) -> Result<()> {
        Err(LidError::UnsupportedPlatform(std::env::consts::OS))
    }

    fn is_held(&self) -> Result<bool> {
        Err(LidError::UnsupportedPlatform(std::env::consts::OS))
    }

    fn describe(&self) -> &'static str {
        "unsupported platform"
    }
}
