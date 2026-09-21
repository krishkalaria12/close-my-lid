//! Building the panel's views once, and placing them.
//!
//! Split out from `mod.rs` so the refresh path — which is what runs every few
//! seconds — is not buried under construction code that runs exactly once.

use super::*;

/// Creates every view the panel owns. Nothing is positioned here: the frames
/// come from [`place`], which also runs whenever the battery section appears
/// or disappears.
pub(super) fn build_views(mtm: MainThreadMarker, app: &Rc<App>) -> Views {
    let content = frosted_background(mtm);
    let mut targets = Vec::new();

    let (header, status, toggle) = build_header(mtm, app, &mut targets);
    content.addSubview(&header);

    let (battery_section, battery_bar, battery_level, battery_caption) = build_battery(mtm);
    content.addSubview(&battery_section);

    let (agents_section, agents_summary, agent_rows) = build_agents(mtm);
    content.addSubview(&agents_section);

    let (hold_section, pills) = build_hold(mtm, app);
    content.addSubview(&hold_section);

    let (footer_section, updates_row, settings_row, quit_row) = build_footer(mtm, app);
    content.addSubview(&footer_section);

    // One rule above every section after the header; four when the battery
    // section is showing, three when it is not, so the spare one is simply
    // hidden rather than created and destroyed.
    let separators: Vec<Retained<NSView>> = (0..4)
        .map(|_| {
            let rule = container(
                mtm,
                NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0)),
            );
            if let Some(layer) = rule.layer() {
                layer.setBackgroundColor(Some(&separator_color().CGColor()));
            }
            content.addSubview(&rule);
            rule
        })
        .collect();

    Views {
        content,
        header,
        status,
        toggle,
        battery_section,
        battery_bar,
        battery_level,
        battery_caption,
        agents_section,
        agents_summary,
        agent_rows,
        hold_section,
        pills,
        footer_section,
        updates_row,
        settings_row,
        quit_row,
        separators,
        targets,
    }
}

fn frosted_background(mtm: MainThreadMarker) -> Retained<NSVisualEffectView> {
    let view = NSVisualEffectView::initWithFrame(
        NSVisualEffectView::alloc(mtm),
        NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(layout::WIDTH, 0.0)),
    );
    view.setMaterial(NSVisualEffectMaterial::Popover);
    view.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
    view.setState(NSVisualEffectState::Active);
    view.setWantsLayer(true);
    if let Some(layer) = view.layer() {
        layer.setCornerRadius(layout::CORNER);
        // Continuous, so the corners match the squircle AppKit draws on menus.
        unsafe { layer.setCornerCurve(objc2_quartz_core::kCACornerCurveContinuous) };
        layer.setMasksToBounds(true);
    }
    view
}

fn build_header(
    mtm: MainThreadMarker,
    app: &Rc<App>,
    targets: &mut Vec<Retained<ActionTarget>>,
) -> (Retained<NSView>, Retained<NSTextField>, Retained<NSSwitch>) {
    let section = container(mtm, NSRect::ZERO);

    let title = label(
        mtm,
        lidcore::APP_NAME,
        &system_font(17.0, Weight::Bold),
        &NSColor::labelColor(),
    );
    section.addSubview(&title);

    // Monospaced digits so the countdown does not jitter as it ticks down.
    let status = label(
        mtm,
        &SleepControlState::Inactive.summary(chrono::Utc::now()),
        &monospaced_digit_font(12.0, Weight::Regular),
        &NSColor::secondaryLabelColor(),
    );
    section.addSubview(&status);

    let toggle = NSSwitch::new(mtm);
    let flip = app.clone();
    let target = ActionTarget::new(move || flip.toggle_holding());
    unsafe {
        toggle.setTarget(Some(&target));
        toggle.setAction(Some(ActionTarget::selector()));
    }
    targets.push(target);
    section.addSubview(&toggle);

    (section, status, toggle)
}

fn build_battery(
    mtm: MainThreadMarker,
) -> (
    Retained<NSView>,
    Retained<BarView>,
    Retained<NSTextField>,
    Retained<NSTextField>,
) {
    let section = container(mtm, NSRect::ZERO);
    section.addSubview(&section_title(mtm, "Battery"));

    let bar = BarView::new(mtm, NSRect::ZERO);
    section.addSubview(&bar);

    let row = container(mtm, NSRect::ZERO);
    let level = label(
        mtm,
        "",
        &system_font(14.0, Weight::Regular),
        &NSColor::labelColor(),
    );
    let caption = label(
        mtm,
        "",
        &system_font(13.0, Weight::Regular),
        &NSColor::secondaryLabelColor(),
    );
    row.addSubview(&level);
    row.addSubview(&caption);
    section.addSubview(&row);

    (section, bar, level, caption)
}

