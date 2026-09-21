//! Visual tokens for the panel.
//!
//! Deliberately not a copy of the macOS panel. That design leans on
//! `NSVisualEffectView` vibrancy, SF Symbols and Command-key affordances, all
//! of which read as foreign on Windows. The *structure* is shared; the skin is
//! not. These values target a Windows 11 flyout.

use gpui::{Hsla, rgb, rgba};

pub const PANEL_WIDTH: f32 = 320.0;

/// Acrylic-ish base. GPUI paints this itself rather than asking the compositor
/// for a backdrop, so it is a solid colour with a hint of transparency.
pub fn surface() -> Hsla {
    rgba(0x202020f2).into()
}

pub fn surface_raised() -> Hsla {
    rgba(0x2d2d2dff).into()
}

pub fn divider() -> Hsla {
    rgba(0xffffff14).into()
}

pub fn text_primary() -> Hsla {
    rgb(0xf3f3f3).into()
}

pub fn text_secondary() -> Hsla {
    rgba(0xffffffa0).into()
}

pub fn text_tertiary() -> Hsla {
    rgba(0xffffff66).into()
}

/// Windows 11 system accent blue.
pub fn accent() -> Hsla {
    rgb(0x60cdff).into()
}

pub fn good() -> Hsla {
    rgb(0x6ccb5f).into()
}

pub fn danger() -> Hsla {
    rgb(0xff99a4).into()
}

pub fn hover() -> Hsla {
    rgba(0xffffff14).into()
}
