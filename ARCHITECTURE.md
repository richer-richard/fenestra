# fenestra architecture

## The frame pipeline

Every redraw runs the same pure pipeline over the app's view:

1. **View.** `app.view()` rebuilds the whole `Element<Msg>` tree — plain
   structs, no diffing, no macros. `WidgetId`s are assigned during the build
   as `fnv1a(parent_id, child_index | user key)`, so identity is stable
   across rebuilds and `.id("…")` pins it where children reorder.
2. **Style resolution.** Per element: the deferred `themed` closure runs
   (tokens to concrete values — this is how kit widgets color themselves
   with no theme in scope), hover/active/focus variant overlays apply from
   `FrameState`, shadow tokens expand against the theme, role defaults fill
   (text color, divider fill), and the transition engine advances any
   animated style toward its new target (colors in OKLCH).
3. **Layout.** The resolved styles map 1:1 onto taffy (flexbox + grid);
   text and input leaves register parley-backed measure functions. A second
   pass realizes absolute rects, applying baseline alignment (parley's real
   first-line baselines), scroll offsets, and clip propagation. Overlay
   children lay out separately against the canvas and anchor to their
   parent's rect.
4. **Input** (between frames). winit events and headless `SyntheticEvent`s
   both become `InputEvent`s; `events::dispatch` hit-tests the realized
   frame (topmost branch, clip-aware, overlays first), maintains
   hover/active capture/focus, drives text editors, and returns the `Msg`s
   handlers emitted, which the runner feeds to `app.update()`.
5. **Paint.** The frame walks into a vello scene: shadows (blurred rounded
   rects, std-dev = CSS blur/2), fills (solid or gradient), borders snapped
   to the physical pixel grid, clip and alpha layers, glyph runs, carets and
   selections, then overlays with their backdrops.
6. **Present.** The window surface (vello renders to an intermediate
   texture, blitted via `TextureBlitter`), or headless: an offscreen
   texture read back into an `image::RgbaImage`.

All retained state — scroll offsets, hover times, the pressed/focused
element, transition clocks, text editors, the overlay stack — lives in one
`FrameState`, keyed by those stable `WidgetId`s. Rendering is event-driven:
the runner idles at zero CPU and schedules frames only while something
animates.

Headless rendering is the product thesis: `render_element` /
`render_app(app, events, size, theme)` run the identical pipeline at scale
1.0 with embedded fonts, reduced motion, an in-memory clipboard, and one
settle frame — deterministic enough for 3/255-tolerance PNG goldens across
Metal and lavapipe. `Frame::dump()` serializes the resolved layout tree
(ids, rects, key style props) for text snapshots.

Decisions below are recorded as they were made, milestone by milestone.

## Workspace

```
fenestra/        facade: prelude, re-exports, run(), examples/
fenestra-core/   Element IR, Style, Theme and tokens, style resolution,
                 taffy integration, parley integration, vello scene build,
                 hit testing, FrameState, transition engine
fenestra-shell/  winit + wgpu runner (windowed) and the headless renderer
fenestra-kit/    design-system widgets built only on core's public API
```

`fenestra-core` and `fenestra-kit` build and test with no window: tests use an
offscreen wgpu adapter via `fenestra-shell`'s headless renderer (a
dev-dependency only). `fenestra-shell` isolates all OS glue.

## Dependency versions (resolved 2026-06-10, latest stable)

winit 0.30.13, wgpu 29.0.3, vello 0.9.0, kurbo 0.13.1, peniko 0.6.1,
parley 0.10.0, fontique 0.10.0, taffy 0.10.1, color 0.3.3, arboard 3.6.1,
image 0.25.10, insta 1.47.2. The tree is mutually consistent: vello 0.9
requires wgpu ^29.0.3 and peniko ^0.6.1; parley 0.10 requires fontique ^0.10;
peniko 0.6.1 requires color ^0.3.3 and kurbo ^0.13.1.

vello 0.9 is the classic compute-shader renderer (`vello_encoding` +
`vello_shaders` + wgpu), not the newer sparse-strips crates (`vello_cpu`,
`vello_hybrid`).

## M0 decisions

