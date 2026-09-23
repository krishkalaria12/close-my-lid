//! The pieces every page is built from.
//!
//! Each is a plain function returning an element: pages differ in what they
//! say, not in how a card or a button looks, and keeping these in one place is
//! what keeps the pages reading as one app. Built from gpui primitives rather
//! than gpui-component's styled widgets where the palette needs to be exact.

use std::rc::Rc;

use gpui_kit::component::{Icon, IconName, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{
    App, ClickEvent, Div, ElementId, FontWeight, Hsla, SharedString, Stateful, Window, div, img,
    px, rgb,
};
use lidcore::AgentHarness;

use crate::icons;
use crate::theme::Palette;

pub type Handler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// A page's title and the sentence under it.
pub fn page_header(title: &'static str, subtitle: impl Into<SharedString>, p: &Palette) -> Div {
    v_flex()
        .gap(px(4.0))
        .mb(px(20.0))
        .child(
            div()
                .text_size(px(24.0))
                .line_height(px(30.0))
                .font_weight(FontWeight::BOLD)
                .text_color(p.label)
                .child(title),
        )
        .child(
            div()
                .text_size(px(13.5))
                .line_height(px(19.0))
                .text_color(p.secondary)
                .child(subtitle.into()),
        )
}

/// A raised surface with a hairline edge.
pub fn card(p: &Palette) -> Div {
    v_flex()
        .rounded(px(14.0))
        .bg(p.card)
        .border_1()
        .border_color(p.card_edge)
        .shadow_xs()
}

/// The small caption above a group of cards or rows.
pub fn group_label(text: &'static str, p: &Palette) -> Div {
    div()
        .px(px(4.0))
        .pb(px(8.0))
        .text_size(px(12.0))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(p.secondary)
        .child(text)
}

/// A hairline inside a card.
pub fn rule(p: &Palette) -> Div {
    div().h(px(1.0)).w_full().bg(p.rule)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonStyle {
    /// The one action a view is about.
    Primary,
    /// Everything else.
    Secondary,
    /// Stopping something; quiet at rest, red under the pointer.
    Stop,
}

pub fn button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    icon: Option<IconName>,
    style: ButtonStyle,
    enabled: bool,
    p: &Palette,
    on_click: Handler,
) -> Stateful<Div> {
    let (bg, hover, fg) = match style {
        ButtonStyle::Primary => (p.accent, p.accent_hover, p.on_accent),
        ButtonStyle::Secondary => (p.fill, p.fill_hover, p.label),
        ButtonStyle::Stop => (p.red_wash, p.red_wash.opacity(1.6), p.red),
    };

    h_flex()
        .id(id.into())
        .flex_none()
        .h(px(34.0))
        .px(px(16.0))
        .gap(px(7.0))
        .justify_center()
        .rounded(px(9.0))
        .bg(bg)
        .text_color(fg)
        .text_size(px(13.5))
        .font_weight(FontWeight::SEMIBOLD)
        .whitespace_nowrap()
        .when_some(icon, |button, icon| {
            button.child(Icon::new(icon).size(px(15.0)).text_color(fg))
        })
        .child(label.into())
        .when(enabled, |button| {
            button
                .cursor_pointer()
                .hover(move |style| style.bg(hover))
                .on_click(on_click)
        })
        .when(!enabled, |button| button.opacity(0.45))
}

/// A row of mutually exclusive choices on one track, the selected one raised.
pub fn segmented<T: Copy + PartialEq + 'static>(
    id: &'static str,
    options: impl IntoIterator<Item = (T, SharedString)>,
    selected: T,
    enabled: bool,
    p: &Palette,
    on_pick: impl Fn(&T, &mut Window, &mut App) + 'static,
) -> Div {
    // Shared by every segment's click handler.
    let on_pick = Rc::new(on_pick);
    let segments = options
        .into_iter()
        .enumerate()
        .map(|(index, (value, label))| {
            let chosen = value == selected;
            let hover = p.fill;
            let on_pick = Rc::clone(&on_pick);
            div()
                .id(ElementId::NamedInteger(id.into(), index as u64))
                .flex_1()
                .h(px(28.0))
                .px(px(12.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(7.0))
                .text_size(px(13.0))
                .whitespace_nowrap()
                .when(chosen, |segment| {
                    segment
                        .bg(p.card)
                        .shadow_xs()
                        .text_color(p.label)
                        .font_weight(FontWeight::SEMIBOLD)
                })
                .when(!chosen, |segment| {
                    segment
                        .text_color(p.secondary)
                        .font_weight(FontWeight::MEDIUM)
                })
                .when(enabled && !chosen, |segment| {
                    segment
                        .cursor_pointer()
                        .hover(move |style| style.bg(hover))
                        .on_click(move |_, window, cx| on_pick(&value, window, cx))
                })
                .child(label)
        });

    h_flex()
        .p(px(3.0))
        .gap(px(2.0))
        .rounded(px(10.0))
        .bg(p.fill)
        .when(!enabled, |track| track.opacity(0.5))
        .children(segments)
}

/// A small rounded label, like "Working" or "Idle".
pub fn tag(text: impl Into<SharedString>, fg: Hsla, bg: Hsla) -> Div {
    div()
        .flex_none()
        .px(px(8.0))
        .py(px(2.0))
        .rounded_full()
        .bg(bg)
        .text_color(fg)
        .text_size(px(11.5))
        .font_weight(FontWeight::SEMIBOLD)
        .child(text.into())
}

/// A filled dot, for live status.
pub fn dot(color: Hsla, size: f32) -> Div {
    div().flex_none().size(px(size)).rounded_full().bg(color)
}

/// An agent's brand mark on its brand colour.
pub fn agent_badge(harness: AgentHarness, size: f32, p: &Palette) -> Div {
    // The marks' own insets were tuned for a 28px badge; scale them with it.
    let inset = icons::inset(harness) * size / 28.0;
    div()
        .flex_none()
        .size(px(size))
        .rounded(px(size / 4.0))
        .bg(rgb(harness.badge_rgb()))
        .border_1()
        .border_color(p.badge_ring)
        .flex()
        .items_center()
        .justify_center()
        .child(img(icons::mark(harness)).size(px(size - inset * 2.0)))
}

/// A horizontal meter.
pub fn meter(fraction: f32, color: Hsla, height: f32, p: &Palette) -> Div {
    div()
        .h(px(height))
        .w_full()
        .rounded_full()
        .bg(p.track)
        .child(
            div()
                .h_full()
                .w(gpui_kit::relative(fraction.clamp(0.0, 1.0)))
                .min_w(px(height))
                .rounded_full()
                .bg(color),
        )
}

/// How a keyboard shortcut is written on this platform.
pub fn shortcut(key: &str) -> SharedString {
    if cfg!(target_os = "macos") && !crate::preview::pc_shortcuts() {
        format!("⌘{key}").into()
    } else {
        format!("Ctrl+{key}").into()
    }
}
