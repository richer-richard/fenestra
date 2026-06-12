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

Live regions: `.live()` marks an element whose content changes should
be announced without focus moving there; the kit's toasts set it
themselves. Text inputs expose their selected byte range (collapsed =
caret) on `AccessNode::selection` — assert selection state headlessly.

Out of scope so far: the full screen-reader text-editing protocol
(per-character inline text boxes for braille routing and
character-by-character navigation). Field-level value, caret, and
selection are exposed; run-level geometry is not.
