//! The flagship example: a SaaS-style dashboard with sidebar navigation, a
//! working search input, a light/dark theme toggle, stat cards, a table, a
//! form, and a modal.
//!
//! `cargo run --example dashboard` opens it in a window.
//! `cargo run --example dashboard -- shot` renders both themes headlessly
//! to `gallery/dashboard_{light,dark}.png` (no display needed).

use fenestra::prelude::*;
use fenestra::shell::render_app;

struct Dashboard {
    dark: bool,
    nav: usize,
    search: String,
    show_modal: bool,
    report_name: String,
    plan: usize,
    notifications: bool,
    form_name: String,
}

#[derive(Clone)]
enum Msg {
    SetDark(bool),
    SetNav(usize),
    Search(String),
    OpenModal,
    CloseModal,
    ReportName(String),
    SetPlan(usize),
    SetNotifications(bool),
    FormName(String),
}

impl Default for Dashboard {
    fn default() -> Self {
        Self {
            dark: false,
            nav: 0,
            search: String::new(),
            show_modal: false,
            report_name: String::new(),
            plan: 1,
            notifications: true,
            form_name: "Acme Inc.".into(),
        }
    }
}

impl App for Dashboard {
    type Msg = Msg;

    fn update(&mut self, msg: Msg) {
        match msg {
            Msg::SetDark(d) => self.dark = d,
            Msg::SetNav(i) => self.nav = i,
            Msg::Search(s) => self.search = s,
            Msg::OpenModal => self.show_modal = true,
            Msg::CloseModal => self.show_modal = false,
            Msg::ReportName(s) => self.report_name = s,
            Msg::SetPlan(i) => self.plan = i,
            Msg::SetNotifications(b) => self.notifications = b,
            Msg::FormName(s) => self.form_name = s,
        }
    }

    fn theme(&self) -> Theme {
        if self.dark {
            Theme::dark()
        } else {
            Theme::light()
        }
    }

    fn view(&self) -> Element<Msg> {
        let theme = self.theme();
        row()
            .w_full()
            .h_full()
            .bg(theme.bg)
            .children([sidebar(&theme, self.nav), main_column(&theme, self)])
    }
}

fn sidebar(theme: &Theme, active: usize) -> Element<Msg> {
    let nav_item = |i: usize, label: &str| {
        let is_active = i == active;
        let mut item = row()
            .items_center()
            .gap(SP2)
            .px(SP3)
            .h(34.0)
            .rounded(R_MD)
            .shrink0()
            .cursor(Cursor::Pointer)
            .focusable(true)
            .on_click(Msg::SetNav(i))
            .transition(Transition::colors())
            .children([
                div().w(5.0).h(5.0).rounded_full().bg(if is_active {
                    theme.accent
                } else {
                    theme.neutrals.step(7)
                }),
                text(label)
                    .size(TextSize::Sm)
                    .weight(if is_active {
                        Weight::Medium
                    } else {
                        Weight::Regular
                    })
                    .color(if is_active {
                        theme.accent_text
                    } else {
                        theme.text_muted
                    }),
            ]);
        if is_active {
            item = item.bg(theme.accent_bg);
        } else {
            let hover = theme.neutrals.step(3);
            item = item.hover(move |s| s.bg(hover));
        }
        item
    };

    col()
        .w(228.0)
        .h_full()
        .p(SP3)
        .gap(SP1)
        .bg(theme.surface)
        .children([
            // Brand.
            row()
                .items_center()
                .gap(SP2)
                .px(SP3)
                .h(48.0)
                .shrink0()
                .children([
                    div().w(18.0).h(18.0).rounded(5.0).bg(theme.accent),
                    text("fenestra").weight(Weight::Semibold),
                ]),
            div().h(SP2),
        ])
        .children([
            nav_item(0, "Overview"),
            nav_item(1, "Reports"),
            nav_item(2, "Customers"),
            nav_item(3, "Billing"),
            nav_item(4, "Settings"),
        ])
        .children([
            spacer(),
            divider(),
            row().items_center().gap(SP2).p(SP2).shrink0().children([
                avatar("RH"),
                col().gap(0.0).children([
                    text("Richard Huang")
                        .size(TextSize::Sm)
                        .weight(Weight::Medium),
                    text("richard@acme.dev")
                        .size(TextSize::Xs)
                        .color(theme.text_subtle),
                ]),
            ]),
        ])
}

fn main_column(theme: &Theme, app: &Dashboard) -> Element<Msg> {
    col()
        .grow()
        .h_full()
        .children([top_bar(theme, app), divider(), content(theme, app)])
}

fn top_bar(_theme: &Theme, app: &Dashboard) -> Element<Msg> {
    row()
        .items_center()
        .gap(SP4)
        .px(SP6)
        .h(64.0)
        .shrink0()
        .children([
            text("Overview").size(TextSize::Lg).weight(Weight::Semibold),
            spacer(),
            Element::from(
                text_input(&app.search)
                    .placeholder("Search…")
                    .width(240.0)
                    .on_input(Msg::Search)
                    .id("search"),
            ),
            Element::from(
                switch(app.dark)
                    .label("Dark")
                    .on_toggle(Msg::SetDark(!app.dark))
                    .id("dark"),
            ),
            Element::from(
                button("New report")
                    .on_click(Msg::OpenModal)
                    .id("new-report"),
            ),
        ])
        .children(if app.show_modal {
            vec![new_report_modal(app)]
        } else {
            vec![]
        })
}

