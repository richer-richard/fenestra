# Getting started

```sh
cargo add fenestra
```

```rust,no_run
use fenestra::prelude::*;

struct Counter { n: i64 }

#[derive(Clone)]
enum Msg { Inc, Dec }

impl App for Counter {
    type Msg = Msg;

    fn update(&mut self, msg: Msg) {
        match msg { Msg::Inc => self.n += 1, Msg::Dec => self.n -= 1 }
    }

    fn view(&self) -> Element<Msg> {
        col().p(SP6).gap(SP4).items_center().children([
            text(self.n.to_string()).size(TextSize::Xl2).weight(Weight::Semibold),
            row().gap(SP3).children([
                button("Decrement").variant(ButtonVariant::Secondary).on_click(Msg::Dec),
                button("Increment").on_click(Msg::Inc),
            ]),
        ])
    }
}

fn main() { fenestra::run(Counter { n: 0 }, WindowOptions::titled("Counter")) }
```

`cargo run`, and you have a themed, antialiased, GPU-rendered window.

Linux needs `libfontconfig1-dev pkg-config` and a Vulkan driver. The same
crate compiles to `wasm32-unknown-unknown` and runs on WebGPU (see the
repository's `pages.yml` for the exact bundling steps).

The examples directory is the tour: `counter`, `dashboard` (the flagship),
`gallery`, `clock`, `toasts`, `web_demo`, `poster`, `bench`.
