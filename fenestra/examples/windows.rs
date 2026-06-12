//! Multi-window: one app state, several OS windows. The main window
//! lists probes; Inspect (or double-clicking a row) opens a per-probe
//! inspector window. [`App::windows`] declares the open set — adding a
//! key opens a window, removing it closes one, and the OS close button
//! emits `on_close`. State is shared: boosts applied in an inspector
//! update the main list live. Native only (the web runner ignores
//! secondary windows).

use fenestra::prelude::*;

const PROBES: [(&str, &str); 4] = [
    ("Voyager", "Interstellar survey probe, launched 1977."),
    ("Cassini", "Saturn orbiter, ended in the Grand Finale."),
    ("Rosetta", "Comet chaser that landed Philae on 67P."),
    ("Dawn", "Ion-drive visitor to Vesta and Ceres."),
];

struct Fleet {
    /// Which probes have an inspector window open.
    open: Vec<usize>,
    boosts: [u32; PROBES.len()],
}

#[derive(Clone)]
enum Msg {
    Inspect(usize),
    CloseInspector(usize),
    Boost(usize),
}

impl App for Fleet {
    type Msg = Msg;

    fn update(&mut self, msg: Msg) {
        match msg {
            Msg::Inspect(i) => {
                if !self.open.contains(&i) {
                    self.open.push(i);
                }
            }
            Msg::CloseInspector(i) => self.open.retain(|&o| o != i),
            Msg::Boost(i) => self.boosts[i] += 1,
        }
    }

    fn view(&self) -> Element<Msg> {
        col().p(SP6).gap(SP4).children([
            col().gap(SP1).children([
                text("Probe fleet").size(TextSize::Xl).weight(Weight::Semibold),
                text("Inspect (or double-click) opens one window per probe; all windows share the app state.")
                    .size(TextSize::Sm)
                    .themed(|t: &Theme, s| s.color(t.text_muted)),
            ]),
            col().gap(SP2).children(PROBES.iter().enumerate().map(|(i, (name, _))| {
                row()
                    .items_center()
                    .gap(SP3)
                    .p(SP3)
                    .rounded(R_MD)
                    .themed(|t: &Theme, s| s.bg(t.elevated_surface(1)).border(1.0, t.border_subtle))
                    .on_double_click(Msg::Inspect(i))
                    .children([
                        text(*name).weight(Weight::Medium).grow(),
                        text(format!("boost ×{}", self.boosts[i]))
                            .size(TextSize::Sm)
                            .themed(|t: &Theme, s| s.color(t.text_muted)),
                        Element::from(
                            button("Inspect")
                                .variant(ButtonVariant::Secondary)
                                .on_click(Msg::Inspect(i)),
                        ),
                    ])
            })),
        ])
    }

    fn windows(&self) -> Vec<WindowDesc<Msg>> {
        self.open
            .iter()
            .map(|&i| {
                WindowDesc::new(
                    format!("probe-{i}"),
                    format!("Inspector — {}", PROBES[i].0),
                    (380.0, 260.0),
                    Msg::CloseInspector(i),
                )
            })
            .collect()
    }

    fn view_for(&self, key: &str) -> Element<Msg> {
        if key == MAIN_WINDOW {
            return self.view();
        }
        let i = key
            .strip_prefix("probe-")
            .and_then(|n| n.parse::<usize>().ok())
            .filter(|&i| i < PROBES.len())
            .unwrap_or(0);
        let (name, blurb) = PROBES[i];
        col().p(SP6).gap(SP4).children([
            text(name).size(TextSize::Xl).weight(Weight::Semibold),
            text(blurb).themed(|t: &Theme, s| s.color(t.text_muted)),
            text(format!("Signal boost: ×{}", self.boosts[i])),
            row().gap(SP3).children([
                button("Boost signal").on_click(Msg::Boost(i)),
                button("Close")
                    .variant(ButtonVariant::Secondary)
                    .on_click(Msg::CloseInspector(i)),
            ]),
        ])
    }
}

fn main() {
    fenestra::run(
        Fleet {
            open: Vec::new(),
            boosts: [0; PROBES.len()],
        },
        WindowOptions::titled("Probe fleet").with_size(520.0, 420.0),
    );
}
