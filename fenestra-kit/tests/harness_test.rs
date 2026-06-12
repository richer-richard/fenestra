//! The 0.5 verification harness itself: strict queries, message logging,
//! keyboard focus, drag, and the deterministic clock.

use fenestra_core::{App, Element, Semantics, Theme, by, col, row, text};
use fenestra_kit::button;
use fenestra_shell::Harness;

#[derive(Default)]
struct Counter {
    n: i64,
}

#[derive(Clone, Debug, PartialEq)]
enum Msg {
    Inc,
    Dec,
}

impl App for Counter {
    type Msg = Msg;

    fn update(&mut self, msg: Msg) {
        match msg {
            Msg::Inc => self.n += 1,
            Msg::Dec => self.n -= 1,
        }
    }

    fn view(&self) -> Element<Msg> {
        col().p(16.0).gap(8.0).children([
            text(format!("count: {}", self.n)),
            row().gap(8.0).children([
                button("Increment").on_click(Msg::Inc),
                button("Decrement").on_click(Msg::Dec),
            ]),
        ])
    }
}

#[test]
fn clicks_resolve_by_role_and_name() {
    let mut h = Harness::new(Counter::default(), Theme::light(), (400, 200));
    h.click(&by::role(Semantics::Button).name("Increment"));
    h.click(&by::role(Semantics::Button).name("Increment"));
    h.click(&by::role(Semantics::Button).name("Decrement"));
    assert_eq!(h.app().n, 1);
    // The view reflects it, found by content like a user would.
    assert!(h.query(&by::label("count: 1")).is_some());
    // And the message log captured exactly what the UI emitted.
    assert_eq!(h.take_messages(), vec![Msg::Inc, Msg::Inc, Msg::Dec]);
    assert!(h.take_messages().is_empty(), "taking drains the log");
}

#[test]
fn keyboard_activation_through_tab_order() {
    let mut h = Harness::new(Counter::default(), Theme::light(), (400, 200));
    h.tab(); // Increment
    h.key(fenestra_core::KeyInput::plain(fenestra_core::Key::Enter));
    assert_eq!(h.app().n, 1);
    h.tab(); // Decrement
    h.key(fenestra_core::KeyInput::plain(fenestra_core::Key::Enter));
    assert_eq!(h.app().n, 0);
}

#[test]
#[should_panic(expected = "is ambiguous")]
fn ambiguous_queries_panic_instead_of_guessing() {
    let h = Harness::new(Counter::default(), Theme::light(), (400, 200));
    // Two buttons match a bare role query — silently picking one would
    // make the test lie, so this panics with the tree in the message.
    let _ = h.get(&by::role(Semantics::Button));
}

#[test]
#[should_panic(expected = "no node matches")]
fn missing_nodes_panic_with_the_tree() {
    let h = Harness::new(Counter::default(), Theme::light(), (400, 200));
    let _ = h.get(&by::label("does not exist"));
}

// ----------------------------------------------------------------- drag

#[derive(Default)]
struct Board {
    dropped: Option<String>,
}

#[derive(Clone)]
struct Dropped(String);

impl App for Board {
    type Msg = Dropped;

    fn update(&mut self, Dropped(payload): Dropped) {
        self.dropped = Some(payload);
    }

    fn view(&self) -> Element<Dropped> {
        row().p(16.0).gap(40.0).children([
            col()
                .w(120.0)
                .h(80.0)
                .id("card")
                .drag_source("card-7")
                .children([text("Card 7")]),
            col()
                .w(120.0)
                .h(80.0)
                .id("slot")
                .on_drop(|payload| Some(Dropped(payload.to_owned()))),
        ])
    }
}

#[test]
fn drag_between_semantic_targets() {
    let mut h = Harness::new(Board::default(), Theme::light(), (400, 200));
    h.drag(&by::id("card"), &by::id("slot"));
    assert_eq!(h.app().dropped.as_deref(), Some("card-7"));
}
