//! The dead-man switch: restores normal closed-lid sleep when a hold looks
//! stranded — the owning app crashed, was force-quit, or missed a timed
//! session's end by more than the policy grace.
//!
//! Carried over from the Swift app's `HoldHeartbeat` / `WatchdogPolicy` /
//! `WatchdogRunner` trio. The running app refreshes the heartbeat file while a
//! hold is active; the watchdog LaunchAgent invokes the same binary with
//! `--watchdog` every minute, which performs one [`run_once`] pass.
//!
//! That pass must never prompt, so it uses the passwordless-only executor: when
//! the sudoers grant is missing, releasing fails and the heartbeat is left for
//! the next tick instead of spawning an administrator dialog from a background
//! agent.

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::config;
use crate::error::{LidError, Result};
use crate::power::LidPowerBackend;

/// A liveness record written by the app while a closed-lid hold is active.
///
/// The watchdog compares this file against wall-clock time to detect a stranded
/// `pmset disablesleep 1` after a crash or force-quit. `updated_at` is
/// refreshed by the running app; `ends_at` is the session's scheduled end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HoldHeartbeat {
    pub ends_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

/// Reads and writes the heartbeat file shared between the app and the watchdog
/// agent. Both run as the current user, so no elevation is needed.
#[derive(Debug, Clone)]
pub struct HoldHeartbeatStore {
    path: PathBuf,
}

impl HoldHeartbeatStore {
    pub fn new() -> Result<Self> {
        Ok(Self {
            path: config::heartbeat_file()?,
        })
    }

    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    pub fn write(&self, heartbeat: &HoldHeartbeat) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|error| LidError::io("create", parent, error))?;
        }
        let encoded = serde_json::to_string(heartbeat).map_err(|source| LidError::Encode {
            path: self.path.clone(),
            source,
        })?;
        fs::write(&self.path, encoded).map_err(|error| LidError::io("write", &self.path, error))
    }

    pub fn load(&self) -> Option<HoldHeartbeat> {
        let raw = fs::read_to_string(&self.path).ok()?;
        serde_json::from_str(&raw).ok()
    }

    pub fn remove(&self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// Decides when the watchdog should force normal sleep behaviour back on.
///
/// Two failure modes are covered:
/// - **Dead app:** the heartbeat stopped refreshing (crash, force quit, power
///   loss), so the app cannot clean up after itself.
/// - **Missed expiry:** a timed hold ran past its end plus grace without the
///   app releasing it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WatchdogPolicy {
    /// A heartbeat older than this means the owning app is gone. The app
    /// refreshes its heartbeat every supervision tick (~15s).
    pub liveness_interval: Duration,
    /// Extra time past a timed session's end before forced release.
    pub expiry_grace: Duration,
}

impl Default for WatchdogPolicy {
    fn default() -> Self {
        Self {
            liveness_interval: Duration::from_secs(3 * 60),
            expiry_grace: Duration::from_secs(2 * 60),
        }
    }
}

impl WatchdogPolicy {
    /// True when the recorded hold looks stranded at `now`.
    pub fn should_release(&self, heartbeat: Option<HoldHeartbeat>, now: DateTime<Utc>) -> bool {
        let Some(heartbeat) = heartbeat else {
            return false;
        };

        let app_is_gone = now
            .signed_duration_since(heartbeat.updated_at)
            .to_std()
            .unwrap_or(Duration::ZERO)
            > self.liveness_interval;
        let hold_expired = match heartbeat.ends_at {
            Some(ends_at) => {
                now.signed_duration_since(ends_at)
                    .to_std()
                    .unwrap_or(Duration::ZERO)
                    > self.expiry_grace
            }
            None => false,
        };

        app_is_gone || hold_expired
    }
}

/// Performs one watchdog pass. Returns true when normal sleep behaviour was
/// restored.
pub fn run_once(
    store: &HoldHeartbeatStore,
    policy: &WatchdogPolicy,
    backend: &mut impl LidPowerBackend,
    now: DateTime<Utc>,
) -> Result<bool> {
    let heartbeat = store.load();

    if !policy.should_release(heartbeat, now) {
        return Ok(false);
    }

    if !backend.is_held()? {
        // Nothing stranded; drop any stale marker so future passes are free.
        store.remove();
        return Ok(false);
    }

    backend.release()?;
    store.remove();
    debug!("watchdog restored normal sleep behaviour");
    Ok(true)
}

