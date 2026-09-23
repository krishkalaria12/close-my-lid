//! The window: a sidebar of pages beside the page itself.
//!
//! The macOS app is a menu bar panel because that is where a Mac keeps small
//! utilities. On Windows and Linux there is no dependable place to anchor one,
//! so the same features are laid out as an ordinary desktop app: Overview to
//! start and stop a hold, Agents for what is running, Settings for the rest.

use chrono::Utc;
use gpui_kit::component::{ActiveTheme, Icon, IconName, TitleBar, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, Context, Div, Entity, FocusHandle, FontWeight, MouseButton, SharedString, Window,
    WindowControlArea, div, img, px, transparent_black,
};

use crate::icons;
use crate::state::{AppState, UpdateStatus};
use crate::tasks;
use crate::theme::{self, Palette};
use crate::updates;
use crate::widgets::{self, dot};

gpui_kit::actions!(
    close_my_lid,
    [Quit, ToggleHold, ShowOverview, ShowAgents, ShowSettings]
);

/// The key context the bindings in `main.rs` are scoped to.
pub const KEY_CONTEXT: &str = "CloseMyLid";

const SIDEBAR_WIDTH: f32 = 224.0;

/// Readable line length for page content on a wide window.
const CONTENT_MAX_WIDTH: f32 = 680.0;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Overview,
    Agents,
    Settings,
}

impl Page {
    const ALL: [Self; 3] = [Self::Overview, Self::Agents, Self::Settings];

    fn title(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Agents => "Agents",
            Self::Settings => "Settings",
        }
    }

    fn icon(self) -> IconName {
        match self {
            Self::Overview => IconName::LayoutDashboard,
            Self::Agents => IconName::Bot,
            Self::Settings => IconName::Settings,
        }
    }

    fn shortcut(self) -> &'static str {
        match self {
            Self::Overview => "1",
            Self::Agents => "2",
            Self::Settings => ",",
        }
    }
}

pub struct Shell {
    pub(crate) state: Entity<AppState>,
    page: Page,
    focus: FocusHandle,
}

impl Shell {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        // Re-render whenever the supervision loop or a readout changes state.
        cx.observe(&state, |_, _, cx| cx.notify()).detach();

        let focus = cx.focus_handle();
        // Focused from the start so the keyboard shortcuts work without a
        // click into the window first.
        window.focus(&focus, cx);

        Self {
            state,
            page: Page::Overview,
            focus,
        }
    }

    pub(crate) fn show(&mut self, page: Page, cx: &mut Context<Self>) {
        self.page = page;
        cx.notify();
    }

    /// Runs a change against the app state and re-renders.
    pub(crate) fn act(&mut self, cx: &mut Context<Self>, change: impl FnOnce(&mut AppState)) {
        self.state.update(cx, |state, cx| {
            change(state);
            cx.notify();
        });
    }
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(cx);
        let page = match self.page {
            Page::Overview => self.overview_page(&p, cx).into_any_element(),
            Page::Agents => self.agents_page(&p, cx).into_any_element(),
            Page::Settings => self.settings_page(&p, window, cx).into_any_element(),
        };

        h_flex()
            .id("window")
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &Quit, _, cx| tasks::quit(&this.state, cx)))
            .on_action(cx.listener(|this, _: &ToggleHold, _, cx| this.act(cx, |s| s.toggle())))
            .on_action(cx.listener(|this, _: &ShowOverview, _, cx| this.show(Page::Overview, cx)))
            .on_action(cx.listener(|this, _: &ShowAgents, _, cx| this.show(Page::Agents, cx)))
            .on_action(cx.listener(|this, _: &ShowSettings, _, cx| this.show(Page::Settings, cx)))
            .size_full()
            .items_start()
            .bg(p.window)
            .text_color(p.label)
            .font_family(cx.theme().font_family.clone())
            .child(self.sidebar(&p, cx))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    // Carries the window controls on Windows and Linux; the
                    // page's own header is the title.
                    .child(
                        TitleBar::new()
                            .bg(transparent_black())
                            .border_color(transparent_black()),
                    )
                    .child(
                        div()
                            .id("page")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .child(
                                div()
                                    .w_full()
                                    .max_w(px(CONTENT_MAX_WIDTH))
                                    .mx_auto()
                                    .px(px(32.0))
                                    .pt(px(4.0))
                                    .pb(px(32.0))
                                    .child(page),
                            ),
                    ),
            )
    }
}

// MARK: sidebar

