//! The Overview page: the hold, front and centre, with battery and agents
//! beside it.

use chrono::{DateTime, Local, Utc};
use gpui_kit::component::{ActiveTheme, Icon, IconName, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, Context, Div, FontWeight, SharedString, div, px};
use lidcore::{AgentHarness, BatteryStatus, SessionDuration, SleepControlState};

use crate::shell::{Page, Shell};
use crate::theme::Palette;
use crate::widgets::{
    ButtonStyle, agent_badge, button, card, meter, page_header, rule, segmented, tag,
};

impl Shell {
    pub(crate) fn overview_page(&mut self, p: &Palette, cx: &mut Context<Self>) -> Div {
        let mut page = v_flex().child(page_header(
            "Overview",
            "Keep your laptop awake with the lid closed, for as long as the work needs.",
            p,
        ));
        if let Some(banner) = self.banner(p, cx) {
            page = page.child(banner);
        }

        page.child(self.hero(p, cx)).child(
            div()
                .mt(px(16.0))
                .flex()
                .gap(px(16.0))
                .child(self.battery_card(p, cx))
                .child(self.agents_card(p, cx)),
        )
    }

    fn hero(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        let state = self.state.read(cx);
        let hold = state.state();
        let active = hold.is_active();
        let can_hold = state.can_hold();
        let selected = state.selected;
        let now = Utc::now();

        let status = if active {
            tag("Holding", p.green_text, p.green_wash)
        } else {
            tag("Not holding", p.secondary, p.fill)
        };
        let (title, subtitle) = describe(hold);

        let clock = clock(hold, now).map(|(time, caption)| {
            v_flex()
                .items_end()
                .flex_none()
                .child(
                    div()
                        .text_size(px(34.0))
                        .line_height(px(40.0))
                        .font_family(cx.theme().mono_font_family.clone())
                        .font_weight(FontWeight::MEDIUM)
                        .child(time),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(p.tertiary)
                        .child(caption),
                )
        });

        let presets = SessionDuration::PRESETS
            .into_iter()
            .map(|duration| (duration, SharedString::from(duration.label())));
        let picker = segmented(
            "duration",
            presets,
            selected,
            can_hold,
            p,
            cx.listener(|this, duration: &SessionDuration, _, cx| {
                let duration = *duration;
                this.act(cx, |state| state.select(duration));
            }),
        );

        let action = if active {
            button(
                "stop",
                "Stop Holding",
                Some(IconName::Pause),
                ButtonStyle::Stop,
                can_hold,
                p,
                Box::new(cx.listener(|this, _, _, cx| this.act(cx, |s| s.toggle()))),
            )
        } else {
            button(
                "start",
                "Start Holding",
                Some(IconName::Play),
                ButtonStyle::Primary,
                can_hold,
                p,
                Box::new(cx.listener(|this, _, _, cx| this.act(cx, |s| s.toggle()))),
            )
        };

        card(p)
            .p(px(24.0))
            .gap(px(20.0))
            .child(
                h_flex()
                    .items_start()
                    .justify_between()
                    .gap(px(16.0))
                    .child(
                        v_flex()
                            .min_w_0()
                            .gap(px(8.0))
                            .child(h_flex().child(status))
                            .child(
                                div()
                                    .text_size(px(26.0))
                                    .line_height(px(32.0))
                                    .font_weight(FontWeight::BOLD)
                                    .child(title),
                            )
                            .child(
                                div()
                                    .text_size(px(13.5))
                                    .line_height(px(19.0))
                                    .text_color(p.secondary)
                                    .child(subtitle),
                            ),
                    )
                    .children(clock),
            )
            .children(progress(hold, now).map(|fraction| meter(fraction, p.accent, 6.0, p)))
            .child(rule(p))
            .child(
                v_flex()
                    .gap(px(10.0))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(p.secondary)
                            .child(if active {
                                "Duration — picking another restarts the hold"
                            } else {
                                "Duration"
                            }),
                    )
                    .child(
                        h_flex()
                            .gap(px(16.0))
                            .justify_between()
                            .flex_wrap()
                            .child(picker)
                            .child(action),
                    ),
            )
    }

    fn battery_card(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        let state = self.state.read(cx);
        let policy = state.battery_policy;

        let body = match state.battery {
            Some(battery) => {
                let low = policy.should_release(battery);
                let (caption, caption_color) = if battery.is_charging {
                    ("Charging".to_string(), p.secondary)
                } else if low {
                    ("Low — releasing to protect it".to_string(), p.red)
                } else {
                    (
                        format!("Holds end on their own at {}%", policy.threshold),
                        p.secondary,
                    )
                };
                v_flex()
                    .gap(px(10.0))
                    .child(big_number(battery.percentage.to_string(), Some("%"), p))
                    .child(meter(
                        f32::from(battery.percentage.min(100)) / 100.0,
                        if low { p.red } else { p.green },
                        6.0,
                        p,
                    ))
                    .child(
                        div()
                            .text_size(px(12.5))
                            .text_color(caption_color)
                            .child(caption),
                    )
            }
            None => v_flex()
                .gap(px(10.0))
                .child(big_number("Plugged in".to_string(), None, p))
                .child(
                    div()
                        .text_size(px(12.5))
                        .text_color(p.secondary)
                        .child("No battery found, so there is no battery limit."),
                ),
        };

        card(p)
            .flex_1()
            .min_w_0()
            .p(px(18.0))
            .gap(px(12.0))
            .child(card_title(battery_icon(state.battery), "Battery", p))
            .child(body)
    }

    fn agents_card(&self, p: &Palette, cx: &mut Context<Self>) -> gpui_kit::Stateful<Div> {
        let state = self.state.read(cx);
        let counts: Vec<(AgentHarness, usize)> = AgentHarness::ALL
            .into_iter()
            .map(|harness| (harness, state.sessions(harness)))
            .collect();
        let working = counts.iter().filter(|(_, count)| *count > 0).count();
        let sessions: usize = counts.iter().map(|(_, count)| count).sum();

        let caption = match (working, sessions) {
            (0, _) => "Nothing running right now.".to_string(),
            (1, 1) => "1 session".to_string(),
            (1, sessions) => format!("{sessions} sessions in one agent"),
            (agents, sessions) => format!("{sessions} sessions across {agents} agents"),
        };
        let badges = counts.into_iter().map(|(harness, count)| {
            agent_badge(harness, 24.0, p).opacity(if count > 0 { 1.0 } else { 0.3 })
        });

        let hover = p.fill_hover;
        card(p)
            .id("agents-card")
            .flex_1()
            .min_w_0()
            .p(px(18.0))
            .gap(px(12.0))
            .cursor_pointer()
            .hover(move |style| style.border_color(hover))
            .on_click(cx.listener(|this, _, _, cx| this.show(Page::Agents, cx)))
            .child(
                h_flex()
                    .justify_between()
                    .child(card_title(IconName::Bot, "Agents", p))
                    .child(
                        Icon::new(IconName::ChevronRight)
                            .size(px(15.0))
                            .text_color(p.tertiary),
                    ),
            )
            .child(
                v_flex()
                    .gap(px(10.0))
                    .child(if working == 0 {
                        big_number("All idle".to_string(), None, p)
                    } else {
                        big_number(working.to_string(), Some(" working"), p)
                    })
                    .child(h_flex().gap(px(6.0)).children(badges))
                    .child(
                        div()
                            .text_size(px(12.5))
                            .text_color(p.secondary)
                            .child(caption),
                    ),
            )
    }

    /// The startup failure, which stays, or the last refusal, which can be
    /// dismissed. A backend that failed to initialise outranks everything,
    /// since nothing else in the window will work.
    pub(crate) fn banner(&self, p: &Palette, cx: &mut Context<Self>) -> Option<AnyElement> {
        let state = self.state.read(cx);
        let (error, dismissable) = match (&state.startup_error, &state.banner) {
            (Some(error), _) => (error, false),
            (None, Some(error)) => (error, true),
            (None, None) => return None,
        };
        let headline = error.headline();
        let hint = error.hint().map(str::to_owned);

        let tertiary = p.tertiary;
        let fill = p.fill;
        Some(
            h_flex()
                .mb(px(16.0))
                .p(px(12.0))
                .gap(px(10.0))
                .items_start()
                .rounded(px(12.0))
                .bg(p.red_wash)
                .child(
                    Icon::new(IconName::TriangleAlert)
                        .size(px(16.0))
                        .text_color(p.red),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap(px(2.0))
                        .child(
                            div()
                                .text_size(px(13.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(p.red)
                                .child(headline),
                        )
                        .when_some(hint, |text, hint| {
                            text.child(
                                div()
                                    .text_size(px(12.5))
                                    .line_height(px(17.0))
                                    .text_color(p.secondary)
                                    .child(hint),
                            )
                        }),
                )
                .when(dismissable, |banner| {
                    banner.child(
                        div()
                            .id("dismiss")
                            .flex_none()
                            .p(px(4.0))
                            .rounded(px(6.0))
                            .cursor_pointer()
                            .hover(move |style| style.bg(fill))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.act(cx, |state| state.dismiss_banner());
                            }))
                            .child(
                                Icon::new(IconName::Close)
                                    .size(px(13.0))
                                    .text_color(tertiary),
                            ),
                    )
                })
                .into_any_element(),
        )
    }
}

