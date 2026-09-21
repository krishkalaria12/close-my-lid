//! Where to put the panel.
//!
//! GPUI exposes `tray_icon_bounds()`, but in adabraka-gpui 0.5.1 only the
//! macOS backend implements it — Windows and Linux fall through to the `None`
//! default. So this falls back to the bottom-right of the primary display's
//! work area, which is where the Windows tray lives anyway.
//!
//! Upstreaming a Windows implementation is tractable: `Shell_NotifyIconGetRect`
//! returns the icon's screen rectangle. Until then this is close enough that
//! users will not notice on a standard taskbar, and wrong only for a taskbar
//! moved to another edge.

use gpui::{App, Bounds, Pixels, Point, Size, px};

use crate::config::{PANEL_MARGIN as MARGIN, TASKBAR_INSET};

pub fn panel_bounds(size: Size<Pixels>, cx: &App) -> Bounds<Pixels> {
    if let Some(tray) = cx.tray_icon_bounds() {
        // Centre under the icon, which is what the macOS app does.
        let x = tray.origin.x + tray.size.width / 2.0 - size.width / 2.0;
        let y = tray.origin.y + tray.size.height + px(MARGIN / 2.0);
        return clamp_to_display(
            Bounds {
                origin: Point { x, y },
                size,
            },
            cx,
        );
    }

    let Some(display) = cx.primary_display() else {
        return Bounds {
            origin: Point {
                x: px(MARGIN),
                y: px(MARGIN),
            },
            size,
        };
    };

    let screen = display.bounds();
    let origin = Point {
        x: screen.origin.x + screen.size.width - size.width - px(MARGIN),
        y: screen.origin.y + screen.size.height - size.height - px(TASKBAR_INSET),
    };
    clamp_to_display(Bounds { origin, size }, cx)
}

/// Keeps the panel fully on screen.
fn clamp_to_display(bounds: Bounds<Pixels>, cx: &App) -> Bounds<Pixels> {
    let Some(display) = cx.primary_display() else {
        return bounds;
    };
    let screen = display.bounds();

    let max_x = screen.origin.x + screen.size.width - bounds.size.width - px(MARGIN);
    let max_y = screen.origin.y + screen.size.height - bounds.size.height - px(MARGIN);

    Bounds {
        origin: Point {
            x: bounds.origin.x.max(screen.origin.x + px(MARGIN)).min(max_x),
            y: bounds.origin.y.max(screen.origin.y + px(MARGIN)).min(max_y),
        },
        size: bounds.size,
    }
}
