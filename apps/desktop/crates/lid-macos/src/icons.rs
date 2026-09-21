//! The agent marks shown on the panel's badges.
//!
//! Carried over from the Swift app's `AgentIcons`. The marks are SVG rather
//! than PNG because CoreSVG (`NSImage(data:)`) renders them at whatever size
//! the badge asks for, so one asset covers every scale factor. They live in
//! `src/assets/` and are compiled in, which keeps the app bundle a single
//! executable plus an icon.
//!
//! Every image is built once on first use and then reused: an `NSImage` is
//! immutable here, the panel redraws its badges on every refresh, and parsing
//! seven SVGs per redraw would be wasted work.

use std::cell::RefCell;
use std::collections::HashMap;

use objc2::AnyThread;
use objc2::rc::Retained;
use objc2_app_kit::NSImage;
use objc2_foundation::NSData;

use lidcore::AgentHarness;

/// The badge's mark, or `None` if CoreSVG refused the asset.
pub fn mark(harness: AgentHarness) -> Option<Retained<NSImage>> {
    thread_local! {
        // Main-thread only in practice — the panel is the only caller — so a
        // thread-local cache needs no synchronisation.
        static CACHE: RefCell<HashMap<AgentHarness, Option<Retained<NSImage>>>> =
            RefCell::new(HashMap::with_capacity(AgentHarness::ALL.len()));
    }

    CACHE.with(|cache| {
        cache
            .borrow_mut()
            .entry(harness)
            .or_insert_with(|| decode(svg(harness)))
            .clone()
    })
}

/// How much breathing room the mark leaves inside its 28pt badge.
///
/// Codex's mark reaches the edge of its own 24pt viewBox, and Antigravity's is
/// a wide silhouette, so both need more inset than a mark drawn with its own
/// margin already.
pub fn inset(harness: AgentHarness) -> f64 {
    match harness {
        AgentHarness::Codex | AgentHarness::Antigravity => 7.0,
        _ => 6.0,
    }
}

fn decode(svg: &str) -> Option<Retained<NSImage>> {
    let data = NSData::with_bytes(svg.as_bytes());
    // SAFETY: `initWithData:` copies the bytes and returns nil on a format it
    // cannot read, which is handled as `None`.
    let image = NSImage::initWithData(NSImage::alloc(), &data)?;
    // Template rendering would flatten these to a single tint; the marks are
    // brand colours on a colour badge, so they stay as drawn.
    image.setTemplate(false);
    Some(image)
}

fn svg(harness: AgentHarness) -> &'static str {
    match harness {
        AgentHarness::ClaudeCode => include_str!("assets/claude-code.svg"),
        AgentHarness::Codex => include_str!("assets/codex.svg"),
        AgentHarness::OpenCode => include_str!("assets/opencode.svg"),
        AgentHarness::Antigravity => include_str!("assets/antigravity.svg"),
        AgentHarness::Copilot => include_str!("assets/copilot.svg"),
        AgentHarness::Cursor => include_str!("assets/cursor.svg"),
        AgentHarness::Pi => include_str!("assets/pi.svg"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_harness_ships_a_mark() {
        for harness in AgentHarness::ALL {
            let source = svg(harness);
            assert!(
                source.contains("<svg"),
                "{harness:?} has no SVG root element"
            );
            assert!(source.contains("</svg>"), "{harness:?}'s mark is truncated");
        }
    }
}