fn card_title(icon: IconName, text: &'static str, p: &Palette) -> Div {
    h_flex()
        .gap(px(7.0))
        .child(Icon::new(icon).size(px(15.0)).text_color(p.secondary))
        .child(
            div()
                .text_size(px(12.5))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(p.secondary)
                .child(text),
        )
}

/// A figure with an optional quieter unit after it: "19%", "2 working".
fn big_number(value: String, unit: Option<&'static str>, p: &Palette) -> Div {
    h_flex()
        .items_baseline()
        .child(
            div()
                .text_size(px(28.0))
                .line_height(px(34.0))
                .font_weight(FontWeight::BOLD)
                .child(value),
        )
        .when_some(unit, |row, unit| {
            row.child(
                div()
                    .text_size(px(16.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(p.secondary)
                    .child(unit),
            )
        })
}

fn battery_icon(battery: Option<BatteryStatus>) -> IconName {
    match battery {
        None => IconName::Cpu,
        Some(battery) if battery.is_charging => IconName::BatteryCharging,
        Some(battery) if battery.percentage >= 75 => IconName::BatteryFull,
        Some(battery) if battery.percentage >= 35 => IconName::BatteryMedium,
        Some(battery) if battery.percentage >= 15 => IconName::BatteryLow,
        Some(_) => IconName::BatteryWarning,
    }
}

/// The hero's headline and the sentence under it.
fn describe(hold: SleepControlState) -> (&'static str, String) {
    match hold {
        SleepControlState::Inactive => (
            "Sleeps normally",
            "Closing the lid puts your laptop to sleep, as usual.".to_string(),
        ),
        SleepControlState::Active {
            ends_at: Some(ends_at),
            ..
        } => (
            "Staying awake",
            format!(
                "Until {}, then normal sleep comes back on its own.",
                ends_at.with_timezone(&Local).format("%-I:%M %p")
            ),
        ),
        SleepControlState::Active { ends_at: None, .. } => (
            "Staying awake",
            "Until you stop it. Close the lid whenever you like.".to_string(),
        ),
    }
}

/// The hero's clock: time left on a timed hold, time elapsed on an unlimited
/// one, nothing when idle. Seconds shown, since the window ticks every second
/// while a hold runs.
fn clock(hold: SleepControlState, now: DateTime<Utc>) -> Option<(String, &'static str)> {
    if let Some(left) = hold.remaining(now) {
        return Some((stopwatch(left), "remaining"));
    }
    let started = hold.started_at()?;
    Some((stopwatch(now - started), "elapsed"))
}

/// How far through a timed hold is, for the progress bar.
fn progress(hold: SleepControlState, now: DateTime<Utc>) -> Option<f32> {
    let (started, ends) = (hold.started_at()?, hold.ends_at()?);
    let total = (ends - started).num_seconds().max(1) as f32;
    Some(((now - started).num_seconds() as f32 / total).clamp(0.0, 1.0))
}

/// `1:05:09`, or `5:09` under an hour.
fn stopwatch(span: chrono::Duration) -> String {
    let total = span.num_seconds().max(0);
    let (hours, minutes, seconds) = (total / 3600, (total % 3600) / 60, total % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

/// `1h 5m`, `45m`, or `under a minute`: the compact form for the sidebar.
pub(crate) fn span(span: chrono::Duration) -> String {
    let total = span.num_seconds().max(0);
    let (hours, minutes) = (total / 3600, (total % 3600) / 60);
    match (hours, minutes) {
        (0, 0) => "under a minute".to_string(),
        (0, minutes) => format!("{minutes}m"),
        (hours, minutes) => format!("{hours}h {minutes}m"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn the_stopwatch_drops_the_hour_under_an_hour() {
        assert_eq!(stopwatch(Duration::seconds(309)), "5:09");
        assert_eq!(stopwatch(Duration::seconds(3909)), "1:05:09");
        assert_eq!(stopwatch(Duration::seconds(-4)), "0:00");
    }

    #[test]
    fn compact_spans_never_claim_a_minute_that_is_not_there() {
        assert_eq!(span(Duration::seconds(30)), "under a minute");
        assert_eq!(span(Duration::minutes(45)), "45m");
        assert_eq!(span(Duration::minutes(65)), "1h 5m");
    }

    #[test]
    fn progress_runs_from_start_to_end() {
        let start = Utc::now();
        let hold = SleepControlState::Active {
            started_at: start,
            ends_at: Some(start + Duration::minutes(60)),
        };
        assert_eq!(progress(hold, start), Some(0.0));
        assert_eq!(progress(hold, start + Duration::minutes(30)), Some(0.5));
        assert_eq!(progress(hold, start + Duration::minutes(90)), Some(1.0));
        assert_eq!(progress(SleepControlState::Inactive, start), None);
    }
}
