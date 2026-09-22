use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

/// Whether a hold is running, and until when.
///
/// Carried over from the Swift app's `SleepControlState`, so all platforms
/// agree on what a session is and how it is persisted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum SleepControlState {
    #[default]
    Inactive,
    Active {
        started_at: DateTime<Utc>,
        /// `None` for an unlimited hold.
        ends_at: Option<DateTime<Utc>>,
    },
}

impl SleepControlState {
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Active { .. })
    }

    pub fn ends_at(&self) -> Option<DateTime<Utc>> {
        match self {
            Self::Active { ends_at, .. } => *ends_at,
            Self::Inactive => None,
        }
    }

    /// When the running session began, or `None` if none is running.
    ///
    /// Needed by anything that has to line up with the session's own clock —
    /// notably the notification plan, whose fire dates must match the end the
    /// session actually recorded rather than the moment the request was made.
    pub fn started_at(&self) -> Option<DateTime<Utc>> {
        match self {
            Self::Active { started_at, .. } => Some(*started_at),
            Self::Inactive => None,
        }
    }

    /// True once a timed session has reached its end. Unlimited sessions never
    /// expire.
    pub fn has_expired(&self, now: DateTime<Utc>) -> bool {
        matches!(self.ends_at(), Some(end) if end <= now)
    }

    pub fn remaining(&self, now: DateTime<Utc>) -> Option<Duration> {
        self.ends_at().map(|end| (end - now).max(Duration::zero()))
    }

    /// The one-line status shown in the tray panel and by `close-my-lid status`.
    pub fn summary(&self, now: DateTime<Utc>) -> String {
        match self {
            Self::Inactive => "Off — sleeps normally".to_string(),
            Self::Active {
                started_at,
                ends_at: None,
            } => {
                format!("Awake — lid-proof · {}", clock(now - *started_at))
            }
            Self::Active {
                ends_at: Some(_), ..
            } => {
                let left = self.remaining(now).unwrap_or_else(Duration::zero);
                format!("Awake — lid-proof · {} left", clock(left))
            }
        }
    }
}

/// Formats a span the way the macOS panel does: `45m`, `1h 5m`.
///
/// Whole minutes, floored. The minutes used to be floored *and* then raised to
/// at least one, which made a session with nothing left read "1m left" — the
/// countdown's one chance to say something untrue, at the exact moment the user
/// is watching it to see whether the hold is about to end.
fn clock(span: Duration) -> String {
    let total = span.num_seconds().max(0);
    let hours = total / 3600;
    let minutes = (total % 3600) / 60;
    if hours == 0 {
        format!("{minutes}m")
    } else {
        format!("{hours}h {minutes}m")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn active_for(minutes: i64) -> (SleepControlState, DateTime<Utc>) {
        let now = Utc::now();
        let state = SleepControlState::Active {
            started_at: now,
            ends_at: Some(now + Duration::minutes(minutes)),
        };
        (state, now)
    }

    #[test]
    fn timed_sessions_expire_at_their_end() {
        let (state, now) = active_for(30);
        assert!(!state.has_expired(now));
        assert!(state.has_expired(now + Duration::minutes(30)));
    }

    #[test]
    fn unlimited_sessions_never_expire() {
        let state = SleepControlState::Active {
            started_at: Utc::now(),
            ends_at: None,
        };
        assert!(!state.has_expired(Utc::now() + Duration::days(365)));
        assert!(state.remaining(Utc::now()).is_none());
    }

    #[test]
    fn a_finished_countdown_does_not_claim_a_minute_it_has_not_got() {
        let (state, now) = active_for(30);
        let summary = state.summary(now + Duration::minutes(30));
        assert!(summary.contains("0m left"), "{summary}");

        let summary = state.summary(now + Duration::minutes(29));
        assert!(summary.contains("1m left"), "{summary}");
    }

    #[test]
    fn remaining_never_goes_negative() {
        let (state, now) = active_for(10);
        assert_eq!(
            state.remaining(now + Duration::minutes(45)),
            Some(Duration::zero())
        );
    }

    #[test]
    fn round_trips_through_json() {
        let (state, _) = active_for(60);
        let encoded = serde_json::to_string(&state).unwrap();
        assert_eq!(
            serde_json::from_str::<SleepControlState>(&encoded).unwrap(),
            state
        );
    }
}
