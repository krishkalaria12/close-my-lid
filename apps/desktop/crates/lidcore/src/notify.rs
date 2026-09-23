//! What a hold session should tell the user, and when.
//!
//! Carried over from the Swift app's `SessionNotificationPlanner`. Pure and
//! deterministic so the timing and the copy can be tested without a
//! notification framework: the platform layer only has to deliver what this
//! produces.

use chrono::{DateTime, Duration, Utc};

use crate::config::APP_NAME;
use crate::duration::SessionDuration;

/// What the copy calls the machine. "Mac" is what people call theirs; on the
/// other platforms the closed lid is the one thing every target has in common.
#[cfg(target_os = "macos")]
const MACHINE: &str = "Mac";
#[cfg(not(target_os = "macos"))]
const MACHINE: &str = "laptop";

/// How far ahead of the end the "ending soon" warning fires.
pub const ENDING_SOON_LEAD: Duration = Duration::minutes(5);

/// One notification to deliver at a specific time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledNotification {
    pub fire_at: DateTime<Utc>,
    pub title: String,
    pub body: String,
}

/// Every notification for one hold session: an immediate "started" message
/// plus, for timed sessions, an "ending soon" warning and an "ended" message
/// scheduled at their own times.
///
/// The scheduled two are handed to the system rather than fired by a timer in
/// the app, so their delivery does not depend on the app still running — or on
/// a reconciliation tick landing at the right moment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionNotificationPlan {
    pub start_title: String,
    pub start_body: String,
    pub ending_soon: Option<ScheduledNotification>,
    pub ended: Option<ScheduledNotification>,
}

pub fn plan(duration: SessionDuration, started_at: DateTime<Utc>) -> SessionNotificationPlan {
    let start_body = start_body(duration);

    let Some(ends_at) = duration.end_at(started_at) else {
        return SessionNotificationPlan {
            start_title: APP_NAME.to_string(),
            start_body,
            ending_soon: None,
            ended: None,
        };
    };

    SessionNotificationPlan {
        start_title: APP_NAME.to_string(),
        start_body,
        ending_soon: ending_soon(started_at, ends_at),
        ended: Some(ScheduledNotification {
            fire_at: ends_at,
            title: APP_NAME.to_string(),
            body: format!("Your {MACHINE} now sleeps normally when the lid is closed."),
        }),
    }
}

fn start_body(duration: SessionDuration) -> String {
    match duration {
        SessionDuration::Indefinite => {
            format!("Your {MACHINE} will stay awake with the lid closed until you stop it.")
        }
        SessionDuration::Timed { .. } => format!(
            "Your {MACHINE} will stay awake with the lid closed for the next {}.",
            duration.label().to_lowercase()
        ),
    }
}

/// `None` for a session shorter than the lead time — a warning that would fire
/// before, or at, the moment the hold started tells the user nothing.
fn ending_soon(started_at: DateTime<Utc>, ends_at: DateTime<Utc>) -> Option<ScheduledNotification> {
    let fire_at = ends_at - ENDING_SOON_LEAD;
    (fire_at > started_at).then(|| ScheduledNotification {
        fire_at,
        title: APP_NAME.to_string(),
        body: format!(
            "About 5 minutes left before your {MACHINE} sleeps normally with the lid closed."
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unlimited_sessions_schedule_nothing() {
        let plan = plan(SessionDuration::Indefinite, Utc::now());
        assert!(plan.ending_soon.is_none());
        assert!(plan.ended.is_none());
        assert!(plan.start_body.contains("until you stop it"));
    }

    #[test]
    fn timed_sessions_warn_five_minutes_out_and_announce_the_end() {
        let started = Utc::now();
        let plan = plan(SessionDuration::ONE_HOUR, started);

        let ended = plan.ended.expect("a timed session ends");
        assert_eq!(ended.fire_at, started + Duration::hours(1));

        let warning = plan.ending_soon.expect("an hour is long enough to warn");
        assert_eq!(warning.fire_at, ended.fire_at - Duration::minutes(5));
        assert!(plan.start_body.contains("1 hour"), "{}", plan.start_body);
    }

    #[test]
    fn short_sessions_skip_a_warning_that_would_already_be_due() {
        let started = Utc::now();
        // Exactly the lead time: the warning would fire at the start.
        let plan = plan(SessionDuration::Timed { minutes: 5 }, started);
        assert!(plan.ending_soon.is_none());
        assert!(plan.ended.is_some(), "the end is still worth announcing");

        let plan = plan_for(4, started);
        assert!(plan.ending_soon.is_none());
    }

    fn plan_for(minutes: i64, started: DateTime<Utc>) -> SessionNotificationPlan {
        plan(SessionDuration::Timed { minutes }, started)
    }

    #[test]
    fn every_message_is_titled_with_the_app_name() {
        let plan = plan(SessionDuration::FOUR_HOURS, Utc::now());
        assert_eq!(plan.start_title, APP_NAME);
        assert_eq!(plan.ending_soon.unwrap().title, APP_NAME);
        assert_eq!(plan.ended.unwrap().title, APP_NAME);
    }
}