/// Current wall-clock time as a [`DateTime`].
pub fn now_utc() -> DateTime<Utc> {
    DateTime::from(SystemTime::now())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeDelta;

    fn make_heartbeat(
        store_name: &str,
        ends_in_minutes: Option<i64>,
        updated_seconds_ago: i64,
    ) -> (HoldHeartbeatStore, HoldHeartbeat) {
        let now = Utc::now();
        let store = HoldHeartbeatStore::at(std::env::temp_dir().join(store_name));
        store.remove();
        let heartbeat = HoldHeartbeat {
            ends_at: ends_in_minutes.map(|minutes| now + TimeDelta::minutes(minutes)),
            updated_at: now - TimeDelta::seconds(updated_seconds_ago),
        };
        (store, heartbeat)
    }

    #[test]
    fn a_fresh_heartbeat_is_left_alone() {
        let policy = WatchdogPolicy::default();
        let now = Utc::now();
        let (_, heartbeat) = make_heartbeat("cml-hb-fresh.json", Some(60), 10);
        assert!(!policy.should_release(Some(heartbeat), now));
    }

    #[test]
    fn no_heartbeat_means_nothing_to_release() {
        let policy = WatchdogPolicy::default();
        assert!(!policy.should_release(None, Utc::now()));
    }

    #[test]
    fn a_stale_heartbeat_means_the_app_is_gone() {
        let policy = WatchdogPolicy::default();
        let now = Utc::now();
        let (_, heartbeat) = make_heartbeat("cml-hb-stale.json", None, 10 * 60);
        assert!(policy.should_release(Some(heartbeat), now));
    }

    #[test]
    fn a_hold_past_its_end_plus_grace_is_released() {
        let policy = WatchdogPolicy::default();
        let now = Utc::now();
        // Ended 3 minutes ago: past the 2-minute grace.
        let (_, heartbeat) = make_heartbeat("cml-hb-expired.json", Some(-3), 30);
        assert!(policy.should_release(Some(heartbeat), now));

        // Ended 1 minute ago: still inside the grace.
        let (_, recent) = make_heartbeat("cml-hb-recent.json", Some(-1), 30);
        assert!(!policy.should_release(Some(recent), now));
    }

    #[test]
    fn an_unlimited_hold_with_a_live_app_survives() {
        let policy = WatchdogPolicy::default();
        let now = Utc::now();
        let (_, heartbeat) = make_heartbeat("cml-hb-unlimited.json", None, 30);
        assert!(!policy.should_release(Some(heartbeat), now));
    }

    #[test]
    fn the_store_round_trips_through_json() {
        let (store, heartbeat) = make_heartbeat("cml-hb-roundtrip.json", Some(30), 5);
        store.write(&heartbeat).unwrap();
        assert_eq!(store.load(), Some(heartbeat));
        store.remove();
        assert_eq!(store.load(), None);
    }

    /// Records calls so the watchdog pass can be tested without `pmset`.
    struct FakeBackend {
        held: bool,
        releases: usize,
    }

    impl LidPowerBackend for FakeBackend {
        fn acquire(&mut self) -> Result<()> {
            self.held = true;
            Ok(())
        }
        fn release(&mut self) -> Result<()> {
            self.held = false;
            self.releases += 1;
            Ok(())
        }
        fn is_held(&self) -> Result<bool> {
            Ok(self.held)
        }
        fn describe(&self) -> &'static str {
            "fake"
        }
    }

    #[test]
    fn a_stranded_hold_is_released_and_its_marker_removed() {
        let (store, heartbeat) = make_heartbeat("cml-hb-runonce.json", None, 10 * 60);
        store.write(&heartbeat).unwrap();
        let mut backend = FakeBackend {
            held: true,
            releases: 0,
        };

        let released =
            run_once(&store, &WatchdogPolicy::default(), &mut backend, Utc::now()).unwrap();
        assert!(released);
        assert_eq!(backend.releases, 1);
        assert!(!backend.held);
        assert_eq!(store.load(), None);
    }

    #[test]
    fn a_stale_marker_with_no_hold_is_cleaned_up_silently() {
        let (store, heartbeat) = make_heartbeat("cml-hb-stalemarker.json", None, 10 * 60);
        store.write(&heartbeat).unwrap();
        let mut backend = FakeBackend {
            held: false,
            releases: 0,
        };

        let released =
            run_once(&store, &WatchdogPolicy::default(), &mut backend, Utc::now()).unwrap();
        assert!(!released);
        assert_eq!(backend.releases, 0);
        assert_eq!(store.load(), None);
    }

    #[test]
    fn a_live_hold_is_never_touched() {
        let (store, heartbeat) = make_heartbeat("cml-hb-live.json", Some(60), 10);
        store.write(&heartbeat).unwrap();
        let mut backend = FakeBackend {
            held: true,
            releases: 0,
        };

        let released =
            run_once(&store, &WatchdogPolicy::default(), &mut backend, Utc::now()).unwrap();
        assert!(!released);
        assert_eq!(backend.releases, 0);
        assert!(store.load().is_some());
        store.remove();
    }
}
