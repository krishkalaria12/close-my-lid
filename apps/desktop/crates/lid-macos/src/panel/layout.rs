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

/// A placed band of the panel: its bottom edge and its height, in the panel's
/// own bottom-left-origin coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Slot {
    pub y: f64,
    pub height: f64,
}

impl Slot {
    /// The band's upper edge.
    #[cfg(test)]
    pub fn top(self) -> f64 {
        self.y + self.height
    }
}

/// Where every section and separator sits.
///
/// Split out from the AppKit code so the stacking is checkable: `place` can
/// only be exercised with a window and a main thread, but getting these
/// numbers wrong is exactly how sections end up overlapping or leaving a band
/// of dead space, and that is pure arithmetic.
#[derive(Debug)]
pub struct PanelStack {
    pub header: Slot,
    /// `None` on a Mac with no battery, which hides the section entirely.
    pub battery: Option<Slot>,
    pub agents: Slot,
    pub hold: Slot,
    pub footer: Slot,
    /// One rule above every section after the header, top-down.
    pub rules: Vec<Slot>,
    pub total: f64,
}

impl PanelStack {
    pub fn new(agent_rows: usize, has_battery: bool) -> Self {
        let heights = PanelHeights::new(agent_rows, has_battery);

        let mut cursor = heights.total;
        let mut rules = Vec::with_capacity(4);

        let section = |height: f64, cursor: &mut f64| {
            *cursor -= height;
            Slot { y: *cursor, height }
        };

        let header = section(heights.header, &mut cursor);

        let rule = |cursor: &mut f64, rules: &mut Vec<Slot>| {
            *cursor -= SEPARATOR;
            rules.push(Slot {
                y: *cursor,
                height: SEPARATOR,
            });
        };

        rule(&mut cursor, &mut rules);
        let battery = has_battery.then(|| {
            let slot = section(heights.battery, &mut cursor);
            rule(&mut cursor, &mut rules);
            slot
        });

        let agents = section(heights.agents, &mut cursor);
        rule(&mut cursor, &mut rules);

        let hold = section(heights.hold, &mut cursor);
        rule(&mut cursor, &mut rules);

        let footer = section(heights.footer, &mut cursor);

        Self {
            header,
            battery,
            agents,
            hold,
            footer,
            rules,
            total: heights.total,
        }
    }

    /// Every band in the panel, top-down.
    #[cfg(test)]
    pub fn bands(&self) -> Vec<Slot> {
        let sections = [
            Some(self.header),
            self.battery,
            Some(self.agents),
            Some(self.hold),
            Some(self.footer),
        ];
        let mut bands: Vec<Slot> = sections
            .into_iter()
            .flatten()
            .chain(self.rules.iter().copied())
            .collect();
        // Top-down: a larger `y` is further up the panel.
        bands.sort_by(|a, b| b.y.total_cmp(&a.y));
        bands
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

    /// Every band in the panel, in order, with nothing overlapping and nothing
    /// left over. This is what `place` relies on to hand each section a frame.
    fn assert_tiles(stack: &PanelStack) {
        let bands = stack.bands();
        assert!(!bands.is_empty());

        let mut edge = stack.total;
        for band in &bands {
            assert!(
                band.height > 0.0,
                "a zero-height band draws nothing: {band:?}"
            );
            assert_eq!(
                band.top(),
                edge,
                "bands must meet exactly; {band:?} does not start at {edge}"
            );
            edge = band.y;
        }
        assert_eq!(edge, 0.0, "the stack must reach the bottom of the panel");
    }

    #[test]
    fn every_section_tiles_the_panel_with_a_battery() {
        let stack = PanelStack::new(7, true);
        assert!(stack.battery.is_some());
        assert_eq!(stack.rules.len(), 4);
        assert_tiles(&stack);
    }

    #[test]
    fn every_section_tiles_the_panel_without_one() {
        let stack = PanelStack::new(7, false);
        assert!(stack.battery.is_none());
        assert_eq!(
            stack.rules.len(),
            3,
            "the battery rule goes with its section"
        );
        assert_tiles(&stack);
    }

    #[test]
    fn the_stack_survives_a_change_in_the_number_of_agents() {
        // The agent list is the one section whose height depends on data, so
        // it is the one that could silently start overlapping the rest.
        for rows in [1, 2, 7, 12] {
            for has_battery in [true, false] {
                assert_tiles(&PanelStack::new(rows, has_battery));
            }
        }
    }

    #[test]
    fn sections_run_in_reading_order() {
        let stack = PanelStack::new(7, true);
        let battery = stack.battery.unwrap();
        assert!(
            stack.header.y > battery.y,
            "the header sits above the battery"
        );
        assert!(battery.y > stack.agents.y);
        assert!(stack.agents.y > stack.hold.y);
        assert!(stack.hold.y > stack.footer.y);
        assert_eq!(stack.footer.y, 0.0, "the footer sits on the bottom edge");
    }

    #[test]
    fn a_single_agent_row_adds_no_gap() {
        let one = PanelHeights::new(1, false);
        let two = PanelHeights::new(2, false);
        assert_eq!(two.agents - one.agents, AGENT_ROW + AGENT_GAP);
    }
}
