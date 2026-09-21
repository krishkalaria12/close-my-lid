//! Fallback so the workspace still builds on platforms with no backend.
//!
//! Linux, Windows and macOS each have a real backend; anything else gets this
//! error so the failure names the platform instead of failing to compile.

use crate::error::{LidError, Result};
use crate::power::LidPowerBackend;

pub struct UnsupportedBackend;

impl LidPowerBackend for UnsupportedBackend {
    fn acquire(&mut self) -> Result<()> {
        Err(LidError::UnsupportedPlatform {
            os: std::env::consts::OS,
        })
    }

    fn release(&mut self) -> Result<()> {
        Err(LidError::UnsupportedPlatform {
            os: std::env::consts::OS,
        })
    }

    fn is_held(&self) -> Result<bool> {
        Err(LidError::UnsupportedPlatform {
            os: std::env::consts::OS,
        })
    }

    fn describe(&self) -> &'static str {
        "unsupported platform"
    }
}
