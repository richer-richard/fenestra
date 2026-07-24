# Ship a tool in ten minutes

The shortest path from nothing to a distributable binary someone else can
run. The worked example is a log dashboard — the shape most internal
tools take (load a file, show stats, stay live) — and the full version is
`examples/agent_dashboard.rs` in the repo.

## 1. Start from the template (1 min)

```sh
cargo generate richer-richard/fenestra-template
cd my-tool && cargo run
```

You get a windowed app, a headless UI test, and CI that runs it.

## 2. State, messages, view (3 min)

Model the tool as data; the view is a pure function of it:

```rust,ignore
struct Tool { stats: Stats, status: String, live: bool }

#[derive(Clone)]
enum Msg { Reload, Loaded(Stats, String), ToggleLive }

fn view(&self) -> Element<Msg> {
    col().p(SP5).gap(SP4).children((
        row().gap(SP3).children((
            text("My tool").size(TextSize::Lg).weight(Weight::Semibold),
            text(&self.status).size(TextSize::Sm).grow(),
            button("Reload").on_click(Msg::Reload),
        )),
        row().gap(SP3).children((
            stat_card::<Msg>("Rows", self.stats.rows.to_string()),
            stat_card::<Msg>("Errors", self.stats.errors.to_string()),
        )),
        virtual_list(self.stats.lines.len(), 28.0, /* row builder */),
    ))
}
```

## 3. Load data with an effect; stay live with a subscription (2 min)

```rust,ignore
fn update_with(&mut self, msg: Msg) -> Cmd<Msg> {
    match msg {
        Msg::Reload => Cmd::task(|| load("tool.log")),   // worker thread
        Msg::Loaded(stats, status) => { /* set state */ Cmd::none() }
        Msg::ToggleLive => { self.live = !self.live; Cmd::none() }
    }
}

fn init_cmd(&mut self) -> Cmd<Msg> { Cmd::msg(Msg::Reload) }

fn subscriptions(&self) -> Vec<Sub<Msg>> {
    if self.live {
        vec![Sub::every("tail", Duration::from_secs(2), || Msg::Reload)]
    } else { Vec::new() }
}
```

## 4. Prove it works, headlessly (2 min)

The test drives the app like a user and resolves the effects
deterministically — this runs in CI with no display:

```rust,ignore
let mut h = Harness::new(tool, Theme::light(), (900, 600));
h.run_effects();                            // the init load, resolved now
assert!(h.query(&by::label_contains("Rows")).is_some());
h.click(&by::label("Reload"));
assert_eq!(h.pending_effects(), 1);
```

Add a golden when pixels matter: `assert_png_snapshot(dir, "tool", &h.render())`.

## 5. Ship it (2 min)

```sh
cargo build --release
```

The result is one self-contained binary — fonts embedded, no runtime
dependencies beyond the platform's GPU stack. Hand it to a teammate, or
attach it to a release from CI. On macOS, `App::menu()` gives it a real
menu bar; a proper `.app` bundle/installer is packaging territory
(`cargo-bundle`, `cargo-dist`) and works like any Rust binary.

That's the loop: template → state/view → effects → headless proof →
`--release`. Everything else in this book deepens one of those steps.
