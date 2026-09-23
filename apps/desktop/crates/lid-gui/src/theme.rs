//! Visual tokens for the app.
//!
//! A quiet, native-feeling desktop palette: a sidebar a shade apart from the
//! content, raised cards with hairline edges, three levels of label, and the
//! system blue, green and red used only where they mean something. There is a
//! light and a dark set, and the set follows the system appearance through
//! gpui-component's theme.
//!
//! Geometry lives with the views; only appearance belongs here.

use gpui_kit::component::{ActiveTheme, Theme};
use gpui_kit::{App, Hsla, Window, rgba};

#[derive(Clone, Copy)]
pub struct Palette {
    pub window: Hsla,
    pub sidebar: Hsla,
    /// The hairline between the sidebar and the content.
    pub sidebar_edge: Hsla,
    pub card: Hsla,
    pub card_edge: Hsla,

    pub label: Hsla,
    pub secondary: Hsla,
    pub tertiary: Hsla,
    pub rule: Hsla,

    /// Resting fill of a quiet control, and its hover.
    pub fill: Hsla,
    pub fill_hover: Hsla,
    /// Hover behind a list row or a link.
    pub row_hover: Hsla,
    /// The selected sidebar item.
    pub nav_active: Hsla,
    /// The empty part of a bar.
    pub track: Hsla,

    pub accent: Hsla,
    pub accent_hover: Hsla,
    pub on_accent: Hsla,
    pub green: Hsla,
    /// Green for text, which needs more contrast than a dot or a bar.
    pub green_text: Hsla,
    pub green_wash: Hsla,
    pub red: Hsla,
    pub red_wash: Hsla,
    /// A hairline around the agent badges, so a black badge still reads
    /// against a dark card.
    pub badge_ring: Hsla,
}

fn c(value: u32) -> Hsla {
    rgba(value).into()
}

fn dark() -> Palette {
    Palette {
        window: c(0x1b1b1dff),
        sidebar: c(0x222224ff),
        sidebar_edge: c(0xffffff12),
        card: c(0x252527ff),
        card_edge: c(0xffffff10),
        label: c(0xf5f5f7ff),
        secondary: c(0xebebf599),
        tertiary: c(0xebebf552),
        rule: c(0xffffff12),
        fill: c(0xffffff10),
        fill_hover: c(0xffffff1c),
        row_hover: c(0xffffff0a),
        nav_active: c(0xffffff14),
        track: c(0xffffff17),
        accent: c(0x0a84ffff),
        accent_hover: c(0x3d9bffff),
        on_accent: c(0xffffffff),
        green: c(0x30d158ff),
        green_text: c(0x4ade80ff),
        green_wash: c(0x30d15824),
        red: c(0xff453aff),
        red_wash: c(0xff453a24),
        badge_ring: c(0xffffff14),
    }
}

fn light() -> Palette {
    Palette {
        window: c(0xfbfbfcff),
        sidebar: c(0xf1f1f3ff),
        sidebar_edge: c(0x00000014),
        card: c(0xffffffff),
        card_edge: c(0x00000014),
        label: c(0x1d1d1fff),
        secondary: c(0x3c3c43a8),
        tertiary: c(0x3c3c4366),
        rule: c(0x00000012),
        fill: c(0x0000000b),
        fill_hover: c(0x00000014),
        row_hover: c(0x00000008),
        nav_active: c(0x00000012),
        track: c(0x00000012),
        accent: c(0x007affff),
        accent_hover: c(0x0068dbff),
        on_accent: c(0xffffffff),
        green: c(0x28cd41ff),
        green_text: c(0x1a8a33ff),
        green_wash: c(0x28cd411f),
        red: c(0xff3b30ff),
        red_wash: c(0xff3b301a),
        badge_ring: c(0x00000012),
    }
}

/// The palette for the current system appearance.
pub fn palette(cx: &App) -> Palette {
    if cx.theme().is_dark() {
        dark()
    } else {
        light()
    }
}

/// Follows the system appearance, then brings gpui-component's switch in line
/// with the rest of the app: a white knob on a quiet track, where the
/// library's default is a dark knob that reads as disabled.
///
/// Run whenever the appearance changes, since a theme change resets these.
pub fn sync(window: &mut Window, cx: &mut App) {
    Theme::sync_system_appearance(Some(window), cx);
    let track = palette(cx).fill_hover;
    let theme = Theme::global_mut(cx);
    theme.tokens.switch = track.into();
    theme.tokens.switch_thumb = Hsla::white().into();
}