- **Surface presentation.** vello renders with a compute shader and cannot
  bind most surface textures directly. We follow vello's own recommended
  pattern: render into the intermediate `Rgba8Unorm` STORAGE_BINDING texture
  that `vello::util::RenderSurface` maintains, then blit to the surface with
  `wgpu::util::TextureBlitter`. wgpu 29's `get_current_texture()` returns a
  `CurrentSurfaceTexture` enum (no longer `Result`); `Outdated`/`Suboptimal`
  reconfigure and retry, `Occluded`/`Timeout` just request a new redraw.
- **Headless readback.** Offscreen target texture is `Rgba8Unorm` with
  `STORAGE_BINDING | COPY_SRC`, copied to a `MAP_READ` buffer with rows padded
  to 256 bytes, then unpadded into an `image::RgbaImage`. We block with
  `std::sync::mpsc` + `device.poll(PollType::wait_indefinitely())` instead of
  pulling in `futures-intrusive`.
- **Antialiasing.** `AaConfig::Area` everywhere (window and headless), with
  renderers built `AaSupport::area_only()`. Area AA is deterministic and
  software-rasterizer friendly (Mesa lavapipe in CI), and using the same mode
  in both paths keeps on-screen output identical to the golden images.
- **Logical coordinates.** Frames are built in logical pixels into a fragment
  `Scene`, then appended to the root scene under `Affine::scale(scale_factor)`.
  Headless renders at scale 1.0, so logical == physical there.
- **Dev profile.** vello/wgpu are unusably slow unoptimized;
  `[profile.dev.package."*"] opt-level = 2` keeps our crates fast to compile
  while dependencies stay fast to run.
- **Shadow blur mapping.** Resolved in M1 (see below): `std_dev = blur / 2`
  per the CSS definition.

## M1 decisions

- **Theme generation.** `Theme::from_accent(hue, mode)` builds every color
  from the spec's L/C tables in OKLCH via the `color` crate, gamut-mapping by
  binary-searching the largest in-gamut chroma (lightness is never touched).
  Status hues (danger 25, warning 80, success 150) reuse the accent table at
  steps 3/7/9/11. All generated values are locked by insta snapshots in
  `fenestra-core/tests/theme_tokens.rs`.
- **Elevation in dark mode.** `Theme::elevated_surface(level)`: level 0 is
  `surface`, level 1 is `surface_raised` (N3 in dark), and each level above
  adds +0.025 L in dark mode. Light mode raised surfaces are always pure
  white. Overlay widgets (menus, modals) will use level 2 in M6.
- **One `Style` type, two stages.** Authored styles may carry token
  references (`shadow_token`); style resolution expands them against the
  theme into concrete values (`shadows`), fills role defaults (text color,
  divider fill), and from M4 will overlay interaction variants. The painter
  only reads resolved values. This avoids a parallel `ResolvedStyle` struct.
- **Variant overlays are closures.** `.hover(|s| s.bg(...))` stores
  `Box<dyn Fn(Style) -> Style>`; the same fluent methods exist on `Style`
  itself, so the element builders simply delegate. No macros.
- **WidgetId.** FNV-1a over `(parent_id, child_index | user key)` with tag
  bytes separating the keyed and indexed domains. FNV is deterministic across
  runs and platforms, unlike std's hasher.
- **Z-stack via grid.** `stack()` is a single-cell taffy grid; every child is
  forced into cell (1,1), so the stack sizes itself to its largest child and
  children paint in document order.
- **Shadow blur mapping.** CSS Backgrounds & Borders 3 §7.1.1 defines the
  box-shadow blur as "a Gaussian blur with a standard deviation equal to half
  the blur radius", so `std_dev = blur / 2` exactly; vello takes std_dev
  directly. Locked by the specimen shadow-stack goldens.
- **Alpha layer bounds.** vello layers always clip, but CSS opacity groups
  must not clip overflowing children: alpha layers are bounded by the element
  path when the element clips anyway, otherwise by the full canvas rect.
- **Root sizing.** A root element with `Auto` width/height is stretched to
  the canvas size, so `view()` trees fill the window like a web page body.