fn build_agents(mtm: MainThreadMarker) -> (Retained<NSView>, Retained<NSTextField>, Vec<AgentRow>) {
    let section = container(mtm, NSRect::ZERO);

    let header = container(mtm, NSRect::ZERO);
    header.addSubview(&section_title(mtm, "Agents"));
    let summary = label(
        mtm,
        "",
        &system_font(13.0, Weight::Regular),
        &NSColor::secondaryLabelColor(),
    );
    header.addSubview(&summary);
    section.addSubview(&header);

    let rows = AgentHarness::ALL
        .iter()
        .map(|harness| {
            let row = container(mtm, NSRect::ZERO);

            let badge = BadgeView::new(
                mtm,
                NSRect::new(
                    NSPoint::new(0.0, 0.0),
                    NSSize::new(layout::BADGE, layout::BADGE),
                ),
                rgb(harness.badge_rgb()),
                layout::BADGE_RADIUS,
                icons::mark(*harness).as_deref(),
                icons::inset(*harness),
            );
            row.addSubview(&badge);

            let name = label(
                mtm,
                harness.display_name(),
                &system_font(14.0, Weight::Regular),
                &NSColor::labelColor(),
            );
            row.addSubview(&name);

            let detail = label(
                mtm,
                "",
                &system_font(13.0, Weight::Regular),
                &NSColor::tertiaryLabelColor(),
            );
            row.addSubview(&detail);

            let dot = container(mtm, NSRect::ZERO);
            if let Some(layer) = dot.layer() {
                layer.setBackgroundColor(Some(&NSColor::systemGreenColor().CGColor()));
                layer.setCornerRadius(layout::DOT / 2.0);
            }
            dot.setHidden(true);
            row.addSubview(&dot);

            section.addSubview(&row);
            AgentRow {
                row,
                detail,
                dot,
                name,
            }
        })
        .collect();

    (section, summary, rows)
}

fn build_hold(
    mtm: MainThreadMarker,
    app: &Rc<App>,
) -> (Retained<NSView>, Vec<Retained<ClickView>>) {
    let section = container(mtm, NSRect::ZERO);
    section.addSubview(&section_title(mtm, "Hold for"));

    let pills = SessionDuration::PRESETS
        .into_iter()
        .map(|duration| {
            let pill = ClickView::new(mtm, NSRect::ZERO, ClickStyle::PILL);
            let caption = label(
                mtm,
                &duration.label(),
                &system_font(12.0, Weight::Medium),
                &NSColor::labelColor(),
            );
            pill.addSubview(&caption);

            let hold = app.clone();
            pill.set_action(move || hold.start_hold(duration));
            section.addSubview(&pill);
            pill
        })
        .collect();

    (section, pills)
}

fn build_footer(
    mtm: MainThreadMarker,
    app: &Rc<App>,
) -> (Retained<NSView>, FooterRow, FooterRow, FooterRow) {
    let section = container(mtm, NSRect::ZERO);

    let updates = footer_row(mtm, &section, "Check for Updates…", "");
    let updater = app.clone();
    updates.view.set_action(move || updater.open_updates());

    let settings = footer_row(mtm, &section, "Settings…", "⌘ ,");
    let opener = app.clone();
    settings.view.set_action(move || opener.open_settings());

    let quit = footer_row(mtm, &section, "Quit Close My Lid", "⌘ Q");
    let quitter = app.clone();
    quit.view.set_action(move || quitter.quit());

    (section, updates, settings, quit)
}

fn footer_row(mtm: MainThreadMarker, section: &NSView, title: &str, trailing: &str) -> FooterRow {
    let view = ClickView::new(mtm, NSRect::ZERO, ClickStyle::ROW);

    let title = label(
        mtm,
        title,
        &system_font(14.0, Weight::Regular),
        &NSColor::labelColor(),
    );
    let trailing = label(
        mtm,
        trailing,
        &system_font(13.0, Weight::Regular),
        &NSColor::secondaryLabelColor(),
    );
    view.addSubview(&title);
    view.addSubview(&trailing);
    section.addSubview(&view);

    FooterRow {
        view,
        title,
        trailing,
    }
}

