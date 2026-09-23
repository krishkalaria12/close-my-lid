//! The Settings page.
//!
//! The macOS app's Settings window carries launch at login, the one-time
//! administrator grant and a Battery settings shortcut. There is no grant to
//! manage on Windows or Linux, so its place goes to what the mechanism is and
//! a way into the system's own power settings; notifications get a switch,
//! since not every desktop has a per-app control for them.

use gpui_kit::component::{Icon, IconName, h_flex, switch::Switch, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, App, ClickEvent, Context, Div, SharedString, Window, div, px};

use crate::config::{ISSUES_URL, RELEASES_URL};
use crate::error::GuiError;
use crate::shell::Shell;
use crate::state::UpdateStatus;
use crate::system;
use crate::tasks;
use crate::theme::Palette;
use crate::updates;
use crate::widgets::{ButtonStyle, button, card, group_label, page_header, rule};

impl Shell {
    pub(crate) fn settings_page(
        &mut self,
        p: &Palette,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let state = self.state.read(cx);
        let launch_at_login = state.launch_at_login;
        let notifications = state.prefs.notifications;
        let mechanism = state
            .mechanism()
            .unwrap_or("No lid control is available on this system.");
        let update = state.update.clone();

        let mut page = v_flex().child(page_header(
            "Settings",
            "How Close My Lid starts, what it tells you, and how it holds the lid.",
            p,
        ));
        if let Some(banner) = self.banner(p, cx) {
            page = page.child(banner);
        }

        let general = group(
            p,
            vec![
                toggle_row(
                    "launch-at-login",
                    "Launch at login",
                    "Starts minimised when you sign in.",
                    launch_at_login,
                    p,
                    cx.listener(|this, enabled: &bool, _, cx| {
                        let enabled = *enabled;
                        this.act(cx, |state| state.set_launch_at_login(enabled));
                    }),
                ),
                toggle_row(
                    "notifications",
                    "Notifications",
                    "When a hold starts, is about to end, and ends.",
                    notifications,
                    p,
                    cx.listener(|this, enabled: &bool, _, cx| {
                        let enabled = *enabled;
                        this.act(cx, |state| state.set_notifications(enabled));
                    }),
                ),
            ],
        );

        let lid = group(
            p,
            vec![
                text_row("How it works", mechanism, p),
                link_row(
                    "power-settings",
                    "Open power settings",
                    IconName::ExternalLink,
                    p,
                    cx.listener(|this, _, _, cx| {
                        if !system::open_power_settings() {
                            this.act(cx, |state| {
                                state.show_error(GuiError::Settings {
                                    detail: "No power settings app was found on this desktop."
                                        .to_string(),
                                });
                            });
                        }
                    }),
                ),
            ],
        );

        let updates = group(p, vec![self.update_row(update, p, cx)]);

        let about = group(
            p,
            vec![
                link_row(
                    "release-notes",
                    "Release notes",
                    IconName::ExternalLink,
                    p,
                    cx.listener(|_, _, _, cx| cx.open_url(RELEASES_URL)),
                ),
                link_row(
                    "report-issue",
                    "Report an issue",
                    IconName::ExternalLink,
                    p,
                    cx.listener(|_, _, _, cx| cx.open_url(ISSUES_URL)),
                ),
                link_row(
                    "data-folder",
                    "Open data folder",
                    IconName::Folder,
                    p,
                    cx.listener(|this, _, _, cx| match lidcore::config::config_dir() {
                        Ok(dir) => {
                            let _ = std::fs::create_dir_all(&dir);
                            cx.open_with_system(&dir);
                        }
                        Err(error) => this.act(cx, |state| state.show_error(error.into())),
                    }),
                ),
            ],
        );

        page.child(group_label("General", p))
            .child(general)
            .child(group_label("Lid Control", p))
            .child(lid)
            .child(group_label("Updates", p))
            .child(updates)
            .child(group_label("About", p))
            .child(about)
            .child(
                div()
                    .pt(px(4.0))
                    .px(px(4.0))
                    .text_size(px(12.0))
                    .text_color(p.tertiary)
                    .child("Quitting Close My Lid always restores normal sleep."),
            )
    }

