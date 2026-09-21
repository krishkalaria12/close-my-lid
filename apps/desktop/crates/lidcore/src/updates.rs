//! Update discovery without Sparkle.
//!
//! Earlier releases embedded Sparkle 2, which discovered, downloaded,
//! installed and relaunched updates. This does the one job that matters
//! unattended — noticing that a newer release exists — against the same
//! committed `appcast.xml` feed the site and those releases used. Download and
//! install stay a deliberate user action: the panel shows the new version and
//! opens the release page on click.
//!
//! That trade is what lets the bundle be a single executable instead of an
//! executable plus a 4 MB framework.
//!
//! Fetching uses the system `curl`, which is always present on macOS, so this
//! adds no dependency. Parsing is string-based for the same reason; the feed
//! format is fixed and validated by `scripts/validate-appcast.rb`.

use std::process::Command;

use crate::config::VERSION;

/// The stable update feed, mirroring the `SUFeedURL` the packaged app used.
pub const APPCAST_URL: &str =
    "https://raw.githubusercontent.com/krishkalaria12/close-my-lid/main/appcast.xml";

/// A newer release found on the feed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateInfo {
    pub version: String,
    pub url: String,
}

/// Checks the feed for a release newer than this build. Returns `Ok(None)`
/// when up to date, and an error — deliberately quiet in the UI — when the
/// network or feed is unavailable.
pub fn check_for_updates() -> Result<Option<UpdateInfo>, String> {
    let output = Command::new("/usr/bin/curl")
        .args(["-fsSL", "--max-time", "20", APPCAST_URL])
        .output()
        .map_err(|error| format!("could not fetch the update feed: {error}"))?;

    if !output.status.success() {
        return Err(format!(
            "the update feed request failed with status {}",
            output.status
        ));
    }

    let feed = String::from_utf8_lossy(&output.stdout);
    Ok(latest_release(&feed).filter(|info| is_newer(&info.version, VERSION)))
}

/// The newest `<item>` on the feed: its `<sparkle:shortVersionString>` and its
/// enclosure download URL. Pure so the feed format is testable offline.
pub fn latest_release(feed: &str) -> Option<UpdateInfo> {
    // Only the first item matters; the feed is newest-first.
    let item = feed.split("<item>").nth(1)?.split("</item>").next()?;

    let version = tag_contents(item, "sparkle:shortVersionString")?;
    let url = enclosure_url(item)?;

    Some(UpdateInfo {
        version: version.trim().to_string(),
        url,
    })
}

/// True when `candidate` is a newer dotted version than `current`.
/// Non-numeric suffixes are ignored, so `0.5.0-beta.1` still compares.
pub fn is_newer(candidate: &str, current: &str) -> bool {
    fn parts(version: &str) -> Vec<u64> {
        version
            .split('.')
            .map(|part| {
                part.chars()
                    .take_while(char::is_ascii_digit)
                    .collect::<String>()
                    .parse()
                    .unwrap_or(0)
            })
            .collect()
    }

    let (mut candidate, mut current) = (parts(candidate), parts(current));
    let width = candidate.len().max(current.len());
    candidate.resize(width, 0);
    current.resize(width, 0);
    candidate > current
}

fn tag_contents(item: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = item.find(&open)? + open.len();
    let end = item[start..].find(&close)?;
    Some(item[start..start + end].to_string())
}

fn enclosure_url(item: &str) -> Option<String> {
    let start = item.find("<enclosure")?;
    let rest = &item[start..];
    let url_attr = "url=\"";
    let url_start = rest.find(url_attr)? + url_attr.len();
    let url_end = rest[url_start..].find('"')?;
    Some(rest[url_start..url_start + url_end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FEED: &str = r#"<?xml version="1.0" standalone="yes"?>
<rss xmlns:sparkle="http://www.andymatuschak.org/xml-namespaces/sparkle" version="2.0">
    <channel>
        <title>Close My Lid</title>
        <item>
            <title>0.5.0</title>
            <sparkle:version>9</sparkle:version>
            <sparkle:shortVersionString>0.5.0</sparkle:shortVersionString>
            <enclosure url="https://github.com/krishkalaria12/close-my-lid/releases/download/v0.5.0/Close-My-Lid-v0.5.0-macOS.zip" length="1" type="application/octet-stream" sparkle:edSignature="x"/>
        </item>
        <item>
            <title>0.4.4</title>
            <sparkle:version>8</sparkle:version>
            <sparkle:shortVersionString>0.4.4</sparkle:shortVersionString>
            <enclosure url="https://github.com/krishkalaria12/close-my-lid/releases/download/v0.4.4/Close-My-Lid-v0.4.4-macOS.zip" length="1" type="application/octet-stream" sparkle:edSignature="y"/>
        </item>
    </channel>
</rss>"#;

    #[test]
    fn parses_the_newest_item() {
        let info = latest_release(FEED).unwrap();
        assert_eq!(info.version, "0.5.0");
        assert!(
            info.url.ends_with("Close-My-Lid-v0.5.0-macOS.zip"),
            "{}",
            info.url
        );
    }

    #[test]
    fn garbage_feeds_yield_nothing() {
        assert_eq!(latest_release(""), None);
        assert_eq!(latest_release("<rss></rss>"), None);
        assert_eq!(latest_release("<item><title>x</title></item>"), None);
    }

    #[test]
    fn version_comparison_ignores_width_and_suffixes() {
        assert!(is_newer("0.5.0", "0.4.4"));
        assert!(is_newer("0.4.10", "0.4.9"));
        assert!(is_newer("1.0.0", "0.99.99"));
        assert!(!is_newer("0.4.4", "0.4.4"));
        assert!(!is_newer("0.4.3", "0.4.4"));
        assert!(!is_newer("0.4.4", "0.5.0"));
    }
}
