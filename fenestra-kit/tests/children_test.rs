//! The first-hour papercut, fixed: `.children(...)` takes tuples mixing
//! kit builders and core elements directly — no `Element::from`.

use fenestra_core::{App, Element, Semantics, Theme, by, col, row, text};
use fenestra_kit::{Status, badge, button};
use fenestra_shell::Harness;

struct Mixed;

#[derive(Clone)]
struct Hi;

impl App for Mixed {
    type Msg = Hi;

    fn update(&mut self, Hi: Hi) {}

    fn view(&self) -> Element<Hi> {
        col().p(8.0).gap(4.0).children((
            // Heterogeneous tuple: Element, kit Button, kit Badge, Element.
            text("title"),
            button("Go").on_click(Hi),
            badge("new", Status::Accent),
            row().children([text("a"), text("b")]), // iterators still work
        ))
    }
}

#[test]
fn tuples_mix_builders_and_elements() {
    let h = Harness::new(Mixed, Theme::light(), (300, 200));
    assert!(h.query(&by::role(Semantics::Button).name("Go")).is_some());
    assert!(h.query(&by::label("new")).is_some());
    assert!(h.query(&by::label("title")).is_some());
    assert!(h.query(&by::label("b")).is_some());
}
