//! The widget gallery: every kit widget in every static state, used as the
//! visual regression corpus, the headless `gallery` example, and README art.

use fenestra_core::{
    Element, FamilyRole, SP2, SP3, SP4, SP6, TextSize, Theme, Weight, col, div, divider, row, text,
};

use crate::{
    ButtonVariant, ControlSize, Status, avatar, badge, button, callout, card, checkbox, icons, kbd,
    progress, radio, segmented, select, skeleton, skeleton_circle, skeleton_text, slider, spinner,
    stat_card, status, switch, table, tabs, text_area, text_input, wavy_progress,
};

fn section<Msg>(title: &str, content: Element<Msg>) -> Element<Msg> {
    col().gap(SP3).shrink0().children([
        text(title)
            .size(TextSize::Sm)
            .weight(Weight::Semibold)
            .themed(|t: &Theme, s| s.color(t.text_muted)),
        content,
    ])
}

/// Buttons, toggles, sliders, selects, and inputs in all states.
pub fn gallery_controls(theme: &Theme) -> Element<()> {
    col().p(SP6).gap(SP6).bg(theme.bg).children([
        section(
            "BUTTONS",
            col().gap(SP3).children([
                row().gap(SP3).items_center().children([
                    button("Primary"),
                    button("Secondary").variant(ButtonVariant::Secondary),
                    button("Ghost").variant(ButtonVariant::Ghost),
                    button("Danger").variant(ButtonVariant::Danger),
                    button("Disabled").disabled(true),
                ]),
                row().gap(SP3).items_center().children([
                    button("Small").size(ControlSize::Sm),
                    button("Medium").size(ControlSize::Md),
                    button("Large").size(ControlSize::Lg),
                ]),
            ]),
        ),
        section(
            "TOGGLES",
            col().gap(SP3).children([
                row().gap(SP4).items_center().children([
                    checkbox(false).label("Unchecked"),
                    checkbox(true).label("Checked"),
                    checkbox(true).label("Disabled").disabled(true),
                ]),
                row().gap(SP4).items_center().children([
                    switch(false).label("Off"),
                    switch(true).label("On"),
                    switch(true).label("Disabled").disabled(true),
                ]),
                row().gap(SP4).items_center().children([
                    radio(false).label("Unselected"),
                    radio(true).label("Selected"),
                    radio(true).label("Disabled").disabled(true),
                ]),
            ]),
        ),
        section(
            "SLIDERS",
            row().gap(SP4).items_center().children([
                slider(0.0),
                slider(0.62),
                slider(1.0).disabled(true),
            ]),
        ),
        section(
            "SELECT",
            row().gap(SP4).items_center().children([
                select(1, ["Daily", "Weekly", "Monthly"]).id("sel-a"),
                select(0, ["Disabled"]).disabled(true).id("sel-b"),
            ]),
        ),
        section(
            "TEXT INPUTS",
            col().gap(SP3).items_start().children([
                text_input("").placeholder("Placeholder…").id("in-a"),
                text_input("Filled value").id("in-b"),
                text_input("Invalid value").invalid(true).id("in-c"),
                text_input("Disabled").disabled(true).id("in-d"),
            ]),
        ),
        section(
            "TEXT AREA",
            col().items_start().children([Element::from(
                text_area("Wraps to its width and grows with the content.\nNewlines too.")
                    .width(280.0)
                    .min_height(64.0)
                    .id("ta-a"),
            )]),
        ),
    ])
}