- **PNG harness.** `fenestra_shell::testing::assert_png_snapshot` compares
  with a 3/255 per-channel tolerance and a 0.2% differing-pixel budget
  (absorbs Metal vs lavapipe rasterization variance);
  `FENESTRA_UPDATE_SNAPSHOTS=1` regenerates; failures write `*.actual.png`
  next to the golden. A process-wide shared `Headless` renderer keeps test
  suites fast (vello shader compilation happens once).

## M2 decisions

- **Embedded Inter.** Inter 4.1 statics (Regular/Medium/SemiBold, OFL — see
  `fenestra-core/assets/inter/LICENSE.txt`) are `include_bytes!`-embedded and
  registered with fontique. Inter heads the `sans-serif` generic; the mono
  role resolves `monospace` through SF Mono / Cascadia Code / JetBrains Mono
  when installed, with Inter appended as the last resort so mono text never
  vanishes in embedded-only collections (which have no system generic
  mappings at all).
- **Two font modes.** `Fonts::embedded()` (no system fonts; deterministic,
  used by headless rendering) and `Fonts::with_system()` (windowed runner).
- **Color-free layout cache.** Parley layouts are cached keyed by (text,
  size, weight, line-height, letter-spacing, family role, align, max-lines,
  quarter-px-quantized wrap width). The parley brush is the default `[u8;4]`
  and is never set; text color is applied at draw time via
  `DrawGlyphs::brush`, so recolors (hover, theme flips) hit the cache.
- **Measure/paint width agreement.** Taffy measures text via
  `compute_layout_with_measure`; measured sizes are `ceil()`ed so the paint
  pass (which re-wraps at the final box width) reproduces the same line
  breaks.
- **Ellipsis truncation.** Parley has no built-in max-lines; fenestra binary
  searches the longest prefix whose layout plus `…` fits, over char
  boundaries, and caches the result.
- **True baseline alignment.** Taffy hardcodes `first_baselines: NONE` for
  measured leaves, so `items_baseline()` rows are laid out flex-start and the
  frame pipeline shifts each in-flow child down by `max_baseline - baseline`,
  using parley's first-line baseline for text and the bottom edge for boxes
  (CSS synthesized baseline). The same offsets will feed hit testing in M4.

## M3 decisions

- **The `Frame` object.** `build_frame` now produces a `Frame`: resolved
  styles plus final absolute logical rects for every node (baseline shifts
  and scroll offsets already applied), ancestor clip rects, and resolved
  scrollbar geometry. Paint, input routing (`scrollable_at`), and the serde
  layout dump all read this one structure, so what you hit-test is exactly
  what you painted. M4's hover/click hit testing extends the same walk.
- **Scroll state.** `FrameState` owns per-`WidgetId` scroll offsets and a
  clock (`tick(seconds)`). Offsets are clamped to the taffy `content_size`
  range during frame builds (state is mutated by the build — the one
  deliberate impurity, since clamping needs content heights). Wheel routing:
  deepest scrollable under the cursor wins, so nested lists scroll before
  the page.
- **Scrollbars.** Overlay-style (no reserved gutter, `scrollbar_width: 0`),
  6px rounded thumb in `text_subtle` at 0.6 alpha, painted after children
  inside the clip layer. Visibility: full alpha while scrolling and for
  0.8s after, then a 300ms fade; `Frame::animating` tells the runner to keep
  scheduling 16ms frames during the fade. `reduced_motion` turns the fade
  into a step function so headless renders are deterministic.
