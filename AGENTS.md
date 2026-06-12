# Building fenestra UIs as an agent

fenestra is designed so that an agent authoring a UI can *see and verify*
its work without a display server. This file is the working manual.

## The loop

1. Write a view (or a full `App`).
2. Drive it with the harness — semantic queries, not coordinates — and
   assert structure and messages.
3. Render it headlessly to a PNG. **Open and look at the PNG.** Layout
   bugs, clipped text, and bad spacing are visible; do not skip this.
4. Iterate until it looks right, then lock it with a golden test.

```rust
use fenestra::prelude::*;
use fenestra::shell::render_element;

let view: Element<()> = col().p(SP6).gap(SP4).children([
    text("Hello").size(TextSize::Xl2).weight(Weight::Semibold),
    button("Save").into(),
]);
let image = render_element(view, &Theme::light(), (480, 240));
image.save("preview.png").unwrap(); // now actually look at it
```

Headless rendering is deterministic: embedded Inter fonts, scale 1.0,
reduced motion, in-memory clipboard. The same tree renders the same pixels
on every machine of the same GPU class (cross-rasterizer runs use a small
tolerance; see Golden tests).

## Driving a real app: the harness

`Harness` runs the full Elm loop — dispatch, state, focus, editing —
driven by semantic queries (find things the way a user would; never
hardcode coordinates):

```rust
use fenestra::prelude::*;
use fenestra::shell::Harness;

let mut h = Harness::new(MyApp::default(), Theme::light(), (800, 600));
h.click(&by::role(Semantics::Button).name("Add"));   // strict: 0 or 2+ matches panic
h.type_text("hello");                                 // into the focused input
h.key(KeyInput::plain(Key::Enter));

assert_eq!(h.app().items.len(), 1);                   // state
assert!(h.query(&by::label("hello")).is_some());      // structure (None = absent)
let msgs = h.take_messages();                          // behavior: what the UI emitted
h.render().save("after.png").unwrap();                 // pixels — now look at it
```

Verbs: `click right_click double_click triple_click shift_click hover
type_text key tab shift_tab focus drag drop_file wheel`; `pump(ms)` advances the
deterministic clock; `activate_window(key)` / `render_window(key)` for
multi-window apps. Failed lookups print the whole accessibility tree —
read it, it names every role and label on screen.

Two inspector dumps when lost: `h.frame().debug_tree()` (layout rects,
flags, `src=file:line` builder provenance) and
`h.frame().access_yaml()` (Playwright aria-snapshot grammar).

### JSON scenarios (no recompile)

For quick probes, drive any app from JSON instead of Rust:

```json
{"steps": [
  {"click": {"role": "button", "name": "Add"}},
  {"type": "hello"},
  {"assert": {"exists": {"label": "hello"}}},
  {"shot": "after-add"}
]}
```

`fenestra::shell::run_scenario(&mut harness, json, shots_dir)` — typos
and missing targets are loud errors carrying the step index and the
accessibility tree.

`render_app(&mut app, &[SyntheticEvent::...], size, &theme)` remains
for coordinate-level pixel probes.

## Asserting structure (accessibility tree)

Every widget exposes role, state, name, and value. Assert on it instead of
pixels when you care about structure:

```rust
use fenestra::prelude::*;

let frame = build_frame(&view, &theme, &mut fonts, &mut state, (800.0, 600.0), 1.0);
let tree = frame.access_tree();
// Walk AccessNode { id, semantics, label, value, rect, focusable, children }
// e.g. find Some(Semantics::Button) with label == Some("Save").
```

The same tree feeds real assistive technology (AccessKit) in windowed
runs, so labeling your widgets is both testable and genuinely accessible.
Icon-only buttons need `.label("...")`.

## Golden tests

```rust
use fenestra_shell::testing::assert_png_snapshot;

assert_png_snapshot(snapshot_dir(), "my_widget", &image);
```

- `FENESTRA_UPDATE_SNAPSHOTS=1 cargo test` writes/updates goldens.
- Failures write `<name>.actual.png` next to the golden — look at both.
- Tolerance: 3/255 per channel, 0.2% of pixels; macOS/Metal is the
  reference platform. Other rasterizers (CI software adapters) set
  `FENESTRA_SNAPSHOT_BUDGET=0.006`.
- Failures write `<name>.actual.png`, `<name>.diff.png` (offending
  pixels in red — look here first), and `<name>.side.png`
  (golden | actual | diff) next to the golden.

## Vocabulary cheat sheet

Constructors: `div() row() col() stack() text(s) spacer() divider()
path(bez, viewbox, stroke) image_rgba8(w, h, px) raw_input(v, ph)
raw_text_area(v, ph) rich_text([span(s).weight(..).color(..)
.size_px(..).family(..).italic(), ..])`

