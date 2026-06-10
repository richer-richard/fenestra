//! The widget gallery: every kit widget in every static state, used as the
//! visual regression corpus, the headless `gallery` example, and README art.

use fenestra_core::{
    Element, SP2, SP3, SP4, SP6, TextSize, Theme, Weight, col, divider, row, text,
};

use crate::{
    ButtonVariant, ControlSize, Status, avatar, badge, button, callout, card, checkbox, progress,
    radio, select, slider, spinner, stat_card, switch, table, tabs, text_input,
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
    ])
}

fn svec<const N: usize>(items: [&str; N]) -> Vec<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}
