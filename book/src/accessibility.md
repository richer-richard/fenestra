# Accessibility

Every interactive kit widget exposes its role, state, name, and value.
Custom elements opt in:

```rust,ignore
div()
    .on_click(Msg::Open)
    .semantics(Semantics::Button)
    .label("Open settings")
```

Text, image, and input leaves project automatically. Icon-only buttons
need `.label("...")` — they have no accessible name otherwise.

Two consumers see this tree:

1. **Tests** — `frame.access_tree()` returns plain data (`AccessNode`:
   id, semantics, label, value, rect, focusable, children). Assert your
   screens are labeled, in CI, with no platform involved.
2. **Assistive technology** — the windowed runner drives an AccessKit
   adapter: the tree pushes after every frame, and screen-reader
   activation (Click/Focus actions) routes back through your messages.

Out of scope so far: the screen-reader text-editing protocol and live
regions.
