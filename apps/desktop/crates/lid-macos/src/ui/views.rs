//! The three custom `NSView` subclasses the panel is drawn from.

use std::cell::{Cell, RefCell};

use objc2::rc::Retained;
use objc2::{AnyThread, DefinedClass, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSBezierPath, NSColor, NSEvent, NSImage, NSImageView, NSTrackingArea, NSTrackingAreaOptions,
    NSView,
};
use objc2_foundation::{MainThreadMarker, NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize};

use super::contains;

/// What a [`ClickView`] does when clicked, and how it looks at rest.
pub struct ClickStyle {
    pub corner_radius: f64,
    /// Fill opacity when idle. `0.0` for a footer row, which only appears on
    /// hover; the hold pills keep a faint resting fill.
    pub rest_alpha: f64,
    /// Fill opacity under the pointer.
    pub hover_alpha: f64,
}

impl ClickStyle {
    /// A hold preset: always tinted, brighter under the pointer.
    pub const PILL: Self = Self {
        corner_radius: 8.0,
        rest_alpha: 0.10,
        hover_alpha: 0.20,
    };

    /// A footer row: invisible until hovered, like an AppKit menu item.
    pub const ROW: Self = Self {
        corner_radius: 8.0,
        rest_alpha: 0.0,
        hover_alpha: 0.08,
    };
}

pub struct ClickIvars {
    style: ClickStyle,
    hovered: Cell<bool>,
    enabled: Cell<bool>,
    action: RefCell<Option<Box<dyn Fn()>>>,
}

define_class!(
    // SAFETY:
    // - NSView has no subclassing requirements beyond being used on the main
    //   thread, which `MainThreadOnly` enforces.
    // - ClickView does not implement Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "CMLClickView"]
    #[ivars = ClickIvars]
    pub struct ClickView;

    impl ClickView {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let ivars = self.ivars();
            let alpha = if !ivars.enabled.get() {
                ivars.style.rest_alpha * 0.5
            } else if ivars.hovered.get() {
                ivars.style.hover_alpha
            } else {
                ivars.style.rest_alpha
            };
            if alpha <= 0.0 {
                return;
            }

            let radius = ivars.style.corner_radius;
            let path =
                NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(self.bounds(), radius, radius);
            // `labelColor` is near-black in light mode and near-white in dark,
            // so one alpha fill reads correctly in both appearances.
            let fill = NSColor::labelColor().colorWithAlphaComponent(alpha);
            fill.setFill();
            path.fill();
        }

        #[unsafe(method(mouseEntered:))]
        fn mouse_entered(&self, _event: &NSEvent) {
            self.set_hovered(true);
        }

        #[unsafe(method(mouseExited:))]
        fn mouse_exited(&self, _event: &NSEvent) {
            self.set_hovered(false);
        }

        // NSResponder's default `mouseDown:` forwards up the chain, which
        // would hand the whole gesture to the window and leave `mouseUp:`
        // undelivered. Accepting the press here makes this view the target
        // for the rest of the click.
        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, _event: &NSEvent) {}

        // The panel is non-activating, so the click that opens it is also the
        // click that must work. Without this the first press is swallowed.
        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, event: &NSEvent) {
            if !self.ivars().enabled.get() {
                return;
            }
            // Only a release that lands inside counts, so dragging off the
            // control cancels it the way every other button does.
            let location = self.convertPoint_fromView(event.locationInWindow(), None);
            if !contains(self.bounds(), location) {
                return;
            }
            let action = self.ivars().action.borrow();
            if let Some(action) = action.as_ref() {
                action();
            }
        }

        // Labels placed inside a row must not swallow its clicks.
        #[unsafe(method(hitTest:))]
        fn hit_test(&self, point: NSPoint) -> *mut NSView {
            // SAFETY: `superview` only reads the view hierarchy, from the
            // main thread, which this class is pinned to.
            let inside = unsafe { self.superview() }
                .map(|parent| parent.convertPoint_toView(point, Some(self)))
                .is_some_and(|local| contains(self.bounds(), local));
            if inside {
                let this: *const Self = self;
                this.cast_mut().cast()
            } else {
                std::ptr::null_mut()
            }
        }
    }
);

impl ClickView {
    pub fn new(mtm: MainThreadMarker, frame: NSRect, style: ClickStyle) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(ClickIvars {
            style,
            hovered: Cell::new(false),
            enabled: Cell::new(true),
            action: RefCell::new(None),
        });
        let this: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: frame] };

        // `InVisibleRect` keeps the tracking area in step with the frame by
        // itself, so nothing has to be rebuilt when the panel is re-laid out.
        let tracking = unsafe {
            NSTrackingArea::initWithRect_options_owner_userInfo(
                NSTrackingArea::alloc(),
                NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0)),
                NSTrackingAreaOptions::MouseEnteredAndExited
                    | NSTrackingAreaOptions::ActiveInKeyWindow
                    | NSTrackingAreaOptions::InVisibleRect,
                Some(&this),
                None,
            )
        };
        this.addTrackingArea(&tracking);
        this
    }

    pub fn set_action(&self, action: impl Fn() + 'static) {
        *self.ivars().action.borrow_mut() = Some(Box::new(action));
    }

    fn set_hovered(&self, hovered: bool) {
        if self.ivars().hovered.replace(hovered) != hovered {
            self.setNeedsDisplay(true);
        }
    }
}

