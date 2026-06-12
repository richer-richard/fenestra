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

## 0.4: apps feel native

- **Input depth rides the existing dispatch, no new systems.**
  Right-click is two events (`RightDown`/`RightUp`) resolved to the
  deepest enabled `.on_right_click` in the hit chain. Double-click is
  detected in `FrameState` (same target twice within 0.4 s); both single
  clicks still fire, so selection-then-open composes. PageUp/PageDown/
  Home/End scroll the container nearest the focused element by 90%
  viewport pages — but only when the focused element didn't consume the
  key, so text inputs keep Home/End for the caret.
- **Drag-and-drop is payload strings, not a type system.** Sources carry
  `.drag_source("payload")`; `PointerDown` records the payload in
  `FrameState::dragging`, `PointerUp` over an `.on_drop` target delivers
  it. OS file drops hit-test at the last pointer position, falling back
  to the first `.on_file_drop` in tree order because some platforms
  report drops with no position.
- **`.autofocus()` is newly-appearing, not always.** The frame records
  which autofocus id it saw last; focus moves only when the id differs
  or skipped a frame — so a dialog's field focuses on open without
  stealing focus on every rebuild.
- **Sticky scroll is a cleared flag, not a position check.** A
  `.stick_to_bottom()` container records "was at bottom" in its scroll
  entry; the per-frame clamp re-pins while the flag holds, and every
  manual setter (`scroll_to`, `scroll_by`) clears it. New entries start
  pinned so chat logs open at their tail.
- **IME positioning is paint-derived.** The input painter returns the
  caret rect (even mid-blink); the frame stores it in `FrameState`, and
  the runner calls `set_ime_cursor_area` after present — the candidate
  window tracks the caret with no extra layout pass.
- **Context menus pin at the pointer.** `OverlayPlacement::Pointer{gap}`
  captures the pointer position into `FrameState::pointer_pins` when the
  overlay opens and reuses it until close — the menu must not follow the
  mouse. `Overlay::context()` = app-driven open, pointer placement, no
  backdrop. Kit: `menu` (styled panel) composed into `dropdown_menu`,
  `context_menu`, `popover`, and `combobox` (text input + filtered
  listbox, Elm-pure: the app owns value and open flag).
- **Multi-window is reconciliation, exactly like modals.** `App::windows`
  declares the open set (`WindowDesc {key, title, size, on_close}`);
  after every update the runner opens new keys, closes missing ones, and
  retitles live. Each `SecondaryWindow` bundles its own `WindowShell`,
  `FrameState`, cursor, last frame, and AccessKit adapter; app state and
  fonts stay shared. Every event handler routes by `window_id` (input,
  IME, wheel, file drop, redraw, access actions); animation timers wake
  every window, and the main window's idle `Wait` defers to any
  animating secondary. The OS close button only emits `on_close` — apps
  decide (confirm/veto) by keeping or removing the desc. Native only:
  the web runner ignores `windows()`. `view_for(key)` defaults to
  `view()`, so single-window apps never see any of it.
