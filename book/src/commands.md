# Commands and async

`update` is synchronous on purpose — effects are *values*. `update_with`
returns a `Cmd<Msg>` describing what should happen outside the pure state
machine, and the runner executes it, delivering the result back as a
message:

```rust,ignore
impl App for Fetcher {
    type Msg = Msg;
    fn update(&mut self, _: Msg) {}

    fn update_with(&mut self, msg: Msg) -> Cmd<Msg> {
        match msg {
            Msg::Fetch => {
                self.status = Status::Loading;
                // Blocking is fine: this runs on a worker thread.
                Cmd::task(|| Msg::Got(fetch_example_com()))
            }
            Msg::Got(result) => {
                self.status = Status::from(result);
                Cmd::none()
            }
        }
    }
    // ...
}
```

The vocabulary:

- `Cmd::none()` — what plain state updates return (the default
  `update_with` delegates to `update` with no effect, so existing apps
  never see any of this).
- `Cmd::task(f)` — run `f` on a worker thread (HTTP with a blocking
  client, file IO, heavy compute) and deliver its return value as a
  message. The blessed HTTP pattern is `examples/http_fetch.rs` (ureq in
  a task).
- `Cmd::future(fut)` — drive a *runtime-agnostic* future off the UI
  thread. Futures that need a specific reactor (tokio IO types) should
  keep their own runtime and report back through the `Proxy` instead.
- `Cmd::msg(m)` — a follow-up message on the next loop turn. Immediate
  messages and their follow-up commands apply FIFO: a batch delivering
  `A` then `B` runs `A`'s whole follow-up chain before `B`'s, and the
  runner and the harness share one executor (`apply_cmd`), so the order
  is identical in production and tests by construction.
- `Cmd::batch([...])` — several effects; deferred work (tasks, futures)
  is delivered by completion, not position.
- `cmd.map(f)` — lift a child component's `Cmd<ChildMsg>` into the
  parent's message space, mirroring `Element::map`.
- `App::init_cmd()` — the startup effect (initial data loads).

Recurring work is a *subscription*: declare what should keep happening
from state, and the runner reconciles by key after every update — start,
stop, and restart follow your data, with no thread bookkeeping:

```rust,ignore
fn subscriptions(&self) -> Vec<Sub<Msg>> {
    if self.live {
        vec![Sub::every("tail", Duration::from_secs(2), || Msg::Reload)]
    } else {
        Vec::new()
    }
}
```

## Deterministic in tests

The harness executes the same values synchronously: deferred effects
queue instead of spawning, `run_effects()` resolves them FIFO on the test
thread, and `Sub::every` ticks fire from the explicit `pump` clock —
`pump(1000.0)` with a 300 ms timer delivers exactly the ticks at
300/600/900. Effectful apps stay pixel- and message-verifiable in CI
with no races and no wall clock:

```rust,ignore
let mut h = Harness::new(app, Theme::light(), (480, 320));
h.update(Msg::Fetch);
assert_eq!(h.pending_effects(), 1);   // queued, not run
h.run_effects();                      // resolves synchronously
assert!(h.query(&by::label("Loaded")).is_some());
```

## The escape hatch

`Proxy<Msg>` from `App::init` remains for work the framework cannot
schedule — your own tokio runtime, device callbacks, `rfd` file dialogs
(`examples/file_dialog.rs`). It is cloneable and thread-safe; sends wake
the loop and apply through `update_with`. After the window closes, sends
drop silently. (`A::Msg: Send` — messages cross threads.)
