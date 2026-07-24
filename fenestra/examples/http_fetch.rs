//! The effect layer end to end: `update_with` returns a `Cmd` describing
//! work (an HTTP GET on a worker thread), the runner executes it, and the
//! result comes back as a message — no hand-rolled threads, no shared
//! state. A `Sub::every` ticker shows elapsed time while the fetch runs.
//!
//! `cargo run --example http_fetch`

use std::time::Duration;

use fenestra::prelude::*;

struct Fetcher {
    status: Status,
    elapsed_ms: u64,
}

enum Status {
    Idle,
    Loading,
    Done(String),
    Failed(String),
}

#[derive(Clone)]
enum Msg {
    Fetch,
    Got(Result<String, String>),
    Tick,
}

/// The blocking work itself: plain synchronous code on a worker thread.
fn fetch_example_com() -> Result<String, String> {
    let mut res = ureq::get("https://example.com")
        .call()
        .map_err(|e| e.to_string())?;
    let body = res.body_mut().read_to_string().map_err(|e| e.to_string())?;
    // The headline of the fetched page is proof enough.
    let title = body
        .split("<title>")
        .nth(1)
        .and_then(|rest| rest.split("</title>").next())
        .unwrap_or("(no title)")
        .trim()
        .to_owned();
    Ok(format!("{title} — {} bytes", body.len()))
}

impl App for Fetcher {
    type Msg = Msg;

    fn update(&mut self, _: Msg) {}

    fn update_with(&mut self, msg: Msg) -> Cmd<Msg> {
        match msg {
            Msg::Fetch => {
                self.status = Status::Loading;
                self.elapsed_ms = 0;
                Cmd::task(|| Msg::Got(fetch_example_com()))
            }
            Msg::Got(Ok(summary)) => {
                self.status = Status::Done(summary);
                Cmd::none()
            }
            Msg::Got(Err(e)) => {
                self.status = Status::Failed(e);
                Cmd::none()
            }
            Msg::Tick => {
                self.elapsed_ms += 100;
                Cmd::none()
            }
        }
    }

    /// The ticker exists only while a request is in flight: declaring
    /// subscriptions from state starts and stops them automatically.
    fn subscriptions(&self) -> Vec<Sub<Msg>> {
        if matches!(self.status, Status::Loading) {
            vec![Sub::every("elapsed", Duration::from_millis(100), || {
                Msg::Tick
            })]
        } else {
            Vec::new()
        }
    }

    fn view(&self) -> Element<Msg> {
        let status: Element<Msg> = match &self.status {
            Status::Idle => text("Press fetch to load example.com"),
            Status::Loading => text(format!("Loading… {} ms", self.elapsed_ms)),
            Status::Done(s) => text(format!("Loaded: {s}")),
            Status::Failed(e) => text(format!("Failed: {e}")),
        };
        col()
            .p(SP6)
            .gap(SP4)
            .items_center()
            .justify_center()
            .children((status, button("Fetch").on_click(Msg::Fetch)))
    }
}

fn main() {
    fenestra::run(
        Fetcher {
            status: Status::Idle,
            elapsed_ms: 0,
        },
        WindowOptions::titled("HTTP fetch").with_size(480.0, 240.0),
    );
}
