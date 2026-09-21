//! The small AppKit vocabulary the rest of the app is written in.
//!
//! Three custom views cover everything the SwiftUI panel used to draw, so the
//! panel code below stays a layout description rather than a pile of
//! Objective-C:
//!
//! - [`ClickView`] — a rounded, hover-highlighting, clickable surface. Both
//!   the hold preset pills and the footer rows are this one class with
//!   different metrics.
//! - [`BarView`] — the battery capsule.
//! - [`BadgeView`] — an agent's rounded colour chip with its mark inside.
//!
//! Everything is laid out with explicit frames. The panel is a fixed width
//! with a computed height, so there is nothing for Auto Layout to solve, and
//! skipping it keeps the whole panel one pass of arithmetic.

mod views;

use objc2::MainThreadOnly;
use objc2::rc::Retained;
use objc2_app_kit::{
    NSColor, NSFont, NSFontWeightBold, NSFontWeightMedium, NSFontWeightRegular,
    NSFontWeightSemibold, NSLineBreakMode, NSTextAlignment, NSTextField, NSView,
};
use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize, NSString};

pub use views::{ActionTarget, BadgeView, BarView, ClickStyle, ClickView};

/// Text weights, named as the SwiftUI code named them.
#[derive(Clone, Copy)]
pub enum Weight {
    Regular,
    Medium,
    Semibold,
    Bold,
}

impl Weight {
    fn value(self) -> f64 {
        // SAFETY: these are constants exported by AppKit, read only.
        unsafe {
            match self {
                Self::Regular => NSFontWeightRegular,
                Self::Medium => NSFontWeightMedium,
                Self::Semibold => NSFontWeightSemibold,
                Self::Bold => NSFontWeightBold,
            }
        }
    }
}

pub fn system_font(size: f64, weight: Weight) -> Retained<NSFont> {
    NSFont::systemFontOfSize_weight(size, weight.value())
}

/// Used for the countdown, so the status line does not jitter as digits change.
pub fn monospaced_digit_font(size: f64, weight: Weight) -> Retained<NSFont> {
    NSFont::monospacedDigitSystemFontOfSize_weight(size, weight.value())
}

/// A non-editable, non-selectable text label — SwiftUI's `Text`.
pub fn label(
    mtm: MainThreadMarker,
    text: &str,
    font: &NSFont,
    color: &NSColor,
) -> Retained<NSTextField> {
    let field = NSTextField::labelWithString(&NSString::from_str(text), mtm);
    field.setFont(Some(font));
    field.setTextColor(Some(color));
    // One line, truncated: the panel is a fixed width and a wrapped status
    // line would push everything below it out of place.
    field.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
    field
}

pub fn set_text(field: &NSTextField, text: &str) {
    let current = field.stringValue();
    // Comparing first avoids a relayout and redraw on every reconciliation
    // tick, which for the status line is once every 30 seconds forever.
    if current.to_string() != text {
        field.setStringValue(&NSString::from_str(text));
    }
}

pub fn set_frame(view: &NSView, x: f64, y: f64, width: f64, height: f64) {
    view.setFrame(NSRect::new(NSPoint::new(x, y), NSSize::new(width, height)));
}

/// Places a label at its natural size, right-aligned to `right`.
pub fn place_trailing(field: &NSTextField, right: f64, y: f64, height: f64) {
    field.sizeToFit();
    let width = field.frame().size.width;
    set_frame(field, right - width, y, width, height);
    field.setAlignment(NSTextAlignment::Right);
}

/// A plain container with no drawing of its own.
pub fn container(mtm: MainThreadMarker, frame: NSRect) -> Retained<NSView> {
    let view = NSView::initWithFrame(NSView::alloc(mtm), frame);
    view.setWantsLayer(true);
    view
}

/// The hairline AppKit uses between menu sections.
pub fn separator_color() -> Retained<NSColor> {
    NSColor::separatorColor()
}

/// AppKit has no `CGRect`-contains-`CGPoint` in these bindings.
pub fn contains(rect: NSRect, point: NSPoint) -> bool {
    let (min, max) = (rect.min(), rect.max());
    point.x >= min.x && point.x < max.x && point.y >= min.y && point.y < max.y
}

/// Splits a packed `0xRRGGBB` into an `NSColor`, for the agent badge colours
/// `lidcore` publishes.
pub fn rgb(packed: u32) -> Retained<NSColor> {
    let component = |shift: u32| f64::from((packed >> shift) & 0xff) / 255.0;
    NSColor::colorWithSRGBRed_green_blue_alpha(component(16), component(8), component(0), 1.0)
}
