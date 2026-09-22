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
//! Everything here is pure: fetching the feed belongs to the platform layer,
//! which on macOS uses `NSURLSession` rather than a subprocess. What is left
//! is parsing (`roxmltree`, because a feed is XML and `str::find` is not an
//! XML parser), version comparison (`semver`, because the ordering of
//! prereleases is not obvious and getting it wrong offers people a downgrade),
//! and the check that the URL being handed to the browser is one of ours.

use semver::Version;
use tracing::warn;

use crate::config::VERSION;

/// The stable update feed, mirroring the `SUFeedURL` the packaged app used.
pub const APPCAST_URL: &str =
    "https://raw.githubusercontent.com/krishkalaria12/close-my-lid/main/appcast.xml";

/// Where a genuine release archive lives.
///
/// Sparkle verified every download against an embedded Ed25519 public key.
/// Nothing is downloaded now, but the URL still reaches `NSWorkspace` and so
/// still decides what the user's browser is pointed at — a feed that had been
/// tampered with could otherwise name any scheme and any host. Constraining it
/// to the project's own releases is the check that replaces the signature.
pub const RELEASE_URL_PREFIX: &str = "https://github.com/krishkalaria12/close-my-lid/releases/";

/// A newer release found on the feed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateInfo {
    pub version: String,
    pub url: String,
}

/// The release to offer given the feed's contents, or `None` when this build
/// is current, the feed is unreadable, or its newest item does not name a
/// download on the project's own releases page.
pub fn newer_release(feed: &str) -> Option<UpdateInfo> {
    latest_release(feed).filter(|info| is_newer(&info.version, VERSION))
}

/// The newest `<item>` on the feed: its `<sparkle:shortVersionString>` and its
/// enclosure download URL. Pure so the feed format is testable offline.
///
/// Rejects an item whose enclosure points anywhere but [`RELEASE_URL_PREFIX`].
pub fn latest_release(feed: &str) -> Option<UpdateInfo> {
    let document = roxmltree::Document::parse(feed)
        .inspect_err(|error| warn!(%error, "could not parse the update feed"))
        .ok()?;

    // The highest version on the feed, not the first element in the document.
    // Ordering was assumed rather than checked, so an item appended to the
    // bottom — or one restored out of order in a merge — published a release
    // nobody was ever offered.
    let newest = document
        .descendants()
        .filter(|node| node.has_tag_name("item"))
        .filter_map(read_item)
        .max_by(|left, right| parse_version(&left.version).cmp(&parse_version(&right.version)))?;

    // Checked on the winner alone. Falling back to the runner-up when the
    // newest item's download is not ours would let a tampered feed steer
    // someone at an older release of its choosing.
    if !is_release_url(&newest.url) {
        warn!(url = %newest.url, "the update feed named a download outside the project's releases");
        return None;
    }

    Some(newest)
}

/// One `<item>`'s version and download URL, or `None` if it carries neither.
fn read_item(item: roxmltree::Node<'_, '_>) -> Option<UpdateInfo> {
    // Matched on local name alone: the version element is namespaced to
    // Sparkle's URI, which the feed is free to bind to any prefix.
    let version = item
        .children()
        .find(|node| node.tag_name().name() == "shortVersionString")
        .and_then(|node| node.text())?
        .trim()
        .to_string();
    if version.is_empty() {
        return None;
    }

    let url = item
        .children()
        .find(|node| node.has_tag_name("enclosure"))
        .and_then(|node| node.attribute("url"))?
        .trim()
        .to_string();

    Some(UpdateInfo { version, url })
}

/// Whether a URL is a download on the project's own releases page.
pub fn is_release_url(url: &str) -> bool {
    // A prefix test only means anything because the prefix is an absolute
    // `https://` URL with the host in it: no scheme or host can be swapped
    // underneath it, and a path cannot climb back out of it.
    url.starts_with(RELEASE_URL_PREFIX)
}

/// True when `candidate` is a newer release than `current`.
///
/// Semantic versioning, which is what the feed's numbers have always been.
/// Notably a prerelease sorts *below* the release it leads to, so `0.5.0-beta.1`
/// is not offered to someone already on `0.5.0`.
pub fn is_newer(candidate: &str, current: &str) -> bool {
    match (parse_version(candidate), parse_version(current)) {
        (Some(candidate), Some(current)) => candidate > current,
        // A version neither side can read is never an update: offering one on
        // a guess sends people to a release page for no reason.
        _ => false,
    }
}

