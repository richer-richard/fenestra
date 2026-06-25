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

## 0.11: the craft release (Tier 1)

Web-grade sophistication is structural, not per-widget effort: scales
with *semantics*, *derivation rules*, and *state machinery* so every
widget inherits craft. 0.11 builds that layer on top of the OKLCH ramps
that have existed since 0.7.

- **Semantic element states + mode-correct pressed states.** The
  neutral ramp's steps 3/4/5 are named `element` / `element_hover` /
  `element_active` (Radix's UI-element-fill model), so kit interaction
  styling is scale arithmetic, not hand-picked colors. Pressed states
  (`accent_active`, `StatusColors::solid_active`) drop one OKLCH
  lightness notch (`ACTIVE_DL = 0.045`) below the step-10 hover — in
  light mode the accent lands exactly on A11's lightness at A10's
  chroma, and because steps 9/10 are mode-invariant the pressed colors
  are too. This replaced a wart where the danger button's pressed fill
  reused `danger.text` (a text role) as a background.
- **Alpha twins.** Each ramp gets a translucent twin (`neutral_alpha`,
  `accent_alpha`): the smallest-alpha color that, composited over `bg`,
  reproduces the solid step (per channel, the minimal alpha keeping the
  back-solved foreground in `[0,1]`; the max across channels wins, so a
  tint both bluer and darker than a near-white bg forces alpha toward
  opaque). The reconstruction is exact at f32, so a property test
  round-trips every twin over `bg` back to its solid step. Twins let
  overlays and state layers read correctly over any surface, not just
  `bg`.
- **APCA-validated legibility — the differentiator.** `apca::lc` is the
  APCA-W3 `0.98G-4g` lightness-contrast (verified against the published
  reference vectors to <0.01). `Theme::validate_contrast` checks every
  text/background role pair against a tiered floor — primary text Lc 75
  (the stock themes reach 90+), secondary/muted 55, control labels 60,
  colored component text 40 — and headless tests assert every built-in
  theme *and* every shipped Look passes. No CSS framework can enforce
  this, because none resolves its colors at construction. Deliberate
  scope: only text pairs are checked. APCA models text legibility, not
  delineation, and on dark themes opaque low-contrast borders score ~0
  Lc; `text_subtle` (a hint color at ~28 Lc in dark) is likewise not a
  body-text role and is excluded.
- **Size/weight-aware APCA + `text_on` (0.21).** Two additions sit on top
  of the fixed role floors without changing them. `apca::required_lc(size_px,
  weight)` turns APCA's readability criterion into a *function*: it returns
  the minimum Lc that text of a given size/weight needs, monotonically
  decreasing in both axes (heavier weight maps to a larger effective px via
  `eff = px·(weight/400)^0.5`), calibrated to the APCA "in a nutshell"
  anchors (14px/400 → ~90, 16px/400 → 75, 24px/400 → 60, 36px → ~45, down to
  a ~15 spot floor) by a small monotone interpolation table, clamped to
  `[15, 108]`. `Theme::contrast_ok(text, bg, size_px, weight)` pairs it with
  `lc_abs` so an app can prove a *specific* label legible at its real
  rendered size, not just against a tier average. `Theme::text_on(bg)`
  generalizes the `on_accent` rule to any custom/status surface: it returns
  whichever ramp extreme (`neutrals.step(1)` paper / `step(12)` ink) wins Lc
  on `bg`, always theme-tinted, never raw white/black (ties break toward the
  ink). The role floors (75/60/55/40) are unchanged regression sentinels;
  `required_lc` now anchors them to the same scale, with the load-bearing
  identity `PRIMARY_TEXT_MIN == required_lc(16px, 400)` asserted literally.
  - **Two framing deviations from the 0.21 blueprint, recorded here at
    implementation time.** (1) The blueprint's literal tie-in — each role
    floor ≥ `required_lc` at the role's *typical render* size/weight — is
    infeasible for CONTROL_LABEL/SECONDARY/COMPONENT, because those floors
    are deliberately *relaxed* below APCA-at-render-size (real button labels
    are 16px/500 needing ~Lc 70, yet `CONTROL_LABEL_MIN = 60`). The tie-in
    test instead asserts `floor ≥ required_lc(rep)` where `rep` is the
    documented APCA size/weight *tier the floor encodes* (CONTROL_LABEL ←
    25px/400 ≈ 58.8, SECONDARY ← 31px/400 ≈ 51.3, COMPONENT ← 50px/400 ≈
    39.2) — a real point on the curve, making the two systems share one
    scale, honest that `rep` is the floor's tier, not the role's smallest
    render size. Distorting `required_lc`'s weight response to make 16px/500
    → 60 was rejected: it would give APCA-wrong guidance to apps. (2) The
    blueprint's `text_on` acceptance asserts `lc_abs(text_on(bg), bg) ≥ 60`
    (control-label grade) on every tested surface; the real worst case is
    ~59.3 Lc on a few dark status/accent solids, because `text_on` returns
    the theme-*tinted* paper (`step(1)`/`step(12)`) by design while the
    pure-white `on_accent` clears 60 — a ~0.7 Lc tint cost. The test asserts
    the honest, role-tied guarantee `text_on` always meets — secondary-text
    grade (`≥ SECONDARY_TEXT_MIN`, 55, with margin) — rather than weakening
    the tinted-color invariant to chase the last Lc.
- **Layered, hued elevation.** Shadows carry the surface hue at low
  chroma (`Theme::shadow_tint`, a near-black derived from `bg`'s OKLCH
  hue) instead of flat `#000` — subtle on neutral themes, visible on the
  tinted ones (the editorial field casts green-black). A new
  `ShadowToken::Xl` is a three-layer contact+ambient ramp for modals.
  Solid buttons get a 1px inset top highlight (`Style::highlight_top`,
  white at low alpha, clipped to the corner radius) — the premium-flat
  top sheen Linear/Vercel use; the skeuomorphic glossy version is not
  in fenestra's flat-modern language and was not adopted. Dark-mode
  elevation continues to lighten surfaces (shadows read poorly on dark).
- **Typography from a formula.** Letter spacing follows Inter's
  published dynamic-metrics tracking curve
  `-0.0223 + 0.185·e^(-0.1745·px)`, applied at the actual size (so
  free-form display sizes track correctly too) instead of three
  hand-set steps. Tabular figures (`tnum`) are a one-call
  `Style::tabular` / `Element::tabular`, applied to numeric kit widgets
  (stat cards, tables, chart labels) so digits align in columns.
  - **Line height stays the per-size scale, deliberately.** The
    research prescribed a line-height *curve* too, but the existing
    hand-tuned scale already implements smaller-looser / larger-tighter
    more aggressively than a naive linear (12→48px ⇒ 1.5→1.0) fit, and
    `Base = 1.5 × 16px = 24px` is the line box variable-height
    virtualization is pinned to. A formula would loosen mid-size
    headings and disturb that invariant for no gain, so it was not
    adopted.

## 0.12: the interaction release (Tier 2)

Three interaction systems on top of the 0.11 tokens — a uniform state layer,
Material 3 motion tokens, and a shadcn-grade focus ring. The headline: kit
interaction is now *one recipe* instead of per-widget color swaps.

- **State layer — the engine.** `Element::state_layer(|theme| content_color)`
  declares the color drawn *on* a control; `frame.rs::resolve` composites a
  translucent veil of it over the container — hover 8%, keyboard focus / press
  12%, drag 16% (`STATE_LAYER`) — taking the strongest active state. The veil
  is baked into the fill via exact source-over (`anim::over`), so it animates
  through the existing color transition with no new paint primitive. The
  widened `events.rs` hover predicate tracks any `state_layer` id.
  - **Neutral surfaces use the layer; solid brand fills keep their ramp
    steps.** Ghost/Secondary buttons and menu/select/tree/date/table/toast rows
    route through the layer (content = `text`). Primary/Danger buttons keep
    `accent_hover`/`accent_active` and `solid_hover`/`solid_active` — Material's
    white content-veil would lighten and desaturate the gamut-mapped accent,
    undoing the 0.11 ramp craft. This is Radix's split (solids step the scale;
    everything else uses an alpha layer) and is a deliberate deviation from
    "veil everything." DECISION.
  - **Disabled.** The engine fades a disabled control's container toward the
    resting surface and drops its border/shadow/highlight; content (the
    label/icon) is a *separate child* the container can't recolor, so widgets
    dim it to the `text_disabled` token — fenestra's equivalent of Material's
    38%-content figure. Solid buttons (no state layer) keep the simpler subtree
    `opacity(0.5)`. DECISION (tree model: a container cannot reach into a
    child's text color, so content dimming lives at the widget).
  - **Snap/fade preserved.** The veil materializes only when a state is active,
    so controls that faded before still fade and rows that snapped before still
    snap — no behavior change and no phantom resting fills in debug dumps.

- **Press-scale + motion tokens.** `Style::scale` (default 1.0, always
  interpolated in `lerp_style`) plus `Element::press_scale` dip a pressed
  control to `PRESS_SCALE` (0.97). It is a *paint-time* transform: `paint_node`
  renders the control into a child `Scene` and appends it with
  `Affine::scale_about(center)`, so layout and hit-testing are untouched and
  springs may overshoot for a tactile bounce. Motion families fill out to M3:
  `EASE_DECELERATE` (entrances), `EASE_ACCELERATE` / `EASE_EXIT` (exits),
  `MotionDuration::Micro` (100 ms) and `exit_ms` (×0.75).
  - **Keyboard-driven changes snap.** `resolve` skips the transition for the
    keyboard-focused element (`focus_visible && focused() == id`), so tabbing
    shows the ring and state layer instantly rather than lagging behind a fast
    keyboard user. Pointer hover/press still animate. DECISION.

- **Focus ring — shadcn v4.** `FocusRing` is now a 3px halo at 0.5 alpha flush
  outside the border (was a 2px ring offset 2px). On keyboard focus `resolve`
  swaps the control's border to the ring color and `painter::focus_ring` paints
  the halo; `Element::invalid` recolors both to the danger hue (threaded
  `NodeMeta::invalid` → `Frame::ring_color_invalid`). The swap is keyboard-gated
  (`focus_visible`) to match the ring; pointer focus on inputs keeps their own
  accent-border affordance.

- **Control sizes.** `ControlSize` spans a shared height grid — `Xs` 24 / `Sm`
  32 / `Md` 36 / `Lg` 40 — resolving to a `ControlMetrics { height, pad_x, gap,
  font, icon }` bundle so a button, input, and select on the same row align.
  Default `Md` is unchanged (36 px) so default-sized goldens did not move; `Sm`
  (28→32) and `Lg` (44→40) shifted onto the grid.

## 0.13: derivation as product (Tier 3)

The palette becomes a function of three numbers, and two new Looks prove the
generator's range.

- **`Theme::derive(base, accent, contrast, mode)`.** Linear collapsed ~98
  variables into three; on fenestra's OKLCH scales that is almost free.
  `BaseField { hue, chroma }` is the neutral field (chroma is a multiplier on
  the table's base chroma: 1 = stock SaaS tint, 4–10 = duotone, 0 = gray),
  `accent_hue` the brand, and `Contrast { Low, Standard, High }` a separation
  level. `from_accent` and `duotone` are now special cases of `derive` — at
  `Standard` contrast it reproduces them byte-for-byte (verified by snapshot
  dumps), so the generalization carries zero regression. The accent ramp,
  status colors, and shadows are untouched; only the neutral field is rebuilt,
  through a shared `apply_neutral_field` that `duotone` also uses.
  - **Contrast = distance from the background, not from mid-gray.** Each
    neutral step's lightness is remapped `L' = L_bg + (L − L_bg) · k`, with
    `k` 0.92 / 1.0 / 1.10. Scaling against the fixed page color (step 1) keeps
    `bg` stable and widens or softens everything else against it, which is what
    "contrast" means perceptually; scaling around 0.5 would drift the
    background itself. Every level still clears the APCA floors (asserted), so
    `derive` cannot ship an illegible theme. DECISION.
  - **`ThemeSpec` gains a `derive` recipe** (precedence derive > duotone >
    accent_hue), so the three-input model round-trips through theme files.

- **Radius knob — a standalone family, not a theme field.** `RadiusScale::
  from_base(b)` derives `{sm, md, lg, xl}` at `0.6 / 1.0 / 1.4 / 2.0 × b`; the
  default base (`R_MD` = 10) reproduces `R_SM`…`R_XL` exactly. It is deliberately
  *not* a `Theme.radius` field the kit reads: kit widgets set radii outside
  their `themed` closures (`.rounded(R_MD)`), with no theme in scope, so a
  per-theme radius would mean threading it through ~90 call sites for little
  gain. The derivation primitive is the deliverable; apps and Looks opt in.
  DECISION.

- **Two new Looks (proof-of-range).** `warm_editorial` is `derive` with a warm
  paper field (hue 80, low chroma) + a terracotta accent (hue 40) at `High`
  contrast, with Playfair carrying `Serif`/`Display` for serif prose under sans
  chrome — the Claude-like voice, generated rather than hand-placed. `playful`
  is a cool pastel field (hue 280, low chroma) + a saturated magenta accent
  (hue 330), the whiteboard/FigJam color character. Both are golden-locked and
  pass the APCA gate in both modes; `all()` now returns five Looks.
  - **The playful Look's hand-drawn typeface is deferred.** A FigJam voice
    wants a hand-drawn face (Excalifont, OFL); vendoring a new font binary
    (with its license, and the history-bloat risk) is its own change, so the
    Look ships its palette now on the base sans, with the face noted as a
    follow-up. The palette — saturated accent over pastel fills — is the
    substance and is fully delivered. DEVIATION.

## 0.14: kit and showcase (Tier 4)

The token system reaches dense app chrome, the charts gain a real palette, and
two showcases prove the range is real, not aspirational.

- **Editor-chrome tier (`chrome.rs`).** `ChromeText` (11/12/13/14px, Figma's
  per-size tracking, 16/24px line boxes) is a SEPARATE register from the product
  `TextSize` scale (which starts at 12px reading text): dense chrome is a
  different context, so it gets its own tokens rather than stretching the
  product scale downward. `ChromeElevation` (Popover/Modal/Thumb) encodes
  Figma's flat, layered panel shadows — two soft black drops over a 0.5px
  hairline ring (a zero-blur *spread* shadow, which the painter renders as a
  crisp sub-pixel edge) — deliberately flat and mode-independent, unlike the
  hue-tinted, themed `ShadowToken` used for product surfaces. The 32px control
  row is `ControlSize::Sm` (no new token). DECISION: chrome is its own tier, not
  an extension of the product tokens.
- **Canvas substrate (`canvas.rs`).** Pure geometry — no rendering, no state —
  so it composes with the element tree and runs headless: tldraw's `ZOOMS`, the
  step logic, a `Camera` with eased zoom (`EASE_IN_OUT_CUBIC`, the CSS-bezier
  approximation of tldraw's piecewise `easeInOutCubic` — noted in the source),
  world↔screen transforms, the 8px snap grid, and the `world_len`/`screen_len`
  zoom-compensation that keeps selection chrome a constant size on screen.
  Placed in core (it is geometry, usable headless) though app-facing.
- **`oklch` / `oklch_of` made public.** The framework's gamut-safe color
  constructor and its inverse — used by the theme ramps, Looks, and now the
  chart palette — are the principled escape hatch for data-viz and custom
  palettes, which legitimately need colors beyond the named theme tokens.
- **Chart palette (`charts`).** Observable10 light verbatim; the dark variant is
  re-picked generatively (each swatch lifted +0.08 L and eased ×0.82 C in
  OKLCH), never inverted — the recognized data-viz exception to "color only
  through theme tokens," kept principled and mode-aware. The sequential and
  diverging generators follow the standard OKLCH recipes (linear lightness ramp;
  two arms through a light neutral midpoint). The charts crate still depends
  ONLY on fenestra-core's public API — the reference widget-crate constraint.
- **Showcases.** `editor_panel` (a Figma inspector, golden-locked light+dark)
  and `ai_chat` (the Claude-look AI reading view: a `ch`-based reading column
  (see 0.15), bubble/flat turn asymmetry, a streaming caret, a thinking shimmer
  — golden under the warm-editorial theme with a Playfair serif).
- **Deferred:** the playful Look's hand-drawn typeface (Tier 3) is still a
  font-vendoring follow-up; the canvas substrate is math only (no canvas
  *widget* ships yet) — both are noted rather than silently scoped out.

## 0.15: the reading measure (ch-based prose column)

A `ch`-based reading measure caps prose near the optimal line length (~66
characters) independent of window width — the web-canonical `max-width` for an
article, expressed in the unit that actually matters for legibility.

- **`Length::Ch(f32)` + `Style::measure(chars)` (and `w_ch`/`min_w_ch`/
  `max_w_ch`).** 1ch is the advance of the digit `'0'` in the element's resolved
  text style (CSS `ch` semantics, letter-spacing ignored). `measure` is a
  `ch`-based `max-width`; `MEASURE_CH = 52.0` is the default. `reading_column()`
  (kit) is `col().measure(MEASURE_CH)`.
- **CALIBRATION (`ch` ≠ characters): `MEASURE_CH = 52`, not 66.** `'0'` is wider
  than the average glyph, so a column of N `ch` holds noticeably *more* than N
  real characters. At `66ch` the embedded Inter renders ~83 characters per line
  (verified in the golden) — above the comfortable 45–75 band the feature
  targets. `52ch` lands the rendered body line near the ~66-character optimum
  while keeping `Length::Ch` faithful to the CSS `'0'`-advance definition.
  (Tailwind's `prose` uses 65ch and reads wide; we tune the default to the
  *rendered* character count instead.) Found in review and recalibrated before
  release.
- **Resolution timing — the crux.** taffy has no font context, so `Ch` cannot
  reach it. It is resolved to `Px` in `frame::build`, right after `resolve()`
  returns the concrete (animation-applied) `Style` and *before* `to_taffy`: if
  `style.has_ch()`, `Fonts::ch_width(&resolve_text(&style.text, theme))` gives
  the `'0'` advance and `Style::resolve_ch` rewrites every `Ch` length to `Px`.
  Because this mutates the stored `BuiltNode.style`, every later `to_taffy`
  (root override, overlay layout) sees only `Px`. The `'0'` shaping is paid only
  by ch-using elements (guarded by `has_ch`, protecting `perf_gate`). To thread
  metrics in, `build` gained a `&mut Fonts` parameter. DECISION: resolve `Ch`
  during build, not in `to_taffy` (no font context there) and not lazily in the
  measure closure (the cap is a container property, not a leaf measurement).
- **DEVIATION (semantics): `Ch` resolves against the element's OWN resolved
  text style**, not the prose nested inside a container (fenestra has no style
  inheritance — each leaf calls `resolve_text` on its own `style.text`). So
  `measure` on a container needs `.size(..)` **and `.family(..)`** set to match
  its prose; documented on `Style::measure`. `ai_chat` therefore sets
  `.size(TextSize::Lg).family(FamilyRole::Serif)` on the column (20px Playfair
  prose — the family was added in review so the measure tracks the actual serif
  `'0'`, not Inter's) and markdown leaves the body default (16px sans).
- **DEVIATION (per-block vs single cap): measure is one column cap, not a
  per-block `ch` width.** Per-paragraph/per-heading caps give ragged measures
  (headings wrap wider than body); the web-canonical single `max-width` on the
  article container — resolved at body size — yields one consistent reading
  column that still caps paragraphs, list items, and headings (all inside it).
  markdown applies it to its outer `col`; narrower containers (the 460px doc in
  the existing golden) don't bind, so that golden is unchanged.
- **Measured metric.** The embedded Inter `'0'` advance is ~10.09px at 16px
  (~12.6px at 20px). With `MEASURE_CH = 52`, the body reading column is ~525px
  and the `ai_chat` guard column (Serif → Inter fallback under embedded fonts)
  is ~655px. Test bounds and the `ai_chat` width guard are pinned to the real
  macOS/Metal metric; the `markdown_measure` and `ai_chat` goldens are
  regenerated and eyeballed.
- **Known limitation (follow-up): the document-level measure also caps fenced
  code blocks.** Mono code inside the prose column wraps at the reading measure
  rather than extending; the existing markdown golden's 460px container is below
  the cap so nothing ships visibly broken. A later pass can let code blocks
  scroll horizontally (Tailwind `prose` keeps `<pre>` in the column with
  overflow-x) instead of wrapping.
- **API decisions (reviewed, kept):** `measure(chars)` and `max_w_ch(chars)`
  intentionally coexist — `measure` is the intent-revealing prose name,
  `max_w_ch` the mechanical setter symmetric with `w_ch`/`min_w_ch` (same
  pattern as `w_full`/`rounded_full`). `Length` is left non-`#[non_exhaustive]`,
  consistent with every other fenestra enum (`TextSize`, `ShadowToken`, …): the
  workspace is the only consumer pre-1.0 and snapshots lock the surface.
- **Animation.** `Ch` is resolved after `resolve()` (which applies animation),
  so the animator never sees it; `lerp_length` snaps any `Ch` to its target — a
  changed measure snaps rather than tweening, which is correct (measures are
  static caps, not animated values). `layout::dimension` treats a leaked `Ch` as
  `Auto` defensively — unreachable in the normal pipeline.

## 0.16: richer font features

Typed `FontFeatures` (figure shape + figure spacing axes, plus `small_caps`,
`ligatures`, and `fractions` toggles) replaces the single `tabular_nums` bool.
The web-canonical `font-feature-settings`, expressed as autocompleting builders
instead of a CSS string.

- **One value type, one source of truth.** `FontFeatures::feature_string()`
  emits the CSS `font-feature-settings` list in a fixed tag order (figures,
  spacing, small caps, ligatures, fractions) and is the only place tags are
  produced. All three former feature-push sites (`text.rs` plain + rich shaping
  and `input.rs` editor styling) now call it through one shared path
  (`parley::FontFeatures::Source(Cow::Owned(s))` — owned so it satisfies the
  `'static` `PlainEditor::edit_styles()` slot without lifetime gymnastics), and
  our public `FontFeatures` is dropped from the two `use parley::{…}` globs to
  avoid the name clash.
- **Orthogonal axes.** Figure shape (`onum`/`lnum`) and figure spacing
  (`pnum`/`tnum`) are independent enums, so tabular + old-style is expressible;
  `.tabular()` is unchanged (`spacing = Tabular` ⇒ `"tnum" 1`) so every prior
  golden is byte-identical. `.tabular()`/`.proportional_nums()` share the
  spacing slot and `.oldstyle_nums()`/`.lining_nums()` share the figure slot
  (last builder wins).
- **Cache key (the bug the regression locks).** Every flag is now hashed into
  `LayoutKey`; the prior key carried only `tabular_nums`, so any new feature
  would have been cached away (flip a flag, hit the stale layout). Per-axis
  `LayoutKey` regression tests were written first and watched fail (keys equal
  across a flag flip) before `features` was added to the key.
- **DEVIATION (font-dependent golden split).** Feature support is a property of
  the face, not the framework. The embedded Inter (the headless golden font)
  carries `tnum/pnum/frac` but **not** `onum/lnum/smcp/liga`; the bundled
  Playfair Display carries `onum/lnum/smcp/liga/frac` but **not** `tnum/pnum`.
  So the `font_features` golden demonstrates figure shape, small caps, and
  fractions on the **Serif** role (Playfair) and tabular↔proportional on
  **Sans** (Inter), and lives in the `fenestra` crate (which registers
  Playfair, the pattern proven by `ai_chat_golden`) rather than the
  embedded-only kit suite. The unit acceptance criteria (feature-string
  contents, `LayoutKey` distinctness) are font-independent and fully covered.
- **Eyeballed.** The light-theme golden shows old-style figures descending
  below the baseline (3/4/5/7/9) and rising above x-height (6/8) against the
  flat lining row, lowercase becoming small capitals, `1/2 3/4 7/8` collapsing
  to single fraction glyphs, and the tabular digit column aligning on a fixed
  grid where the proportional one is ragged.
- **API decisions (reviewed, kept).** `.tabular()` keeps its bare name (it is
  the established, widely-used builder) while its spacing sibling is
  `.proportional_nums()` — a small naming asymmetry accepted in exchange for not
  churning every `.tabular()` call site. `FontFeatures` is left
  non-`#[non_exhaustive]`, consistent with every other open public-field struct
  in the crate (`TextStyle`, `Style`, `Border`, `Shadow`); downstream code uses
  the builders, not struct literals, so growth lands through new builders.
- **Known limitation (follow-up): live editors don't clear a removed feature.**
  `input.rs::apply_style` inserts the `FontFeatures` style property only when
  `feature_string()` is `Some`; toggling a feature *off* on a persistent
  `text_input` instance leaves the prior property in the editor's style set
  until it is recreated (pre-existing in form — the old `tnum`-only path gated
  the same way; this change widens it from one feature to six). Static text is
  unaffected (it rebuilds styles per layout). A later pass should `remove` the
  property in the `else` branch, with a live-editor regression test.

## 0.17: balanced and pretty text wrapping

`TextWrap::{Normal, Balance, Pretty}` (CSS `text-wrap: balance / pretty`),
exposed as `.balance()` / `.pretty()` / `.text_wrap(TextWrap)` on `Style` and
`Element`. Greedy `Normal` stays the default and costs nothing. Markdown
headings (the no-links fast path) opt into `.balance()` automatically.

- **Refinement re-breaks, never re-shapes.** parley's `Layout::break_all_lines`
  is re-runnable on one already-built layout (`BreakLines::new` clears prior
  line data — verified in parley 0.10's `line_break.rs`), so balance is
  `O(log W)` re-break passes (binary search for the smallest grid width still
  yielding the greedy line count `n`) and pretty a bounded downward grid scan
  (largest width that keeps `n` lines and un-orphans the last line) — both with
  **zero glyph re-shaping**. `TextWrap::Normal` (≈ all text) skips refinement
  entirely, so `perf_gate` (all-Normal leaves) is untouched — confirmed by
  running it `--ignored` green.
- **Measure/paint break reproduction via `layout_max_advance`.** taffy measures
  a text leaf at the column width `W`; paint re-wraps at the leaf's final box
  width — with balance these differ. The fix: a refined leaf reports its *wrap
  width* `w*` (`= layout_max_advance().ceil()`, the width the last re-break
  used), not its longest-line width, as the measured box width (`box_width()`).
  The paint-time box is then always `>= w*`, so re-deriving the refinement at
  the box width reproduces the identical break. `TextWrap::Normal` has
  `layout_max_advance() == W`, so `box_width` returns `width().ceil()` exactly
  as before (plain-text goldens byte-identical). Pinned by
  `balance_idempotent_reproduces_break` (the fixpoint).
- **Cache key (regression-locked).** `wrap` joins `LayoutKey`; measure (at `W`)
  and paint (at `w*`) land in two quarter-px buckets that each compute the same
  break. `WRAP_GRID = 0.25` is deliberately equal to the cache's
  `width_bucket` quantum so the searched width and the cache bucket quantize on
  the same grid. `layout_key_differs_on_wrap` was written first and watched
  fail; the two load-bearing behavior tests (`balance_evens_a_two_line_heading`,
  `pretty_pulls_word_onto_last_line`) were also red before the refinement
  landed (balance left the longest line unchanged; pretty left the orphan).
- **Balance scope.** Auto-width leaves in `items_start`/center/end containers
  take the balanced (narrower) box; width-pinned and stretch leaves keep their
  width and balance within it. Markdown applies `.balance()` only to the
  no-links fast-path heading; a heading-with-inline-link falls back to Normal
  (that path is a flex wrap-row of per-word pieces, not one parley layout).
- **DEVIATION — rich (markdown) headings are not cached.** Plain balanced text
  caches via the wrap-keyed `LayoutKey`; markdown headings render through
  `rich_text` ⇒ `shape_rich`, which is uncached (pre-existing decision: span
  lists are poor hash keys). Balance re-shapes them per frame, but they are
  short and the cost is cheap re-breaks. (Future: a rich-layout cache makes this
  free; out of scope.)
- **DEVIATION — `TextWrap::Pretty` is best-effort.** When no narrower width removes
  the orphan without adding a line, the greedy break is kept unchanged.
  Guaranteed by `pretty_never_worse_when_no_orphan`: pretty never increases the
  line count and never reduces the last-line word count. The downward scan is
  the clearest "never worse" formulation (stops at the first/largest qualifying
  width, or gives up the instant narrowing would add a line); for very wide
  paragraphs a binary search keyed on the orphan predicate would be cheaper, but
  pretty is for headings/short paragraphs and is opt-in (markdown body text
  stays greedy).
- **DEVIATION — example surfaces as a dedicated golden, not a kit panel.** The
  blueprint suggested a `specimen` panel; instead the flagship eyeball artifact
  is a self-contained `fenestra/tests/text_wrap_golden.rs` that stacks each
  refinement directly under its greedy twin (ragged vs even heading; orphan vs
  pulled-down paragraph), so the comparison is in one PNG and no kit golden
  churns. Width is derived (heading at the panel width; paragraph capped at a
  300px column where the macOS/Metal Inter metrics strand the last word).
- **Eyeballed.** The `text_wrap` golden shows the heading going from
  `[full, full, "and tidy"]` to three visually even lines (N=3 preserved), and
  the paragraph's stranded `"anywhere."` pulled up to `"paragraph anywhere."`
  (N=4 preserved). The `markdown`, `markdown_measure`, `poster`, `ai_chat`, and
  `font_features` goldens stay byte-identical (their headings are single-line at
  their widths ⇒ balance no-op; nothing else opts in).
- **API naming (reviewed): the enum is `TextWrap`, not `Wrap`.** It matches the
  text-style group's convention (`TextAlign`, `TextStyle`, `TextSize`) and
  disambiguates from the pre-existing flexbox `.wrap()` / `Style::wrap` — a bare
  `Wrap` next to `TextAlign` in the facade glob would read as flex-wrap. Renamed
  pre-release (the builders `.balance()`/`.pretty()`/`.text_wrap(TextWrap)` are
  unaffected). A `pretty_idempotent_reproduces_break` test mirrors the balance
  fixpoint so pretty's measure/paint agreement is directly verified too.

## 0.18: themed OKLCH gradient builder

`oklch_stops(anchors, steps)` plus the `linear_gradient` / `radial_gradient`
free fns and `Theme::accent_gradient` — token-sourced gradients whose stops are
pre-expanded so the rendered ramp tracks the OKLCH curve instead of sRGB's
straight chord. `GRADIENT_STEPS = 16` is the default sub-segment count.

- **vello ignores `interpolation_cs`.** Confirmed against `vello-0.9.0`: it
  builds its gradient ramp LUT in sRGB and contains no `interpolation_cs` /
  `ColorSpaceTag::Oklch` handling (`peniko-0.6.1` exposes the field, but vello
  never reads it). Tagging the `peniko::Gradient` color space would be a no-op,
  so fenestra **pre-expands** OKLCH stops in core: each anchor pair is walked in
  `steps` sub-segments via `crate::anim::lerp_color` (the transition engine's
  exact OKLCH lerp — shortest hue arc, powerless-hue endpoints, gamut clamp),
  and vello's piecewise-linear sRGB interpolation between the dense stops tracks
  the OKLCH path. Revisit if a future vello honors `interpolation_cs` — a
  2-stop + Oklch-cs path could then replace the dense stops with **no public
  API change** (the builders' signatures are renderer-agnostic).
- **Pre-expansion over a new `Paint::OklchGradient` variant.** Chosen for
  renderer-independence (goldens are byte-identical no matter whether the
  renderer ever honors OKLCH), zero `painter.rs` / IR churn, and testability:
  the chroma-floor acceptance test asserts on *emitted* intermediate stops,
  which only exist under pre-expansion. `Paint` / `GradientStop` stay
  `Copy`-friendly `Clone` and unchanged.
- **`GRADIENT_STEPS = 16` is a calibrated default, not a hard spec number.**
  Each sub-segment maps to ≈32 texels of vello's ~512-texel ramp LUT; the
  residual sRGB-chord deviation from the OKLCH curve over ~32 texels is below a
  perceptible step even across a ~180° hue arc. Verified by eye on the cross-hue
  golden (no banding at sub-segment joints at 1× or 2×); raise it only if a
  wide-hue ramp ever bands.
- **Shared lerp with the transition engine.** `lerp_color` is the single OKLCH
  path behind both animated color changes and gradient stop generation, so an
  animated fill and a pre-expanded gradient between the same two colors trace
  the identical perceptual curve. Endpoints are exact (`lerp_color` returns `a`
  at `t≤0`, `b` at `t≥1`), so pre-expansion never shifts the anchor colors —
  locked by `endpoints_are_exact`.
- **Acceptance: no gray dead-zone.** `midpoint_keeps_chroma_no_gray_deadzone`
  asserts the offset-0.5 stop of an accent→warning ramp keeps OKLCH chroma
  >1.5× the naive sRGB average's (a regression floor; the real ratio is far
  higher). `lightness_is_monotonic_across_stops` guards against a mid-ramp dark
  bump; `offsets_sorted_and_span_anchors` covers sort + span + count;
  `degenerate_inputs` pins empty / single-anchor / `steps == 0`.
- **DEVIATION — eyeball artifact is a dedicated golden, not specimen-only**
  (mirrors the 0.17 `text_wrap_golden` decision). `fenestra-kit/tests/`
  `oklch_gradient_golden.rs` stacks a naive two-stop sRGB cross-hue gradient
  directly above the OKLCH-built one over the same accent→warning anchors, so
  the dead-zone elimination is unmistakable in one PNG (light + dark). The
  specimen's own gradients are same-hue (accent ramp), so they change little —
  the cross-hue A/B panel is where the win is visible.
- **Converted sites stay token-sourced; goldens regenerated intentionally.**
  The specimen's accent linear gradient now comes from `accent_gradient(135.0)`
  and its radial from `radial_gradient` (A4→A9); the poster's paper-grain field
  keeps its explicit 0.0/0.55/1.0 neutral offsets via `oklch_stops` directly.
  `specimen_light/dark` and `poster` regenerated (same-hue / near-gray, so the
  pixel delta is tiny but non-zero); `text_wrap` and all other goldens stayed
  byte-identical.
- **API decisions (reviewed).** The stop-expander is named `oklch_stops` (not
  `oklch_gradient`) so the `*_gradient` family is type-consistent: `*_gradient`
  fns return a `Paint`, `oklch_stops` returns `Vec<GradientStop>` — the name
  tells you the return type and rules out a `.bg(oklch_stops(...))` mistake.
  `linear_gradient`/`radial_gradient` guard degenerate input: fewer than two
  colors collapse to a solid fill (the lone color, or transparent for none), so
  the painter never receives a zero-stop gradient.

## 0.19: surface / material bundle

`Surface` is one typed primitive per elevation *material* (Geist/Apple
"materials"): a semantic role (`Card`, `Raised`, `Popover`, `Menu`, `Modal`,
`Thumb`, `Tooltip`) that bundles a corner radius, a fill role, a border role, a
shadow token, and an optional top-highlight alpha into a `SurfaceBundle`,
resolved against a `Theme` into a `Style` overlay. Two entry points:
`Theme::surface_style(role)` (theme in scope) and `Element::surface(role)`
(deferred via `.themed`, for `view()` with no theme); the low-level
`SurfaceBundle::apply(theme, base)` overlays a material onto an existing style.
Seven kit widgets now derive their elevated look from this one table —
card, menu/popover, select listbox, modal, tooltip, toast row, slider thumb —
instead of re-typing `.rounded(..).shadow(..).themed(|t,s| s.bg(..).border(..))`
at each call site. Pure style composition at frame time; no new paint
primitive, no vello/parley/taffy involvement.

- **DECISION — standalone role enum + resolver, not a `Theme` field.** Mirrors
  the 0.13 radius-knob decision: kit widgets carry no theme at build time, so
  the material defers through `.themed`. The bundle is defined in roles
  (`SurfaceFill` / `SurfaceBorder`), resolved against `&Theme`, so it tracks
  `derive()` / `duotone()` and every Look automatically. `Surface::bundle()` is
  pure and `const` (no theme), which makes the radius / shadow / role *ordering*
  unit-testable without rendering; `SurfaceBundle::apply(theme, base)` does the
  color resolution. `ShadowToken` gained `PartialOrd, Ord` (additive; variants
  already declared `Xs < Sm < Md < Lg < Xl` in ascending depth) so the ordering
  invariant can compare shadow depth via `Option<ShadowToken>: Ord`.
- **Acceptance invariant — floating ≥ resting.** For every floating role
  (`Popover`/`Menu`/`Modal`) and every resting role (`Card`/`Raised`):
  `floating.radius.outer() >= resting.radius.outer()` **and**
  `floating.shadow >= resting.shadow`. "Every floating thing matches the card"
  is therefore structural, not a convention. Locked by
  `ordering_invariant_floating_ge_resting`.
- **DEVIATION — floating radius bumped to satisfy the invariant.** Menu /
  popover / select-listbox (was `R_MD` 10) and toast (was `R_SM+2` = 8) rise to
  `R_LG` (14) because the card's resting radius is 14 and the invariant requires
  floating ≥ resting. The inner menu/select item radius bumps `R_MD-4` (6) →
  `R_LG-4` (10) to stay concentric inside the new 14px panel with 4px padding.
  This is the intended "every floating thing matches the card" change; only
  `select_open` and `toast_stack` goldens move (regenerated, eyeballed light +
  dark). Card / Modal / Thumb / Tooltip bundles reproduce the exact prior
  `(radius, fill, border, shadow)` tuples, so those goldens — and the five Look
  goldens — are byte-identical by construction.
- **DEVIATION — `Thumb` and `Tooltip` are exempt from the ordering invariant.**
  `Thumb` is a pill control handle (`R_FULL`, `Default` border, `Sm` shadow);
  `Tooltip` is an *inverted* chip (`SurfaceFill::Inverted` = `neutrals.step(12)`,
  `R_SM`, no border, `Md` shadow). Both are genuine materials worth centralizing
  in the bundle, but neither belongs to the resting/floating elevation ladder,
  so the invariant test iterates only `{Card, Raised, Popover, Menu, Modal}`.
  Tooltip keeps its inverted fill via the dedicated `SurfaceFill::Inverted` role
  rather than being misclassified as a neutral elevated surface.
- **`SurfaceRadius` is `#[non_exhaustive]` with one `Uniform(f32)` variant** so
  0.20's concentric/squircle rule can add a `Concentric` variant (outer + inset
  child radius) with no API break; `SurfaceRadius::outer()` already names the
  outer radius. The `highlight: Option<f32>` field is the documented home for a
  future per-role / per-Look top sheen (white, like the button's 0.14
  `on_accent`); **all shipped roles set `None`** this phase to keep neutral
  surfaces identical to today, and the highlight resolves through
  `oklch(1,0,0).with_alpha(a)` — no raw literal. Verified by
  `highlight_resolves_to_low_alpha_white`.
- **API decisions (reviewed).** `Surface` and `SurfaceFill` are also
  `#[non_exhaustive]`: 0.22's translucent/glass material will add a `SurfaceFill`
  variant (and likely a `Surface` role), and forward-marking the growable axes
  now keeps that a non-breaking add rather than a downstream `match` break. The
  redundant `Style::surface(&Theme, role)` was dropped — it was the only
  theme-coupled method in the otherwise theme-free `Style` builder vocabulary
  (theming defers through `.themed`); `Theme::surface_style` and
  `SurfaceBundle::apply` already cover the theme-in-scope and low-level paths.
- **Known follow-up (0.20).** The inner menu/select item radius is still a
  hand-typed `R_LG - 4.0` rather than derived from the bundle, so a future
  change to the `Menu` role radius would desync it — exactly the drift the
  concentric-radius rule (next phase, the documented home for this) eliminates.
- **Scope note.** `palette.rs` (command palette) and `date_picker.rs` also
  hand-roll floating surfaces but were left out of this phase's conversion set
  to bound golden churn; converting them to `Surface::Menu`/`Popover` is a clean
  follow-up.

## 0.20: concentric radii + continuous-curvature (squircle) corners

Two independent, low-risk additions, both defaulting to a true no-op so every
prior golden stays byte-identical.

- **Concentric radii.** `SurfaceRadius` grows an `inner(inset) -> max(0, outer -
  inset)` accessor (`outer` stays `const`; `inner` is not — `f32::max` is not
  const-stable). The menu and select item radii now derive from
  `Surface::Menu.bundle().radius.inner(SP1)` instead of the hand-typed
  `R_LG - 4.0` flagged as a 0.19 follow-up, and both panels pad by the same `SP1`
  — one token for both the pad and the radius, so the concentric pair has a
  single source of truth. The derived value is `14 - 4 = 10` (`R_MD`), identical
  to the old literal, so zero pixels move; the win is that the item radius can no
  longer desync from the panel radius.
- **Squircle corners.** `Style::corner_smoothing: f32` (default `0.0`, clamped
  `0.0..=1.0`; builder on `Style` and `Element`). At `0.0` the painter takes the
  *exact existing path* (`kurbo::RoundedRect` via a new crate-private `BoxPath`
  enum), so the default is byte-identical — locked by the
  `corner_path_zero_smoothing_is_exact_arc` test. At `> 0` the painter builds a
  superellipse-blended `BezPath` shared by fill, border, and clip so the three
  stay aligned. The construction is a clean Lamé parametrization,
  `point(θ) = C + r·cos(θ)^(2/n)·u + r·sin(θ)^(2/n)·v` with
  `n = 2 + s·(N_MAX-2)`: `n == 2` is provably the exact circle, endpoints are
  `n`-independent (so smoothing reshapes only corners, never the silhouette
  extent), and the corner-bisector pushes out by `2^(1/2 - 1/n) > 1` for `n > 2`
  ("fuller"). `SQUIRCLE_SEGMENTS = 24` flattening is sub-pixel at kit radii.

- **Deviation recorded (2026-06-14) — corner-smoothing scope.**
  `corner_smoothing` reshapes the **fill, border, and clip** paths only.
  **Shadows, the focus ring, and image clips remain circular** this phase:
  `draw_blurred_rounded_rect` takes a single scalar radius, and those paths carry
  no smoothing parameter. This is invisible because **no shipped widget sets
  `corner_smoothing > 0` in 0.20** — the capability is demonstrated only in the
  new `squircle_corners` golden. Threading smoothing into shadows/focus-ring/
  images is deferred until a widget opts in. The squircle is a clean superellipse
  parametrization, **not** a byte-match of Figma's algorithm; `N_MAX = 5.0` is a
  painter-private perceptual constant calibrated by eye on the new golden, not
  derived from the community `~22.37%`/`60%` figures.
- **API decisions (reviewed).** `corner_smoothing` is a structural opt-in, not
  animated: `lerp_style` clones from the target, so a target state's smoothing
  simply wins (it is never tweened, and no widget animates it). `N_MAX` and
  `SQUIRCLE_SEGMENTS` are painter-private constants, not `tokens.rs` entries —
  perceptual calibration, not spec tokens. Review caught a stored-`inset`
  `Concentric` variant whose field nothing read (a second source of truth for
  the inset — the very desync this feature kills); it was dropped pre-release in
  favor of the single `inner(inset)` accessor on `Uniform`. `SurfaceRadius`
  stays `#[non_exhaustive]` for future shapes.

## 0.22: material / translucency (glass)

A typed translucent-vibrancy `Material` and a `Surface::Glass` role: a frosted
floating pane (command palette / glass popover) whose tint lets the content
behind show through. `Material` carries the three perceptual levers of glass —
`fill_alpha` (how much shows through), `blur_radius` (reserved; see below), and
`saturation` (OKLCH chroma "vibrancy"). `Material::tint(base)` keeps the theme
role color's OKLCH lightness and hue, multiplies chroma by `saturation`
(gamut-mapped via `oklch`, never clipped), and applies `fill_alpha`. The new
`Surface::Glass` bundle is the `Elevated(2)` floating recipe (`R_LG`, `Subtle`
border, `Lg` shadow) plus `highlight: Some(0.16)` (a 1px top sheen — the first
shipped role to set one) and `material: Some(Material::popover())` (`fill_alpha`
0.82, `saturation` 1.5). `SurfaceBundle::apply` runs the fill role through
`Material::tint` when a material is set; with `material: None` it is byte-
identical to every pre-0.22 role. Pure style composition at frame time — the
glass is a single semi-transparent `Paint::Solid` composited by vello in the one
existing Scene, identical live and headless. New flagship: `glass_showcase`
(a frosted palette over an accent-gradient backdrop), locked by the
`glass_showcase_{light,dark}` goldens.

- **DECISION — `Material` rides on `SurfaceBundle`, not a `SurfaceFill::Glass`
  variant** (course-correcting the 0.19 forward-guess at "API decisions" which
  anticipated a `SurfaceFill` glass variant). `SurfaceFill` derives `Eq + Hash`,
  which `Material`'s `f32` levers cannot satisfy; `SurfaceBundle` is only
  `PartialEq`, so it holds `Option<Material>` cleanly, and the new role is the
  unit variant `Surface::Glass` (keeping `Surface: Eq + Hash`). The 0.19
  acceptance allowed "a typed `Material` **or** a glass `SurfaceFill`/`Surface`
  role"; this satisfies it without weakening `SurfaceFill`'s derives. No
  redundant payload-less `SurfaceFill::Glass` was added.
- **Acceptance invariant — Glass joins the floating ladder.** `Surface::Glass`
  is `is_floating()` and satisfies `floating.radius >= resting.radius` and
  `floating.shadow >= resting.shadow` against `Card`/`Raised` (`R_LG` ties
  `Card`; `Lg` shadow exceeds `Card`'s `Sm`). Locked by
  `ordering_invariant_floating_ge_resting` (extended to include `Glass`) and
  `glass_is_floating_satisfies_ordering`.
- **DEVIATION — no live backdrop blur in 0.22; `blur_radius` is reserved.**
  vello 0.9's Scene API exposes no backdrop filter:
  `draw_blurred_rounded_rect` blurs a *solid brush over an analytic rounded-rect
  gaussian* (it is the shadow primitive — it cannot sample or blur the scene
  behind it), and `push_layer`'s `BlendMode` is peniko color mix/compose only —
  no spatial filter, no offscreen/readback primitive. fenestra's renderer also
  paints the whole tree into **one** Scene in painter's order
  (`Frame::paint` → `Headless::render`'s single `render_to_texture`), with no
  notion of "render the backdrop below node X, capture, blur, re-inject." A true
  backdrop blur would need a render-graph split (core is windowless; the GPU pass
  lives in shell) plus a mid-frame readback + CPU gaussian (deterministic) or a
  custom wgpu blur pass (whose float output is **not** guaranteed bit-stable
  across Metal — the golden reference — and lavapipe). So the shipped glass is a
  translucent vibrancy tint + hairline edge + 1px top sheen + `Lg` shadow; the
  golden proves translucency + vibrancy + edge, **not** a blurred backdrop.
  `Material.blur_radius` is a documented, unrendered field carrying the intended
  gaussian for the deferred pass.
- **Calibration (eyeballed light + dark on the golden).** `fill_alpha` 0.82
  (band 0.75–0.88) — backdrop clearly visible but muted, body text crisp; leaned
  **more opaque** than a true-blur material because, without a blur, sharp
  content behind text would hurt legibility. The hard gate is the windowless
  `glass_text_stays_legible` test (`contrast_ok(text, tint-over-brightest-
  backdrop, 16, 400)` ≥ APCA Lc 75 over `accent` and `surface`). `saturation`
  1.5 — a *colored* frosted layer, though on the near-neutral `Elevated(2)`
  surface (chroma ~0.006) the shift is subtle; the field is forward-complete for
  the true-blur pass and locked by chroma-≥-base, not a strong visual.
  `highlight` 0.16 (band 0.12–0.20) — a crisp lifted-glass top edge, stronger
  than the button's 0.14.
- **Test-design note — hue preservation verified on a chroma-rich color.**
  `Material::tint` preserves hue *by construction* (the levers never touch `h`),
  but the recovered hue of `Elevated(2)` is numerically meaningless at its
  near-zero chroma (a sub-1e-3 gamut-clamp perturbation swings `atan2` hue by
  tens of degrees). So `material_tint_is_translucent_theme_derived_and_in_gamut`
  asserts L-kept / chroma-not-reduced / gamut-safe / alpha on the real surface,
  and verifies hue preservation on the accent (where hue is well-defined). This
  departs from the blueprint's literal "hue within 1e-2 on the base" because that
  assertion is not physically robust at near-neutral chroma.
- **Deferred renderer milestone — true backdrop blur.** Multi-pass offscreen
  capture + gaussian + composite. Blocked on vello 0.9 having no backdrop filter
  and on fenestra's single-Scene painter; needs a core→shell render-graph split
  and a determinism proof across Metal/lavapipe before shipping. Tracked for a
  future renderer phase; `Material.blur_radius` already carries the intended
  radius.

## 0.23: density mode

`Density::{Compact, Comfortable, Spacious}` (Comfortable default) packs the
`ControlSize` height grid tighter or looser from one knob, via
`ControlSize::metrics_at(Density)`. The Linear/pro-tool density toggle.

- **Comfortable == today, byte-identical.** `metrics_at(Comfortable)` returns
  `metrics()` verbatim, and the kit's widgets still call `metrics()`, so every
  existing widget golden is unchanged — density is purely opt-in. Pinned by
  `comfortable_is_byte_identical_to_today` (asserts the literal Sm/Lg values too,
  so a future table edit trips the test rather than silently moving pixels).
- **Hand-tuned tables, not a raw multiplier.** Compact/Spacious are explicit
  per-size tables on clean whole-pixel steps (e.g. Sm 32 → Compact 28 / Spacious
  36; Lg 40 → 36 / 48), not `height * 0.875` which would yield fractional px and
  jittery layout. Monotonic by construction (`density_scales_box_monotonically`:
  Compact < Comfortable < Spacious height for every size; padding/gap/icon
  non-decreasing).
- **DECISION — density scales spacing, not type.** The label `font` stays tied
  to the `ControlSize` across all three densities (`density_preserves_legible_
  font`), so a compact control's text never shrinks below its legible size — the
  Linear/Material approach (density = touch target + spacing, not font). This is
  a deliberate deviation from a literal "scale type too": shrinking text in
  compact mode would fight fenestra's provable-legibility stance.
- **Scope.** This ships the `Density` primitive + `metrics_at` + a
  `density_showcase` golden. Threading a chosen density through every kit widget
  builder (so `button(...).density(Compact)` restyles a subtree) is a clean
  follow-up; today an app/widget consumes density via `metrics_at`.

## 0.24: composited ring border

`Style::ring(width, color)` (and `Element::ring`) adds the "ring, not border"
primitive (Geist): a crisp band just outside the box, hugging the corner radius.

- **It's a shadow layer, not a new paint primitive.** A ring is a `Shadow` with
  `dx = dy = 0`, `blur = 0`, `spread = width`. The painter already renders a
  zero-blur spread shadow as a crisp filled rounded rect inflated by `spread`
  (radius = `uniform_radius + spread`), so the uncovered band reads as a ring
  around the element. This is exactly the `ChromeElevation` hairline ring (0.14),
  generalized into a one-call builder. It is pushed onto `Style::shadows`, so it
  composes with shadow tokens (frame.rs appends explicit shadows *after* the
  token's layers, so the ring paints on top of any drop shadow) and animates
  through the existing pairwise shadow lerp.
- **Ring vs border.** fenestra's `border` is a paint-only edge stroke (it does
  not participate in taffy layout, so neither reflows). The ring differs by
  sitting *outside* the box: it never overlaps the element's own edge pixels or
  its children, hugs the corner radius crisply, and is the natural home for
  selection/focus emphasis and sub-pixel hairlines. Both are kept — pick the
  edge stroke or the outer ring per intent.
- **Opt-in; no golden churn.** No existing widget was rewired, so every prior
  golden is byte-identical; the new `ring_showcase` golden demonstrates a 1px
  stroked border vs a 1px ring vs a 2px accent (selection) ring.

## 0.25: optical adjustments

`fenestra_core::optical` adds the geometric corrections that make shapes *look*
right: `CIRCLE_OVERSHOOT` (~1.1284 — a circle must be ~12.84% larger than a
square to read as the same size), `overshoot(size)`, and `centroid(vertices)`
(a polygon's visual-mass center, for centering an asymmetric shape on its mass
rather than its bounding box).

- **Math helpers, not a painter change.** The module is pure geometry; the
  caller applies the correction (e.g. translate a play-triangle path so its
  centroid sits at the circle center). So no existing golden moves — a new
  `optical_play` golden demonstrates the play-button correction: bbox-centered
  (looking left-heavy, mass toward the flat edge) vs centroid-centered (looking
  centered).
- **`CIRCLE_OVERSHOOT` literal.** The empirical bjango figure ~112.84% is
  numerically near `2/sqrt(pi)` (1.12838), so clippy's `approx_constant` flags
  it; but the value is the optical ratio, not an approximation of that constant,
  so it carries an `#[expect(..., reason = ...)]` rather than being replaced by
  `FRAC_2_SQRT_PI`.
- **Scope.** Ships the reusable helpers + the canonical play-in-circle fixture.
  Threading optical overshoot/centering automatically into the icon/path render
  pass (so every circular icon and asymmetric glyph self-corrects) is a clean
  follow-up.

## 0.26: effect nodes (generated fields)

`fenestra_core::effects` ships the "bespoke" effects as deterministic generated
RGBA8 textures, consumed via `image_rgba8`: `mesh` (a multi-point gradient field)
and `grain` (seeded film noise).

- **Generated, not shaded.** vello 0.9 has no custom-shader path, so the
  Stripe-style mesh field and the film grain are computed into pixel buffers in
  pure Rust — which makes them deterministic and golden-lockable (the
  `effects_showcase` golden), unlike a per-frame shader. The inputs (size, color
  points, seed, intensity) tokenize; only the resulting pixels are "art".
- **Mesh blends in OKLab.** Each pixel is an inverse-distance²-weighted blend of
  the color points, performed in OKLab (OKLCH's Cartesian form: `a = C·cosH`,
  `b = C·sinH`) so there is no hue-wraparound and no gray dead-zone through the
  middle — the same perceptual principle as the 0.18 gradient builder,
  generalized to a 2-D field; the result gamut-maps through `oklch`. Colors are
  theme tokens.
- **`grain` is a seeded xorshift64\*** — no `rand` dependency and no clock, so
  the same seed always yields the same texture (CI-stable). Monochrome value
  noise at the requested alpha.
- **Scope.** Ships `mesh` + `grain`; the third common effect, a scroll-edge
  fade, is just a `linear_gradient` from the chrome surface color to its
  transparent twin (no new primitive needed) and is documented as such. A live
  backdrop/refraction shader remains out of scope for the current renderer (see
  0.22).

## 0.27: beautiful by default (radius + elevation knobs, console look)

The 0.15–0.26 work proved fenestra *can* render web-grade sharp/minimal and
editorial UIs; a side-by-side study against best-effort HTML/CSS showed the
"template-ish" feel came only from the stock *defaults* (blue accent, uniform
medium radius, shadowed flat cards), not from any missing capability. 0.27 makes
the range discoverable and switchable from one knob each, rather than something
a user has to re-derive at every call site.

- **Radius is a theme knob the kit reads.** `Theme::radius: RadiusScale` (with
  `with_radius`); widgets and `Surface` materials resolve their corners from it
  (`Surface::radius_px`, and the controls' `.rounded` moved into `themed`
  closures reading `t.radius`). The default `RadiusScale::default()` is
  `from_base(R_MD)`, which equals `R_SM`…`R_XL` to the bit — so **every existing
  golden is unchanged** and `sharp()`/`soft()` are pure opt-in. Pills/avatars
  stay `R_FULL` (a square avatar is wrong, not sharp).
- **Elevation is a theme knob.** `Theme::elevation: Elevation` (`Shadowed` |
  `Flat`). `Flat` clears the shadow on resting `Card`/`Raised` at the two surface
  resolution sites (`Element::surface`, `Theme::surface_style`); floating roles
  keep theirs. Default `Shadowed` → no golden change. Rationale: a shadow on a
  same-plane card is a template tell, and in dark mode shadows barely register —
  border + tone-step is the honest separator.
- **`console` Look** packages the study's winning sharp/minimal direction
  (cool-slate `derive` + lime accent + `sharp()` + `Flat`), enumerable via
  `all()`. The stock `product` blue default is deliberately kept (back-compat);
  the curated Looks are how a non-blue identity is one call away.
- **Per-side borders** (`EdgeBorders`, `border_top/right/bottom/left`) paint as
  straight snapped edge strokes *after* the uniform border. Borders are a
  centered stroke that does not affect layout, so this needed no taffy change;
  square corners only (the uniform `.border` remains for rounded full edges).
- **Mesh dither.** `effects::mesh` applies a 4×4 Bayer ordered dither (±0.5 LSB)
  before 8-bit quantization — deterministic, within snapshot tolerance, so the
  field is clean standalone without leaning on a `grain` overlay.

- **Deferred, with rationale (recorded so it isn't rediscovered):**
  - *Variable-font `opsz`.* The font stack (`Fonts` → text style → parley) has no
    variation-axis plumbing; exposing optical sizing means threading
    `VariationSetting`s through registration and shaping. Worth doing, but a
    feature in its own right, not a 0.27 add. Display faces remain static masters.
  - *True multi-line drop-cap.* Needs text-exclusion / float layout (wrap body
    around a tall initial), which parley does not expose. A **raised initial**
    (oversized first `span`) already works today via `rich_text`, and is the
    documented pattern; the floating version waits on exclusion support.
  - *Bundled text-optical serif.* Playfair is display-only; pairing a text serif
    (Fraunces/Tiempos-class) is most valuable alongside `opsz`, and is an asset +
    OFL-vendoring decision. 0.27 ships the *guidance* (use a text serif or the
    sans for body) rather than a new font binary.

## 0.28: typography, density & optical polish

One release bundling four threads, each defaulting to a true no-op so the entire
prior golden corpus is byte-identical: variable-font optical sizing + a bundled
text serif, density through the widget kit, optical correction in the path
render pass, and a polish/consistency sweep.

### Optical sizing + the bundled text serif (the 0.27-deferred typography)

The two 0.27-deferred typography items, now shipped together (they are most
valuable as a pair): variable-font optical sizing and a bundled text serif.

- **`OpticalSizing` drives the `opsz` axis.** A `Copy` enum in the text-style
  group (`Default` | `Auto` | `Fixed(f32)`), mirroring `FontFeatures`: `Default`
  emits *no* variation (so every static face — Inter, JetBrains Mono — and every
  existing golden is byte-identical), `Auto` tracks the rendered px (CSS
  `font-optical-sizing: auto`), `Fixed` pins one optical master. Builders
  `.optical(OpticalSizing)` / `.optical_auto()` on `Style` and `Element`.
- **Plumbed through the one shaping path, as a CSS source string.** parley 0.10
  exposes `StyleProperty::FontVariations(FontVariations::Source(Cow<str>))`
  (verified in the resolved 0.10 source, `style/font.rs`), parsed by parlance's
  `FontVariation::parse_css_list` — the exact `Source(Cow::Owned(..))` shape the
  feature path already uses. `opsz_source(px)` emits `"opsz" <n>`; pushed in
  `shape_greedy`, `shape_rich` (per-span re-track under `Auto` so a display span
  and a body span in one paragraph each get their own master), and `input.rs`
  (a no-op on the editor's static Inter, kept for consistency). Weight needs no
  push: fontique's `Synthesis` already sets the `wght` axis from the requested
  `FontWeight` for a variable face (parley `shape::variations_iter` chains
  synthesis settings *then* the explicit `FontVariations`), proven by the
  pre-existing variable-Playfair Look goldens.
- **Cache-key regression-locked (the 0.16 lesson).** The resolved `opsz` value
  joins `LayoutKey` as `f32::to_bits` (a finite non-negative value is always
  `< u32::MAX`, the no-axis sentinel, so they never collide). `Auto`-at-px and
  `Fixed(px)` resolve to the same `opsz` ⇒ the same key — a correct cache hit,
  not a bug. `layout_key_differs_on_opsz` was written first and watched fail
  before `opsz` was added to the key.
- **Bundled text serif: Fraunces (SIL OFL).** A variable text-optical serif
  (`opsz` 9–144, `wght` 100–900), instanced with `SOFT`/`WONK` pinned to 0 for
  clean letterforms (upright + a true italic, both family `Fraunces` so one role
  registers both and the italic is selected, not synthetically skewed). It
  becomes `warm_editorial`'s `Serif` role — the documented "Playfair is
  display-only" gap fix — with **Playfair Display kept for `Display` headlines**:
  a real display + text serif pairing. Only `look_warm_editorial` moves
  (regenerated, eyeballed); the other five Look goldens and every kit/charts/
  facade golden are byte-identical (default-preservation holds).
- **Eyeball artifact: a dedicated `optical_sizing` golden** (the 0.17/0.18
  dedicated-golden convention): the same Fraunces glyphs at one size pinned to
  `opsz 9` (text master, sturdy/low-contrast) vs `opsz 144` (display master,
  fine/high-contrast) — only the axis differs — plus an `Auto` block proving the
  size-tracked everyday usage.
- **Deferred (unchanged):** arbitrary variation axes beyond `opsz` (`TextStyle`
  is `Copy`, so a `Vec` of axes would break it; `wght` already rides the weight
  attribute, and other axes — `grad`, `slnt` — are a future typed addition);
  the true multi-line drop-cap (still waits on parley text-exclusion); plumbing
  registered display/serif faces into *editors* (static-text only today).

### Density through the widget kit

The 0.23 `Density` primitive becomes a real product feature: the kit's
ControlSize-driven widgets take a `.density(Density)` builder and resolve their
geometry through `ControlSize::metrics_at(density)` instead of the
Comfortable-only `metrics()`.

- **Per-widget `.density()`, not a subtree context.** fenestra has no style
  inheritance (each leaf resolves its own style — see the 0.15 `Ch` note), so a
  subtree-wide `col().density(..)` would need an ambient/context system, which
  fights the no-hidden-state model. The honest, idiomatic delivery is an
  explicit `.density(Density)` on each control: `button(..)`, `icon_button(..)`,
  `text_input(..)`, `select(..)` — the four widgets that sit on the shared
  `ControlSize` height grid. Checkbox/radio/switch/slider have fixed geometry
  and are deliberately excluded (density scales the control grid, not glyph-sized
  toggles). DECISION.
- **Comfortable is byte-identical.** Every widget defaults to
  `Density::Comfortable`, and `metrics_at(Comfortable) == metrics()` (pinned by
  `comfortable_is_byte_identical_to_today`), so the entire existing golden corpus
  is unchanged — density is pure opt-in. Verified: every kit/gallery golden
  passed without regeneration.
- **Density scales spacing, not type.** A widget resolves the full
  `ControlMetrics` bundle once (`let m = size.metrics_at(density)`) and reads
  `m.height`/`m.pad_x`/`m.gap`/`m.font`/`m.icon`. Because `metrics_at` holds the
  label `font` across all three densities (the 0.23 invariant), a Compact
  control is shorter and tighter but its text never drops below its legible size
  — visible in the showcase where every column's labels are the same size.
- **The per-field `ControlSize` accessors were removed.** `padding_x`,
  `text_size`, `gap`, and `icon` (`pub(crate)`, each wrapping the
  Comfortable-only `metrics()`) had no remaining callers once widgets resolved
  through `metrics_at`, and were density-blind — kept-but-unused would be a trap.
  Deleted (root-cause fix, not a `#[allow(dead_code)]`); `height()` stays as a
  public nominal-height accessor.
- **`density_showcase` now dogfoods the real builders.** It was a hand-rolled
  geometry mock (raw `row()`s with manual `metrics_at`) precisely because no
  `.density()` API existed; it is rewritten to render actual `button` /
  `text_input` / `select` at Compact / Comfortable / Spacious. The
  `density_showcase` golden is regenerated (intended, eyeballed). Its signature
  gained `Msg: 'static` (the `select` bound).

### Optical correction in the path render pass

The 0.25 `optical` helpers (overshoot, centroid) were math-only — a caller had
to apply them by hand (the `optical_play` golden shifted triangle vertices
itself). 0.30 wires them into the path render pass as opt-in `path()` builders.

- **Opt-in, not auto-detected.** `Element::optical_overshoot()` and
  `optical_center()` set an `OpticalCorrection { overshoot, center }` on
  `PathData` (both default false). The painter's `optical_pretransform` returns
  `Affine::IDENTITY` when neither is set, so every existing icon/path golden is
  byte-identical. fenestra has no style inheritance, and detecting "is this glyph
  circular / asymmetric?" from an arbitrary `BezPath` is heuristic and would
  silently shift the whole icon set — so correction is explicit per icon, exactly
  like every other 0.2x knob (radius, elevation, opsz, density) defaults to a
  no-op. DECISION (the "auto" in the original follow-up framing is unsafe; opt-in
  preserves the verification-first guarantee).
- **A viewbox-space pre-transform.** The correction composes *before* the
  viewbox→rect scale in `draw_path_rotated`: centroid centering
  (`translate(viewbox_center − centroid)`) then overshoot
  (`scale_about(CIRCLE_OVERSHOOT, viewbox_center)`). The centroid reuses
  `optical::centroid` over the path's on-curve anchor points
  (`path_anchor_centroid`) — the helper is now actually exercised by the
  renderer, not just by callers.
- **Equivalence proven, not assumed.** The `optical_play` golden was rewritten to
  use `.optical_center()` instead of hand-shifting vertices and stays
  byte-identical — the builder applies the identical centroid shift in viewbox
  space. A new `optical_overshoot` golden shows a square, a same-size circle (reads
  smaller), and an `.optical_overshoot()` circle (reads the same size). Four
  painter unit tests pin the pre-transform geometry (identity-when-off, centroid
  mean, centroid→center, scale-about-center by the overshoot ratio).
- **Deferred:** auto-applying correction across a curated icon set (e.g. an
  "optical" Lucide variant that opts the whole set in) — a set-level decision, not
  a per-render heuristic; and threading smoothing/overshoot into shadows and image
  clips (still circular, as noted in 0.20).

### Polish & consistency sweep

A small, low-risk sweep of documented known-limitations — each fixed test-first
or golden-verified, none changing default output it shouldn't.

- **Editors clear a toggled-off feature / `opsz` (the 0.16 limitation).**
  `input.rs::apply_style` was insert-only, so flipping an OpenType feature (or
  the new optical-sizing axis) *off* on a persistent editor left the prior
  `StyleProperty` stuck. It is now insert-or-`remove` (by variant discriminant —
  parley's `edit_styles()` is a discriminant-keyed `StyleSet`). White-box
  regression `editor_clears_toggled_off_feature_and_opsz` was written first and
  watched fail. The new opsz path is covered by the same fix, so it never had
  the bug.
- **Command palette derives from `Surface::Menu` (the 0.19 follow-up).** The
  panel's hand-rolled `rounded(radius.md) + shadow(Lg) + bg(elevated(2)) +
  border(subtle)` is replaced by `.surface(Surface::Menu)` — one source of truth
  shared with menus/popovers, and it now tracks the radius knob. Only change:
  the corner radius rises `R_MD`→`R_LG` (the bundle's floating radius). The
  palette had **no** golden, so a new `command_palette` golden was added to lock
  and eyeball it (closing a coverage gap as well as the consistency follow-up).
  `date_picker` is deliberately left hand-rolled: it renders *inline* and
  shadowless, which maps to no floating role cleanly (forcing `Raised` would only
  swap its fill for no win) — recorded rather than forced.
- **Markdown code blocks read the radius token.** The fenced-code panel used a
  hardcoded `rounded(6.0)`; it now uses `t.radius.sm` so a sharp/soft theme
  re-rounds code blocks too. `radius.sm` defaults to `R_SM` = 6, so the markdown
  goldens are byte-identical.
- **Deferred — horizontal code-block scroll.** The 0.15 note imagined fenced code
  scrolling horizontally (Tailwind `prose pre { overflow-x }`) instead of
  wrapping at the reading measure. fenestra's scroll machinery is **vertical
  only** (`FrameState` tracks `offset_y`, clamps against content *height*, and
  the scrollbar/wheel routing are y-axis); horizontal scroll is a real feature
  (an `offset_x` axis, x clamping, shift-wheel routing, a horizontal scrollbar),
  not a polish item. Code still wraps at the measure; recorded for a dedicated
  horizontal-scroll milestone.

### Serialized description boundary (fenestra-describe / -cli / -mcp)

A serde `Description` (a JSON mirror of an element tree) parses to the same
`Element` the builders produce, then runs the identical render + verification
pipeline — so an out-of-process caller (a CLI, or an MCP server) can build,
render, query, and assert a UI over one stable boundary.

- **Three new crates.** `fenestra-describe` (windowless: core + kit) owns the
  `Description` format, the `to_element` parser, the output DTOs, and the
  *structural* engine (access tree / query / aria snapshot / a11y) built on
  `build_frame` — no GPU needed. `fenestra-render` adds the *pixel/stateful* engine
  (render to PNG, `interact` via `Harness`, screenshot match) and the `fenestra`
  binary. `fenestra-mcp` is a thin MCP server over the cli engine.
- **Format rules.** Schema-tagged (`"fenestra/1"`) from day one; every struct is
  `deny_unknown_fields` (a typo is an error, not a dropped field); colors are
  theme role names or an `oklch` escape hatch (never raw hex); handlers are inert
  intent strings (no logic crosses the boundary); the parser clamps over panic
  (an unresolvable color degrades to a default and records a path-pointed error).
  Style is nested under a `style` key rather than flattened, because serde's
  `deny_unknown_fields` and `#[serde(flatten)]` are mutually exclusive and
  strictness wins.
- **Additive core change.** Per-text-node legibility needs the resolved
  foreground/background/size/weight, which only the private `FrameNode` tree
  holds, so it lives in core: `Frame::legibility(window_bg) -> Vec<TextLegibility>`
  reports each text run's APCA `Lc` and WCAG 2 ratio against the floor for its
  rendered size (`window_bg` is passed in because the frame does not store the
  composite background). `apca` gains `wcag2_ratio` / `wcag2_passes` (the WCAG 2
  piecewise-luminance ratio, distinct from APCA's straight-2.4 estimate), and
  `Semantics::aria_role` makes the role vocabulary public. All additive — the
  existing surface is byte-for-byte unchanged.
- **Crate rename (0.29.1).** The CLI crate published as `fenestra-render`, not
  `fenestra-cli`: that name was already taken on crates.io by an unrelated
  project. The installed binary is still `fenestra`; only the crate / `cargo
  install` name changed.

### Description format follow-ups + deferred phase-2

- **0.29.1 → 0.30 additions.** Button `variant` (primary / secondary / ghost /
  danger) and slider `step` are now in the format (additive optional fields,
  mapped to the kit builders, surfaced in `describe_vocabulary`). The
  description-parser libFuzzer target was run (1.9M executions, no panics).

The within-track-A roadmap is static+intent → declarative state → full builder
parity. As of 0.31, **declarative state** and the **MCP output contract** have
shipped; the design that was recorded here as deferred is now the implementation.

- **Declarative state (the Elm wall) — shipped 0.31.** Logic stays out of the
  JSON: a root `state` map plus a per-widget `bind: "key"`, where the framework
  owns the transition (toggle a bool, set an input's text, set a slider number)
  — no expressions cross the boundary. An `Action` message type
  (`Intent(String)` | `SetBool/SetText/SetNumber(key, value)`) is threaded
  through the parser (now returning `Element<Action>`), `DescribedApp` (which
  owns the runtime state map and applies `Set*` in `update`), and `interact`
  (whose result now carries the post-interaction `state`). The breaking
  `to_element` change was absorbed with `_with(state)` variants so most call
  sites kept the simple form. An unbound handler still emits an inert `Intent`
  (observed via `take_messages`, not applied); only a `bind` writes state. Radio
  remains intent-only (group semantics, no single bound key). (The rejected
  alternative — echoing the core editor buffer into the access tree — would have
  changed `fenestra-core`'s input model for *every* app, not just descriptions.)
- **MCP `outputSchema` — shipped 0.31.** The four assertion tools (`query_ui`,
  `check_a11y`, `match_aria_snapshot`, `describe_vocabulary`) return rmcp
  `Json<T>`, so rmcp derives a formal `outputSchema` from the describe DTOs
  (which now derive `schemars::JsonSchema`). The three image-bearing tools
  (render/interact/match_screenshot) and `validate` still return a rich
  `CallToolResult` (image + `structuredContent`, or `isError`): rmcp cannot
  derive a schema *and* carry image content from a single return, so this is a
  deliberate, documented split, not an omission.
- **Full-resolution image as a `resource_link` — shipped 0.31.** The full-res
  PNG is written to a temp file and returned as a `Content::resource_link`
  (a `file://` URI, mime `image/png`) next to the inline downscaled preview, so a
  large image never bloats the response yet stays one fetch away. The temp
  footprint is bounded: each process keeps at most the last 64 renders on disk
  (older files are removed as new ones are written). The link is local-only by
  design — the shipped transport is stdio, so a same-machine client resolves the
  path; a networked transport would rely on the inline preview instead.

**0.31 hardening (adversarial review).** A bounded fan-out review of the new
crates surfaced one real DoS and several validation gaps, all fixed with
regression tests:

- **Non-finite / enormous font size hung the text layout (the real find).** A
  `size_px` of `∞` / `NaN` / `f32::MAX` made parley's line breaker spin forever
  on wrapping text (its per-glyph advance arithmetic overflows and never fits a
  line) — a worse failure than a panic for a long-lived MCP server. Fixed in
  `fenestra-core`: `resolve_text` (and the rich-span size path) now clamp font
  size to a finite `0..=4096`, mirroring the existing `clamp_advance` for wrap
  width, so *every* fenestra app is protected, not only descriptions.
- **`validate()` now rejects what would render badly.** The describe boundary
  validated structure and color roles but not style *numbers*: a non-finite
  dimension/border width, an out-of-range `size_px`, or an out-of-gamut `oklch`
  (lightness outside `0..=1`, negative chroma, or a non-finite component) passed
  `validate()` then rendered as garbage. All are now path-pointed errors, so
  "valid" means "renders sanely" — the boundary's core promise.
- **Deferred (defense-in-depth, not reachable today).** The parser owns no
  explicit recursion-depth guard; a pathologically deep `Description` is bounded
  upstream by serde_json's default 128-level deserialization limit (and the MCP
  transport bounds the incoming `Value`), so no live entry point can overflow. A
  self-owned depth bound on the public `to_element` is a future hardening item.
  Minor declarative-state foot-guns (a `bind` co-existing with an intent handler
  silently prefers the bind; an out-of-range *initial* slider value in `state`
  reads back un-normalized until first interaction; `SetNumber` widens f32 to
  f64 in the returned state JSON) are recorded as known low-severity behaviors.

## 0.32: feedback & vocabulary (research-driven)

A five-strand survey of contemporary design systems (Linear/Raycast, Vercel
Geist + Radix, shadcn / Tailwind v4 / Base UI, Material 3 Expressive, Apple HIG)
named the same gaps repeatedly; this release ships the universal primitives
fenestra lacked. Pure-additive — every prior golden is byte-identical; only new
widgets and a new `gallery_feedback` golden scene (light + dark) were added.

- **Segmented control.** A contained track (neutral `element` fill, 3px inner
  padding) with a raised thumb (`surface_raised` + `ShadowToken::Sm`) behind the
  active segment, corners concentric (thumb radius = track radius − padding).
  Elm-pure like `tabs` (active index in, `on_select` out) and, like `tabs`, the
  thumb **cross-fades** rather than slides: a true position slide needs
  measured-position (shared-element) animation the per-element transition engine
  deliberately does not do (the M6 tabs decision, reaffirmed). Each segment
  carries `Semantics::Tab { selected }`.
- **Skeleton fill is the translucent neutral twin.** `skeleton` / `skeleton_text`
  / `skeleton_circle` fill with `neutral_alpha.step(4)`, not the solid `element`
  step: a solid neutral vanishes against an elevated dark card (fill and surface
  land on the same tone), whereas the alpha twin composites to a visible veil
  over *any* surface. Motion is a slow opacity pulse (`Keyframes`, 1↔0.45 over
  1.6s) that pins flat under reduced motion, so headless renders are
  deterministic (the `progress_indeterminate` pattern).
- **Live status ring rides the existing keyframe engine.** `status(..).live()`
  stacks a sonar ring under the dot and animates scale 1→3 + opacity 0.7→0 on a
  fast-bloom/slow-fade ease; reduced motion pins the first stop (ring hidden
  behind the dot), so live and static indicators are identical in goldens and the
  pulse appears only in a live window. The dot is decorative; the text label
  carries the accessible meaning.
- **`kbd` is sans, not mono, and keeps obscure keys as words.** Per Geist/Linear,
  key-caps render in the body sans at `Xs` / Medium (not a monospace face), as
  flat chips (neutral `element` fill + `border_subtle` hairline, no shadow) sized
  to sit in palette/menu rows. Modifier names map to glyphs (⌘ ⇧ ⌥ ⌃) and arrows
  to arrow glyphs, but Esc / Tab / Del render as short words — ⎋ / ⇥ are too
  obscure at 12px. The chord is one `Semantics::Image` node with a combined
  label; its glyph text projects as child `Label`s, the same shape a `button` has
  (so tests query by role + name, not bare label).
- **Wavy progress is a static parametric path.** `wavy_progress(fraction, width)`
  strokes a sine polyline (amplitude 2.5, wavelength 16, sampled every 1.5px) in
  the accent for the active span over a flat neutral track, built at a 1:1
  viewbox so the wavelength is width-independent. The wave does *not* scroll (M3
  Expressive animates `waveSpeed`; a static wave is deterministic and still
  distinctive), so no reduced-motion handling is needed.

### Considered and deferred (recorded at decision time)

- **Two-track spring presets (M3 Expressive spatial vs effects).** Not added as
  core tokens: fenestra's closed-form spring already *intrinsically* clamps the
  effects-track properties (colors/opacity/shadows clamp at target; only lengths/
  offsets overshoot — the 0.9 decision), so the headline "effects never bounce"
  property already holds. The marginal value was calibrated named presets; the
  new widgets use the existing `Transition::colors()` / `spring()` directly. A
  named M3-calibrated preset set remains a cheap future addition.
- **Tinted-gray neutral pairing (Radix's accent-biased grays).** The single
  highest-leverage *visual-language* idea from the survey (inject ~0.003–0.010
  OKLCH chroma at the accent hue across all 12 neutral steps, auto-paired by
  accent), but it repaints every border, divider, and secondary-text role in
  every theme — it changes the entire golden corpus and needs a per-theme APCA
  re-validation. It earns its own release, not a slot in an additive one.
- **Other surveyed primitives** — accordion/collapsible and DataList (both want a
  measured-height open/close animation), the Sonner stack-and-expand toast
  refinement, sheet/drawer, breadcrumb, a field/validation wrapper, input-group +
  number stepper, pagination, OTP, and hover-card — are real gaps logged for
  future releases. Linear's "moving specular glass" and FLIP layout animation are
  larger, single-purpose efforts (each its own release).

## 0.33: the craft pass (advanced widget internals)

The 0.32 widgets shipped as MVPs; 0.33 deepens each to the form the research
actually specified, working within two hard constraints of the renderer: layout
insets are pixel-only (no percentage or translate on `left`/`top`), and `Style`
exposes no translate — only `scale`. So anything that *moves* horizontally is
done by animating an absolutely-positioned element's pixel `left`, the way the
switch travels its knob.

- **The segmented thumb slides on known pixels.** Because insets are px-only, a
  sliding thumb needs known segment positions — so segments are *equal width*
  (the standard segmented-control shape anyway): total width is the longest
  label's estimate × n (or a pinned `.width`), and one absolutely-positioned
  thumb animates its `left` to `pad + active·seg_w` on a spatial spring
  (`with_spring(380, 30)`). A single stable id (child 0 of the track) keeps the
  travel continuous across rebuilds. This is the slide `tabs` could not do for
  variable-width tabs; equal width is what unlocks it.
- **Skeleton shimmer is a clipped, swept band.** With no translate, the highlight
  is an absolutely-positioned band whose pixel `left` is keyframed across the
  block (clipped by `overflow_hidden`); it exits fully off the right edge and
  wraps, so the loop is seamless. Headless forces reduced motion (stop 0 pinned),
  so the first stop sits near the left edge with its bright centre on-screen — a
  frozen frame shows the sheen, not a flat block. The highlight step is chosen
  per mode (the ramps invert), lighter than the base. Text lines keep the opacity
  pulse (they have no definite pixel width to sweep).
- **Wavy amplitude is a function of two things.** Per-sample amplitude =
  base · edge-taper(x) · completion-taper(fraction): the edge taper eases the
  last wavelength into the gap (skipped on short bars so they keep full waves),
  and the completion taper flattens the whole wave over the final ~12% so 100%
  reads as a line. There is no phase scroll — a static path cannot translate — so
  the wave is still, which keeps it deterministic.
- **A raised keycap is a thick bottom border.** `kbd_raised` uses per-side borders
  (square corners suit a near-square key): a hairline top/sides and a 2.5px
  `border_strong` bottom that reads as the key's front lip, over a `surface_raised`
  fill. The flat `kbd` chip is unchanged.
- **The status glow is the ring's resting frame.** Rather than add a
  layout-enlarging glow element, the live sonar ring's first (reduced-motion) stop
  is a visible halo (scale 1.7, alpha 0.3) instead of hidden behind the dot, so
  the glow shows in a static frame and blooms outward live.
- **API note.** `segmented` and `wavy_progress` now return builders (`Segmented` /
  `WavyProgress`) to carry their new options; inside `children([..])` they coerce
  through `Into<Element>`, so most call sites are unchanged.

## The verify loop closed (R2b): unified scenario verification

The per-command engine (render / query / interact / check / match-aria / match-png)
left two gaps an agent had to bridge by hand. R2b closes both with one declarative
artifact — a `Scenario` — that `fenestra_render::scenario::verify` runs in a single
pass, returning one `VerifyReport` (an overall `ok` plus a per-check breakdown).

- **Scenario shot compares (the headline gap).** `match_screenshot` only diffed the
  *static* render, so "after I click Submit, the screen looks like this baseline"
  was unverifiable. A scenario's screenshot expectation is now diffed against the
  *post-interaction* pixels when the scenario has steps: `verify` drives the steps
  on the headless `Harness`, then compares `harness.render()` to the baseline. A
  regression test proves it — the same bound-checkbox UI matches a blessed
  after-click baseline but a static (pre-click) render does not.
- **Unified verify.** One scenario replaces a hand-reconciled sequence of separate
  commands and exit codes. Every requested expectation — `emitted` (exact intent
  list), `a11y` (legible + every control named), `aria` (snapshot match, any mode),
  `screenshot` (baseline + tolerance/budget/masks), and `queries` (selector → match
  count) — runs against the *same* frame and folds into a single verdict. An empty
  `expect` is a smoke gate (parses + renders). A failed *check* is a normal report
  (`ok: false`), not an error.
- **Checks read the post-interaction frame, uniformly.** With steps, every
  structural check sees the driven state, not the static description. This needed
  three pure-additive refactors in `fenestra-describe::inspect` so the same logic
  runs against a live `Frame` as against a `Description`: `frame_a11y(frame, theme)`
  (the a11y assembly `check_a11y` now delegates to), `match_aria_text(actual,
  expected, mode)` (the matcher `match_aria` delegates to), and `query_tree(tree,
  selector)` (the matcher `query` delegates to). The existing public functions are
  byte-for-byte behavior-preserving; the new ones are the frame-level primitives.
- **`engine::drive` + `diff_images`.** `interact` was refactored to share its
  harness-driving spine (`drive`, `pub(crate)`) with `verify`, and the private
  image comparator is now public as `diff_images`, so `verify` diffs an
  already-rendered (post-interaction) image without re-rendering. `verify` itself
  composes existing engine pieces; no interaction logic is duplicated.
- **`bless`: the authoring affordance.** `bless(scenario)` renders the scenario's
  final (post-interaction) frame and writes it to the `expect.screenshot.baseline`
  path — so you capture a baseline once, then verify against it. The CLI exposes it
  as `fenestra verify <scenario> --bless`; without `--bless` the command prints the
  report and signals the verdict through the exit code (`0` ok, `1` a check failed,
  `3` a setup/IO error), with `--out` writing the diff PNG on a screenshot miss.
- **`EngineError::Scenario` separates setup from verdict.** A scenario that cannot
  *run* (an unknown schema tag, a bad theme/size, an unreadable baseline, a
  malformed expected pattern) is an `EngineError::Scenario` (exit 3 / an MCP
  `invalid_params`), distinct from a check that ran and did not pass (a normal
  report). This is the same "a verification mismatch is not an error" contract the
  MCP `match_*` tools already follow.
- **`run_scenario` is the ninth MCP tool.** It deserializes a scenario, runs
  `verify` off the async runtime, and returns the structured `VerifyReport` plus a
  preview — the diff image on a screenshot miss, else the final render. Like the
  other image-bearing tools it returns a `CallToolResult` (no derived
  `outputSchema`), and like `match_screenshot` a failing verdict is a normal
  result, not `isError`.
- **`verify` always renders pixels (a deliberate layering call).** Even a
  structural-only scenario produces a final-render preview, because `fenestra-render`
  *is* the GPU layer; an agent or CI gate that wants no-GPU structural checks calls
  `fenestra-describe` (`query` / `check_a11y` / `match_aria`) directly, as before.
  The committed `tests/scenarios/login.json` fixture exercises the full multi-step
  flow (focus, type, toggle, submit) through both the engine and the CLI.

## Responsive grid track vocabulary (R3 layout)

Grid layout was wired to taffy from M3, but the track vocabulary was just `Px`
and `Fr` — no `minmax`, `repeat`, or the `auto-fit` that makes a grid *responsive*
(reflow the column count to the width, no media queries). This pass completes the
responsive half. taffy 0.10 already computes all of it; the work was modelling the
CSS `<track-size>` / `<track-list>` grammar in fenestra's `Style` and mapping it.

- **`Track` is the single track-size; `GridTemplate` is the template entry.**
  `Track` gains `Auto`, `MinContent`, `MaxContent`, `FitContent(px)`, and
  `MinMax(TrackMin, TrackMax)` (the floor/ceiling pair — `TrackMin` forbids `fr`,
  as CSS does). A new `GridTemplate` is `Single(Track)` or `Repeat(Repeat, Vec<Track>)`
  where `Repeat` is `Count(n)` / `AutoFit` / `AutoFill`. The `Style.grid_template_{columns,rows}`
  fields change from `Vec<Track>` to `Vec<GridTemplate>`.
- **One builder, backward compatible.** `grid_cols` / `grid_rows` are now generic
  over `Item: Into<GridTemplate>`, and `Track: Into<GridTemplate>` wraps as
  `Single`. So every existing caller (`grid_cols([Track::Px(_), Track::Fr(_)])` in
  data_table, table, the markdown table, the demo) compiles unchanged, while
  `grid_cols([GridTemplate::auto_fit_minmax(240.0)])` unlocks the responsive form.
  `GridTemplate::{auto_fit_minmax, auto_fill_minmax, repeat}` are the ergonomic
  constructors; `auto_fit_minmax(min)` is `repeat(auto-fit, minmax(min, 1fr))` —
  the canonical responsive card grid.
- **The taffy mapping is one generic function.** `layout::template<S: CheapCloneStr>`
  maps a `GridTemplate` to taffy's `GridTemplateComponent<S>` (S — taffy's
  custom-ident type for *named* lines/areas — is inferred from the collection
  target, so the responsive core needs no idents). Single tracks go through
  `track_sizing` → `TrackSizingFunction`; `MinMax` splits into `track_min` /
  `track_max`; `Repeat` calls taffy's `repeat(RepetitionCount, Vec<_>)`. All sizing
  uses taffy's own `length` / `fr` / `auto` / `fit_content` / `minmax` helpers, so
  fenestra never reimplements track resolution.
- **Verified at the layout level, not just visually.** `fenestra-core/tests/grid.rs`
  asserts the resolved geometry headlessly: `repeat(auto-fit, minmax(180px, 1fr))`
  in a 600px container yields three 200px columns with the fourth item wrapping;
  `repeat(3, [1fr])` yields three equal columns; a fixed+`fr` template still splits
  100/400. The `responsive_grid(min_col, children)` kit helper + a light/dark golden
  give the web-grade visual proof.
- **Authorable in the JSON vocabulary.** `fenestra-describe`'s `style` block gains
  `grid_cols` / `grid_rows`, each an array of `TrackSpec`: a track *string*
  (`"200px"`, `"1fr"`, `"auto"`, `"min-content"`, `"max-content"`) or a structured
  `{"minmax": ["180px", "1fr"]}` / `{"fit_content": px}` / `{"repeat": {"count":
  "auto-fit", "tracks": [...]}}`. So the responsive grid is authorable —
  `{"repeat": {"count": "auto-fit", "tracks": [{"minmax": ["180px", "1fr"]}]}}` —
  and therefore *verifiable* through the R2b scenario loop. The parser is
  clamp-over-panic: a bad track string degrades to `1fr` with a path-pointed error
  (`.../style/grid_cols/0`), `fr` is rejected as a `minmax` floor, and nested
  `repeat` is an error.
- **Named grid lines + `grid-template-areas` (the follow-up, now shipped).** The
  second grid layer: author a 2-D area map (`grid_template_areas(["header header",
  "nav main", "footer footer"])`) and place children by name (`grid_area("main")`),
  or name the lines positionally (`grid_col_names([...])`) and place by line name
  (`grid_col_lines("sidebar", "content")`). Decision: **fenestra resolves names to
  numeric taffy lines itself**, deterministically, rather than threading taffy's
  ident-generic API through the whole style. taffy 0.10's `grid_template_areas`
  wants *resolved* `{name, row/col start/end}` rectangles (it does not parse the
  ASCII form), so a per-frame `grid::ResolvedGrid` (built in `frame.rs:build` from a
  container's style, before its children) maps every name — area-derived
  `<name>-start`/`<name>-end` lines and explicit positional names — to its first
  1-based line; a child's `grid_area`/named-line placement resolves against the
  parent's table just before `to_taffy`. Areas validate as rectangles (a
  non-rectangular name is dropped, lenient — the child falls back to its numeric
  placement, never a panic); `grid-template-areas` with no explicit tracks implies a
  grid of `auto` tracks shaped to the map (CSS implicit grid). Authorable in JSON
  (`grid_template_areas`, `grid_area`, `grid_col_lines`/`grid_row_lines`,
  `grid_col_names`/`grid_row_names`) and therefore verifiable through the R2b loop.
  Every prior grid golden is byte-identical; the holy-grail layout has exact-rect
  tests in core and JSON plus light/dark goldens.

## Form constraint validation (kit)

A small, pure validation engine mirroring the web's Constraint Validation API,
kept Elm-clean: validation logic never lives in a widget. The app holds the
value, calls `validate` in its `view`, and wires the result.

- **`Constraint` + `validate` + `Validity`.** `validate(value, &[Constraint])`
  returns a `Validity { valid, message }` carrying the *first* failing
  constraint's message (so list constraints most-fundamental-first — usually
  `Required`). Constraints: `Required`, `MinLen`/`MaxLen` (Unicode scalar count),
  `Min`/`Max` (numeric), `Email`, `Integer`, `Number`. Every constraint but
  `Required` passes on an empty value, exactly like HTML — an optional field is
  valid until filled.
- **`Field::validity(&Validity)`** shows the failing message as the field's error
  text; the app pairs it with `.invalid(!v.valid)` on the wrapped control for the
  matching danger ring. A light/dark golden renders an invalid email field (the
  message) above a valid one (muted help).
- **No `regex` in the widget crate (a deliberate dependency boundary).**
  `fenestra-kit` depends only on `fenestra-core` + `kurbo`; a general `pattern`
  validator would pull in `regex`, so it is intentionally omitted — validate a
  pattern app-side, or at the `fenestra-describe` boundary where `regex` already
  lives. The regex-free constraints cover the practical bulk.
- **Validity is verifiable, not just visual.** The danger-ring `invalid` flag now
  surfaces in the access tree: `AccessNode` (and the describe `AccessNodeDto`) gain
  an `invalid` field, `access_yaml` emits `[invalid]`, and the describe `text_input`
  / `text_area` vocab gains an `invalid` field. So an agent can author an invalid
  control and a scenario can assert `- textbox [invalid]` through the R2b loop —
  the moat extends from "looks invalid" to "*is* invalid, provably". `invalid`
  serializes skip-if-false, so every prior tree/aria snapshot is byte-identical.
- **Still deferred (logged on the forms task):** `aria-required` (needs a
  `required` field on `Element`, which does not exist yet). The validation engine
  + the `aria-invalid` surfacing ship complete.

## Forms maturity: value semantics, adornments, multi-select

The forms follow-up closes the remaining gaps from the validation pass.

- **Value-bearing ARIA roles.** Three roles join the [`Semantics`] enum —
  `Spinbutton { value, min, max }`, `Meter { value, min, max }`, and
  `ProgressBar { value: Option<f32> }` (`None` = indeterminate). They project the
  same way Slider does: numeric value/min/max into both the YAML access tree
  (`[value=.. min=.. max=..]`, or `[indeterminate]`) and the live AccessKit
  write-path (`Role::{SpinButton, Meter, ProgressIndicator}` + `set_numeric_value`).
  `meter`, `spin_button`, `progress`, and `progress_indeterminate` set them, so a
  measurement or a stepper is now verifiable, not just pixels.
- **Accessible value, decoupled from inputs.** `Element::value(..)` sets an
  explicit `access_value` (ARIA `valuetext`) on any element, overriding the
  input-derived value — how a spinbutton exposes `"$5.00"` or a meter `"75%"`.
  Serializes only when set, so prior snapshots are byte-identical.
- **Interactive spinbutton.** `spin_button` keeps its app-formatted display
  string but `.range(value, min, max)` publishes the numeric trio and ↑/↓ step it
  (gated at the bounds) — a real ARIA spinbutton, not a pair of buttons.
- **Input adornments.** `text_input(..).prefix(x)`/`.suffix(x)` overlay an
  adornment (icon, `$`, unit) absolutely at one end of the *bordered, focusable*
  input, which is padded clear of it. Decision: overlay rather than a
  bordered-wrapper-with-focus-within, because the framework has no focus-within;
  keeping the border + focus ring on the real input is correct and needs no new
  focus machinery.
- **`multi_select`.** A wrapping set of toggleable chips (each a `checkbox` with
  its checked state), Space/Enter to toggle — the fixed-option-set complement to
  `tag_input` (free text) and `select` (single choice). Built on chips rather than
  a stay-open popup listbox, which the overlay system does not model.
- **Validation engine.** `Constraint::Step { step, base }` (CSS `step`: value on
  the `base + k·step` grid, non-numeric passes) and `validate_all` (every failing
  message in order, for a field that lists all its problems) join the first-failure
  `validate`.

## Navigation router, gestures, and web/wasm

The last maturity strand: app-level navigation, a swipe recognizer, and
confirmation that the web target is first-class.

- **`Nav<R>` is a stack, not a framework.** Navigation is Elm-native: the app
  holds a `Nav<Route>` of its *own* route enum, matches `current()` in `view`, and
  drives it from `update` (`push` on navigate, `pop` on back). No magic — it
  composes with multi-window `view_for` and with `.enter()`/`.exit()` for screen
  transitions. The stack is non-empty by construction (the root never pops), so
  `current()` is infallible; `pop_to`/`pop_to_root`/`replace`/`reset` cover the
  usual flows and `stack()` feeds a breadcrumb. Decision: a value-stack the app
  owns beats a registered-routes table — it stays pure, testable, and serialization
  is the app's choice.
- **Swipe rides the existing capture path.** A press records its origin
  `(x, y, t)` in `FrameState`; on release, `recognize_swipe(dx, dy, dt)` classifies
  a flick that travels ≥ 24 px in ≤ 0.5 s into the dominant `SwipeDir` and fires
  `Element::on_swipe`. `on_swipe` joins the press-target filter, so a swipe-only
  element captures the press without needing `on_drag`; a sub-threshold move stays
  a tap (and because a real flick lifts off the element, click and swipe are
  naturally exclusive). **Envelope:** long-press, pinch, and rotate are not built —
  they need a hold-timer tick and a multi-touch `InputEvent` the single-pointer
  model does not carry yet; documented, not faked.
- **Web/wasm verified first-class.** `cargo check --target wasm32-unknown-unknown`
  passes for all four crates (`fenestra-core`/`-kit` are unconditionally
  wasm-clean; `-shell`/facade gate native-only deps — accesskit, arboard, pollster,
  image — behind `cfg(not(target_arch = "wasm32"))` and pull web-time +
  wasm-bindgen-futures on web). The existing `.github/workflows/pages.yml` builds
  the `web_demo` example to wasm, runs `wasm-bindgen`, and deploys to GitHub Pages
  over WebGPU — the same code as native, byte for byte. Everything added this
  maturity pass stays within that envelope.

## RTL mirroring, Dynamic Type, and i18n

Three locale/accessibility settings ride on the `Theme` (the per-render config
already threaded everywhere), plus a dependency-free i18n module. The AccessKit
write-path that headlines this task already shipped (`fenestra-shell/src/access.rs`,
`accesskit` + `accesskit_winit`); this is the rest of R5.

- **RTL is a final mirror pass, not pervasive logical properties.** Under
  `Theme::rtl()` the realized `FrameNode` tree is mirrored horizontally about the
  canvas width as the *last* step of `build_frame` — every rect and clip flips
  (`x' = w - x`), recursively. Because all layout/motion math runs first in
  logical (unmirrored) space and `prev_rects` is recorded pre-mirror, widths are
  preserved and FLIP magnitudes / container queries are unaffected; paint,
  hit-testing, and the access tree all read the mirrored rects, so they agree.
  Decision: a single geometric mirror (vs. taffy `RowReverse` + per-edge logical
  start/end plumbing) gives *comprehensive* mirroring — flex, grid, absolute, and
  nested subtrees all flip uniformly — for ~30 lines, and `start`/`end` text
  alignment flips in `resolve_text` (where the theme is in hand). **Envelope:** a
  FLIP slide's horizontal direction and a horizontal scrollbar's thumb are not
  re-pointed, and intrinsically directional glyphs/icons are not swapped — noted,
  not pretended. LTR (the default) never mirrors and is byte-identical.
- **Dynamic Type.** `Theme::with_text_scale(f32)` multiplies the requested font
  size inside `resolve_text` before sanitization, clamped `0.5..=3.0`. One point,
  so token sizes and free-form `size_px` both scale, and `letter_spacing` (a
  function of the resolved px) tracks. 1.0 is byte-identical.
- **i18n (`fenestra-core::i18n`).** A `Locale` infers writing direction and
  number separators from a BCP-47-ish tag (`Locale::direction()` → `WritingDir`),
  and formats integers/decimals with locale grouping (`1,234.50` vs `1.234,50`).
  A `Catalog` maps keys → messages with `{name}` interpolation and falls back to
  the key (a view never renders blank). Deliberately ICU-free: enough to localize
  strings, numbers, and direction without a megabyte of CLDR data. It is pure
  utility — apps call it in their `view`, so it needs no framework plumbing.

## Horizontal scrolling + `position: sticky` (R3 layout)

Scrolling was vertical-only; this pass makes it 2D and adds CSS sticky positioning
— the primitives sticky table headers, frozen columns, and horizontal panes need.

- **2D scroll state.** `FrameState`'s `Scroll` gains `offset_x`; `clamp_scroll`
  became `clamp_scroll_2d(id, max_y, max_x, stick_bottom) -> (y, x)` (the old
  vertical-only shim is gone). `ScrollInfo` is now 2D (`offset_x/y`, `thumb_v/h`,
  `can_scroll_x/y`); `realize` clamps and shifts children on whichever axes scroll
  and paints a bottom-edge horizontal thumb mirroring the vertical one. Builders:
  `.scroll_x()` / `.scroll_xy()`.
- **Per-axis wheel routing (the subtle part).** `InputEvent::Wheel` gains `dx`; the
  winit `(dx, dy)` extraction is shared (`wheel_deltas`). Dispatch routes `dy` to
  `scrollable_y_at(point)` and `dx` to `scrollable_x_at(point)` *independently* —
  a horizontal pane nested inside a vertical scroller each receive their own delta
  (an adversarial review caught that routing both deltas to one
  `scrollable_at` would let an inner horizontal pane swallow the page's vertical
  scroll).
- **Sticky is a post-layout rect clamp.** `Position::Sticky` maps to taffy
  `Relative` (normal flow, correct content size), then `realize` clamps the rect
  via `apply_sticky(natural, style, ctx)` to the nearest scroll container's
  *content box* (inside padding — another review fix). `top`/`left` win over
  `bottom`/`right` on conflict (CSS positioned-layout order, achieved by applying
  the `min` edges before the `max` edges). A `StickyCtx { viewport }` threads down
  through `realize`; scroll containers set it to their content rect, other nodes
  pass it through, so a sticky descendant sees its nearest scrolling ancestor.
- **Sticky z-order.** `FrameNode.is_sticky` splits paint into two passes
  (non-sticky, then sticky on top) and hit-testing into two (sticky first), without
  reordering the children vector (which would corrupt baseline-offset indexing).
- **Reduced motion / determinism.** Headless scrollbar alpha is deterministic
  (clock-driven); sticky is a pure geometry clamp with no animation. Verified by
  `fenestra-core/tests/scroll_2d.rs` (offset shift/clamp, axis independence, nested
  per-axis routing, content-box padding) and `tests/sticky.rs` (pin, below-line
  passthrough, top-wins-over-bottom, inert-without-ancestor). The data_table widget
  consumes these (sticky header + frozen columns) in the data_table maturity task.

## data_table maturity (virtualize / resize / reorder / pin / filter)

The data_table grew from sort + multi-select to a full data grid, staying Elm-pure
(the app owns column widths/order/filter; the widget renders them and emits Msgs).
Built on the R3 scroll/sticky primitives and core row virtualization.

- **Virtualization + sticky header.** The body is a dedicated
  `col().id("dt-body-{id}").scroll_y().virtual_rows(n, ROW_H, builder)` — only the
  visible window materializes. The header is a *sibling above* that scroll
  container (not inside it), so it is sticky for free with no `Position::Sticky`.
  A stable `.id(key)` is required (the scroll offset keys off it).
- **Resize via the split-pane drag pattern.** `.column_widths(..)` switches column
  tracks from `Fr` to `Px`. Each header cell carries an inert resize handle; the
  `on_drag` lives on the header *row* (the handle is just a hit target), converts
  `fraction_in` to a content-x, detects the column boundary (or continues
  `.resize_active`), and emits `.on_resize(col, w.clamp(40, 800))`. The new core
  `Element::on_drag_end` fires `.on_resize_end` so `resize_active` can't get stuck.
- **Reorder via internal DnD.** `.column_order` permutes display→data; the header
  sort-button is a `drag_source("col-reorder:{i}")` and each header cell an
  `on_drop` that emits `.on_reorder(from, to)` (guarded `from == to` so a plain
  click still sorts). Selection tracks the *data* column through reorders.
- **Pin via sticky.** `.pinned_left/right(n)` cells get `.sticky_left/right(offset)`
  and the body becomes `.scroll_xy()`, so frozen columns hold during horizontal
  scroll (verified by an interaction test).
- **`on_drag_end` (core).** New `Element::on_drag_end(msg)`: on `PointerUp`, if the
  captured `active` element had `on_drag`, the message fires — even if the pointer
  left the element (capture semantics), never on a plain click. Tested in
  `fenestra-core/tests/drag_end.rs`.
- **Documented limitations (deliberate, not gaps).** (1) `Cursor::ColResize` does
  not exist in the cursor enum (and `map_cursor` is exhaustive), so resize handles
  use `Cursor::Pointer` — matching the split-pane divider. (2) Under virtualization,
  a *pinned* column's freeze works, but the header does not track the body's live
  *horizontal* offset: `virtual_rows` replaces a container's children (header must
  be a sibling), `StickyCtx` resolves a single nearest scroll ancestor, and
  `WidgetId` derives from the parent — so header and virtualized body cannot share
  one horizontal scroll offset without a larger core change. Invisible for vertical
  scroll and for pins where the rest fits; only a live horizontal scroll of an
  overflowing virtualized body desyncs the header. (3) Resize/pins need explicit
  `.column_widths` (the drag closure sees fractions, not laid-out rects, so pixel
  boundaries must be known at build time). All three are documented in code.

## Motion completion: FLIP layout animation + exit animations

Supersedes the earlier deferrals ("Enter animations seed, exit stays out"; the
M6 tabs / 0.32 segmented "no measured-position animation" notes): shared-element
layout slides and exit animations now exist in core. Pure-additive — opt-in per
element, gated on `!reduced_motion`, so every prior golden is byte-identical
(verified: full suite green, no `.actual.png` artifacts).

- **FLIP rides the existing transition engine, not a parallel path.** `realize`
  records every node's absolute rect into `FrameState::prev_rects` (shared,
  by design, with the constraints-aware-layout work). A bare `.animate_layout()`
  element is handed an *implicit* spatial spring in `resolve`, so the retained
  `Anim` already exists and advances each frame like any transition. After
  layout, `apply_flip` compares this frame's center to `prev_rects`; on a move
  it `Anim::inject`s a retarget whose `from` is the natural style shifted by
  `(prev − new)` and paints the node there this frame — so it appears at the old
  spot and the engine springs it to zero over subsequent frames. Endpoints
  differ only in `translate`, so nothing but position moves; the delta *adds* to
  any existing translate. A declared `.transition()`/`.enter()` wins and drives
  the slide at its own timing. Identity is the `WidgetId`: index-keyed reorders
  can't FLIP (per-index id+position are stable), so a stable `.id` is required —
  documented and tested.
- **Exit animations are paint-only ghosts with a retained lifecycle.**
  `FrameNode` is not `Clone` (live editors, scroll, a11y), so an exiting element
  is snapshotted into a clonable `GhostNode`/`GhostPaint` tree (`ghost.rs`; an
  input collapses to a box — no editor behind a ghost). Each frame, *after* the
  FLIP pass, `collect_exits` snapshots live exit-tagged nodes into
  `FrameState::exit_cache` — post-FLIP, so a node leaving mid-slide keeps the
  paint-time translate it last showed rather than snapping to its settled layout
  rect. In `build_frame` a departure (in last frame's cache, absent from this
  frame's `all_rects`) starts an `exiting` record, a reappearance cancels it, and
  a settled record is GC'd at the *top* of the next build. `Frame::paint` draws
  each non-settled ghost above everything inside an opacity layer, replaying the
  ghost's own frozen transform (the `paint_ghost_node` → `_unscaled` split mirrors
  the live `paint_node` split) with the exit's scale/translate sub-scene composed
  on top, advancing its own spring/ease progress and marking itself settled when done.
- **Reduced motion keeps it inert (the golden guarantee).** FLIP is skipped
  wholesale (elements snap; no spring injected). Exits are detected but seeded
  `settled = true`, so they are never painted and are collected the next frame —
  headless removal is immediate. `expand_virtual` strips `.exit`/`.animate_layout`
  from generated rows, so scrolling a virtual row out of the window never spawns
  a ghost or a FLIP slide.
- **Deviations from the brief (deliberate).** (1) The test-introspection hooks
  (`has_anim`/`has_exiting`/`exiting_settled`/`prev_rect`/`exiting_ghost_translate`)
  are `pub` + `#[doc(hidden)]`, not `#[cfg(test)]`: integration tests link the
  library built *without* `cfg(test)`, so a `cfg(test)` method would be invisible
  to them — the same shape the existing `scroll_offset` hooks use. (2) `GhostNode`
  drops the suggested `id` field — it would be a never-read copy of the `exiting`
  map key (a dead field rustc would flag); `visible` is kept and honored as the
  ghost's clip.
- **Refinement (post-review): FLIP and exit compose faithfully.** A first pass
  snapshotted exit ghosts *inside* `realize`, before `apply_flip` set the FLIP
  translate, and `paint_ghost_node` ignored the frozen transform — so an element
  removed mid-slide (a reorderable + deletable list) popped from its slid position
  to its settled rect for one frame, and any static `.translate()`/`.scale()` on an
  exiting element was likewise dropped. Fixed in two halves: snapshotting moved to
  `collect_exits` *after* the FLIP pass (captures the slid translate), and the ghost
  painter now replays the frozen transform like the live path. Both are still inert
  under reduced motion (FLIP never runs, ghosts never paint). Regression:
  `exit_ghost_inherits_a_mid_flip_slide` in `tests/motion.rs`.

## Constraints-aware layout: window breakpoints (`view_at`) + container queries (`responsive`)

Two opt-in tiers for layout that reacts to *size*, not just state. Pure-additive
and fully defaulted, so every prior golden is byte-identical (verified: full
suite green, no `.png`/`.snap` churn). Tier 1 keys off the window; Tier 2 keys off
a container's own measured size.

- **Tier 1 — `App::view_at(key, size)`.** A new defaulted trait method:
  `view_at` → `view_for` → `view`, so existing apps are untouched (the size is
  ignored by the default). The runner/embed/harness redraw sites now pass the
  logical size they already had in scope (each hoisted into one `logical`
  binding so a single `#[expect(cast_possible_truncation)]` covers the f64→f32
  cast used by both `view_at` and `build_frame`). `embed::render` previously
  called `view()` directly; it now routes through `view_at(MAIN_WINDOW, logical)`
  for parity with the windowed runner — a behavioral change only for an app that
  *overrides `view_for`/`view_at` and renders through `embed`* (an unusual combo,
  and arguably a bugfix). `Breakpoint` (`Base/Sm/Md/Lg/Xl/Xxl`, Tailwind logical-px
  thresholds) classifies a width via `Breakpoint::at`, with boolean
  `Breakpoints::up/down/only` + `is_*` shorthands for `view_at` consumers
  (`breakpoints.rs`, unit-tested). `Harness::resize(key, w, h)` rebuilds one slot
  at a new size via `view_at` — the headless analogue of dragging a window edge.

- **Tier 2 — `responsive(|avail| -> Element)`, reusing `prev_rects`.** A container
  query: the closure rebuilds the subtree from the container's *own* size. The key
  decision is to **reuse `FrameState::prev_rects`** — the motion/FLIP map that
  already records every realized node's absolute rect each frame (`state.prev_rects
  = all_rects`, unconditional) — instead of adding a parallel measurement map with
  its own realize/GC machinery. `build` runs before realize, so during this frame's
  build `prev_rects` holds last frame's rects. `responsive`/`responsive_hinted`
  produce a *transparent* wrapper element carrying only a `pub(crate)
  ResponsiveData { hint, f: Rc<dyn Fn((f32,f32)) -> Element> }` (mapped in
  `Element::map` exactly like `virtual_rows`' builder). The very first statement of
  `build` expands it: read `prev_rects[id]` (or the hint on frame 1), call the
  closure, and recurse **under the same `id`** — so next frame `prev_rects[id]` is
  the *generated* container's rect, closing the loop. The wrapper's own style is
  never resolved (early return before `resolve`), so consumers style the returned
  element.

- **One-frame-deferred convergence (the CSS container-query model).** Frame 1 has
  no measurement → hint; frame 2+ uses the measured size; a resize re-converges one
  frame after it lands (the intervening frame reads the stale measurement). This
  trades instant convergence for a guaranteed absence of layout cycles. Traced
  end-to-end in `tests/responsive_layout.rs`
  (`responsive_container_query_converges_one_frame_after_resize`: hint→col,
  measured→row, shrink→stale-row→col).

- **Caveats, documented on the builders.** (1) *Identity*: the loop keys off the
  wrapper's `WidgetId`, so a wrapper whose sibling position can move needs a stable
  `.id` (a fixed position is already stable). (2) *Monotonicity*: a non-monotone
  closure (larger `avail` yields content that shrinks the container back below a
  threshold) can oscillate every frame at a boundary; prefer width-driven
  breakpoints on a parent-sized (`w_full`) container, where width is independent of
  the chosen branch. (3) Nest a finer `responsive(..)` as a *child*, never as the
  closure's root: a direct self-wrapping chain expands under the same id, so
  `expand_responsive` follows it only `RESPONSIVE_MAX_HOPS` (16) times and then
  flattens it to an empty box — graceful degradation, never a stack overflow
  (regression: `responsive_self_wrapping_is_capped_not_a_stack_overflow`).
  `Breakpoint`/`Breakpoints` are the natural threshold vocabulary for both tiers.

## Real frosted-glass backdrop blur (two-pass CPU)

`Material.blur_radius` was reserved since 0.22 (vello 0.9 has no GPU backdrop
filter). It is now a real frosted-glass pane: the content *behind* the glass is
read back and blurred, then composited under the vibrancy tint. The same
machinery also renders a foreground `ElementFilter` (blur / brightness /
saturate of an element's *own* content). The decisions:

- **One walk, three modes — `PaintMode` threaded through `paint_node`
  (`paint_plan.rs`, `frame.rs`).** `Frame::paint()` is now `paint_with(Full)`;
  two siblings join it: `paint_backdrop() -> (Scene, Vec<MultiPassSpec>)` walks in
  `Backdrop` mode, `paint_final(&injected) -> Scene` walks in `Final` mode. A
  node only reads the mode when `style.backdrop_blur` or `style.element_filter`
  is set, so the walk is otherwise the same code. **Fast-path byte-identity:** a
  frame with no filtered node records no specs, the backdrop walk paints exactly
  what `Full` would, and the shell returns that single render untouched — every
  pre-existing golden still passes (verified: only the glass goldens moved). The
  non-glass path is never branched.

- **Two passes, in the shell (`headless.rs::render_plan` →
  `multi_pass.rs::process_specs` → `blur.rs`).** `render_plan` calls
  `paint_backdrop`; if the plan is empty it is the fast path above. Otherwise it
  renders the backdrop scene (every glass subtree painted as *nothing*, so the
  pixels behind survive), reads it back with the existing 256-byte-padded
  staging-buffer template, blurs/filters each spec's region, and renders
  `paint_final`, where each glass pane draws its blurred backdrop image (clipped
  to its rounded silhouette, between the drop shadow and the tint — a new
  `backdrop: Option<&ImageData>` arg on `push_box`) and each filtered element
  draws its processed image in place. All headless render entry points
  (`render_element*`, `Harness::render_window`) route through `render_plan`; the
  font→headless lock nesting is uniform across them.

- **Determinism (the non-negotiable).** Goldens are referenced on macOS/Metal and
  re-run on Linux/lavapipe, so the blur is a pure **integer** box blur — three
  separable running-sum passes (the 3-box ≈ Gaussian construction) with rounded
  means and edge clamping — bit-identical on any platform for identical input.
  The GPU-rendered *input* differs slightly across rasterizers, but blurring only
  shrinks those high-frequency differences, and the golden compare is
  tolerance-based (and budget-widened on lavapipe). The kernel is unit-tested
  against an exact hand-verified literal (`[0,0,90] →(r=1)→ [17,30,43]`).
  Brightness/saturate use per-pixel IEEE-754 `f32`, also platform-stable (Rust
  never fuses to FMA implicitly).

- **`element_filter` captures its own content (deviation from the literal
  brief).** The brief had the backdrop walk *skip* filtered subtrees like glass;
  but a foreground filter must filter the element's *own* pixels, so a filtered
  node is painted **normally** in the backdrop pass (its spec is still recorded),
  its rect is read back, filtered, and drawn in its place in the final pass. The
  `PassKind::BackdropBlur` `corner` field from the brief was dropped as
  redundant: the rounded clip is applied at composite time from the glass node's
  own `corner_radius`, so the plan needn't carry it.

- **Live window: documented single-pass fallback.** The swapchain path
  (`window.rs`, `embed.rs`) still renders one pass — glass shows its translucent
  tint without the CPU blur (which needs a mid-frame GPU read-back/stall). This
  was a deliberate scope call: the headless golden is the source of truth, the
  windowed read-back could not be visually verified in this environment, and a
  wrong unverified read-back would ship a broken live window. The fallback is
  `paint()` (`Full`), so it is exactly the prior look — no regression — and the
  call sites carry the limitation note. Lifting it later means a `present`-time
  read-back feeding the same `process_specs`/`paint_final`.

- **Ergonomics.** `Surface::Glass` resolves `blur_radius` into
  `style.backdrop_blur` (so the existing `glass_showcase` frosts for free);
  `Element::backdrop_blur(px)` / `Element::element_filter(..)` are the raw
  builders; `kit::{glass_surface, glass_panel}` wrap `Surface::Glass` with a
  rounded clip (and concentric padding) for one-call frosted panes.

- **Supported envelope (the plan is one level deep, in layout space).** The
  blur region is the pane's *layout* rect, recorded once per filtered node, so
  three exotic compositions are deliberately out of scope (a frosted floating
  panel over content — what glass is for — hits none of them; none are exercised
  by the goldens):
  - *A glass pane with a paint transform* (translate/scale/rotate from
    `animate_layout`, press-scale, or a static transform): the backdrop is
    sampled at the pane's untransformed layout rect, so the frost can lag the
    pane's transformed position. A blurred panel should be untransformed.
  - *Glass nested inside glass*: the inner pane is painted (its tint shows over
    the outer pane's already-blurred backdrop) but its own `backdrop_blur`
    radius is dropped — no second, independent blur pass.
  - *Glass inside a foreground-`element_filter` subtree*: unsupported — the
    filtered ancestor bakes its subtree into one image and the nested pane,
    which painted nothing into the backdrop, is omitted. (A backdrop-blur pane
    inside a foreground blur is a contradictory request.)
  Lifting any of these means tracking the accumulated transform and/or a
  recursive multi-level plan; the single-level design covers the real use cases.

## Squircle corners (continuous-curvature, kit-wide)

The kit's rounded corners are continuous-curvature **squircles** rather than
circular arcs. A circular corner is only tangent-continuous (G1) with the
straight edge it joins: curvature jumps from `1/r` to `0` at the join, a
discontinuity the eye reads as a faint "kink." A squircle ramps curvature
smoothly to zero at the join (G2), which is the softness Apple's hardware and
UI corners are known for.

- **Shape: a superellipse, not the Figma cubic recipe.** Each corner is a Lamé
  superellipse quadrant, `center + r·cos(θ)^(2/n)·u + r·sin(θ)^(2/n)·v`. It is
  fully derivable (no reverse-engineered magic constants), reduces to the exact
  circle at `n = 2`, and for `n > 2` has *zero* curvature where it meets the
  straight edge — genuine G2 continuity. Smoothing `s ∈ 0..=1` maps to the
  exponent `n = 2 + s·(5−2)`; `N_MAX = 5` is Apple-icon territory.

- **Emitted as true cubic Béziers.** `build_squircle` feeds a
  `SuperellipseQuadrant` (a `kurbo::ParamCurveFit`) to `kurbo::fit_to_bezpath`,
  which returns the minimal run of cubics holding a `0.08`px Fréchet tolerance —
  not a flattened polyline. The quadrant guards the parametric singularity at
  the joins (the superellipse's `dθ`-derivative is unbounded there for `n > 2`):
  endpoint tangents are returned exactly (`+v` / `−u`, the edge directions) and
  the interior derivative is clamped a hair inside `[0, π/2]`. Fill, border,
  clip, focus ring, and image clip all build from the one path so they stay
  aligned. `smoothing <= 0` still takes the exact `kurbo::RoundedRect` arc path,
  so circular rendering is byte-identical to before the knob existed.

- **Theme-wide default, opt-out per element.** `Theme::corner_smoothing`
  (default `DEFAULT_CORNER_SMOOTHING = 0.6`, picked by eye) is the single knob
  that re-curves the kit, mirroring `Theme::with_radius`.
  `Style::corner_smoothing` is an `Option<f32>`: `None` inherits the theme,
  `Some(x)` overrides (and `Some(0.0)` forces circles). `resolve()` fills an
  unset element's smoothing from the theme **only where it has a finite rounded
  corner** — so square boxes keep the exact arc path, and pills/avatars (an
  `R_FULL` = infinite radius) stay perfectly circular. Smoothing is structural,
  never animated: a target state's value simply wins (the lerp carries it
  through unchanged).

- **Supported envelope (what stays circular).** Three paths can't take the
  squircle silhouette and are left as-is by design:
  - *Drop shadows* go through vello's `draw_blurred_rounded_rect`, which takes a
    single uniform radius, so a shadow's silhouette has circular corners. It is
    a blurred penumbra under the element, so the difference is imperceptible.
  - *Per-side hairline borders* (`side_borders`) are straight edge strokes with
    square corners (unchanged); a rounded full edge should use the uniform
    `border`, which does follow the squircle.
  - *A perfect circle expressed as a large finite radius* (half the short side)
    rather than `R_FULL` still receives smoothing, since only the `R_FULL` idiom
    is auto-excluded; pass `corner_smoothing(0.0)` if an exact circle is needed.
  The fit runs per rounded element per frame (a few cubics each — cheap, not
  cached); a glyph-cache-style memo is the obvious lever if it ever shows up.

## Unlocking built-but-off defaults

A pass over capabilities that were implemented and tested but shipped off by
default (or unreachable). Each was verified to keep the stock goldens
byte-identical except where the change *is* the improvement.

- **Optical sizing defaults to `Auto`.** `Style.optical` gained an
  `OpticalSizing::Inherit` default that `resolve()` / `resolve_text()` replace
  with `Theme.optical_sizing` (stock `Auto`, the CSS default); an unresolved
  `Inherit` falls back to `Auto`. The static stock faces (Inter, JetBrains Mono)
  carry no `opsz` axis, so the variation is a proven no-op there — exactly one
  golden moved (the `warm_editorial` look's variable Fraunces, now tracking its
  optical master), so no font-axis gating was needed.

- **Reduced-motion OS bridge.** The honoring logic existed; the live window
  never set `FrameState.reduced_motion`. `shell::reduce_motion` reads the OS
  setting through each platform's own CLI — `defaults` (macOS),
  `gsettings` (Linux/GNOME), `reg` (Windows) — rather than `objc2`/Win32 FFI, so
  `unsafe_code = forbid` holds. `live_state()` seeds every live window from it
  and `WindowEvent::Focused(true)` re-reads it. *Envelope:* a read failure or an
  unrecognized platform reports `false` (full motion); the wasm build defers to
  the browser's `prefers-reduced-motion` (not this path).

- **FLIP needs stable keys, so it is applied where identity is natural.**
  `animate_layout` is wired into tag-input chips (keyed by tag), inline
  `data_table` rows (keyed by joined cell content), and toasts (keyed by
  message). *Envelope:* virtualized `data_table` bodies recycle rows, so FLIP is
  inline-only; duplicate keys (identical chips/rows) collide and won't animate
  distinctly. All of it snaps under reduced motion, so resting goldens are
  unchanged.

- **Overlay enter motion, not exit.** Anchored pop overlays (menus, dropdowns,
  tooltips, submenus, context menus) now rise 8px as they fade in, via the
  existing openness `progress` (modals and drawers already moved). *Envelope:*
  overlays still close instantly — the openness progress is enter-only, and a
  closing-overlay lifecycle (to fade/slide *out*) is follow-up work.

- **Toast motion** reuses `enter` + `exit_to` + `animate_layout`: fade/scale in,
  fade + shrink + drop out, FLIP-reflow the survivors, and `on_swipe` so a flick
  dismisses. Keyed by message; duplicate-message toasts share identity.

## Liquid Glass optics: specular rim, body sheen, edge lensing

A multi-agent research pass (plan → six web-research strands → synthesis) surveyed
Apple Liquid Glass (WWDC25 / iOS 26 / macOS Tahoe 26), Material 3 Expressive,
Microsoft Fluent (Acrylic/Mica), and visionOS, then ranked the gaps fenestra could
close on vello. This implements the top distinctive, deterministic,
golden-lockable ones: a luminous specular **rim**, a directional body **sheen**,
and edge **lensing** (refraction) at the rounded rim. The first two are pure style
composition in the core painter; the third extends the headless backdrop-blur pass
in the shell. Verified against vello 0.9 before building: `Scene::stroke` takes a
gradient brush, and `push_layer` / `Mix` exist for the clipped washes.

Both rim and sheen are orthogonal `Option` fields on `Style`
(`specular_edge`, `sheen`), default `None`, so every non-glass element and every
opaque `Surface` role stays byte-identical. They are switched on in exactly one
place — `SurfaceBundle::apply`, gated on the role carrying a `Material` (today only
`Surface::Glass`) — so the stock frosted pane gains them with no new call, and
`SpecularEdge::glass()` / `Sheen::glass()` hold the calibrated recipes (top-lit rim
at intensity `0.6` / shade `0.18`; a 135° sheen at `0.12` / `0.06`). The painter
draws the rim as a hairline (~1.5px) gradient-brushed stroke just inside the
silhouette — a dark underside at gradient offset `0`, through clear, to bright white
at offset `1` — and the sheen as a full-face gradient wash, both source-over and
clipped to the rounded path. New `tokens::GLASS_LIGHT_DEG` (`0` = top) is the shared
light direction.

- **DECISION — the rim is a linear vertical gradient, not the true angular
  sweep.** The research described a `cos(normal·light)` rim that brightens
  continuously around the perimeter toward the light. The shipped rim is a single
  bright-top / dark-bottom linear gradient — a deliberate, tasteful approximation
  chosen for convention-robustness and fenestra's calm aesthetic. A per-segment /
  angular-sweep variant is a clean later upgrade; the `light_deg` field already
  anticipates it.
- **DECISION — lensing rides the existing headless two-pass, pure f32.**
  `PassKind::BackdropBlur` gained a `radius` (the pane's uniform corner radius in
  logical px; `frame.rs` averages the four corners — `0.25·(tl+tr+br+bl)` — into one
  bevel radius). After the deterministic integer box blur, `blur.rs::refract_edges`
  resamples each pixel in a bevel band along the rounded-rect SDF from further
  *inside* along the corner normal (edge-clamped bilinear), so straight backdrop
  lines bend and compress into the rim. It is pure IEEE-754 `f32` with no implicit
  FMA fusion, so it is bit-stable across Metal and lavapipe exactly like the box
  blur, and the interior beyond the band is byte-identical. `multi_pass.rs` runs it
  for every glass region.
- **Envelope — lensing is headless-only; subtle through the stock glass.** Like the
  backdrop blur it extends, lensing lives in the headless two-pass; the live
  single-pass window keeps the flat tint (the standing glass envelope). Through the
  legibility-first `Surface::Glass` (0.82α) the bend is subtle; it is vivid on a
  glassier pane, which is why the new `liquid_glass` flagship floats a vibrant
  0.5-alpha pane over bold accent stripes so the rim, sheen, and lensing all read at
  once (output gitignored).
- **Reverted — contact / umbra shadow.** A token-level negative-spread occlusion
  layer on `Lg` / `Xl` was implemented, then reverted: fenestra's shadows are
  already multi-layer and surface-hue-tinted, the 0.82 glass is too opaque for an
  inset occlusion to show through, and the layer is hidden under opaque surfaces — a
  barely-perceptible payoff against broad golden churn.
- **SHIPPED (was deferred) — backdrop-adaptive tint, at paint time.** The deferral
  assumed adaptation had to happen during *style resolution* — before the backdrop
  exists — needing the measured luminance plumbed back into the fill. It doesn't:
  `push_box` already receives the composited backdrop `ImageData` as a parameter
  (painter.rs), so the painter measures that crop's mean Rec. 601 luma immediately
  before it fills the tint and shifts the tint's OKLCH lightness — brighter over
  dark backdrops, darker over light. `AdaptiveTint { pivot 0.55, gain 0.20 }` is a
  new orthogonal `Style` field set in `SurfaceBundle::apply` for material roles
  (default `None` everywhere else, so non-glass stays byte-identical). The mean is
  summed in integer channels then reduced once in `f64`, so it is bit-stable across
  Metal/lavapipe exactly like the box blur it reads. This is Fluent Acrylic's
  "luminosity clamp" intent (stabilize the material's luminance against the
  backdrop) reduced to one mean-luminance shift; Apple Liquid Glass does the same
  tonal adaptation without a published formula. *Envelope:* headless-only — the
  single-pass live window passes `backdrop: None` and keeps the flat tint; it is a
  mean-per-pane global shift, not a per-pixel luminosity blend; and through the
  legibility-first 0.82α stock glass it stabilizes the composite lightness rather
  than announcing itself (gentle over mild backdrops, up to ±0.11 lightness over
  pure black/white). *Still deferred:* adaptive shadow opacity.
- **Deferred — lower-ranked research recs.** An SDF bevel-band giving the rim finite
  thickness, capsule-first control shapes with radius-by-size, an inset/recessed
  shadow primitive, and an explicit elevation-tier token model — all recorded, none
  shipped this pass.

## Authoring the glass aesthetic (describe-layer mirror)

The Liquid Glass optics (specular rim, body sheen, edge lensing, backdrop-adaptive
tint) and the squircle / backdrop-blur fields lived in `fenestra-core` but had no
JSON mirror, so an agent could neither author nor — therefore — headlessly *verify*
fenestra's signature surface. This adds them to the `fenestra-describe` authoring
schema: the half of the moat that makes the web-grade visuals agent-reachable.

- **Authoring-only round-trip.** The describe boundary is one-directional — JSON →
  `Element` via `parse::apply_style`; there is no `Element` → JSON `Style` emitter
  (describe/inspect emits the *access tree*, `dto::AccessNodeDto`, not the authoring
  style). So this extends the `format::Style` DTO with new optional fields
  (`deny_unknown_fields` keeps them additive and the parser strict) and adds
  `apply_style` branches mirroring the established `shadow_token` / `finite_num`
  patterns. The standing invariant "the JSON boundary produces the same `Element` the
  builders do" then guarantees identical pixels, already golden-covered.
- **`surface` is a deferred role.** `Element::surface` registers a `themed` closure
  that resolves the whole bundle (fill/border/radius/shadow + the glass optics)
  against the theme at frame-build time, so a role *owns* the paint it sets; for a
  custom look an author uses the individual paint fields instead. The five optic
  fields (`corner_smoothing`, `backdrop_blur`, `specular_edge`, `sheen`,
  `adaptive_tint`) set the style immediately, so they read back from
  `Element::style()` and are unit-tested directly; `surface` is proven by building a
  frame and by an unknown-role rejection test.
- **Preset-or-structured optics.** `specular_edge` / `sheen` / `adaptive_tint` each
  accept the `"glass"` preset string OR an explicit object, modeled as an untagged
  enum (the same shape as `TrackSpec`). The presets resolve to the exact `::glass()`
  recipes the core uses — asserted against `SpecularEdge::glass()` etc. in a
  round-trip test, so the JSON vocabulary can't drift from the builder vocabulary.
- **SHIPPED (the second batch) — the whole visual layer is authorable.** A custom
  translucent `material` background — a dedicated `MaterialSpec { tint, fill_alpha,
  blur, saturation }` field (NOT a third `ColorSpec` variant, which would muddy every
  bg/color/border site) that resolves to `Material::new(...).tint(color)` as the fill
  plus its backdrop blur — and per-corner `corners` / `rounded_full`, paint-time
  `translate` / `rotate` / `skew`, and a foreground `element_filter`
  (externally-tagged `{ "blur" | "brightness" | "saturate": m }`). A `golden.rs` test
  authors the flagship Liquid Glass hero *entirely in JSON* and renders it headlessly
  — the end-to-end "author AND verify the signature visual" proof.
- **Still deferred — discovery.** The Style-grammar section of `describe_vocabulary`
  (it advertises nodes + color roles, not the style-property grammar or the closed
  enum tokens). Authoring works and is golden-proven; making the vocabulary
  self-documenting so an agent can *discover* these props is the remaining gap.

## Headless interaction-state verification (selection, live regions)

The agent-native VERIFY half: an agent should be able to drive input and then assert
the *interaction state* — caret/selection, live-region announcements — straight off
the inspected access tree, no pixels. The core `AccessNode` already carried
`selection: Option<(usize,usize)>` (the editor's caret/selection range) and
`live: bool` (a polite live region), but the agent-facing `AccessNodeDto` dropped both
in `node_to_dto`, so they were unreachable.

- **Exposed in the DTO.** `AccessNodeDto` gains `selection: Option<[usize; 2]>` (the
  `(start, end)` range as a JSON array; collapsed = caret) and `live: bool` (skipped
  when false, like `invalid`), populated in `inspect::node_to_dto`. Both flow through
  `render::interact`'s after-tree, so a `Type` step then a tree read asserts the
  caret — proven in `typed_input_exposes_caret_selection`.
- **Fixed a real a11y gap surfaced by the test.** A `status` with `live: true` only
  drew the visual sonar ring; its container was never marked an aria-live region
  (`Element::live()`), so nothing set `AccessNode.live` for it (only toasts did). The
  `StatusIndicator` `From` impl now calls `Element::live()` when live — the semantic,
  not just the decoration. No pixels change (the ring was already there); the access
  tree now carries the flag, asserted in `live_region_surfaces_in_the_after_tree`.

## Discovering the style vocabulary (describe_vocabulary)

The author → verify arc is only useful if an agent can *discover* what it may write.
`describe_vocabulary` (the "call this first" tool) advertised every node and the color
roles, but nothing about the *style* surface — none of the ~35 style properties
(spacing, sizing, paint, border/radius/shadow, the glass/material vocabulary from the
prior passes, transforms, filter, typography, grid) nor the closed enum tokens
(`surface`, `shadow`, `align`, `justify`, `text_align`, button `variant`, `status`,
drawer `side`, skeleton `kind`, the `"glass"` presets). So the authorable surface was
effectively undiscoverable.

- **Generated from registries, kept honest by coherence tests.** `Vocabulary` gains
  `style: Vec<StyleDoc>` and `enums: Vec<EnumDoc>`, built from a `STYLE_REGISTRY` and
  an `ENUM_REGISTRY` — the same single-source-of-truth discipline as `NODE_REGISTRY`.
  One test authors every advertised style property's example and asserts it parses +
  builds (`every_advertised_style_prop_parses`); another authors every style-key enum
  value (`every_style_enum_value_parses`), so the advertised grammar can never drift
  from what the parser accepts. The MCP `describe_vocabulary` tool returns it all, so
  the discovery surface updates for free. This closes the author → verify → discover
  loop: an agent can now find everything the prior passes made authorable.