- **DPI snapping.** Layout stays logical (taffy's own rounding on). At paint
  time, border strokes round to whole physical pixels
  (`max(1, round(w*scale))/scale`) around a grid-snapped rect, and any fill
  thinner than 1.75 physical px (dividers) snaps its thin axis to the grid
  with a 1-physical-px minimum, so hairlines never straddle device pixels.
- **Grid builders.** `.grid_cols/.grid_rows([Track::Px(..), Track::Fr(..)])`
  and `.grid_col/.grid_row(start, span)` round out the section-6 vocabulary;
  the holy-grail demo and its layout-tree snapshot lock the taffy mapping.
- **`run_static`.** The windowed runner for message-free views: rebuilds the
  element tree per redraw, persists `FrameState`, routes wheel events
  through the last frame, and schedules animation frames only while
  something animates (`ControlFlow::WaitUntil`), idling at zero CPU
  otherwise. The M4 `App` runner extends this skeleton with hit testing and
  message dispatch.

## M4 decisions

- **The `themed` resolution hook.** `App::view(&self)` has no theme
  parameter, so kit widgets defer all coloring to style resolution:
  `.themed(|theme, style| ...)` (and `hover_themed`/`active_themed`/
  `focus_themed`) run during resolve with the live theme. This is the
  spec's "tokens to concrete values" step; app authors with a theme in
  scope can keep using concrete colors and the plain one-param variants.
- **Dispatch is core, runners are thin.** `events::dispatch(tree, frame,
  state, event)` owns hit testing, hover/active/focus bookkeeping, active
  capture, Tab cycling, Enter/Space activation, and message extraction.
  The windowed runner and headless `SyntheticEvent` injection translate
  into the same `InputEvent`s, so test behavior is window behavior.
  Handlers are looked up per dispatch by re-deriving WidgetIds over the
  element tree (same derivation as the frame build).
- **Hit chain = topmost branch.** `Frame::hit_chain` returns every node
  containing the point along the branch that paints last (reverse child
  order wins), clip-aware. Hover applies to all eligible nodes in the
  chain; clicks go to the deepest interactive node. Hover is recomputed on
  release (capture freezes it) and cleared by `PointerLeave`
  (winit `CursorLeft`); `refresh_hover` re-syncs it after scrolling moves
  content under a stationary pointer.
- **Transition engine.** Per-WidgetId `Anim { from, to, t0 }` in
  FrameState, GC'd by frame stamp. Retargeting continues animated
  properties from their current visual value while non-animated properties
  snap immediately (lerp at t=0). Colors lerp in OKLCH with CSS powerless-
  hue handling (an achromatic endpoint adopts the other's hue). A segment
  with equal endpoints reports settled regardless of elapsed time, so the
  runner can stop scheduling frames. `Transition.duration_ms` is a raw f32
  because the Switch travel (160ms) sits between motion tokens.
- **Path elements.** `Kind::Path` carries a kurbo `BezPath` in viewbox
  coordinates, scaled to the element rect and painted in the resolved text
  color (SVG `currentColor` semantics). `Style.path_trim` (0..=1, arclength
  prefix, animatable under the lengths flag) gives the checkbox its 120ms
  draw-on stroke; M6 icons reuse the same primitive.
- **Cursor protocol.** `Dispatch.cursor` is an `Option`: only pointer
  events set it, so keystrokes and wheel ticks never reset the OS cursor.
  Disabled elements with a cursor report NotAllowed.
- **Occluded windows.** vello work happens before the surface texture is
  acquired, so an Occluded result skips the frame entirely and the runner
  waits for `WindowEvent::Occluded(false)` instead of spinning redraws.
- **Wheel routing respects overflow.** A scroll container whose content
  fits reports `can_scroll = false` and is skipped by `scrollable_at`, so
  the wheel falls through to an overflowing ancestor.

## M5 decisions

- **The app owns the text; the editor owns the caret.** `Kind::Input` leaves
  carry the app-provided value (Elm-style: every edit emits `on_input` and
  the app echoes the new value back). The parley `PlainEditor` — caret,
  selection, IME composition, follow-scroll — is retained per `WidgetId` in
  `FrameState` and synced against the app value at every frame build, so a
  rebuilt view never loses the caret. Editors are GC'd by frame stamp like
  animations.
- **Clipboard injection.** Core defines a `Clipboard` trait with an
  in-memory default, so headless copy/paste tests are deterministic and
  display-server-free; the windowed runner injects arboard
  (`fenestra_shell::OsClipboard`).
- **Key vs Text events.** The winit runner sends printable input as
  `InputEvent::Text` (taken from `KeyEvent.text`, which handles dead keys
  and IME commits) and everything else as `Key`. Dispatch routes both to a
  focused editor first; unconsumed keys fall through to `on_key` and
  Enter/Space activation. A bare `Text(" ")` still activates focused
  buttons so Space works for non-editors.
- **Paint needs state.** `Frame::paint(fonts, state)` gained the state
  parameter: input painting refreshes the editor layout, updates the
  horizontal follow-scroll, and computes the caret blink phase (530ms
  half-period from the last edit; `reduced_motion` pins the caret visible).
  A focused input marks the frame as animating so blink frames keep coming.
- **Single line by construction.** Editor wrap width is `None`; the text
  scrolls horizontally inside the clip, caret kept in view with a
  follow-scroll that clamps to the layout width.

## M6 decisions

- **Declarative overlays.** An overlay is an element marked `.overlay(def)`
  as a child of its anchor: it leaves normal flow, lays out against the
  canvas in a second pass, positions relative to the anchor rect (Below /
  BelowCenter, flipping above when out of room) or centered, paints after
  the root, and hit-tests first. Three modes: `Open` (present-in-tree =
  open; app-driven; outside click/Esc emit `on_close`), `Toggle` (clicking
  the anchor toggles; retained in `FrameState.overlays`; closes on outside
  click, Esc, or choosing any clickable inside), and `Hover { delay_ms }`
  (tooltips; never hit-tested). Nested overlays work (a select inside a
  modal): overlay subtrees are processed as a queue with paths rebased onto
  the root element tree.
- **Modality.** A backdrop overlay dims with black 0.4 x enter-progress and
  swallows hits outside the overlay; `focusables()` returns only the top
  focus-trapping overlay's subtree, which is the entire focus trap. Enter
  animation: 200ms standard-eased fade plus an 8px slide for centered
  overlays; `reduced_motion` snaps it, keeping headless renders stable.
- **Select without retained highlight.** The listbox highlight IS the
  selected value (Elm-pure): closed-or-open, arrows step the value via
  `on_change`, Enter/Space toggles the menu, first-letter type-ahead scans
  forward with wrap. No separate highlight state to reconcile.
- **Tabs deviation.** The 2px accent indicator cross-fades between tabs
  (200ms) instead of sliding: a real slide between variable-width tabs
  needs measured-position (shared-element) animation, which the per-element
  transition engine deliberately does not do in v1.
- **Spinner rotation.** `.spin(period_ms)` rotates a path element's paint
  transform from the frame clock (no per-frame view rebuild tricks);
  `reduced_motion` freezes it for deterministic goldens.

## Hardening audit (post-M7)

A security/robustness pass over the whole workspace, with the consumer
threat model "AI agents pass arbitrary values to every public API". Each
fix below is locked by a regression test in the crate's `tests/hardening.rs`
(except the swapchain recovery, which cannot be induced headlessly).

- **Clamp over panic at the API boundary.** Out-of-range inputs to
  total-looking functions clamp to the valid range instead of asserting:
  `Ramp::step` clamps to `1..=12`; headless render sizes clamp to
  `1..=max_texture_dimension_2d` on both axes (a zero request yields a 1x1
  image, an oversized one the device limit) — the clamp happens before
  layout so the frame and the texture agree. Rationale: for agent consumers
  a panic is a DoS, and a clamped result is still inspectable.
- **Widget callbacks keep their index contract.** `select` with zero
  options no longer emits `on_change(0)` on Home (hosts index into their
  data with the emitted value, so an invalid index panics the app).
- **One text-sanitization policy, three entry paths.** Control characters
  are filtered from `Key::Char` exactly as on the text-commit and paste
  paths (Enter arriving as `'\r'` can no longer embed into a single-line
  value), and IME preedit cursor offsets clamp to the preedit length before
  reaching parley (which debug-asserts on out-of-range compose cursors).
- **All retained state is frame-stamped.** `FrameState.scroll` now carries
  the same `seen` GC stamp as `anims` and `editors`: entries whose
  container was not in the frame just built are dropped, so dynamically
  keyed scrollables cannot grow the map without bound. Consequence (same as
  editors): a scrollable unmounted for a frame loses its offset.
- **Lost surfaces recover.** `CurrentSurfaceTexture::Lost` (GPU reset,
  driver update, display change) rebuilds the swapchain on the same window
  via the `activate()` path shared with `resumed()` instead of panicking.
  `Validation` still panics: that one is a programming error.
- **Byte-stability note.** Renders are logically deterministic (embedded
  fonts, fixed scale, reduced motion), but GPU floating point can wobble
  individual pixels run to run (observed on the dark dashboard's shadow
  gradients). The PNG harness's 3/255-channel, 0.2%-pixel tolerance absorbs
  this; raw byte equality of regenerated gallery art is not guaranteed.
- **Supply chain.** CI actions are pinned to full commit SHAs, the
  workflow token is read-only (`permissions: contents: read`), and a
  `cargo audit` job runs on every push/PR plus a weekly schedule
  (vulnerabilities fail; unmaintained-crate warnings stay advisory so
  third-party churn cannot redden CI). `unsafe_code = "forbid"` is set
  workspace-wide; no crate uses unsafe today.

## M8 decisions

- **`Element::map` moves, it does not wrap.** Mapping rebuilds the node
  with converted handlers (message values mapped directly, boxed closures
  rewrapped, children recursed) rather than introducing a wrapper node, so
  widget identity, layout, and styling are untouched by composition.
- **Commands without changing `update`.** Rather than an Elm `Cmd` return
  type (which would churn every app), `App::init` hands the app a
  `Proxy<Msg>`: an `Arc<dyn Fn(Msg) + Send + Sync>`. The windowed runner
  backs it with a winit user-event envelope (`A::Msg: Send` because
  messages cross threads); headless `render_app` backs it with a collector
  drained before each event and the settle frame, keeping tests
  deterministic for synchronous sends. Messages sent after the loop dies
  drop silently.
- **Images are identity-compared.** `Kind::Image` holds `Arc`'d RGBA8
  (`peniko::ImageData`); equality is blob id + dimensions, so full-tree
  rebuilds never hash pixels. Incomplete trailing rows are dropped at
  construction (clamp-over-panic). Paint stretches to the rect and clips
  to the corner radius, which is how round avatars work.
- **Multiline measurement = text measurement.** The text area measures
  through the same parley cache as `Kind::Text` at taffy's content-box
  width (taffy passes content-box known dimensions to leaf measure), so
  measured wrap equals the editor's paint-time wrap exactly; a trailing
  newline measures one extra caret line. The sanitizer is mode-aware:
  multiline keeps normalized `\n`, single-line strips all controls.
  Internal vertical scrolling is out of scope — the area grows, and an
  outer scroll container caps it.
- **Toasts are app state.** Like the modal, the app owns the toast list;
  the kit renders it via `OverlayPlacement::TopRight` with no backdrop,
  no focus trap, and no outside-dismiss. Auto-expiry composes from the
  command proxy (a timer thread sending a removal message).
- **Lucide is vendored as path data.** No usvg dependency: shapes from
  lucide-static 1.17.0 (ISC) were converted to path-d strings (circles
  and rects become arc commands; a leading relative `m` from a
  concatenated second path is absolutized while its implicit linetos stay
  relative) and parsed at construction with `kurbo::BezPath::from_svg`.
  The painter already strokes paths with round caps and joins.
- **Keyframes are looping and clock-anchored.** `Keyframes` stops resolve
  against the element's fully-resolved base style each frame, then lerp
  every animatable property between the surrounding stops (the
  transition lerp with all flags on). No retained per-widget phase: the
  loop derives from the frame clock like `.spin`, and reduced motion pins
  the first stop. One-shot enters remain `Transition`'s job. Implementing
  this exposed and fixed an opacity bug: alpha groups now wrap the
  element's own decoration (CSS semantics), not just children.
- **Accessibility is a projection, not a parallel tree.** Core stays
  AccessKit-free: elements carry an optional `Semantics` role + label
  (text/image/input leaves project automatically), the frame exposes
  `access_tree()` as plain data (headlessly testable), and the shell maps
  it to AccessKit nodes — root `Role::Window` with a scale transform,
  ids equal to `WidgetId`s, Click/Focus actions translated back through
  `click_msg_of` and `set_focus`. The adapter attaches before the window
  first becomes visible (an AccessKit requirement; windows now start
  hidden for one frame). Screen-reader text editing and live regions are
  out of scope this release.
