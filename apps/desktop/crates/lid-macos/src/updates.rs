//! Fetching the release feed.
//!
//! `NSURLSession` rather than a forked `curl`: it is already in the process,
//! it is already asynchronous — so the check needs no worker of its own — and
//! it applies the system's TLS and proxy configuration, which a hand-written
//! `curl` argument list only approximates.
//!
//! Parsing and version comparison live in `lidcore::updates`, which is pure
//! and tested offline. This module only moves bytes.

use block2::RcBlock;
use lidcore::updates::{self, UpdateInfo};
use objc2_foundation::{
    NSData, NSError, NSHTTPURLResponse, NSString, NSURL, NSURLRequest, NSURLRequestCachePolicy,
    NSURLResponse, NSURLSession,
};
use tracing::debug;

/// How long the request may take before the session gives up.
const TIMEOUT_SECONDS: f64 = 20.0;

/// Asks the feed for a release newer than this build.
///
/// Returns immediately; `deliver` runs later on whatever queue the session
/// chose, so callers that touch the UI must hop to the main queue themselves.
/// `None` means "nothing to offer" and covers every failure — a laptop with no
/// network is the normal case for an app that runs all day, so nothing here is
/// worth showing the user.
pub fn check(deliver: impl Fn(Option<UpdateInfo>) + Send + 'static) {
    let Some(url) = NSURL::URLWithString(&NSString::from_str(updates::APPCAST_URL)) else {
        debug!(url = updates::APPCAST_URL, "could not parse the feed URL");
        deliver(None);
        return;
    };

    // Ignore the local cache: the whole point of the check is to learn what
    // changed since last time, and the feed is a few hundred bytes.
    let request = NSURLRequest::requestWithURL_cachePolicy_timeoutInterval(
        &url,
        NSURLRequestCachePolicy::ReloadIgnoringLocalCacheData,
        TIMEOUT_SECONDS,
    );

    let handler = RcBlock::new(
        move |data: *mut NSData, response: *mut NSURLResponse, error: *mut NSError| {
            deliver(found(data, response, error));
        },
    );

    // SAFETY: the handler captures only `deliver`, which the caller declared
    // `Send`, so it is safe for the session to run on its own queue.
    let task = unsafe {
        NSURLSession::sharedSession().dataTaskWithRequest_completionHandler(&request, &handler)
    };
    task.resume();
}

/// Turns one completion into an answer, logging why there is none.
fn found(
    data: *mut NSData,
    response: *mut NSURLResponse,
    error: *mut NSError,
) -> Option<UpdateInfo> {
    if !error.is_null() {
        // SAFETY: a non-null error is a live NSError for the call's duration.
        let error = unsafe { &*error };
        debug!(error = %error.localizedDescription(), "the update check failed");
        return None;
    }

    if let Some(status) = status_code(response)
        && !(200..300).contains(&status)
    {
        debug!(status, "the update feed request was refused");
        return None;
    }

    // SAFETY: a completion with no error hands back the body.
    let data = unsafe { data.as_ref() }?;
    let feed = String::from_utf8_lossy(unsafe { data.as_bytes_unchecked() }).into_owned();
    updates::newer_release(&feed)
}

fn status_code(response: *mut NSURLResponse) -> Option<isize> {
    // SAFETY: a non-null response is live for the call's duration.
    let response = unsafe { response.as_ref() }?;
    // Only an HTTP response carries a status; the feed is always fetched over
    // HTTPS, but a redirect to another scheme would not be.
    Some(response.downcast_ref::<NSHTTPURLResponse>()?.statusCode())
}