fn section_title(mtm: MainThreadMarker, text: &str) -> Retained<NSTextField> {
    label(
        mtm,
        text,
        &system_font(15.0, Weight::Bold),
        &NSColor::labelColor(),
    )
}

/// Positions everything and returns the panel's total height.
///
/// Sections are placed from the top down; each one then lays out its own
/// contents in local coordinates, which is why a section can be moved without
/// touching anything inside it.
pub(super) fn place(views: &Views, has_battery: bool) -> f64 {
    let heights = PanelHeights::new(views.agent_rows.len(), has_battery);

    set_frame(&views.content, 0.0, 0.0, layout::WIDTH, heights.total);

    // Sections, top down. `cursor` is the y of the next section's top edge.
    let mut cursor = heights.total;
    let mut rules = views.separators.iter();

    let section = |view: &NSView, height: f64, cursor: &mut f64| {
        *cursor -= height;
        set_frame(view, 0.0, *cursor, layout::WIDTH, height);
    };
    let rule = |cursor: &mut f64, rules: &mut std::slice::Iter<'_, Retained<NSView>>| {
        let Some(rule) = rules.next() else {
            return;
        };
        *cursor -= layout::SEPARATOR;
        set_frame(
            rule,
            layout::PAD,
            *cursor,
            layout::CONTENT,
            layout::SEPARATOR,
        );
        rule.setHidden(false);
    };

    section(&views.header, heights.header, &mut cursor);
    rule(&mut cursor, &mut rules);

    views.battery_section.setHidden(!has_battery);
    if has_battery {
        section(&views.battery_section, heights.battery, &mut cursor);
        rule(&mut cursor, &mut rules);
    }

    section(&views.agents_section, heights.agents, &mut cursor);
    rule(&mut cursor, &mut rules);

    section(&views.hold_section, heights.hold, &mut cursor);
    rule(&mut cursor, &mut rules);

    section(&views.footer_section, heights.footer, &mut cursor);

    // The spare rule, when there is no battery section to separate.
    for rule in rules {
        rule.setHidden(true);
    }

    place_header(views, heights.header);
    if has_battery {
        place_battery(views, heights.battery);
    }
    place_agents(views, heights.agents);
    place_hold(views, heights.hold);
    place_footer(views, heights.footer);

    heights.total
}

fn place_header(views: &Views, height: f64) {
    let Some(title) = first_label(&views.header) else {
        return;
    };
    let text_width = layout::CONTENT - layout::SWITCH_WIDTH - 12.0;

    let title_y = height - 16.0 - layout::TITLE_LINE;
    set_frame(&title, layout::PAD, title_y, text_width, layout::TITLE_LINE);

    let status_y = title_y - 4.0 - layout::STATUS_LINE;
    set_frame(
        &views.status,
        layout::PAD,
        status_y,
        text_width,
        layout::STATUS_LINE,
    );

    // Centred on the two-line block rather than on the section, so it lines up
    // with the text and not with the padding.
    let block_centre = (status_y + title_y + layout::TITLE_LINE) / 2.0;
    set_frame(
        &views.toggle,
        layout::WIDTH - layout::PAD - layout::SWITCH_WIDTH,
        block_centre - layout::SWITCH_HEIGHT / 2.0,
        layout::SWITCH_WIDTH,
        layout::SWITCH_HEIGHT,
    );
}

fn place_battery(views: &Views, height: f64) {
    if let Some(title) = first_label(&views.battery_section) {
        set_frame(
            &title,
            layout::PAD,
            height - 14.0 - layout::SECTION_LINE,
            layout::CONTENT,
            layout::SECTION_LINE,
        );
    }

    let bar_y = height - 14.0 - layout::SECTION_LINE - 10.0 - layout::BAR_HEIGHT;
    set_frame(
        &views.battery_bar,
        layout::PAD,
        bar_y,
        layout::CONTENT,
        layout::BAR_HEIGHT,
    );

    // SAFETY: the readout row is the caption's superview, created in
    // `build_battery`.
    if let Some(row) = unsafe { views.battery_caption.superview() } {
        set_frame(
            &row,
            layout::PAD,
            bar_y - 10.0 - layout::BODY_LINE,
            layout::CONTENT,
            layout::BODY_LINE,
        );
        set_frame(
            &views.battery_level,
            0.0,
            0.0,
            layout::CONTENT / 2.0,
            layout::BODY_LINE,
        );
    }
}

