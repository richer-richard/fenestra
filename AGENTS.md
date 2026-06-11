# Building fenestra UIs as an agent

fenestra is designed so that an agent authoring a UI can *see and verify*
its work without a display server. This file is the working manual.

## The loop

1. Write a view (or a full `App`).
2. Render it headlessly to a PNG.
3. **Open and look at the PNG.** Layout bugs, clipped text, and bad
   spacing are visible; do not skip this step.
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

## Driving a real app

`render_app` runs the full Elm loop — dispatch, state, focus, editing —
against scripted input, then renders one settle frame:

```rust
use fenestra::shell::{SyntheticEvent, render_app};

let mut app = MyApp::default();
let image = render_app(
    &mut app,
    &[
        SyntheticEvent::Tab,                                  // focus first widget
        SyntheticEvent::Text("hello".into()),                 // type into it
        SyntheticEvent::Key(KeyInput::plain(Key::Enter)),
        SyntheticEvent::MouseMove { x: 50.0, y: 34.0 },
        SyntheticEvent::MouseDown,
        SyntheticEvent::MouseUp,                              // click = press+release
    ],
    (800, 600),
    &Theme::light(),
);
assert_eq!(app.value, "hello"); // state assertions
image.save("after.png").unwrap(); // and pixel inspection
```

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

## Vocabulary cheat sheet

Constructors: `div() row() col() stack() text(s) spacer() divider()
path(bez, viewbox, stroke) image_rgba8(w, h, px) raw_input(v, ph)
raw_text_area(v, ph)`

Layout: `.p/.px/.py/.pt/.pr/.pb/.pl(f32)` padding, `.m*` margins,
`.gap(f32)`, `.w/.h/.min_w/.max_w/.min_h/.max_h(Length)`, `.w_full()
.h_full() .grow() .shrink0() .wrap()`, `.items_start/center/end/baseline()`,
`.justify_start/center/end/between()`, `.absolute() .top/.right/.bottom/
.left(f32)`, `.grid_cols/.grid_rows(tracks) .grid_col/.grid_row(start, span)`,
`.overflow_hidden() .scroll_y()`

Style: `.bg(paint) .border(w, color) .rounded(r) .rounded_full()
.shadow(ShadowToken) .opacity(f32)`; text: `.size(TextSize) .weight(Weight)
.color(c) .mono() .truncate() .text_align(..)`

Interaction: `.on_click(msg) .on_hover(msg) .on_key(f) .on_drag(f)
.on_input(f) .on_close(msg) .focusable(true) .disabled(b) .cursor(..)`;
variants `.hover/.active/.focus(f)` (+ `_themed`); `.transition(Transition::colors())`;
`.keyframes(Keyframes::new(ms).stop(at, f))`; `.spin(ms)`; `.overlay(Overlay::menu())`

Composition: `.semantics(..) .label(..) .id("stable-key")`,
`element.map(WrapMsg)` to embed a child component's messages.

Kit: `button checkbox switch radio slider text_input text_area select
tooltip modal toast_stack tabs card stat_card badge avatar progress
spinner table callout icons::* icons::lucide::*`

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
- **Heterogeneous children arrays need `Element::from`**: kit builders
  convert via `Into`, so `[Element::from(button(..)), text(..).into()]`
  or call `.child(..)` repeatedly.
- **Enter arrives as `Key::Enter`, not text.** Control characters never
  enter single-line inputs; text areas accept `\n`.
- **Async work**: implement `App::init`, keep the `Proxy<Msg>`, send
  messages from any thread; the window repaints. In `render_app`, proxied
  messages drain deterministically before each event.
- **Long lists**: the whole tree rebuilds every frame; for thousands of
  rows use the virtualized list widget rather than mapping every row.
- Sizes are clamped headlessly to the device texture limit (≥1, typically
  ≤8192) — check `image.dimensions()` if you requested something unusual.
- **Fonts**: `Fonts::embedded()` (headless default) is deterministic and
  Latin-only; the windowed runner uses `Fonts::with_system()`, which
  falls back through system fonts per script (CJK works). Custom faces:
  `fonts.register(FamilyRole::Display, bytes)` + `render_element_with`.
  Color emoji coverage depends on vello's COLR support — treat it as
  unreliable for now.

## Workspace map

| Crate | Contents |
| --- | --- |
| `fenestra` | facade, `run()`, prelude, examples |
| `fenestra-core` | element IR, theme/tokens, layout, text, paint, input, transitions, accessibility projection |
| `fenestra-shell` | winit/wgpu window runner, headless renderer, synthetic events, snapshot harness |
| `fenestra-kit` | the themed widget kit (built only on core's public API) |

`ARCHITECTURE.md` records every integration decision; read it before
changing pipeline internals.