fn new_report_modal(app: &Dashboard) -> Element<Msg> {
    Element::from(
        modal("New report")
            .child(
                text("Reports run nightly and land in your inbox.")
                    .size(TextSize::Sm)
                    .themed(|t: &Theme, s| s.color(t.text_muted)),
            )
            .child(labeled(
                "Report name",
                Element::from(
                    text_input(&app.report_name)
                        .placeholder("Q3 retention…")
                        .width(408.0)
                        .on_input(Msg::ReportName)
                        .id("report-name"),
                ),
            ))
            .child(labeled(
                "Schedule",
                Element::from(
                    select(app.plan, ["Daily", "Weekly", "Monthly"])
                        .width(408.0)
                        .on_change(Msg::SetPlan)
                        .id("schedule"),
                ),
            ))
            .child(
                row().gap(SP3).pt(SP2).children([
                    button("Create").on_click(Msg::CloseModal).id("create"),
                    button("Cancel")
                        .variant(ButtonVariant::Ghost)
                        .on_click(Msg::CloseModal)
                        .id("cancel"),
                ]),
            )
            .on_close(Msg::CloseModal)
            .id("report-modal"),
    )
}

fn labeled<T: Into<Element<Msg>>>(label: &str, control: T) -> Element<Msg> {
    col()
        .gap(SP1)
        .children([text(label)
            .size(TextSize::Sm)
            .weight(Weight::Medium)
            .themed(|t: &Theme, s| s.color(t.text_muted))])
        .child(control)
}

fn content(theme: &Theme, app: &Dashboard) -> Element<Msg> {
    col()
        .grow()
        .p(SP6)
        .gap(SP6)
        .scroll_y()
        .id("content")
        .children([stats_row(), middle_row(theme, app)])
}

fn stats_row() -> Element<Msg> {
    div()
        .grid_cols(vec![Track::Fr(1.0); 4])
        .gap(SP4)
        .shrink0()
        .children([
            Element::from(stat_card("Revenue", "$48,210").delta("+12.5%", Status::Success)),
            Element::from(stat_card("Active users", "8,043").delta("+3.2%", Status::Success)),
            Element::from(stat_card("Churn", "2.4%").delta("+0.3%", Status::Danger)),
            Element::from(stat_card("Avg. session", "4m 32s")),
        ])
}

fn middle_row(theme: &Theme, app: &Dashboard) -> Element<Msg> {
    div()
        .grid_cols(vec![Track::Fr(5.0), Track::Fr(3.0)])
        .gap(SP4)
        .shrink0()
        .children([deployments_card(theme, app), settings_card(theme, app)])
}

fn deployments_card(theme: &Theme, app: &Dashboard) -> Element<Msg> {
    let rows: Vec<Vec<String>> = [
        ["api-server", "Healthy", "us-east-1", "2m ago"],
        ["worker-pool", "Degraded", "us-east-1", "11m ago"],
        ["edge-cache", "Healthy", "eu-west-2", "1h ago"],
        ["batch-runner", "Healthy", "ap-south-1", "3h ago"],
        ["webhooks", "Healthy", "us-east-1", "6h ago"],
    ]
    .iter()
    .filter(|r| {
        app.search.is_empty()
            || r.iter()
                .any(|c| c.to_lowercase().contains(&app.search.to_lowercase()))
    })
    .map(|r| r.iter().map(|c| (*c).to_owned()).collect())
    .collect();
    let empty = rows.is_empty();

    let mut c = card().gap(SP3).children([
        row().items_center().justify_between().children([
            text("Recent deployments").weight(Weight::Semibold),
            badge("Live", Status::Success),
        ]),
        table(["Service", "Status", "Region", "Updated"], rows),
    ]);
    if empty {
        c = c.child(
            text(format!("No services match \"{}\".", app.search))
                .size(TextSize::Sm)
                .color(theme.text_subtle),
        );
    }
    c
}

fn settings_card(_theme: &Theme, app: &Dashboard) -> Element<Msg> {
    card().gap(SP4).children([
        text("Workspace").weight(Weight::Semibold),
        labeled(
            "Organization",
            Element::from(
                text_input(&app.form_name)
                    .width(260.0)
                    .on_input(Msg::FormName)
                    .id("org"),
            ),
        ),
        labeled(
            "Plan",
            Element::from(
                select(app.plan, ["Starter", "Growth", "Scale"])
                    .width(260.0)
                    .on_change(Msg::SetPlan)
                    .id("plan"),
            ),
        ),
        Element::from(
            switch(app.notifications)
                .label("Email notifications")
                .on_toggle(Msg::SetNotifications(!app.notifications))
                .id("notify"),
        ),
        divider(),
        row().gap(SP3).children([
            button("Save changes").id("save").on_click(Msg::SetNav(0)),
            button("Reset")
                .variant(ButtonVariant::Secondary)
                .id("reset")
                .on_click(Msg::FormName("Acme Inc.".into())),
        ]),
        callout(
            Status::Accent,
            "Changes apply to all 14 members of this workspace.",
        ),
    ])
}

const SIZE: (f64, f64) = (1120.0, 720.0);

fn main() {
    if std::env::args().any(|a| a == "shot") {
        let out = std::path::Path::new("gallery");
        std::fs::create_dir_all(out).expect("create gallery dir");
        for dark in [false, true] {
            let mut app = Dashboard {
                dark,
                ..Dashboard::default()
            };
            let theme = app.theme();
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "size"
            )]
            let image = render_app(&mut app, &[], (SIZE.0 as u32, SIZE.1 as u32), &theme);
            let name = if dark {
                "dashboard_dark"
            } else {
                "dashboard_light"
            };
            image
                .save(out.join(format!("{name}.png")))
                .expect("write png");
            println!("wrote gallery/{name}.png");
        }
        return;
    }
    fenestra::run(
        Dashboard::default(),
        WindowOptions::titled("fenestra dashboard").with_size(SIZE.0, SIZE.1),
    );
}
