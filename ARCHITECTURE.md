# fenestra architecture

Decisions are recorded here as they are made, milestone by milestone.

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
