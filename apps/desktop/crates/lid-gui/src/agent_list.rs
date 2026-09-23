//! The Agents page: every coding agent Close My Lid recognises, and whether it
//! is running.

use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{Context, Div, FontWeight, div, px};
use lidcore::AgentHarness;

use crate::shell::Shell;
use crate::theme::Palette;
use crate::widgets::{agent_badge, card, page_header, rule, tag};

impl Shell {
    pub(crate) fn agents_page(&mut self, p: &Palette, cx: &mut Context<Self>) -> Div {
        let state = self.state.read(cx);
        let counts: Vec<(AgentHarness, usize)> = AgentHarness::ALL
            .into_iter()
            .map(|harness| (harness, state.sessions(harness)))
            .collect();
        let working = counts.iter().filter(|(_, count)| *count > 0).count();

        let subtitle = match working {
            0 => "None of the agents below is running. The list refreshes every few seconds."
                .to_string(),
            1 => "One agent is running. Close the lid and it keeps going.".to_string(),
            many => format!("{many} agents are running. Close the lid and they keep going."),
        };

        // Running agents first, so what matters is at the top; the order
        // within each group stays fixed, so rows do not shuffle on refresh.
        let mut ordered = counts;
        ordered.sort_by_key(|(_, count)| *count == 0);

        let total = ordered.len();
        let mut list = card(p).p(px(6.0));
        for (index, (harness, count)) in ordered.into_iter().enumerate() {
            list = list.child(agent_row(harness, count, p));
            if index + 1 < total {
                list = list.child(div().px(px(12.0)).child(rule(p)));
            }
        }

        v_flex()
            .child(page_header("Agents", subtitle, p))
            .child(list)
            .child(
                div()
                    .pt(px(14.0))
                    .px(px(4.0))
                    .text_size(px(12.0))
                    .line_height(px(17.0))
                    .text_color(p.tertiary)
                    .child(
                        "Close My Lid only counts agent processes on this machine. \
                         It never reads what they are doing.",
                    ),
            )
    }
}

fn agent_row(harness: AgentHarness, count: usize, p: &Palette) -> Div {
    let busy = count > 0;
    let detail = match count {
        0 => "Not running".to_string(),
        1 => "1 session running".to_string(),
        many => format!("{many} sessions running"),
    };

    h_flex()
        .px(px(12.0))
        .py(px(10.0))
        .gap(px(12.0))
        .child(agent_badge(harness, 34.0, p).opacity(if busy { 1.0 } else { 0.55 }))
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap(px(1.0))
                .child(
                    div()
                        .truncate()
                        .text_size(px(14.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(if busy { p.label } else { p.secondary })
                        .child(harness.display_name()),
                )
                .child(
                    div()
                        .text_size(px(12.5))
                        .text_color(p.tertiary)
                        .child(detail),
                ),
        )
        .child(if busy {
            tag("Working", p.green_text, p.green_wash)
        } else {
            tag("Idle", p.tertiary, p.fill)
        })
}
