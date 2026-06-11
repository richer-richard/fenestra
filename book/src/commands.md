# Commands and async

`update` is synchronous on purpose. Background work flows in through the
command proxy:

```rust,ignore
impl App for Clock {
    type Msg = Msg;

    fn init(&mut self, proxy: Proxy<Msg>) {
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_secs(1));
            proxy.send(Msg::Tick);
        });
    }
    // ...
}
```

`Proxy<Msg>` is cloneable and thread-safe; sends wake the event loop,
apply through `update`, and repaint. After the window closes, sends drop
silently. (`A::Msg: Send` — messages cross threads.)

This composes with anything async: spawn a tokio runtime in `init`, keep
the proxy in your tasks; or pair with `rfd`'s file dialogs
(`examples/file_dialog.rs`). Headlessly, `render_app` drains proxied
messages at deterministic points, so init-time sends are testable.

Toast auto-dismiss is the canonical pattern: push the toast, spawn a
timer thread, send the removal message (`examples/toasts.rs`).