/// Cards, badges, callouts, progress, tabs, and the table.
pub fn gallery_display(theme: &Theme) -> Element<()> {
    col().p(SP6).gap(SP6).bg(theme.bg).children([
        section(
            "STAT CARDS",
            row().gap(SP4).children([
                Element::from(stat_card("Revenue", "$48,210").delta("+12.5%", Status::Success)),
                Element::from(stat_card("Churn", "2.4%").delta("+0.3%", Status::Danger)),
                Element::from(stat_card("Sessions", "12,940")),
            ]),
        ),
        section(
            "BADGES + AVATARS + SPINNER",
            row().gap(SP3).items_center().children([
                badge("Accent", Status::Accent),
                badge("Success", Status::Success),
                badge("Warning", Status::Warning),
                badge("Danger", Status::Danger),
                avatar("RH"),
                avatar("AK"),
                spinner(),
            ]),
        ),
        section(
            "CALLOUTS",
            col().gap(SP3).children([
                callout(Status::Accent, "A new version is available."),
                callout(Status::Warning, "Your trial ends in 3 days."),
                callout(Status::Danger, "Payment failed; update your card."),
                callout(Status::Success, "Backup completed successfully."),
            ]),
        ),
        section(
            "PROGRESS",
            col()
                .gap(SP3)
                .w(320.0)
                .children([progress(0.25), progress(0.7)]),
        ),
        section(
            "TABS",
            tabs(1, ["Overview", "Activity", "Settings"], |_| ()),
        ),
        section(
            "TABLE",
            card().p(SP2).children([table(
                ["Service", "Region", "Status", "Uptime"],
                vec![
                    svec(["api-server", "us-east-1", "Healthy", "99.99%"]),
                    svec(["worker-pool", "us-east-1", "Degraded", "97.20%"]),
                    svec(["edge-cache", "eu-west-2", "Healthy", "99.95%"]),
                ],
            )]),
        ),
        section("DIVIDER", col().w(320.0).children([divider()])),
        section(
            "LUCIDE ICONS",
            col().gap(SP3).children([
                row()
                    .gap(SP3)
                    .items_center()
                    .children(icons::lucide::all().take(12).map(|(_, el)| el)),
                row()
                    .gap(SP3)
                    .items_center()
                    .children(icons::lucide::all().skip(12).map(|(_, el)| el)),
            ]),
        ),
    ])
}

fn svec<const N: usize>(items: [&str; N]) -> Vec<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}

/// The feedback & vocabulary additions: a segmented control, live status
/// indicators, skeleton loaders, and keyboard hints — every state in one frame.
pub fn gallery_feedback(theme: &Theme) -> Element<()> {
    col().p(SP6).gap(SP6).bg(theme.bg).children([
        section(
            "SEGMENTED CONTROL",
            col().gap(SP3).items_start().children([
                segmented(0, ["List", "Board", "Calendar"], |_| ()),
                segmented(1, ["Day", "Week", "Month"], |_| ()),
            ]),
        ),
        section(
            "STATUS",
            row().gap(SP6).items_center().children([
                Element::from(status("Operational", Status::Success).live(true)),
                Element::from(status("Degraded", Status::Warning)),
                Element::from(status("Outage", Status::Danger)),
                Element::from(status("Deploying", Status::Accent).live(true)),
            ]),
        ),
        section(
            "SKELETON",
            card().w(360.0).children([
                row().gap(SP4).items_center().children([
                    skeleton_circle(40.0),
                    col()
                        .gap(SP2)
                        .grow()
                        .children([skeleton(140.0, 12.0), skeleton(90.0, 12.0)]),
                ]),
                skeleton_text(3),
            ]),
        ),
        section(
            "KEYBOARD",
            col().gap(SP3).items_start().children([
                row().gap(SP3).items_center().children([
                    kbd(["cmd", "K"]),
                    kbd(["cmd", "shift", "P"]),
                    kbd(["enter"]),
                    kbd(["esc"]),
                ]),
                // A command-palette-style row with a right-aligned shortcut column.
                row()
                    .w(360.0)
                    .h(40.0)
                    .px(SP3)
                    .gap(SP3)
                    .items_center()
                    .themed(|t: &Theme, s| {
                        s.rounded(t.radius.md)
                            .bg(t.surface)
                            .border(1.0, t.border_subtle)
                    })
                    .children([
                        text("Search commands…")
                            .size(TextSize::Sm)
                            .grow()
                            .themed(|t: &Theme, s| s.color(t.text_muted)),
                        kbd(["cmd", "K"]),
                    ]),
            ]),
        ),
        section(
            "WAVY PROGRESS",
            col()
                .gap(SP4)
                .items_start()
                .children([wavy_progress(0.35, 280.0), wavy_progress(0.7, 280.0)]),
        ),
    ])
}

