//! 0.5 multi-window headless: the harness reconciles `App::windows`,
//! routes input per window, and renders any window — shared app state
//! visibly syncs across them.

use fenestra_core::{App, Element, MAIN_WINDOW, Semantics, Theme, WindowDesc, by, col, text};
use fenestra_kit::button;
use fenestra_shell::Harness;

const PROBES: [&str; 2] = ["Voyager", "Cassini"];

#[derive(Default)]
struct Fleet {
    open: Vec<usize>,
    boosts: [u32; PROBES.len()],
}

#[derive(Clone)]
enum Msg {
    Inspect(usize),
    Close(usize),
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
            Msg::Close(i) => self.open.retain(|&o| o != i),
            Msg::Boost(i) => self.boosts[i] += 1,
        }
    }

    fn view(&self) -> Element<Msg> {
        col()
            .p(16.0)
            .gap(8.0)
            .children(PROBES.iter().enumerate().map(|(i, name)| {
                col().gap(4.0).children([
                    text(format!("{name}: boost x{}", self.boosts[i])),
                    Element::from(button(format!("Inspect {name}")).on_click(Msg::Inspect(i))),
                ])
            }))
    }

    fn windows(&self) -> Vec<WindowDesc<Msg>> {
        self.open
            .iter()
            .map(|&i| {
                WindowDesc::new(
                    format!("probe-{i}"),
                    format!("Inspector - {}", PROBES[i]),
                    (320.0, 200.0),
                    Msg::Close(i),
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
            .expect("probe key");
        col().p(16.0).gap(8.0).children([
            text(format!("{} boost x{}", PROBES[i], self.boosts[i])),
            Element::from(button("Boost").on_click(Msg::Boost(i))),
            Element::from(button("Close").on_click(Msg::Close(i))),
        ])
    }
}

#[test]
fn windows_open_route_input_and_share_state() {
    let mut h = Harness::new(Fleet::default(), Theme::light(), (480, 320));
    assert_eq!(h.window_keys(), vec![MAIN_WINDOW.to_owned()]);

    // Clicking Inspect in the main window opens the inspector window.
    h.click(&by::role(Semantics::Button).name("Inspect Voyager"));
    assert_eq!(
        h.window_keys(),
        vec![MAIN_WINDOW.to_owned(), "probe-0".to_owned()]
    );

    // Drive the inspector window; state is shared, so the main window
    // sees the boost.
    h.activate_window("probe-0");
    assert!(h.query(&by::label("Voyager boost x0")).is_some());
    h.click(&by::role(Semantics::Button).name("Boost"));
    h.click(&by::role(Semantics::Button).name("Boost"));
    assert!(h.query(&by::label("Voyager boost x2")).is_some());

    h.activate_window(MAIN_WINDOW);
    assert!(h.query(&by::label("Voyager: boost x2")).is_some());
    assert!(h.query(&by::label("Cassini: boost x0")).is_some());

    // Each window renders at its own size.
    let main_png = h.render_window(MAIN_WINDOW);
    let probe_png = h.render_window("probe-0");
    assert_eq!(main_png.dimensions(), (480, 320));
    assert_eq!(probe_png.dimensions(), (320, 200));
}

#[test]
fn closing_a_window_drops_it_and_refocuses_main() {
    let mut h = Harness::new(Fleet::default(), Theme::light(), (480, 320));
    h.click(&by::role(Semantics::Button).name("Inspect Cassini"));
    h.activate_window("probe-1");

    // The in-app Close button removes the desc; the harness reconciles
    // and falls back to the main window.
    h.click(&by::role(Semantics::Button).name("Close"));
    assert_eq!(h.window_keys(), vec![MAIN_WINDOW.to_owned()]);
    assert!(h.query(&by::label("Cassini: boost x0")).is_some());
}

#[test]
#[should_panic(expected = "no open window")]
fn activating_an_unknown_window_panics_with_the_open_set() {
    let mut h = Harness::new(Fleet::default(), Theme::light(), (480, 320));
    h.activate_window("probe-9");
}
