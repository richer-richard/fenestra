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

## Beautiful by construction

The web styles with CSS — a universal solvent that can express anything,
including the ugly, the illegible, and the inconsistent. Beauty in CSS is
discipline *you* supply. fenestra inverts that: the architecture makes the
beautiful path the only easy path, and then *proves* the result. The
constraints are the feature.

- **Typed IR, not strings.** `Style` is a typed struct (layout / paint / text);
  every property autocompletes and type-checks. No `style="…"` to typo, no
  cascade or specificity to fight, no `!important`. An invalid style doesn't
  compile.
- **You can't name a raw color.** Color comes only from theme tokens
  (`t.accent`, `element_hover`, …) — the API funnels you to the OKLCH ramps,
  never `#bada55`. Clashing ad-hoc colors are unreachable. (Data viz is the one
  recognized exception, and even it routes through the gamut-safe `oklch`.)
- **Generated, perceptually-uniform, gamut-safe.** Palettes are generated in
  OKLCH and gamut-mapped by reducing *chroma, not lightness*, so the ramp's
  rhythm survives. The 12-step semantic scale makes interaction arithmetic
  ("+1 step"), not art; `Theme::derive` collapses the whole palette to three
  inputs.
- **Provably legible (APCA).** Because color resolves at construction,
  `validate_contrast` proves every text pair clears its floor — for every theme
  and Look, in tests. No CSS framework can guarantee that.
- **One interaction recipe.** The state layer, motion tokens, focus ring, and
  control sizes are uniform tokens, not per-widget — consistency is structural.
- **No cascade.** `themed` closures defer color to the theme; one flip restyles
  everything coherently.
- **Beauty is testable.** Headless golden PNGs lock the rendered output; every
  flagship is rendered and eyeballed against a pixel budget in CI. The aesthetic
  is enforced, not aspirational.

The thesis: a curated, generated, validated, golden-locked design system
expressed as types. You reach for tokens because the tokens are the path of
least resistance, and what they compose to has already been proven legible and
locked to a reference render.

## Influences

fenestra's design steals deliberately, and credit is part of the
contract:

- **Elm / elm-ui** — the architecture itself, and the proof that a
  small typed layout vocabulary replaces CSS.
- **Testing Library & Playwright** — query priority (role > label >
  value > test-id), strict locators, aria snapshots, and
  failure-artifact UX; fenestra's `by::` queries and `access_yaml`
  mirror them on purpose, so the muscle memory transfers.
- **Flutter** — golden tests with self-explaining failure images, the
  widget inspector's source provenance (`debug_tree`'s `src=` lines),
  and the hard-won lesson that cross-platform pixel exactness needs a
  named reference platform, not wishful thinking.
- **Jetpack Compose** — one semantics tree serving accessibility *and*
  tests; Paparazzi's record/verify screenshot ergonomics.
- **SwiftUI** — modifier chaining as the API shape worth copying.
- **Qt & AppKit** — `QUndoStack`/`NSUndoManager` semantics for editing
  (coalesced typing, event-turn boundaries), model/view for big data.
- **Avalonia.Headless** — the stepwise headless harness with explicit
  clock control.
- **egui** — the embedding "narrow waist" that made it ubiquitous, and
  (with Dear ImGui's test engine and iced's message assertions) the
  Rust-native testing prior art fenestra builds past.