/// A sharp / minimal "observability console" — the design-range counterpart to
/// the soft default dashboard: 0px corners, hairline rules instead of cards, a
/// single accent used like punctuation, and mono tabular numerals. Pair it with
/// a slate + lime theme at `RadiusScale::sharp()` + `Elevation::Flat` (the
/// `console` look from `fenestra-looks`); every color routes through theme
/// tokens, so it recolors with the theme.
pub fn console_showcase(theme: &Theme) -> Element<()> {
    let hair_h = |theme: &Theme| div::<()>().h(1.0).w_full().bg(theme.border_subtle);
    let vline = |theme: &Theme| div::<()>().w(1.0).h_full().bg(theme.border_subtle);

    // Top bar: wordmark (sans) left, mono meta right; separated by tone, not box.
    let meta = |label: &str, val: &str| -> Element<()> {
        row().items_center().gap(6.0).children([
            text(label.to_owned())
                .family(FamilyRole::Mono)
                .size_px(11.5)
                .tracking(0.04)
                .color(theme.text_subtle),
            text(val.to_owned())
                .family(FamilyRole::Mono)
                .size_px(11.5)
                .tracking(0.04)
                .color(theme.text_muted),
        ])
    };
    let top = row()
        .h(56.0)
        .items_center()
        .justify_between()
        .px(24.0)
        .bg(theme.surface)
        .children([
            row().items_center().gap(10.0).children([
                div().w(7.0).h(7.0).bg(theme.accent),
                text("fenestra").weight(Weight::Semibold).size(TextSize::Sm),
                text("· console")
                    .size(TextSize::Sm)
                    .color(theme.text_subtle),
            ]),
            row().items_center().gap(22.0).children([
                meta("region", "us-east-1"),
                meta("build", "0bffe11"),
                row().items_center().gap(6.0).children([
                    div().w(6.0).h(6.0).rounded_full().bg(theme.accent),
                    text("live")
                        .family(FamilyRole::Mono)
                        .size_px(11.5)
                        .color(theme.text_muted),
                ]),
            ]),
        ]);

    // Nav: text-only, active = 2px left accent bar (no pill).
    let eyebrow = |s: &str| -> Element<()> {
        col().pl(24.0).pt(18.0).pb(9.0).child(
            text(s.to_owned())
                .family(FamilyRole::Mono)
                .size_px(10.5)
                .tracking(0.16)
                .color(theme.text_subtle),
        )
    };
    let nav_item = |label: &str, active: bool| -> Element<()> {
        row().h(34.0).items_center().children([
            div().w(2.0).h(18.0).bg(if active {
                theme.accent
            } else {
                theme.accent.with_alpha(0.0)
            }),
            col()
                .pl(22.0)
                .child(text(label.to_owned()).size(TextSize::Sm).color(if active {
                    theme.text
                } else {
                    theme.text_muted
                })),
        ])
    };
    let nav = col().w(240.0).h_full().pb(22.0).children([
        eyebrow("OBSERVE"),
        nav_item("Signals", true),
        nav_item("Traces", false),
        nav_item("Logs", false),
        eyebrow("OPERATE"),
        nav_item("Schedules", false),
        nav_item("Policies", false),
    ]);

    // Stat strip: one bordered rectangle, divided by vertical hairlines.
    let stat = |label: &str, num: &str, unit: &str, delta: &str, up_good: bool| -> Element<()> {
        col().grow().px(20.0).py(18.0).gap(10.0).children([
            text(label.to_owned())
                .family(FamilyRole::Mono)
                .size_px(11.0)
                .tracking(0.08)
                .color(theme.text_subtle),
            row().items_baseline().children([
                text(num.to_owned())
                    .family(FamilyRole::Mono)
                    .size_px(30.0)
                    .tracking(-0.01)
                    .color(theme.text),
                text(unit.to_owned())
                    .family(FamilyRole::Mono)
                    .size_px(14.0)
                    .color(theme.text_subtle),
            ]),
            text(delta.to_owned())
                .family(FamilyRole::Mono)
                .size_px(11.5)
                .color(if up_good {
                    theme.success.solid
                } else {
                    theme.danger.solid
                }),
        ])
    };
    let stats = row().border(1.0, theme.border_subtle).children([
        stat("REQUESTS", "48,210", "", "+12.5%", true),
        vline(theme),
        stat("ERROR RATE", "0.24", "%", "+0.03%", false),
        vline(theme),
        stat("P99 LATENCY", "128", "ms", "-6.0%", true),
        vline(theme),
        stat("SATURATION", "61", "%", "-2.0%", true),
    ]);

    // Services table.
    let th = |s: &str, w: f32, right: bool| -> Element<()> {
        let t = text(s.to_owned())
            .family(FamilyRole::Mono)
            .size_px(10.5)
            .tracking(0.1)
            .color(theme.text_subtle);
        if right {
            row().w(w).justify_end().child(t)
        } else {
            row().w(w).child(t)
        }
    };
    let pill = |label: &str, ok: bool| -> Element<()> {
        let c = if ok {
            theme.success.solid
        } else {
            theme.warning.solid
        };
        row()
            .px(8.0)
            .h(22.0)
            .items_center()
            .rounded(2.0)
            .border(1.0, c.with_alpha(0.45))
            .child(
                text(label.to_owned())
                    .family(FamilyRole::Mono)
                    .size_px(11.0)
                    .color(c),
            )
    };
    let cell_txt = |s: &str, w: f32, mono: bool, strong: bool| -> Element<()> {
        let mut t = text(s.to_owned()).size_px(13.5).color(if strong {
            theme.text
        } else {
            theme.text_muted
        });
        if mono {
            t = t.family(FamilyRole::Mono).size_px(13.0);
        }
        row().w(w).child(t)
    };
    let numcell = |s: &str, w: f32| -> Element<()> {
        row().w(w).justify_end().child(
            text(s.to_owned())
                .family(FamilyRole::Mono)
                .size_px(13.0)
                .color(theme.text),
        )
    };
    let trow =
        |svc: &str, region: &str, label: &str, ok: bool, up: &str, p99: &str| -> Element<()> {
            col().children([
                row().h(46.0).items_center().children([
                    cell_txt(svc, 220.0, true, true),
                    cell_txt(region, 170.0, false, false),
                    row().w(160.0).child(pill(label, ok)),
                    numcell(up, 150.0),
                    numcell(p99, 90.0),
                ]),
                div().h(1.0).w_full().bg(theme.border_subtle),
            ])
        };
    let table = col().pt(14.0).children([
        col().children([
            row().h(30.0).items_center().children([
                th("SERVICE", 220.0, false),
                th("REGION", 170.0, false),
                th("STATUS", 160.0, false),
                th("UPTIME", 150.0, true),
                th("P99", 90.0, true),
            ]),
            div().h(1.0).w_full().bg(theme.border),
        ]),
        trow(
            "api-server",
            "us-east-1",
            "healthy",
            true,
            "99.99%",
            "112ms",
        ),
        trow(
            "worker-pool",
            "us-east-1",
            "degraded",
            false,
            "97.20%",
            "340ms",
        ),
        trow("edge-cache", "eu-west-2", "healthy", true, "99.95%", "38ms"),
        trow("scheduler", "us-east-1", "healthy", true, "99.98%", "84ms"),
    ]);

    let section_head = row().pt(36.0).items_baseline().justify_between().children([
        text("SERVICES")
            .family(FamilyRole::Mono)
            .size_px(12.0)
            .tracking(0.1)
            .weight(Weight::Semibold)
            .color(theme.text_muted),
        text("view all →")
            .family(FamilyRole::Mono)
            .size_px(12.0)
            .color(theme.accent),
    ]);

    let main = col().grow().pt(34.0).px(40.0).children([
        col().gap(6.0).children([
            text("Signals")
                .size_px(28.0)
                .weight(Weight::Semibold)
                .tracking(-0.02)
                .color(theme.text),
            text("Service health across the fleet — last 24 hours.")
                .size(TextSize::Sm)
                .color(theme.text_muted),
        ]),
        col().pt(30.0).child(stats),
        section_head,
        table,
    ]);

    col().w_full().h_full().bg(theme.bg).children([
        top,
        hair_h(theme),
        row().grow().children([nav, vline(theme), main]),
    ])
}
