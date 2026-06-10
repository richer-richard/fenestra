//! The canonical fenestra app: the counter from the README.

use fenestra::prelude::*;

struct Counter {
    n: i64,
}

#[derive(Clone)]
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
        col()
            .p(SP6)
            .gap(SP4)
            .items_center()
            .justify_center()
            .children([
                col().items_center().children([text(self.n.to_string())
                    .size(TextSize::Xl2)
                    .weight(Weight::Semibold)]),
                row().gap(SP3).children([
                    button("Decrement")
                        .variant(ButtonVariant::Secondary)
                        .on_click(Msg::Dec),
                    button("Increment").on_click(Msg::Inc),
                ]),
            ])
    }
}

fn main() {
    fenestra::run(Counter { n: 0 }, WindowOptions::titled("Counter"));
}
