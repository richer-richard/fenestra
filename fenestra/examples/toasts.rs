//! Toasts with auto-dismiss: pushing a toast also spawns a timer thread
//! that expires it through the `App::init` proxy four seconds later.
//!
//! `cargo run --example toasts`

use std::time::Duration;

use fenestra::prelude::*;

struct Toasty {
    items: Vec<(u64, String, Status)>,
    next_id: u64,
    proxy: Option<Proxy<Msg>>,
}

#[derive(Clone)]
enum Msg {
    Save,
    Expire(u64),
    Dismiss(usize),
}

impl App for Toasty {
    type Msg = Msg;

    fn init(&mut self, proxy: Proxy<Msg>) {
        self.proxy = Some(proxy);
    }

    fn update(&mut self, msg: Msg) {
        match msg {
            Msg::Save => {
                let id = self.next_id;
                self.next_id += 1;
                self.items
                    .push((id, format!("Report #{id} saved"), Status::Success));
                if let Some(proxy) = self.proxy.clone() {
                    std::thread::spawn(move || {
                        std::thread::sleep(Duration::from_secs(4));
                        proxy.send(Msg::Expire(id));
                    });
                }
            }
            Msg::Expire(id) => self.items.retain(|(i, ..)| *i != id),
            Msg::Dismiss(i) => {
                if i < self.items.len() {
                    self.items.remove(i);
                }
            }
        }
    }

    fn view(&self) -> Element<Msg> {
        col()
            .w_full()
            .h_full()
            .items_center()
            .justify_center()
            .gap(SP4)
            .children([
                text("Each toast expires after 4s, or dismiss it yourself.")
                    .size(TextSize::Sm)
                    .themed(|t: &Theme, s| s.color(t.text_muted)),
                button("Save report").on_click(Msg::Save).into(),
                toast_stack(self.items.iter().map(|(_, m, s)| (m.clone(), *s)))
                    .on_dismiss(Msg::Dismiss)
                    .id("toasts")
                    .into(),
            ])
    }
}

fn main() {
    fenestra::run(
        Toasty {
            items: Vec::new(),
            next_id: 1,
            proxy: None,
        },
        WindowOptions::titled("fenestra toasts").with_size(560.0, 360.0),
    )
}
