//! The tray panel.
//!
//! Same five sections as the macOS panel — header, battery, agents, hold
//! presets, footer — so the two platforms stay conceptually identical, with
//! Windows-appropriate styling and Ctrl-key wording.

use chrono::Utc;
use gpui::prelude::*;
use gpui::{ClickEvent, Context, Entity, FontWeight, Window, div, px};
use lidcore::{AgentHarness, BatterySafetyPolicy, SessionDuration};

use crate::config;
use crate::error::GuiError;
use crate::state::AppState;
use crate::theme;

pub struct Panel {
    state: Entity<AppState>,
    battery_policy: BatterySafetyPolicy,
}

impl Panel {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        // Re-render whenever the supervision loop changes the hold.
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        Self {
            state,
            battery_policy: BatterySafetyPolicy::default(),
        }
    }

    fn toggle(&mut self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        let headline = self.state.update(cx, |state, cx| {
            let result = if state.is_active() {
                state.stop()
            } else {
                state.start(SessionDuration::Indefinite)
            };
            cx.notify();
            report(result, "could not toggle the hold")
        });
        self.announce(headline, cx);
    }

    fn hold_for(&mut self, duration: SessionDuration, cx: &mut Context<Self>) {
        let headline = self.state.update(cx, |state, cx| {
            let result = state.start(duration);
            cx.notify();
            report(result, "could not start the hold")
        });
        self.announce(headline, cx);
    }

    /// Surfaces a refusal the way the tray menu already does.
    ///
    /// A release build hides the console, so the `tracing::error!` these paths
    /// used to stop at reached nobody: clicking the switch or a preset when the
    /// system refused looked exactly like clicking it and having nothing
    /// happen.
    fn announce(&self, headline: Option<String>, cx: &mut Context<Self>) {
        if let Some(headline) = headline {
            let _ = cx.show_notification(lidcore::APP_NAME, &headline);
        }
    }
}

/// Logs a failed hold change and returns the line worth interrupting the user
/// with, if it is one. Transient bus problems usually resolve on the next
/// attempt and only reach the log.
fn report(result: Result<(), GuiError>, context: &'static str) -> Option<String> {
    let error = result.err()?;
    tracing::error!(%error, hint = error.hint(), "{context}");
    error.deserves_notification().then(|| error.headline())
}

