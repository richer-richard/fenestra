//! The interactive demo: a mini dashboard plus the widget galleries behind
//! tabs, with a live theme toggle. Runs in a native window with
//! `cargo run --example web_demo`, and in the browser (WebGPU) as the
//! GitHub Pages demo — the same code, byte for byte.

use fenestra::prelude::*;

struct Demo {
    tab: usize,
    dark: bool,
    search: String,
    plan: usize,
    notifications: bool,
    show_modal: bool,
    report_name: String,
    notes: String,
}

#[derive(Clone)]
enum Msg {
    Tab(usize),
    Dark(bool),
    Search(String),
    Plan(usize),
    Notify(bool),
    Modal(bool),
    Report(String),
    Notes(String),
    Noop,
}

impl Default for Demo {
    fn default() -> Self {
        Self {
            tab: 0,
            dark: false,
            search: String::new(),
            plan: 1,
            notifications: true,
            show_modal: false,
            report_name: String::new(),
            notes: "Multiline notes wrap and grow.\nNewlines too.".to_owned(),
        }
    }
}

impl App for Demo {
    type Msg = Msg;

    fn update(&mut self, msg: Msg) {
        match msg {
            Msg::Tab(i) => self.tab = i,
            Msg::Dark(d) => self.dark = d,
            Msg::Search(s) => self.search = s,
            Msg::Plan(i) => self.plan = i,
            Msg::Notify(b) => self.notifications = b,
            Msg::Modal(b) => self.show_modal = b,
            Msg::Report(s) => self.report_name = s,
            Msg::Notes(s) => self.notes = s,
            Msg::Noop => {}
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
        col()
            .w_full()
            .h_full()
            .bg(theme.bg)
            .children([top_bar(self), divider()])
            .children([match self.tab {
                0 => dashboard(&theme, self),
                1 => wrap(gallery_controls(&theme)),
                2 => wrap(gallery_display(&theme)),
                _ => col().grow().scroll_y().id("spec").child(specimen(&theme)),
            }])
    }
}

/// The message-free gallery trees drop in via `Element::map`.
fn wrap(el: Element<()>) -> Element<Msg> {
    col()
        .grow()
        .scroll_y()
        .id("gallery")
        .child(el.map(|()| Msg::Noop))
}

fn top_bar(app: &Demo) -> Element<Msg> {
    row()
        .items_center()
        .gap(SP6)
        .px(SP6)
        .h(56.0)
        .shrink0()
        .children([
            row().items_center().gap(SP2).children([
                div()
                    .w(16.0)
                    .h(16.0)
                    .rounded(5.0)
                    .themed(|t: &Theme, s| s.bg(t.accent)),
                text("fenestra").weight(Weight::Semibold),
            ]),
            tabs(
                app.tab,
                ["Dashboard", "Controls", "Display", "Typography"],
                Msg::Tab,
            ),
            spacer(),
            switch(app.dark)
                .label("Dark")
                .on_toggle(Msg::Dark(!app.dark))
                .id("dark")
                .into(),
        ])
}

fn dashboard(theme: &Theme, app: &Demo) -> Element<Msg> {
    col()
        .grow()
        .p(SP6)
        .gap(SP6)
        .scroll_y()
        .id("content")
        .children([div()
            .grid_cols(vec![Track::Fr(1.0); 3])
            .gap(SP4)
            .shrink0()
            .children([
                Element::from(stat_card("Revenue", "$48,210").delta("+12.5%", Status::Success)),
                Element::from(stat_card("Active users", "8,043").delta("+3.2%", Status::Success)),
                Element::from(stat_card("Churn", "2.4%").delta("+0.3%", Status::Danger)),
            ])])
        .children([div()
            .grid_cols(vec![Track::Fr(5.0), Track::Fr(3.0)])
            .gap(SP4)
            .shrink0()
            .children([deployments(app), settings(theme, app)])])
}

fn deployments(app: &Demo) -> Element<Msg> {
    let rows: Vec<Vec<String>> = [
        ["api-server", "Healthy", "us-east-1", "2m ago"],
        ["worker-pool", "Degraded", "us-east-1", "11m ago"],
        ["edge-cache", "Healthy", "eu-west-2", "1h ago"],
        ["batch-runner", "Healthy", "ap-south-1", "3h ago"],
    ]
    .iter()
    .filter(|r| {
        app.search.is_empty()
            || r.iter()
                .any(|c| c.to_lowercase().contains(&app.search.to_lowercase()))
    })
    .map(|r| r.iter().map(|c| (*c).to_owned()).collect())
    .collect();

    card().gap(SP3).children([
        row().items_center().gap(SP4).children([
            text("Recent deployments").weight(Weight::Semibold),
            badge("Live", Status::Success),
            spacer(),
            text_input(&app.search)
                .placeholder("Filter…")
                .width(160.0)
                .on_input(Msg::Search)
                .id("search")
                .into(),
        ]),
        table(["Service", "Status", "Region", "Updated"], rows),
        button("New report")
            .on_click(Msg::Modal(true))
            .id("new-report")
            .into(),
    ])
}

fn settings(theme: &Theme, app: &Demo) -> Element<Msg> {
    let mut c = card().gap(SP4).children([
        text("Workspace").weight(Weight::Semibold),
        labeled(
            "Plan",
            select(app.plan, ["Starter", "Growth", "Scale"])
                .width(220.0)
                .on_change(Msg::Plan)
                .id("plan")
                .into(),
        ),
        labeled(
            "Notes",
            text_area(&app.notes)
                .width(220.0)
                .min_height(72.0)
                .on_input(Msg::Notes)
                .id("notes")
                .into(),
        ),
        switch(app.notifications)
            .label("Email notifications")
            .on_toggle(Msg::Notify(!app.notifications))
            .id("notify")
            .into(),
        callout(Status::Accent, "Everything here runs on wgpu + vello."),
    ]);
    let _ = theme;
    if app.show_modal {
        c = c.child(
            modal("New report")
                .child(labeled(
                    "Report name",
                    text_input(&app.report_name)
                        .placeholder("Q3 retention…")
                        .width(360.0)
                        .on_input(Msg::Report)
                        .id("report-name")
                        .into(),
                ))
                .child(
                    row().gap(SP3).pt(SP2).children([
                        button("Create").on_click(Msg::Modal(false)).id("create"),
                        button("Cancel")
                            .variant(ButtonVariant::Ghost)
                            .on_click(Msg::Modal(false))
                            .id("cancel"),
                    ]),
                )
                .on_close(Msg::Modal(false))
                .id("report-modal"),
        );
    }
    c
}

fn labeled(label: &str, control: Element<Msg>) -> Element<Msg> {
    col()
        .gap(SP1)
        .children([text(label)
            .size(TextSize::Sm)
            .weight(Weight::Medium)
            .themed(|t: &Theme, s| s.color(t.text_muted))])
        .child(control)
}

fn main() {
    fenestra::run(
        Demo::default(),
        WindowOptions::titled("fenestra demo").with_size(1120.0, 760.0),
    )
}
