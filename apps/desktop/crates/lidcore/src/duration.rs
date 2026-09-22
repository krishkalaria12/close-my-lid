use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

/// How long a hold should last. Mirrors the presets in the macOS menu panel so
/// all three platforms offer the same choices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionDuration {
    /// Runs until stopped, the battery safety threshold trips, or the app quits.
    Indefinite,
    Timed {
        minutes: i64,
    },
}

/// The longest hold `--for` will accept.
///
/// A bound is needed rather than merely rejecting overflow: `chrono` measures a
/// `Duration` in milliseconds, so a minute count in the quadrillions panics
/// before it ever reaches a session. A year is far past any real lid hold and
/// leaves the error message something a user can act on.
const MAX_MINUTES: i64 = 365 * 24 * 60;

impl SessionDuration {
    pub const THIRTY_MINUTES: Self = Self::Timed { minutes: 30 };
    pub const ONE_HOUR: Self = Self::Timed { minutes: 60 };
    pub const FOUR_HOURS: Self = Self::Timed { minutes: 240 };

    /// The presets shown in the UI, in display order.
    pub const PRESETS: [Self; 4] = [
        Self::THIRTY_MINUTES,
        Self::ONE_HOUR,
        Self::FOUR_HOURS,
        Self::Indefinite,
    ];

    /// When a session started at `start` should end, or `None` if indefinite.
    pub fn end_at(&self, start: DateTime<Utc>) -> Option<DateTime<Utc>> {
        match self {
            Self::Indefinite => None,
            Self::Timed { minutes } => Some(start + Duration::minutes(*minutes)),
        }
    }

    pub fn label(&self) -> String {
        match self {
            Self::Indefinite => "Unlimited".to_string(),
            Self::Timed { minutes } if *minutes < 60 => format!("{minutes} min"),
            Self::Timed { minutes } if *minutes == 60 => "1 hour".to_string(),
            Self::Timed { minutes } if minutes % 60 == 0 => {
                format!("{} hours", minutes / 60)
            }
            // Non-hour multiples (e.g. `--for 90`): previously rendered as
            // "1 hours" via truncating division. Show both parts instead.
            Self::Timed { minutes } => {
                format!("{}h {}m", minutes / 60, minutes % 60)
            }
        }
    }

    /// Parses the CLI's `--for` value: `30m`, `2h`, `90` (minutes), or
    /// `unlimited` / `forever`.
    pub fn parse(input: &str) -> std::result::Result<Self, String> {
        let raw = input.trim().to_lowercase();
        if matches!(raw.as_str(), "unlimited" | "forever" | "indefinite") {
            return Ok(Self::Indefinite);
        }

        let (value, multiplier) = match raw.strip_suffix('h') {
            Some(value) => (value, 60),
            None => (raw.strip_suffix('m').unwrap_or(&raw), 1),
        };

        let parsed: i64 = value.trim().parse().map_err(|_| {
            format!("could not read a duration from {input:?}; try 30m, 2h or unlimited")
        })?;

        if parsed <= 0 {
            return Err("a duration must be greater than zero".to_string());
        }

        // `parsed * multiplier` used to be a plain multiply. Release builds
        // have overflow checks off, so `--for 200000000000000000h` wrapped to a
        // *negative* minute count: the hold was taken and then expired on the
        // spot, or `chrono::Duration::minutes` panicked on the way there.
        let minutes = parsed
            .checked_mul(multiplier)
            .filter(|minutes| *minutes <= MAX_MINUTES)
            .ok_or_else(|| {
                format!("a duration must be no longer than {MAX_MINUTES} minutes (one year)")
            })?;

        Ok(Self::Timed { minutes })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_supported_spellings() {
        assert_eq!(
            SessionDuration::parse("30m").unwrap(),
            SessionDuration::THIRTY_MINUTES
        );
        assert_eq!(
            SessionDuration::parse("2h").unwrap(),
            SessionDuration::Timed { minutes: 120 }
        );
        assert_eq!(
            SessionDuration::parse("90").unwrap(),
            SessionDuration::Timed { minutes: 90 }
        );
        assert_eq!(
            SessionDuration::parse("Unlimited").unwrap(),
            SessionDuration::Indefinite
        );
    }

    #[test]
    fn rejects_nonsense_and_zero() {
        assert!(SessionDuration::parse("soon").is_err());
        assert!(SessionDuration::parse("0m").is_err());
        assert!(SessionDuration::parse("-5").is_err());
    }

    #[test]
    fn a_duration_too_large_to_hold_is_refused_rather_than_wrapped() {
        // Release builds have overflow checks off, so the multiply used to wrap
        // to a negative minute count — a hold that expired the moment it began.
        for absurd in ["200000000000000000h", "9223372036854775807h", "999999999m"] {
            let error = SessionDuration::parse(absurd).unwrap_err();
            assert!(error.contains("no longer than"), "{absurd}: {error}");
        }
        // The bound itself is still accepted, and still lands on a real end.
        let year = SessionDuration::parse(&format!("{MAX_MINUTES}")).unwrap();
        assert!(year.end_at(Utc::now()).is_some());
    }

    #[test]
    fn indefinite_sessions_have_no_end() {
        assert!(SessionDuration::Indefinite.end_at(Utc::now()).is_none());
    }

    #[test]
    fn labels_do_not_truncate_partial_hours() {
        assert_eq!(SessionDuration::Timed { minutes: 90 }.label(), "1h 30m");
        assert_eq!(SessionDuration::Timed { minutes: 120 }.label(), "2 hours");
        assert_eq!(SessionDuration::THIRTY_MINUTES.label(), "30 min");
    }
}
