# Multiple windows

Secondary windows are app state, exactly like overlays: [`App::windows`]
returns the set that should be open, and the runner reconciles it after
every update — new keys open a window, removed keys close one, changed
titles apply live.

```rust,ignore
fn windows(&self) -> Vec<WindowDesc<Msg>> {
    self.open
        .iter()
        .map(|&i| WindowDesc::new(
            format!("probe-{i}"),               // stable key
            format!("Inspector — {}", NAMES[i]), // title, live-updated
            (380.0, 260.0),                      // logical size at open
            Msg::CloseInspector(i),              // the OS close button
        ))
        .collect()
}

fn view_for(&self, key: &str) -> Element<Msg> {
    match key {
        MAIN_WINDOW => self.view(),
        key => self.inspector(key),
    }
}
```

There is one `update` and one source of truth: a message from any window
mutates the same app state, and every window repaints from it. Each
window keeps its own *retained* state — focus, scroll offsets, text
editors — keyed by your stable `key`, plus its own IME anchor and
accessibility tree.

The OS close button closes nothing by itself: it emits `on_close`, and
your `update` removes the desc. That means you can intercept — confirm,
save, or veto by keeping the desc in the list.

Notes: native only (the web runner ignores `windows()`); `view_for`
defaults to `view()`, so single-window apps never see this API.
`examples/windows.rs` is the working pattern.

Per-window themes: override `theme_for(&self, key) -> Theme` (defaults
to `theme()` everywhere) — a dark inspector next to a light main window
is one match away. The runner consults it per window; the test harness
keeps its single explicit theme for determinism.
