//! Recurring effects: `App::subscriptions` declares a once-a-second tick
//! and the runner keeps it firing while the declaration stands — no
//! hand-rolled threads. (For work the framework can't schedule — your own
//! runtime, a device callback — `App::init`'s `Proxy` remains the escape
//! hatch.)
//!
//! `cargo run --example clock`

use std::time::Duration;

use fenestra::prelude::*;

struct Clock {
    seconds: u64,
}

#[derive(Clone)]
enum Msg {
    Tick,
}

impl App for Clock {
    type Msg = Msg;

    fn update(&mut self, msg: Msg) {
        match msg {
            Msg::Tick => self.seconds += 1,
        }
    }

    fn subscriptions(&self) -> Vec<Sub<Msg>> {
        vec![Sub::every("uptime", Duration::from_secs(1), || Msg::Tick)]
    }

    fn view(&self) -> Element<Msg> {
        let (h, m, s) = (
            self.seconds / 3600,
            (self.seconds / 60) % 60,
            self.seconds % 60,
        );
        col()
            .w_full()
            .h_full()
            .items_center()
            .justify_center()
            .gap(SP2)
            .children([
                text(format!("{h:02}:{m:02}:{s:02}"))
                    .size(TextSize::Xl2)
                    .mono()
                    .weight(Weight::Semibold),
                text("uptime, delivered by a subscription")
                    .size(TextSize::Sm)
                    .themed(|t: &Theme, s| s.color(t.text_muted)),
            ])
    }
}

fn main() {
    fenestra::run(
        Clock { seconds: 0 },
        WindowOptions::titled("fenestra clock").with_size(360.0, 200.0),
    )
}
