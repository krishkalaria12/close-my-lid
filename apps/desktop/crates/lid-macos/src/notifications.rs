//! Delivery of the session messages `lidcore::notify` plans.
//!
//! Carried over from the Swift app's `SessionNotificationScheduler`. The
//! "started" message is delivered immediately; "ending soon" and "ended" are
//! handed to the system with their own fire dates, so their timing does not
//! depend on the app still running or on a reconciliation tick landing at the
//! right moment.

use block2::RcBlock;
use lidcore::notify::{ScheduledNotification, SessionNotificationPlan};
use objc2::rc::Retained;
use objc2_foundation::{NSArray, NSBundle, NSString};
use objc2_user_notifications::{
    UNAuthorizationOptions, UNMutableNotificationContent, UNNotificationRequest,
    UNNotificationTrigger, UNTimeIntervalNotificationTrigger, UNUserNotificationCenter,
};
use tracing::debug;

/// Stable identifiers, so re-planning a session replaces its own messages
/// instead of stacking a second set on top.
mod identifier {
    pub const STARTED: &str = "app.closemylid.notification.started";
    pub const ENDING_SOON: &str = "app.closemylid.notification.ending-soon";
    pub const ENDED: &str = "app.closemylid.notification.ended";
    /// The battery safety release, which is not part of any session plan:
    /// posting it under `STARTED` would let the next hold's "started" message
    /// replace an explanation the user has not read yet, and vice versa.
    pub const BATTERY: &str = "app.closemylid.notification.battery";

    /// The planned messages, which a new plan replaces. `BATTERY` is
    /// deliberately absent: it is delivered immediately and never pending, so
    /// there is nothing about it to cancel.
    pub const PLANNED: [&str; 3] = [STARTED, ENDING_SOON, ENDED];
}

pub struct Notifier {
    /// `None` when the process has no bundle identifier.
    center: Option<Retained<UNUserNotificationCenter>>,
}

impl Notifier {
    /// `UNUserNotificationCenter.current()` raises when the process has no
    /// bundle identifier — which is every `cargo run` — so notifications are
    /// wired up only for the packaged app.
    pub fn new() -> Self {
        let bundled = NSBundle::mainBundle().bundleIdentifier().is_some();
        if !bundled {
            debug!("running unbundled; notifications are disabled");
            return Self { center: None };
        }

        let center = UNUserNotificationCenter::currentNotificationCenter();
        let options = UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound;
        // The result is deliberately ignored: a user who declines simply gets
        // no messages, which must not stop the hold from working.
        let completion = RcBlock::new(
            |_granted: objc2::runtime::Bool, _error: *mut objc2_foundation::NSError| {},
        );
        center.requestAuthorizationWithOptions_completionHandler(options, &completion);

        Self {
            center: Some(center),
        }
    }

    /// Replaces this session's messages with the ones in `plan`.
    pub fn apply(&self, plan: &SessionNotificationPlan) {
        let Some(center) = &self.center else {
            return;
        };

        self.cancel_pending();
        add(
            center,
            identifier::STARTED,
            &plan.start_title,
            &plan.start_body,
            None,
        );

        for (identifier, notification) in [
            (identifier::ENDING_SOON, plan.ending_soon.as_ref()),
            (identifier::ENDED, plan.ended.as_ref()),
        ] {
            let Some(notification) = notification else {
                continue;
            };
            let Some(trigger) = trigger_for(notification) else {
                continue;
            };
            add(
                center,
                identifier,
                &notification.title,
                &notification.body,
                Some(&trigger),
            );
        }
    }

    /// Delivers the battery safety notice right now, outside any plan. The
    /// release ends a hold the user did not ask to end, so it has to say so.
    pub fn report_battery_release(&self, title: &str, body: &str) {
        if let Some(center) = &self.center {
            add(center, identifier::BATTERY, title, body, None);
        }
    }

    /// Drops what has not been shown yet, leaving delivered messages — an
    /// "ended" notice the user has just read should not vanish from Notification
    /// Centre because the app tidied up behind it.
    pub fn cancel_pending(&self) {
        let Some(center) = &self.center else {
            return;
        };
        let identifiers: Vec<Retained<NSString>> = identifier::PLANNED
            .iter()
            .map(|id| NSString::from_str(id))
            .collect();
        center.removePendingNotificationRequestsWithIdentifiers(&NSArray::from_retained_slice(
            &identifiers,
        ));
    }
}

fn trigger_for(
    notification: &ScheduledNotification,
) -> Option<Retained<UNTimeIntervalNotificationTrigger>> {
    let seconds = (notification.fire_at - chrono::Utc::now()).as_seconds_f64();
    // A fire date already in the past would be delivered immediately, which
    // for "5 minutes left" is worse than saying nothing.
    if seconds <= 0.0 {
        return None;
    }
    Some(UNTimeIntervalNotificationTrigger::triggerWithTimeInterval_repeats(seconds, false))
}

fn add(
    center: &UNUserNotificationCenter,
    identifier: &str,
    title: &str,
    body: &str,
    trigger: Option<&UNTimeIntervalNotificationTrigger>,
) {
    let content = UNMutableNotificationContent::new();
    content.setTitle(&NSString::from_str(title));
    content.setBody(&NSString::from_str(body));

    let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
        &NSString::from_str(identifier),
        &content,
        trigger.map(|trigger| trigger.as_ref() as &UNNotificationTrigger),
    );
    center.addNotificationRequest_withCompletionHandler(&request, None);
}
