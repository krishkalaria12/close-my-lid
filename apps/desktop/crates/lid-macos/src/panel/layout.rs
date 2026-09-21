//! The panel's metrics, in one place.
//!
//! The panel is a fixed width, so every position in it is arithmetic over
//! these numbers rather than a constraint solve. `PanelHeights` does that
//! arithmetic once and `panel.rs` reads the answers back out, which keeps the
//! layout auditable: if a section moves, exactly one number changed.

/// The width the SwiftUI panel used, kept so the panel reads the same.
pub const WIDTH: f64 = 300.0;

/// Horizontal inset for section content.
pub const PAD: f64 = 16.0;

/// Content width inside the horizontal padding.
pub const CONTENT: f64 = WIDTH - PAD * 2.0;

/// Corner radius of the panel's frosted background.
pub const CORNER: f64 = 14.0;

/// Gap between the status item and the panel's top edge.
pub const ANCHOR_GAP: f64 = 6.0;

/// Minimum distance the panel keeps from the screen's edges.
pub const SCREEN_MARGIN: f64 = 8.0;

// Text line boxes, sized for the fonts each row uses.
pub const TITLE_LINE: f64 = 22.0;
pub const STATUS_LINE: f64 = 16.0;
pub const SECTION_LINE: f64 = 19.0;
pub const BODY_LINE: f64 = 18.0;

pub const SWITCH_WIDTH: f64 = 38.0;
pub const SWITCH_HEIGHT: f64 = 22.0;

pub const BAR_HEIGHT: f64 = 8.0;
pub const BADGE: f64 = 28.0;
pub const BADGE_RADIUS: f64 = 7.0;
pub const DOT: f64 = 7.0;
pub const AGENT_ROW: f64 = BADGE;
pub const AGENT_GAP: f64 = 10.0;

pub const PILL_HEIGHT: f64 = 26.0;
pub const PILL_GAP: f64 = 6.0;

pub const FOOTER_ROW: f64 = 34.0;
pub const FOOTER_GAP: f64 = 2.0;
pub const FOOTER_PAD: f64 = 8.0;

/// Footer rows run wider than the section content, like AppKit menu items.
pub const FOOTER_ROW_WIDTH: f64 = WIDTH - FOOTER_PAD * 2.0;

/// Space kept clear on the right of a footer row for its shortcut or status.
/// The title is truncated to fit what is left, so a long title can never push
/// the shortcut off the row or overlap it.
pub const FOOTER_TRAILING: f64 = 70.0;

/// Where a footer row's text sits, so the title and the shortcut share one
/// line rather than each finding their own.
pub const fn footer_text_y() -> f64 {
    (FOOTER_ROW - BODY_LINE) / 2.0
}

pub const SEPARATOR: f64 = 1.0;

/// Every section's height, and the panel's total.
///
/// A Mac with no battery hides that section entirely — as the SwiftUI panel
/// did — which is why the total is computed rather than a constant.
pub struct PanelHeights {
    pub header: f64,
    pub battery: f64,
    pub agents: f64,
    pub hold: f64,
    pub footer: f64,
    pub total: f64,
}

impl PanelHeights {
    pub fn new(agent_rows: usize, has_battery: bool) -> Self {
        let header = 16.0 + TITLE_LINE + 4.0 + STATUS_LINE + 14.0;

        let battery = if has_battery {
            14.0 + SECTION_LINE + 10.0 + BAR_HEIGHT + 10.0 + BODY_LINE + 14.0
        } else {
            0.0
        };

        let rows = agent_rows as f64;
        let agents = 14.0
            + SECTION_LINE
            + 12.0
            + rows * AGENT_ROW
            + (rows - 1.0).max(0.0) * AGENT_GAP
            + 14.0;

        let hold = 12.0 + SECTION_LINE + 10.0 + PILL_HEIGHT + 12.0;

        let footer = 6.0 + 3.0 * FOOTER_ROW + 2.0 * FOOTER_GAP + 6.0;

        // One separator above every section after the header; the battery
        // section takes its own with it when it is hidden.
        let separators = if has_battery { 4.0 } else { 3.0 } * SEPARATOR;

        Self {
            header,
            battery,
            agents,
            hold,
            footer,
            total: header + battery + agents + hold + footer + separators,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hiding_the_battery_section_shortens_the_panel_by_it_and_its_rule() {
        let with = PanelHeights::new(7, true);
        let without = PanelHeights::new(7, false);

        assert_eq!(without.battery, 0.0);
        assert_eq!(with.total - without.total, with.battery + SEPARATOR);
    }

    #[test]
    fn the_total_is_the_sum_of_what_is_drawn() {
        let heights = PanelHeights::new(7, true);
        let sections =
            heights.header + heights.battery + heights.agents + heights.hold + heights.footer;
        assert_eq!(heights.total, sections + 4.0 * SEPARATOR);
    }

    #[test]
    fn a_single_agent_row_adds_no_gap() {
        let one = PanelHeights::new(1, false);
        let two = PanelHeights::new(2, false);
        assert_eq!(two.agents - one.agents, AGENT_ROW + AGENT_GAP);
    }
}