impl Shell {
    fn sidebar(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        let nav = Page::ALL.into_iter().map(|page| self.nav_item(page, p, cx));

        v_flex()
            .flex_none()
            .w(px(SIDEBAR_WIDTH))
            .h_full()
            .bg(p.sidebar)
            .border_r_1()
            .border_color(p.sidebar_edge)
            // The strip level with the title bar drags the window, as the
            // title bar does beside it. It also clears the traffic lights on
            // a Mac build.
            .child(
                div()
                    .id("sidebar-drag")
                    .h(px(38.0))
                    .flex_none()
                    .window_control_area(WindowControlArea::Drag)
                    .on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move()),
            )
            .child(
                h_flex()
                    .px(px(16.0))
                    .pb(px(18.0))
                    .gap(px(10.0))
                    .child(
                        img(icons::app_icon())
                            .flex_none()
                            .size(px(30.0))
                            .rounded(px(7.0)),
                    )
                    .child(
                        v_flex()
                            .child(
                                div()
                                    .text_size(px(14.0))
                                    .font_weight(FontWeight::BOLD)
                                    .child(lidcore::APP_NAME),
                            )
                            .child(
                                div()
                                    .text_size(px(11.5))
                                    .text_color(p.tertiary)
                                    .child(format!("Version {}", lidcore::VERSION)),
                            ),
                    ),
            )
            .child(v_flex().px(px(10.0)).gap(px(2.0)).children(nav))
            .child(div().flex_1())
            .child(self.sidebar_footer(p, cx))
    }

    fn nav_item(&self, page: Page, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let active = self.page == page;
        let hover = p.row_hover;
        h_flex()
            .id(SharedString::from(format!("nav-{}", page.title())))
            .h(px(32.0))
            .px(px(10.0))
            .gap(px(10.0))
            .rounded(px(8.0))
            .cursor_pointer()
            .when(active, |item| item.bg(p.nav_active))
            .when(!active, |item| item.hover(move |style| style.bg(hover)))
            .on_click(cx.listener(move |this, _, _, cx| this.show(page, cx)))
            .child(Icon::new(page.icon()).size(px(16.0)).text_color(if active {
                p.accent
            } else {
                p.secondary
            }))
            .child(
                div()
                    .flex_1()
                    .text_size(px(13.5))
                    .font_weight(if active {
                        FontWeight::SEMIBOLD
                    } else {
                        FontWeight::NORMAL
                    })
                    .child(page.title()),
            )
            .child(
                div()
                    .text_size(px(11.5))
                    .text_color(p.tertiary)
                    .child(widgets::shortcut(page.shortcut())),
            )
            .into_any_element()
    }

    /// The hold's state, visible from every page, and an update when there is
    /// one.
    fn sidebar_footer(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        let state = self.state.read(cx);
        let hold = state.state();
        let active = hold.is_active();
        let now = Utc::now();
        let detail = match (hold.remaining(now), hold.started_at()) {
            _ if !active => "Sleeps normally".to_string(),
            (Some(left), _) => format!("{} left", crate::overview::span(left)),
            (None, Some(started)) => format!("for {}", crate::overview::span(now - started)),
            (None, None) => String::new(),
        };
        let update = match &state.update {
            UpdateStatus::Available(update) => Some(update.clone()),
            _ => None,
        };

        let accent = p.accent;
        let hover = p.row_hover;
        v_flex()
            .p(px(10.0))
            .gap(px(6.0))
            .when_some(update, |footer, update| {
                footer.child(
                    h_flex()
                        .id("sidebar-update")
                        .px(px(10.0))
                        .h(px(30.0))
                        .gap(px(8.0))
                        .rounded(px(8.0))
                        .cursor_pointer()
                        .hover(move |style| style.bg(hover))
                        .on_click({
                            let page = updates::release_page(&update);
                            move |_, _, cx| cx.open_url(&page)
                        })
                        .child(dot(accent, 7.0))
                        .child(
                            div()
                                .text_size(px(12.5))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(accent)
                                .child(format!("Update to {}", update.version)),
                        ),
                )
            })
            .child(
                h_flex()
                    .id("sidebar-status")
                    .p(px(12.0))
                    .gap(px(10.0))
                    .rounded(px(10.0))
                    .bg(if active { p.green_wash } else { p.fill })
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| this.show(Page::Overview, cx)))
                    .child(dot(if active { p.green } else { p.tertiary }, 8.0))
                    .child(
                        v_flex()
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(if active { p.green_text } else { p.label })
                                    .child(if active { "Holding" } else { "Not holding" }),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(p.secondary)
                                    .truncate()
                                    .child(detail),
                            ),
                    ),
            )
    }
}