Layout: `.p/.px/.py/.pt/.pr/.pb/.pl(f32)` padding, `.m*` margins,
`.gap(f32)`, `.w/.h/.min_w/.max_w/.min_h/.max_h(Length)`, `.w_full()
.h_full() .grow() .shrink0() .wrap()`, `.items_start/center/end/baseline()`,
`.justify_start/center/end/between()`, `.absolute() .top/.right/.bottom/
.left(f32)`, `.grid_cols/.grid_rows(tracks) .grid_col/.grid_row(start, span)`,
`.overflow_hidden() .scroll_y() .stick_to_bottom()`

Style: `.bg(paint) .border(w, color) .rounded(r) .rounded_full()
.shadow(ShadowToken) .opacity(f32)`; text: `.size(TextSize) .weight(Weight)
.color(c) .mono() .truncate() .text_align(..)`

Interaction: `.on_click(msg) .on_right_click(msg) .on_double_click(msg)
.on_hover(msg) .on_key(f) .on_drag(f) .on_input(f) .on_close(msg)
.on_file_drop(f) .drag_source(s) .on_drop(f) .focusable(true)
.autofocus() .disabled(b) .cursor(..)`;
variants `.hover/.active/.focus(f)` (+ `_themed`); `.transition(Transition::colors())`
or `Transition::spring()`; `.enter(t)` fade-in on first appearance;
`.selectable()` copyable text; `.on_type_ahead(f)` buffered jumps;
`.keyframes(Keyframes::new(ms).stop(at, f))`; `.spin(ms)`; `.overlay(Overlay::menu())`

Composition: `.semantics(..) .label(..) .id("stable-key")`,
`element.map(WrapMsg)` to embed a child component's messages;
`.children((a, b, c))` mixes kit builders and elements (tuple, up to
12) — no `Element::from` needed.

Queries: `by::role(Semantics::Button).name("Save")`, `by::label("…")`,
`by::value("…")`, `by::id("key")`, `_contains` variants;
`get` (strict) / `query` (Option) / `get_all`.

Kit: `button checkbox switch radio slider text_input text_area select
tooltip modal toast_stack tabs card stat_card badge avatar progress
spinner table data_table callout virtual_list menu dropdown_menu
context_menu popover combobox command_palette split_pane tree_view
date_picker badge_dot progress_indeterminate icons::* icons::lucide::*`;
sibling crates: fenestra-charts, fenestra-markdown, fenestra-looks

Tokens: spacing `SP0..SP16` (4px grid), radii `R_SM R_MD R_LG R_XL R_FULL`,
`TextSize::{Xs..Xl2}`, `Weight::{Regular,Medium,Semibold}`,
`ShadowToken::{Sm,Md,Lg}`, `Theme::{light,dark,from_accent(hue, mode)}`.

## Rules that prevent the common mistakes

- **The app owns all values** (Elm): `text_input(&self.value).on_input(Msg::Set)`
  — the widget never keeps its own copy. If typing "does nothing", you
  forgot to store the value in `update`.
- **Give stateful widgets stable `.id("...")`** (inputs, selects, scroll
  containers, overlays). Identity is positional otherwise and state
  follows the id.
- **Colors go through the theme.** In reusable widgets use
  `.themed(|t, s| s.bg(t.surface))` — `view()` has no theme parameter on
  purpose. Hardcoded colors break dark mode.
- **Mixing widget types in `children`**: use a tuple —
  `.children((text(..), button(..)))`. Arrays stay for one type or
  pre-converted `Element`s.
- **Enter arrives as `Key::Enter`, not text.** Control characters never
  enter single-line inputs; text areas accept `\n`.
- **Async work**: implement `App::init`, keep the `Proxy<Msg>`, send
  messages from any thread; the window repaints. In `render_app`, proxied
  messages drain deterministically before each event.
- **Long lists**: the whole tree rebuilds every frame; for thousands of
  rows use the virtualized list widget rather than mapping every row.
- **Multiple windows**: `App::windows()` declares the open set
  (`WindowDesc::new(key, title, size, on_close_msg)`), `view_for(key)`
  routes views; the OS close button only emits `on_close` — remove the
  desc in `update` to actually close. Native-only; headless testing
  drives `view_for` + `windows()` directly (they're plain methods).
- Sizes are clamped headlessly to the device texture limit (≥1, typically
  ≤8192) — check `image.dimensions()` if you requested something unusual.
- **Fonts**: `Fonts::embedded()` (headless default) is deterministic and
  Latin-only; the windowed runner uses `Fonts::with_system()`, which
  falls back through system fonts per script (CJK works). Custom faces:
  `fonts.register(FamilyRole::Display, bytes)` + `render_element_with`.
  Color emoji render through system fonts (pixel-proven on macOS);
  embedded fonts have none, and VS16 sequences (❤️) fall back to
  monochrome text presentation.

## Workspace map

| Crate | Contents |
| --- | --- |
| `fenestra` | facade, `run()`, prelude, examples |
| `fenestra-core` | element IR, theme/tokens, layout, text, paint, input, transitions, accessibility projection |
| `fenestra-shell` | winit/wgpu window runner, headless renderer, synthetic events, snapshot harness |
| `fenestra-kit` | the themed widget kit (built only on core's public API) |

`ARCHITECTURE.md` records every integration decision; read it before
changing pipeline internals.