impl Render for Panel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let hold = state.state();
        let is_active = hold.is_active();
        let summary = hold.summary(Utc::now());
        let battery = state.battery;
        let agents: Vec<(AgentHarness, usize)> = AgentHarness::ALL
            .iter()
            .map(|harness| (*harness, state.agents.get(harness).copied().unwrap_or(0)))
            .collect();
        let working = agents.iter().filter(|(_, count)| *count > 0).count();
        let startup_error = state
            .startup_error
            .as_ref()
            .map(|error| (error.headline(), error.hint().map(str::to_owned)));

        let mut root = div()
            .flex()
            .flex_col()
            .w(px(config::PANEL_WIDTH))
            .bg(theme::surface())
            .text_color(theme::text_primary())
            .rounded(px(8.0))
            .shadow_lg();

        // Header: name, live status, on/off.
        root = root.child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .px(px(16.0))
                .pt(px(16.0))
                .pb(px(14.0))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(4.0))
                        .child(
                            div()
                                .text_size(px(16.0))
                                .font_weight(FontWeight::BOLD)
                                .child(lidcore::APP_NAME),
                        )
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(theme::text_secondary())
                                .child(summary),
                        ),
                )
                .child(
                    div()
                        .id("toggle")
                        .px(px(12.0))
                        .py(px(6.0))
                        .rounded(px(4.0))
                        .bg(if is_active {
                            theme::accent()
                        } else {
                            theme::surface_raised()
                        })
                        .text_size(px(12.0))
                        .text_color(if is_active {
                            theme::surface()
                        } else {
                            theme::text_secondary()
                        })
                        .hover(|style| style.bg(theme::hover()))
                        .on_click(cx.listener(Self::toggle))
                        .child(if is_active { "On" } else { "Off" }),
                ),
        );

        // A backend that failed to initialise is the one thing worth shouting
        // about, since nothing else in the panel will work. The hint is shown
        // under it because it is the only actionable part.
        if let Some((headline, hint)) = startup_error {
            let mut block = div()
                .flex()
                .flex_col()
                .gap(px(4.0))
                .px(px(16.0))
                .py(px(12.0))
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(theme::danger())
                        .child(headline),
                );

            if let Some(hint) = hint {
                block = block.child(
                    div()
                        .text_size(px(11.0))
                        .text_color(theme::text_secondary())
                        .child(hint),
                );
            }

            root = root.child(block);
        }

        root = root.child(rule());

        if let Some(battery) = battery {
            let low = self.battery_policy.should_release(battery);
            let track = config::PANEL_WIDTH - 32.0;
            let filled = (track * f32::from(battery.percentage) / 100.0).max(8.0);

            root = root.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    .px(px(16.0))
                    .py(px(14.0))
                    .child(section_title("Battery"))
                    .child(
                        div()
                            .w(px(track))
                            .h(px(6.0))
                            .rounded(px(3.0))
                            .bg(theme::surface_raised())
                            .child(div().w(px(filled)).h(px(6.0)).rounded(px(3.0)).bg(if low {
                                theme::danger()
                            } else {
                                theme::good()
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .text_size(px(12.0))
                            .child(format!("{}% left", battery.percentage))
                            .child(
                                div()
                                    .text_color(if low {
                                        theme::danger()
                                    } else {
                                        theme::text_secondary()
                                    })
                                    .child(if battery.is_charging {
                                        "charging".to_string()
                                    } else if low {
                                        "stopping to protect battery".to_string()
                                    } else {
                                        format!("stops at {}%", self.battery_policy.threshold)
                                    }),
                            ),
                    ),
            );
            root = root.child(rule());
        }

        // Agents.
        let mut agent_rows = div().flex().flex_col().gap(px(8.0));
        for (harness, count) in agents {
            let detail = match count {
                0 => "idle".to_string(),
                1 => "1 session".to_string(),
                many => format!("{many} sessions"),
            };
            agent_rows = agent_rows.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .child(
                        div()
                            .w(px(24.0))
                            .h(px(24.0))
                            .rounded(px(6.0))
                            .bg(gpui::rgb(harness.badge_rgb())),
                    )
                    .child(div().text_size(px(13.0)).child(harness.display_name()))
                    .child(div().flex_grow())
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(if count > 0 {
                                theme::text_secondary()
                            } else {
                                theme::text_tertiary()
                            })
                            .child(detail),
                    ),
            );
        }

        root = root
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.0))
                    .px(px(16.0))
                    .py(px(14.0))
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .items_center()
                            .child(section_title("Agents"))
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(theme::text_secondary())
                                    .child(if working == 0 {
                                        "all idle".to_string()
                                    } else {
                                        format!("{working} working")
                                    }),
                            ),
                    )
                    .child(agent_rows),
            )
            .child(rule());

        // Hold presets.
        let mut presets = div().flex().gap(px(6.0));
        for duration in SessionDuration::PRESETS {
            presets = presets.child(
                div()
                    .id(gpui::SharedString::from(duration.label()))
                    .px(px(10.0))
                    .py(px(6.0))
                    .rounded(px(4.0))
                    .bg(theme::surface_raised())
                    .text_size(px(12.0))
                    .hover(|style| style.bg(theme::hover()))
                    .on_click(cx.listener(move |this, _, _, cx| this.hold_for(duration, cx)))
                    .child(duration.label()),
            );
        }

        root.child(
            div()
                .flex()
                .flex_col()
                .gap(px(10.0))
                .px(px(16.0))
                .py(px(12.0))
                .child(section_title("Hold for"))
                .child(presets),
        )
    }
}

fn section_title(text: &'static str) -> impl IntoElement {
    div()
        .text_size(px(13.0))
        .font_weight(FontWeight::SEMIBOLD)
        .child(text)
}

fn rule() -> impl IntoElement {
    div().h(px(1.0)).w_full().bg(theme::divider())
}