pub struct BarIvars {
    /// 0.0–1.0.
    fraction: Cell<f64>,
    low: Cell<bool>,
}

define_class!(
    // SAFETY: as for `ClickView` above.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "CMLBarView"]
    #[ivars = BarIvars]
    pub struct BarView;

    impl BarView {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let bounds = self.bounds();
            let radius = bounds.size.height / 2.0;

            let track = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(
                bounds, radius, radius,
            );
            NSColor::quaternaryLabelColor().setFill();
            track.fill();

            let ivars = self.ivars();
            // A minimum width keeps a nearly empty battery visible as a
            // capsule rather than a sliver, matching the SwiftUI original.
            let width = (bounds.size.width * ivars.fraction.get().clamp(0.0, 1.0))
                .max(bounds.size.height);
            let filled = NSRect::new(bounds.origin, NSSize::new(width, bounds.size.height));
            let level = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(
                filled, radius, radius,
            );
            let color = if ivars.low.get() {
                NSColor::systemRedColor()
            } else {
                NSColor::systemGreenColor()
            };
            color.setFill();
            level.fill();
        }
    }
);

impl BarView {
    pub fn new(mtm: MainThreadMarker, frame: NSRect) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(BarIvars {
            fraction: Cell::new(0.0),
            low: Cell::new(false),
        });
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }

    pub fn set_level(&self, fraction: f64, low: bool) {
        let ivars = self.ivars();
        if ivars.fraction.replace(fraction) != fraction || ivars.low.replace(low) != low {
            self.setNeedsDisplay(true);
        }
    }
}

pub struct BadgeIvars {
    color: Retained<NSColor>,
    radius: f64,
}

define_class!(
    // SAFETY: as for `ClickView` above.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "CMLBadgeView"]
    #[ivars = BadgeIvars]
    pub struct BadgeView;

    impl BadgeView {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let ivars = self.ivars();
            let path = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(
                self.bounds(),
                ivars.radius,
                ivars.radius,
            );
            ivars.color.setFill();
            path.fill();
        }
    }
);

impl BadgeView {
    /// A rounded colour chip with a mark centred inside it.
    ///
    /// `inset` leaves breathing room around the image: Codex's mark reaches
    /// the edge of its own viewBox and would otherwise touch the chip.
    pub fn new(
        mtm: MainThreadMarker,
        frame: NSRect,
        color: Retained<NSColor>,
        radius: f64,
        image: Option<&NSImage>,
        inset: f64,
    ) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(BadgeIvars { color, radius });
        let this: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: frame] };

        if let Some(image) = image {
            let side = frame.size.width - inset * 2.0;
            let view = NSImageView::initWithFrame(
                NSImageView::alloc(mtm),
                NSRect::new(NSPoint::new(inset, inset), NSSize::new(side, side)),
            );
            view.setImage(Some(image));
            view.setImageScaling(objc2_app_kit::NSImageScaling::ScaleProportionallyUpOrDown);
            this.addSubview(&view);
        }
        this
    }
}

pub struct TargetIvars {
    action: RefCell<Option<Box<dyn Fn()>>>,
}

define_class!(
    // SAFETY:
    // - NSObject has no subclassing requirements.
    // - ActionTarget does not implement Drop.
    #[unsafe(super(NSObject))]
    #[name = "CMLActionTarget"]
    #[ivars = TargetIvars]
    pub struct ActionTarget;

    impl ActionTarget {
        #[unsafe(method(fire:))]
        fn fire(&self, _sender: Option<&objc2::runtime::AnyObject>) {
            let action = self.ivars().action.borrow();
            if let Some(action) = action.as_ref() {
                action();
            }
        }
    }

    unsafe impl NSObjectProtocol for ActionTarget {}
);

impl ActionTarget {
    /// Wires a Rust closure to an `NSControl`'s target/action pair.
    ///
    /// `target` is an unretained reference in AppKit, so the returned object
    /// must be kept alive by the caller for as long as the control is; the
    /// panel holds them in its own struct for exactly that reason.
    pub fn new(action: impl Fn() + 'static) -> Retained<Self> {
        let this = Self::alloc().set_ivars(TargetIvars {
            action: RefCell::new(Some(Box::new(action))),
        });
        unsafe { msg_send![super(this), init] }
    }

    pub fn selector() -> objc2::runtime::Sel {
        objc2::sel!(fire:)
    }
}
