# Thinking in fenestra

fenestra is Elm-shaped, aggressively so.

- **The app owns all state.** `view(&self)` builds the whole element tree
  from scratch on every redraw. There is no diffing and no retained widget
  objects; the tree is cheap plain data (a full real screen builds, lays
  out, and paints in ~0.3 ms — see BENCHMARKS.md).
- **Handlers carry messages, not closures over state.**
  `button("Save").on_click(Msg::Save)` — `update(&mut self, msg)` is the
  only place state changes.
- **Widgets never keep their own copies of your data.** A `text_input`
  shows exactly `&self.value` and emits `on_input`; if you do not store
  the new value, typing does nothing. This is the number-one beginner
  surprise and it is by design: one source of truth.
- **Identity is explicit where it matters.** Retained UI state (scroll
  offsets, editor carets, animations, open menus) keys off a stable
  `WidgetId` derived from tree position — or from `.id("...")`, which you
  should set on inputs, selects, scroll containers, and overlays.
- **Composition is `Element::map`.** A component written around its own
  message type embeds anywhere:

```rust,ignore
fn card_component() -> Element<CardMsg> { /* ... */ }

let el: Element<AppMsg> = card_component().map(AppMsg::Card);
```