- **Window polish lives in `WindowOptions`.** `with_min_size`,
  `with_resizable`, `maximized`, `fullscreen` (borderless), `with_icon`
  (RGBA8; malformed data opens without an icon, never panics), and
  `with_font` — per-window custom faces registered on the system font
  stack, closing the "windowed custom fonts" gap (#9).

## 0.5: the verification wedge, sharpened

Research drove this release: a four-strand survey (declarative natives,
web tooling, desktop veterans, Rust/immediate-mode) — the synthesis
lives in the book's Influences section. Decisions:

- **Queries follow Testing Library, strictness follows Playwright.**
  `by::role/label/value/id` (+ `_contains`), `Query::name` refinement;
  `get` panics on zero *or several* matches and prints the whole
  accessibility tree (the error message is the debugger);
  `try_get -> Result<_, QueryError>` is the machine-facing form. Role
  matching compares discriminants so `by::role(Checkbox{checked:false})`
  finds every checkbox; payload assertions read the returned node. No
  regex matching (would cost a dependency); exact + contains covers the
  real cases. Our tree is unmerged (kit labels live on the interactive
  node), unlike Compose's merged default — documented, not emulated.
- **One harness, three assertion levels.** `Harness` owns app + per-
  window `FrameState`s + an explicit clock; structure (queries),
  behavior (`take_messages` — iced's `into_messages` idea), pixels
  (`render`, only on demand — Avalonia's no-render fast mode). Verbs
  re-dispatch through the same `dispatch` path the runners use;
  `render_app` is now a thin wrapper over it, so there is exactly one
  input path to trust (the 0.4 golden suite passed unchanged over the
  rewrite — the behavioral proof).
- **Multi-window headless mirrors the runner.** Slots reconcile against
  `App::windows()` after every update; the active window falls back to
  main when its desc disappears; `render_window(key)` renders at that
  window's own clamped size.
- **Golden failures explain themselves** (Flutter's failure-artifact
  contract): `.actual.png` + `.diff.png` (offending pixels red over the
  dimmed golden) + `.side.png` (golden|actual|diff), worst-pixel stats
  in the panic, stale artifacts cleaned on pass. The diff is masked
  (offenders over context), not isolated — one image answers "where".
- **`access_yaml` is verbatim Playwright aria-snapshot grammar** —
  agents already know it; uninteresting containers flatten away.
  `debug_tree` adds rects, flags, and `src=file:line` provenance from
  `#[track_caller]` on the constructors (Flutter's
  `--track-widget-creation`, zero proc macros: 16 bytes per element).
- **Scenarios are externally-tagged serde enums** with
  `deny_unknown_fields` everywhere: a typo'd verb or field is a parse
  error, not a skipped step. Key chords parse from `"cmd+shift+z"`
  strings. Serde stays a plain (native-only) shell dependency — core
  already carried it.
- **Property tests pin the invariants** the wedge rests on: layout and
  paint are total over arbitrary trees x viewports (plan-generated, so
  shrunk counterexamples print), Tab order is a permutation of enabled
  focusables, ids are unique per frame. proptest is dev-only.
- **Heterogeneous children via marker-disambiguated trait** (the axum
  trick): `.children((text(..), button(..)))` — tuple impls and the
  blanket `IntoIterator` impl coexist because they instantiate
  `IntoChildren<Msg, Marker>` at different markers. The papercut was
  found while writing `examples/windows.rs`, which now compiles without
  a single `Element::from`.

## 0.6: text is real

- **Selection on press, platform-style.** Press chains count per input
  (1 = caret, 2 = word via parley's `select_word_at_point`, 3 = line)
  with the same 0.4 s window as double-click messages, tracked
  separately (`last_press`) so `on_double_click` semantics are
  untouched. Shift-click extension existed in the editor but was dead
  code — the dispatch hardcoded `shift: false` because pointer events
  carry no modifiers. The fix is an additive `InputEvent::Modifiers`
  that runners forward on `ModifiersChanged`; `FrameState` remembers,
  pointer placement reads it.
- **Undo/redo is QUndoStack with Elm honesty.** Snapshots (text +
  selected byte range) per editor; typing/deleting coalesce into runs
  keyed by edit kind; caret/selection moves, pointer placement, paste,
  cut, and programmatic value changes are boundaries; redo clears on
  fresh edits; history is bounded at 100. Crucially, undo emits
  `on_input` like any edit — the app stays the source of truth, and a
  programmatic value change becomes its own undoable unit (the first
  fill of a brand-new editor is exempt). Clean-index dirty tracking was
  deliberately skipped: persistence lives in apps, not fields.
- **Rich text is ranged styles over one layout, uncached.** `rich_text`
  + `span` resolve to parley ranged-builder pushes (weight/size/brush/
  family/italic); per-run brushes carry span colors to paint; spans
  concatenate into one accessible label. The layout cache keys on
  (text, style, width) — span lists make poor keys, rich paragraphs are
  short, so rich shaping skips the cache. Inputs stay plain text.
- **Bidi rides parley; coverage rides the font stack.** Mixed-direction
  shaping is total on embedded fonts; RTL glyphs come from system
  fallback, proven by a macOS-gated pixel test mirroring the CJK one.
  App-wide UI mirroring is future work (Qt's lesson when it lands:
  mirror flow, never content).
- **A11y state, honestly scoped.** `.live()` -> AccessKit
  `Live::Polite` (toasts set it themselves); inputs expose their
  selected byte range headlessly on `AccessNode::selection`. The
  per-run inline-text-box protocol stays out of scope and the docs say
  so explicitly.

## 0.7: ecosystem seams

- **Embedded mode is egui's narrow waist, adapted to a compute
  rasterizer.** `Embedded::new(app, theme, &device, target_format)`
  builds the vello renderer on the caller's device; `render` paints
  into an internal premultiplied-alpha Rgba8 texture and composites
  onto any target view via wgpu's `TextureBlitter` with
  `PREMULTIPLIED_ALPHA_BLENDING` (vello output is premultiplied — a
  transparent base color makes the UI a floating layer). egui hands
  back meshes for the caller's pass; vello is compute-first, so the
  texture+blit contract replaces the mesh contract, with
  `texture_view()` as the custom-compositing escape. `EventResponse
  {consumed, repaint}` arbitration: pointer-over-content (hit chain
  deeper than the root) or focused-keystroke. The shell re-exports
  wgpu/winit/vello so integration code can't version-skew. Proven by a
  readback test on a headless device: caller pixels intact outside the
  panel, composite verified, consumption contract asserted.
- **`fenestra-charts` exists to prove the widget-crate path.** It
  depends on fenestra-core alone, follows every rule the new
  widget-crate guide states (theme tokens, semantics+labels, builders,
  no panics on hostile data, golden tests), and joins the publish order
  right after core. Charts are paths-in-viewboxes (sparkline, line) and
  flexbox bars (bar chart) — no plotting engine, deliberately.
- **Theme files serialize the recipe, not the palette.** `ThemeSpec
  {mode, accent_hue?, duotone?}` resolves through the same builders
  apps call; files stay tiny and survive theme-generation changes.
  `deny_unknown_fields` keeps typos loud.
- **Kit v2 stays Elm-pure.** split_pane emits fractions (drag lives on
  the container; interactive content wins hit-testing, so only inert
  areas resize — documented v1 tradeoff); tree_view renders from the
  app's expanded/selected state; command_palette is a modal with an
  autofocused filter input where Enter runs the first match;
  data_table emits sort-column and row-select messages and only draws
  the indicators — sorting itself happens in `update`. The
  QAbstractItemModel-style pull-based model trait was considered and
  deferred: Vec rows + virtual_list cover current scale; revisit when
  a real app outgrows them.
- **Per-window themes**: `App::theme_for(key)` defaulting to `theme()`;
  the runner consults it per window, the harness deliberately does not
  (single explicit theme = deterministic goldens).

## 0.8: trusted, formally

- **cargo-deny joins cargo-audit.** License allowlist (everything in
  the tree resolves to permissive licenses; BSL-1.0 is Boost, not
  BUSL), `wildcards = "deny"`, crates.io-only sources, yanked = deny.
  Unencountered allowances are trimmed so the list documents reality.
- **Fuzzing complements the property tests.** Three libFuzzer targets
  through the *public* API only (no fuzzing feature holes): theme-file
  parsing, arbitrary-driven layout/paint totality, and the text-input
  pipeline (arbitrary text commits + key chords against a focused
  editor, value threaded back Elm-style). Weekly + on-demand via
  workflow_dispatch; the fuzz crate sits outside the workspace
  (nightly-only) and never publishes.
- **MSRV is empirical**: 1.88, the maximum declared rust-version in
  the dependency graph (image) — and what edition-2024 let-chains need
  anyway. Declared in every crate, proven by a dedicated CI job
  building the workspace on exactly that toolchain.
- **Perf gates are ceilings, not benchmarks.** Plain timed tests
  (median of N) with generous absolute limits (~20x the M3 Pro numbers
  in BENCHMARKS.md), `#[ignore]`d locally, run in release mode on the
  macOS CI runner. They catch order-of-magnitude regressions; criterion
  was considered and skipped — its statistics don't survive shared-
  runner variance, and the dependency isn't worth a detector that only
  needs one digit of precision.
- **The coverage floor is measured, not aspired.** fenestra-core's own
  suite covers 47.28% of core lines (kit/shell suites exercise much of
  the rest but don't count toward it); the CI floor sits at 45 and
  ratchets up, never down without a recorded decision.
- **Releases attest provenance.** The release workflow packages every
  crate, generates GitHub build-provenance attestations binding the
  .crate files to repo+workflow+commit, and attaches both to the
  GitHub release (`gh attestation verify <file> --repo ...`). The
  id-token/attestations permissions are scoped to that single job.
- **Private vulnerability reporting is on** (GitHub Security tab);
  SECURITY.md states scope — hostile-input panics ARE in scope
  (scenario JSON, theme files, element trees, font/image bytes) —
  response expectations, and the credit policy.

### Day-one fuzz finding (recorded 2026-06-12)

The layout fuzzer's first three minutes found that hostile text
(combining marks adjacent to a newline; crash input bytes `83 0b ff 48
61 dd 82 32 0a dd 82 32 0a 08 00 97 97 94`) trips a `debug_assert`
inside parley 0.10 (`layout/data.rs:718`, ligature-cluster vs newline
classification). Shipped builds are unaffected (debug assertions
compile out; the code path is safe Rust either way), so the fuzz jobs
now run the shipped configuration (`-O`, assertions off). Worth
reporting upstream to Linebender with the crash input — debug-build
apps showing untrusted text could panic until then.

## 0.9: text grows up, looks arrive

- **Selectable static text rides the editor's machinery.** One
  selection at a time (browser semantics), press chains shared with
  inputs (1/2/3 = caret/word/line), parley `Selection` over the cached
  layout, copy gated behind `key_handled` so focused editors keep
  their own Cmd+C. The highlight paints under glyphs in the input
  selection color; `AccessNode::selection` exposes the range.
- **Markdown is word-level inline emulation where links live.**
  Link-free paragraphs are one wrapped `rich_text` (fast path);
  paragraphs with links split into word pieces in a wrap row so each
  link is its own correctly-hit-tested clickable — the inspector
  caught segment-level wrapping breaking hit-testing. Spaces attach to
  the *following* word (trailing-space advances are trimmed by
  measurement); one accessible button per link run, not per word.
- **Looks bundle theme + typefaces; two font-stack truths surfaced.**
  Registered faces now win for every family role (Sans/Mono were
  hardcoded), and looks ship 400–700 weights because requesting a
  weight a family lacks falls back out of the family entirely — the
  terminal headline rendered Inter until the golden was *looked at*.
  Typefaces vendored under OFL with licenses beside them.
- **Springs are closed-form, not simulated.** The damped step response
  maps elapsed time to progress directly (deterministic under the
  harness clock, no integration state); underdamped motion overshoots
  on geometry while colors/opacity/shadows clamp (extrapolated colors
  aren't colors). Settled = envelope < 0.1%.
- **Enter animations seed, exit stays out.** `.enter` initializes a
  new id's retained animation from the target faded out — no retained
  removal machinery exists, so exit animations are explicitly
  unsupported rather than half-built.
- **Type-ahead is a core primitive** (`on_type_ahead`): dispatch owns
  the focused-element buffer (1s window, Escape clears) and hands the
  whole buffer to the handler. Select implements both idioms: single
  letters cycle past the current entry; growing buffers prefix-match
  inclusively.
- **Emoji resolved (#11)**: COLR/sbix renders through system fallback
  on vello 0.9 (chromatic-pixel proof, macOS-gated); VS16 sequences
  select the text presentation — pinned in-test so a fallback
  improvement surfaces.
- date_picker uses inline civil-date math (Sakamoto) — no chrono;
  tooltips flip above at the bottom edge; the issue tracker is empty
  except the 1.0 RFC.

## 0.10: performance honesty

- **Clean frames are memoized at the runner.** `AppRunner` keeps the
  last painted scene keyed by (logical w, h, scale) and re-presents it
  when nothing changed — expose, un-occlude, and timer redraws skip
  build/layout/paint entirely. A dirty flag is set by input, app
  updates, accessibility focus, hover refresh, resize, scale change,
  and resume; `frame.animating` keeps it set while anything is
  time-driven (carets, springs, spinners, tooltip delays, scrollbar
  fades), so memoization can never starve an animation. Headless paths
  are untouched: tests always rebuild.
- **Subtree scene caching is deferred, deliberately.** Caching below
  the frame level requires knowing a subtree's paint is a pure
  function of its retained inputs (no hover, no animation, no clock
  reads) — that purity is not tracked per-subtree today, and a wrong
  cache shows stale pixels, the exact failure mode a verification-
  first framework cannot ship. It returns only with per-subtree
  resolve-purity tracking and golden coverage to prove it.
- **Variable-height virtualization self-corrects.**
  `virtual_rows_variable` places rows from a prefix-sum height index
  seeded with an estimate; realized rows write their measured heights
  back after layout, so offsets, spacer sizes, and the total height
  converge as the user scrolls. Handlers mirror the realized window.
  The bottom is the true bottom once its neighborhood has been
  measured — pinned by tests that scroll 500 mixed-height rows end to
  end and check every visible neighbor pair for overlap.
- **vello sparse-strips: watch, don't move (assessed 2026-06).**
  The successor rasterizers (`vello_cpu`, `vello_hybrid`, releasing in
  lockstep with vello at 0.0.9) matter to us for two reasons: a CPU
  u8 pipeline is plausibly bit-exact across platforms (today goldens
  are referenced against macOS/Metal and lavapipe needs a 1% budget —
  bit-exact CPU rendering would collapse that to zero and make
  verification truly platform-independent), and `vello_hybrid` avoids
  compute shaders entirely (which would unblock WARP, the reason
  Windows CI is compile-only). COLR emoji already render. Against
  that: the crates self-describe as not production-ready, text lands
  through a different stack, and API churn is constant. Migrate when
  ALL of: a 0.1.0+ release exists, the production-readiness
  disclaimer is removed, Servo's adoption issue (servo/servo#38345)
  closes, Xilem switches its default renderer, and our own spike
  proves bit-exactness on the full golden corpus. Until then we track
  releases and re-run the spike at each minor.
