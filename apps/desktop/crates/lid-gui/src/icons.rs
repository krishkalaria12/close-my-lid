//! The agent marks shown on the panel's badges.
//!
//! The same SVGs the macOS app compiles in, read from its crate rather than
//! copied a third time (the website serves them too). They go through gpui's
//! image path rather than its `svg()` element: that one renders a single-colour
//! mask for tinting icons, and these marks are drawn in their brands' colours.
//!
//! The app icon is the one the macOS bundle is built from.

use std::collections::HashMap;
use std::sync::Arc;

use gpui_kit::{Image, ImageFormat};
use lidcore::AgentHarness;

/// The badge's mark. Built once per harness and then shared, so gpui's image
/// cache sees the same asset on every frame instead of a fresh one to decode.
pub fn mark(harness: AgentHarness) -> Arc<Image> {
    thread_local! {
        static CACHE: HashMap<AgentHarness, Arc<Image>> = AgentHarness::ALL
            .into_iter()
            .map(|harness| {
                let bytes = svg(harness).as_bytes().to_vec();
                (harness, Arc::new(Image::from_bytes(ImageFormat::Svg, bytes)))
            })
            .collect();
    }
    CACHE.with(|cache| cache[&harness].clone())
}

/// The app's own icon, for the sidebar. The 64px rendition, so it stays sharp
/// at 2x on a 32px slot.
pub fn app_icon() -> Arc<Image> {
    thread_local! {
        static ICON: Arc<Image> = Arc::new(Image::from_bytes(
            ImageFormat::Png,
            include_bytes!("../../../assets/AppIcon.iconset/icon_32x32@2x.png").to_vec(),
        ));
    }
    ICON.with(Arc::clone)
}

/// How much breathing room the mark leaves inside its 28px badge.
///
/// Codex's mark reaches the edge of its own viewBox and Antigravity's is a wide
/// silhouette, so both need more inset than marks drawn with a margin already.
/// The same numbers as the macOS badges.
pub fn inset(harness: AgentHarness) -> f32 {
    match harness {
        AgentHarness::Codex | AgentHarness::Antigravity => 7.0,
        _ => 6.0,
    }
}

fn svg(harness: AgentHarness) -> &'static str {
    match harness {
        AgentHarness::ClaudeCode => include_str!("../../lid-macos/src/assets/claude-code.svg"),
        AgentHarness::Codex => include_str!("../../lid-macos/src/assets/codex.svg"),
        AgentHarness::OpenCode => include_str!("../../lid-macos/src/assets/opencode.svg"),
        AgentHarness::Antigravity => include_str!("../../lid-macos/src/assets/antigravity.svg"),
        AgentHarness::Copilot => include_str!("../../lid-macos/src/assets/copilot.svg"),
        AgentHarness::Cursor => include_str!("../../lid-macos/src/assets/cursor.svg"),
        AgentHarness::Pi => include_str!("../../lid-macos/src/assets/pi.svg"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_harness_ships_a_mark() {
        for harness in AgentHarness::ALL {
            let source = svg(harness);
            assert!(source.contains("<svg"), "{harness:?} has no SVG root");
            assert!(source.contains("</svg>"), "{harness:?}'s mark is truncated");
        }
    }
}