/// Parses a dotted version, tolerating the two-component spellings that have
/// appeared on feeds (`0.5`) which strict semver rejects.
fn parse_version(raw: &str) -> Option<Version> {
    let raw = raw.trim();
    if let Ok(version) = Version::parse(raw) {
        return Some(version);
    }

    let split = raw.find(['-', '+']).unwrap_or(raw.len());
    let (core, suffix) = raw.split_at(split);
    let mut parts: Vec<&str> = core.split('.').collect();
    while parts.len() < 3 {
        parts.push("0");
    }
    Version::parse(&format!("{}{suffix}", parts.join("."))).ok()
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
    fn the_newest_release_wins_whatever_order_the_feed_lists_it_in() {
        // The feed's ordering was taken on trust. An item appended below the
        // others — or reordered by a merge — used to hide the release.
        let out_of_order = FEED
            .replace("<title>0.5.0</title>", "<title>PLACEHOLDER</title>")
            .replace("0.4.4", "0.6.0")
            .replace("<title>PLACEHOLDER</title>", "<title>0.5.0</title>");
        assert_eq!(latest_release(&out_of_order).unwrap().version, "0.6.0");
    }

    #[test]
    fn the_sparkle_prefix_is_not_hardcoded() {
        // The namespace is what identifies the element; a feed is free to bind
        // it to any prefix, and a string search for `<sparkle:…>` would miss.
        let rebound = FEED
            .replace("xmlns:sparkle=", "xmlns:s=")
            .replace("<sparkle:", "<s:")
            .replace("</sparkle:", "</s:")
            .replace("sparkle:edSignature", "s:edSignature");
        assert_eq!(latest_release(&rebound).unwrap().version, "0.5.0");
    }

    #[test]
    fn garbage_feeds_yield_nothing() {
        assert_eq!(latest_release(""), None);
        assert_eq!(latest_release("<rss></rss>"), None);
        assert_eq!(latest_release("<item><title>x</title></item>"), None);
        // Not even well-formed: the old string scan happily matched this.
        assert_eq!(latest_release("<item><enclosure url=\"x\"></item>"), None);
    }

    #[test]
    fn a_download_outside_the_project_releases_is_refused() {
        for hostile in [
            "https://example.com/Close-My-Lid.zip",
            "file:///tmp/Close-My-Lid.zip",
            "javascript:alert(1)",
            // Lookalike host: the prefix test pins the scheme and host too.
            "https://github.com.example.com/krishkalaria12/close-my-lid/releases/x.zip",
            "https://github.com/someone-else/close-my-lid/releases/download/v9/x.zip",
        ] {
            let feed = FEED.replace(
                "https://github.com/krishkalaria12/close-my-lid/releases/download/v0.5.0/Close-My-Lid-v0.5.0-macOS.zip",
                hostile,
            );
            assert_eq!(latest_release(&feed), None, "{hostile}");
        }
    }

    #[test]
    fn version_comparison_ignores_width() {
        assert!(is_newer("0.5.0", "0.4.4"));
        assert!(is_newer("0.4.10", "0.4.9"));
        assert!(is_newer("1.0.0", "0.99.99"));
        assert!(is_newer("0.5", "0.4.4"));
        assert!(!is_newer("0.4.4", "0.4.4"));
        assert!(!is_newer("0.4.3", "0.4.4"));
        assert!(!is_newer("0.4.4", "0.5.0"));
    }

    #[test]
    fn a_prerelease_never_supersedes_its_release() {
        assert!(!is_newer("0.5.0-beta.1", "0.5.0"));
        assert!(!is_newer("0.5.0-rc1", "0.5.0"));
        assert!(is_newer("0.5.0", "0.5.0-beta.1"));
        assert!(is_newer("0.5.0-beta.2", "0.5.0-beta.1"));
        // A prerelease of the next version is still an update.
        assert!(is_newer("0.6.0-beta.1", "0.5.0"));
    }

    #[test]
    fn an_unreadable_version_is_never_an_update() {
        assert!(!is_newer("soon", "0.4.4"));
        assert!(!is_newer("", "0.4.4"));
        assert!(!is_newer("0.5.0", "who knows"));
    }

    #[test]
    fn this_build_is_not_offered_its_own_release() {
        let feed = FEED.replace("0.5.0", VERSION).replace("0.4.4", VERSION);
        assert_eq!(newer_release(&feed), None);
    }
}
