//! Background-thread commands: `App::init` hands the app a `Proxy`, a
//! spawned thread ticks once a second, and each tick repaints the window.
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

    fn init(&mut self, proxy: Proxy<Msg>) {
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_secs(1));
                proxy.send(Msg::Tick);
            }
        });
    }

    fn update(&mut self, msg: Msg) {
        match msg {
            Msg::Tick => self.seconds += 1,
        }
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
                text("uptime, sent from a background thread")
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
