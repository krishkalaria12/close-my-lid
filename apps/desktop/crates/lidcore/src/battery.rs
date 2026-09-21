//! Battery reading and the low-battery safety release.
//!
//! Holding the lid open on a draining battery is how you come back to a dead
//! laptop, so a hold is dropped at the threshold when unplugged. Ported from
//! the Swift app's `BatterySafetyPolicy`.

use std::cell::RefCell;

use starship_battery::{Manager, State};
use tracing::debug;

use crate::config;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BatteryStatus {
    /// 0–100.
    pub percentage: u8,
    pub is_charging: bool,
}

/// Releases a hold at `threshold` percent while on battery.
#[derive(Debug, Clone, Copy)]
pub struct BatterySafetyPolicy {
    pub threshold: u8,
}

impl Default for BatterySafetyPolicy {
    fn default() -> Self {
        Self {
            threshold: config::BATTERY_RELEASE_THRESHOLD,
        }
    }
}

impl BatterySafetyPolicy {
    pub fn should_release(&self, status: BatteryStatus) -> bool {
        !status.is_charging && status.percentage <= self.threshold
    }
}

thread_local! {
    /// Built once per thread and reused.
    ///
    /// `Manager::new` opens a platform handle — an IOKit service match on
    /// macOS — and the panel reads the battery every five seconds while it is
    /// open, on top of every reconciliation pass. Rebuilding that handle for
    /// each reading was the most expensive part of a read that is otherwise
    /// a couple of property lookups.
    ///
    /// Thread-local rather than global because `Manager` is not `Sync`, and
    /// the readers — the menu bar app's main thread, the CLI's supervision
    /// loop — are one per thread anyway.
    static MANAGER: RefCell<Option<Manager>> = const { RefCell::new(None) };
}

/// Current battery, or `None` on a desktop with no battery — in which case the
/// UI hides the section entirely, as the macOS panel does.
pub fn read() -> Option<BatteryStatus> {
    MANAGER.with(|cell| {
        let mut cell = cell.borrow_mut();
        if cell.is_none() {
            *cell = Some(Manager::new().ok()?);
        }
        let manager = cell.as_ref()?;

        let mut batteries = manager.batteries().ok()?;
        let battery = batteries.next()?.ok()?;

        let percentage = (battery.state_of_charge().value * 100.0)
            .round()
            .clamp(0.0, 100.0) as u8;
        let is_charging = matches!(battery.state(), State::Charging | State::Full);

        debug!(percentage, is_charging, "read battery");
        Some(BatteryStatus {
            percentage,
            is_charging,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn releases_at_the_threshold_on_battery() {
        let policy = BatterySafetyPolicy::default();
        assert!(policy.should_release(BatteryStatus {
            percentage: 5,
            is_charging: false
        }));
        assert!(policy.should_release(BatteryStatus {
            percentage: 1,
            is_charging: false
        }));
    }

    #[test]
    fn holds_above_the_threshold() {
        let policy = BatterySafetyPolicy::default();
        assert!(!policy.should_release(BatteryStatus {
            percentage: 6,
            is_charging: false
        }));
    }

    #[test]
    fn repeated_reads_agree() {
        // The manager is built once and reused; a second reading must still
        // work and must not contradict the first.
        let (first, second) = (read(), read());
        assert_eq!(first.is_some(), second.is_some());
        if let (Some(first), Some(second)) = (first, second) {
            assert!(first.percentage.abs_diff(second.percentage) <= 1);
        }
    }

    #[test]
    fn never_releases_while_charging() {
        let policy = BatterySafetyPolicy::default();
        assert!(!policy.should_release(BatteryStatus {
            percentage: 1,
            is_charging: true
        }));
    }
}