    fn update_row(&self, update: UpdateStatus, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let (detail, color) = match &update {
            UpdateStatus::Unchecked => (
                "Checked automatically every few hours.".to_string(),
                p.secondary,
            ),
            UpdateStatus::Checking => ("Checking…".to_string(), p.secondary),
            UpdateStatus::Current => ("You're up to date.".to_string(), p.green_text),
            UpdateStatus::Available(info) => {
                (format!("Version {} is available.", info.version), p.accent)
            }
        };

        let action = match &update {
            UpdateStatus::Available(info) => {
                let page = updates::release_page(info);
                button(
                    "download",
                    "Download",
                    None,
                    ButtonStyle::Primary,
                    true,
                    p,
                    Box::new(move |_, _, cx| cx.open_url(&page)),
                )
            }
            _ => button(
                "check",
                "Check Now",
                None,
                ButtonStyle::Secondary,
                update != UpdateStatus::Checking,
                p,
                Box::new(cx.listener(|this, _, _, cx| {
                    tasks::check_for_updates(this.state.clone(), true, cx);
                })),
            ),
        };

        h_flex()
            .px(px(12.0))
            .py(px(10.0))
            .gap(px(12.0))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.0))
                    .child(row_title(format!("Close My Lid {}", lidcore::VERSION), p))
                    .child(caption(detail, color)),
            )
            .child(action)
            .into_any_element()
    }
}

/// Rows on one card, separated by hairlines.
fn group(p: &Palette, rows: Vec<AnyElement>) -> Div {
    let count = rows.len();
    let mut card = card(p).mb(px(20.0)).p(px(4.0));
    for (index, row) in rows.into_iter().enumerate() {
        card = card.child(row);
        if index + 1 < count {
            card = card.child(div().px(px(12.0)).child(rule(p)));
        }
    }
    card
}

fn row_title(text: impl Into<SharedString>, p: &Palette) -> Div {
    div()
        .text_size(px(13.5))
        .line_height(px(18.0))
        .text_color(p.label)
        .child(text.into())
}

fn caption(text: impl Into<SharedString>, color: gpui_kit::Hsla) -> Div {
    div()
        .text_size(px(12.5))
        .line_height(px(17.0))
        .text_color(color)
        .child(text.into())
}

fn toggle_row(
    id: &'static str,
    title: &'static str,
    detail: &'static str,
    checked: bool,
    p: &Palette,
    on_change: impl Fn(&bool, &mut Window, &mut App) + 'static,
) -> AnyElement {
    h_flex()
        .px(px(12.0))
        .py(px(11.0))
        .gap(px(12.0))
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap(px(2.0))
                .child(row_title(title, p))
                .child(caption(detail, p.secondary)),
        )
        .child(
            Switch::new(id)
                .checked(checked)
                .color(p.accent)
                .accessibility_label(title)
                .on_click(on_change),
        )
        .into_any_element()
}

/// A title over a sentence of read-only text.
fn text_row(title: &'static str, text: &str, p: &Palette) -> AnyElement {
    v_flex()
        .px(px(12.0))
        .py(px(11.0))
        .gap(px(2.0))
        .child(row_title(title, p))
        .child(caption(text.to_string(), p.secondary))
        .into_any_element()
}

fn link_row(
    id: &'static str,
    title: &'static str,
    icon: IconName,
    p: &Palette,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let hover = p.row_hover;
    h_flex()
        .id(id)
        .px(px(12.0))
        .h(px(42.0))
        .justify_between()
        .rounded(px(9.0))
        .cursor_pointer()
        .hover(move |style| style.bg(hover))
        .on_click(on_click)
        .child(row_title(title, p))
        .child(Icon::new(icon).size(px(14.0)).text_color(p.tertiary))
        .into_any_element()
}