fn place_agents(views: &Views, height: f64) {
    // SAFETY: the summary's superview is the header row from `build_agents`.
    if let Some(header) = unsafe { views.agents_summary.superview() } {
        let header_y = height - 14.0 - layout::SECTION_LINE;
        set_frame(
            &header,
            layout::PAD,
            header_y,
            layout::CONTENT,
            layout::SECTION_LINE,
        );
        if let Some(title) = first_label(&header) {
            set_frame(
                &title,
                0.0,
                0.0,
                layout::CONTENT / 2.0,
                layout::SECTION_LINE,
            );
        }
    }

    let mut y = height - 14.0 - layout::SECTION_LINE - 12.0 - layout::AGENT_ROW;
    for row in &views.agent_rows {
        set_frame(&row.row, layout::PAD, y, layout::CONTENT, layout::AGENT_ROW);
        set_frame(
            &row.name,
            layout::BADGE + 10.0,
            (layout::AGENT_ROW - layout::BODY_LINE) / 2.0,
            layout::CONTENT - layout::BADGE - 10.0 - 90.0,
            layout::BODY_LINE,
        );
        set_frame(
            &row.dot,
            layout::CONTENT - layout::DOT,
            (layout::AGENT_ROW - layout::DOT) / 2.0,
            layout::DOT,
            layout::DOT,
        );
        y -= layout::AGENT_ROW + layout::AGENT_GAP;
    }
}

fn place_hold(views: &Views, height: f64) {
    if let Some(title) = first_label(&views.hold_section) {
        set_frame(
            &title,
            layout::PAD,
            height - 12.0 - layout::SECTION_LINE,
            layout::CONTENT,
            layout::SECTION_LINE,
        );
    }

    let y = height - 12.0 - layout::SECTION_LINE - 10.0 - layout::PILL_HEIGHT;
    // Each pill gets its own text plus an equal share of the leftover space,
    // so "Unlimited" is never clipped and the row always spans the content
    // width exactly.
    let captions: Vec<Retained<NSTextField>> = views
        .pills
        .iter()
        .filter_map(|pill| first_label(pill))
        .collect();
    let measured: Vec<f64> = captions
        .iter()
        .map(|caption| {
            caption.sizeToFit();
            caption.frame().size.width
        })
        .collect();

    let count = measured.len() as f64;
    let gaps = (count - 1.0).max(0.0) * layout::PILL_GAP;
    let spare = (layout::CONTENT - gaps - measured.iter().sum::<f64>()).max(0.0) / count.max(1.0);

    let mut x = layout::PAD;
    for (pill, (caption, text_width)) in views.pills.iter().zip(captions.iter().zip(&measured)) {
        let width = text_width + spare;
        set_frame(pill, x, y, width, layout::PILL_HEIGHT);
        set_frame(
            caption,
            (width - text_width) / 2.0,
            (layout::PILL_HEIGHT - 15.0) / 2.0,
            *text_width,
            15.0,
        );
        x += width + layout::PILL_GAP;
    }
}

fn place_footer(views: &Views, height: f64) {
    let mut y = height - 6.0 - layout::FOOTER_ROW;
    for row in [&views.updates_row, &views.settings_row, &views.quit_row] {
        set_frame(
            &row.view,
            layout::FOOTER_PAD,
            y,
            layout::FOOTER_ROW_WIDTH,
            layout::FOOTER_ROW,
        );
        place_footer_text(row);
        y -= layout::FOOTER_ROW + layout::FOOTER_GAP;
    }
}

/// Puts a footer row's title and its shortcut on the same line: the title from
/// the left, the shortcut right-aligned, both centred on the row.
///
/// The updates row runs this again on every refresh, because its trailing text
/// changes length and a right-aligned label has to be re-placed when it does.
pub(super) fn place_footer_text(row: &FooterRow) {
    set_frame(
        &row.title,
        layout::FOOTER_PAD,
        layout::footer_text_y(),
        layout::FOOTER_ROW_WIDTH - layout::FOOTER_PAD * 2.0 - layout::FOOTER_TRAILING,
        layout::BODY_LINE,
    );
    place_trailing(
        &row.trailing,
        layout::FOOTER_ROW_WIDTH - layout::FOOTER_PAD,
        layout::footer_text_y(),
        layout::BODY_LINE,
    );
}

/// The first `NSTextField` directly inside a view — every section puts its
/// heading there first, which saves keeping a handle to a label that never
/// changes.
fn first_label(view: &NSView) -> Option<Retained<NSTextField>> {
    view.subviews()
        .iter()
        .find_map(|subview| subview.downcast::<NSTextField>().ok())
}
