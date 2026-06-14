# Changelog

## 0.19.0 — 2026-06-14

Surface materials: one typed primitive per elevation role, so every elevated
surface in the kit derives its radius, fill, border, shadow, and highlight from
a single table instead of re-typing the recipe at each call site.

### Added

- **`Surface`** (`Card`, `Raised`, `Popover`, `Menu`, `Modal`, `Thumb`,
  `Tooltip`) — a semantic material role that bundles corner radius + fill role +
  border role + shadow token + optional top-highlight into a `SurfaceBundle`,
  resolved against the theme. `Element::surface(role)` (deferred via `.themed`),
  `Theme::surface_style(role)` (theme in scope), and the low-level
  `SurfaceBundle::apply(theme, base)`. Floating roles carry radius and shadow
  depth ≥ resting roles by construction (locked by an ordering-invariant test).
- Seven kit widgets (card, menu/popover, select listbox, modal, tooltip, toast,
  slider thumb) now derive their elevated look from the bundle.

### Changed

- Floating surfaces (menu/popover/select/toast) unify on the card's 14px radius
  so "every floating thing matches." `select_open` and `toast_stack` goldens
  regenerated; all other widget and Look goldens are byte-identical.

### Decided

- See ARCHITECTURE.md "0.19: surface / material bundle" — the standalone
  role-enum + resolver, the floating ≥ resting invariant, the `Thumb`/`Tooltip`
  exemptions, `#[non_exhaustive]` on the growable axes (for 0.22's glass fill),
  and dropping the convention-breaking `Style::surface`.

## 0.18.0 — 2026-06-14

Themed, OKLCH-interpolated gradient builders: gradients are pre-expanded into
dense stops that ride the OKLCH curve, so a wide-hue ramp stays vivid through
the middle instead of collapsing into a gray dead-zone the way a two-stop sRGB
gradient does.

### Added

- **`oklch_stops(anchors, steps)`** expands `(offset, color)` anchors into
  perceptually-even `GradientStop`s by walking each anchor pair through OKLCH
  (the same shortest-hue, achromatic-aware, gamut-clamped path the transition
  engine animates colors along), then **`linear_gradient(angle_deg, colors)`**
  and **`radial_gradient(center, radius, colors)`** build a `Paint` from
  evenly-spaced token colors expanded with `GRADIENT_STEPS`. Anchors are sorted
  and endpoints preserved exactly.
- **`GRADIENT_STEPS`** (16): the calibrated sub-segments-per-anchor-pair
  default, tuned so a full hue-arc ramp shows no banding once vello resamples
  the stops into its ~512-texel sRGB LUT.
- **`Theme::accent_gradient(angle_deg)`**: the brand accent ramp (A7 → A10) as
  a one-call smooth OKLCH linear gradient.
- The **painting specimen** and the editorial **poster** field now build their
  gradients through the new API (the specimen's accent linear via
  `accent_gradient`); both stay token-sourced.

### Decided

- See ARCHITECTURE.md "0.18: themed OKLCH gradient builder" — pre-expansion in
  core (vello 0.9 ignores `peniko`'s `interpolation_cs`, so the only perceptual
  path is dense stops), chosen over a new `Paint` variant for
  renderer-independence and testability; `GRADIENT_STEPS = 16` as a calibrated
  default; and the dedicated A/B eyeball golden.

## 0.17.0 — 2026-06-14

Balanced and pretty text wrapping: headings break into even lines and
paragraphs stop stranding a lone last word — CSS `text-wrap: balance / pretty`
as a typed mode, built on top of parley's greedy line-breaker.

### Added

- **`TextWrap::{Normal, Balance, Pretty}`** with `.balance()` / `.pretty()` /
  `.text_wrap(TextWrap)` builders on `Style` and `Element`. `Balance` binary-
  searches the narrowest wrap width that preserves the greedy line count (even
  lines); `Pretty` nudges the width down to pull a second word onto an orphaned
  last line (best-effort, never adds a line). `Normal` is the default and costs
  nothing. Refinement re-breaks an already-shaped layout (no glyph re-shaping),
  is keyed into the layout cache, and reports its wrap width so measure and
  paint reproduce the same break.
- The **markdown** widget balances headings automatically (the no-links path).

### Decided

- See ARCHITECTURE.md "0.17: balanced and pretty text wrapping" — the re-break
  (not re-shape) approach, the measure/paint fixpoint via `layout_max_advance`,
  the `TextWrap` naming (vs flex `.wrap()`), and pretty as documented
  best-effort.

## 0.16.0 — 2026-06-14

Richer OpenType typography: the single `tabular_nums` bool grows into a typed
`FontFeatures` set covering figure shape, figure spacing, small caps,
ligatures, and fractions.

### Added

- **`FontFeatures`** (with `FigureStyle` and `NumericSpacing` axes) on
  `TextStyle.features`, and the builders `proportional_nums`, `oldstyle_nums`,
  `lining_nums`, `small_caps`, `ligatures(bool)`, and `fractions` on both
  `Style` and `Element` (alongside the unchanged `tabular`). Figure shape
  (`onum`/`lnum`) and figure spacing (`pnum`/`tnum`) are orthogonal and
  compose; small caps (`smcp`), ligatures (`liga`), and fractions (`frac`) are
  independent toggles. All flow into the parley `font-feature-settings` string
  through a single source of truth and are part of the layout cache key.
- **`font_feature_specimen()`** (kit): a showcase of every feature, shown side
  by side against the font's default.

### Changed

- `TextStyle.tabular_nums: bool` is replaced by `TextStyle.features:
  FontFeatures`. `.tabular()` keeps identical behavior (`"tnum" 1`), so every
  existing golden is byte-identical.

### Fixed

- Every font feature now participates in the layout cache key. (The prior
  `tabular_nums`-only key would have cached away any new feature flag — caught
  by the new per-axis `LayoutKey` regression tests, written first to fail.)

### Decided

- See ARCHITECTURE.md "0.16: richer font features" — the feature support is
  font-dependent, so the golden splits figure-shape/small-caps onto the Serif
  role (Playfair) and tabular↔proportional onto Sans (Inter).

## 0.15.0 — 2026-06-14

The reading measure: a `ch`-based prose column, the single biggest readability
lever, expressed as a typed primitive.

### Added

- **`Length::Ch(f32)` + `Style::measure(chars)`** (with `w_ch`/`min_w_ch`/
  `max_w_ch`): a reading-column cap in CSS `ch` units — 1ch is the advance of
  `'0'` in the element's own resolved text style. Resolved to pixels during
  layout (taffy has no font context), guarded so only `ch`-using elements pay
  the metric lookup. `MEASURE_CH = 52` is the default, calibrated so a
  proportional body face renders ~66 characters per line (`'0'` is wider than
  the average glyph, so `ch` < characters).
- **`reading_column()`** (kit): a prose column pre-capped at `MEASURE_CH`.
- The **markdown** widget caps its prose at the default measure, and the
  **`ai_chat`** showcase now uses the measure (matched to its 20px serif prose)
  instead of a hard-coded 768px.

### Decided

- See ARCHITECTURE.md "0.15: the reading measure" — the `ch`-resolution timing
  (in `frame::build`, before taffy), the `ch` ≠ characters calibration (52, not
  66; found and fixed in review), the per-container vs per-block cap, and the
  code-block follow-up.

## 0.14.0 — 2026-06-13

The showcase release: an editor-chrome tier and canvas substrate, an upgraded
chart palette, and an AI-chat showcase.

### Added

- **Editor-chrome token tier** (`ChromeText`, `ChromeElevation`): Figma's dense
  panel anatomy — 11–14px text with per-size tracking, the 32px control row
  (reusing `ControlSize::Sm`), and the floating two-drop + 0.5px hairline-ring
  elevation (popover / modal / thumb), flat black in contrast to the themed,
  hue-tinted `ShadowToken`.
- **Canvas substrate** (`fenestra_core::canvas`): tldraw's camera/zoom/snap math
  — `ZOOMS` (5%–800%), `zoom_in`/`zoom_out`, a `Camera` with eased zoom-to-fit
  (`EASE_IN_OUT_CUBIC`, `CAMERA_MS` = 320 ms), `world_len`/`screen_len`
  zoom-compensated strokes, and `snap` (8px). The substrate for building a
  Figma-class tool. The `editor_panel` demo shows the chrome tier in one panel.
- **Chart palette** (`fenestra_charts::ChartPalette`): Observable10 categorical
  (light verbatim, dark *re-picked* — lifted in lightness and eased in chroma
  in OKLCH, never inverted), OKLCH `sequential` and `diverging` generators, and
  a `multi_line_chart`.
- **AI-chat showcase** (`ai_chat`): a 768px reading column with turn asymmetry
  (the human in an accent bubble, the assistant in flat serif prose), a
  streaming caret, and a thinking shimmer — wearing the warm-editorial look.
- **Color primitives**: `oklch` (gamut-mapped OKLCH → sRGB) and its inverse
  `oklch_of` are now public — the framework's color constructor, for custom
  palettes and Looks.

### Decided

- See ARCHITECTURE.md "0.14: kit and showcase (Tier 4)", and the book's
  "Thinking in fenestra" essay on why fenestra's styling is beautiful by
  construction — a typed, generated, validated, golden-locked design system,
  the inverse of CSS's permissive solvent.

## 0.13.0 — 2026-06-13

Derivation as product: the whole palette from three inputs, and two new Looks
that prove the range.

### Added

- **`Theme::derive(base, accent, contrast, mode)`** — the entire palette from
  three inputs (Linear's model on fenestra's OKLCH scales): a neutral
  `BaseField` (hue + chroma-from-gray), an accent hue, and a `Contrast` level
  (`Low`/`Standard`/`High`) that scales every step's lightness distance from the
  background. `from_accent` and `duotone` are special cases — `derive` at
  `Standard` reproduces them byte-for-byte — and every level still clears the
  APCA floors. Carried in `ThemeSpec` as a `derive` recipe (precedence
  derive > duotone > accent_hue).
- **`RadiusScale::from_base(f32)`** — a corner-radius family (`sm`/`md`/`lg`/`xl`
  at 0.6 / 1.0 / 1.4 / 2.0 ×) from one knob; the default base (10) reproduces
  `R_SM`…`R_XL`.
- **Two new Looks** (`fenestra-looks`): **warm-editorial** — a derived
  cream-and-terracotta paper field with Playfair serif prose under sans chrome;
  **playful** — a soft pastel canvas with a saturated magenta accent for
  whiteboard-class tools. Both are golden-locked and APCA-asserted in both
  modes. `all()` now returns five voices.

### Changed

- `duotone` is now a thin wrapper over the shared neutral-field path that
  `derive` uses (identical output).

### Decided

- See ARCHITECTURE.md "0.13: derivation as product (Tier 3)" — the contrast
  model (distance-from-background), why the radius knob is a standalone family
  rather than a per-theme field the kit reads, and the playful Look's deferred
  hand-drawn typeface.

## 0.12.0 — 2026-06-13

The interaction release: a uniform state-layer engine, Material 3 motion
tokens, and a shadcn-grade focus ring.

### Added

- **Uniform state layer** (`Element::state_layer`): a translucent veil of a
  control's *content* color, composited over its container on hover (8%),
  keyboard focus / press (12%), and drag (16%) — Material's recipe, one call,
  replacing per-widget hover-color swaps. Tokens in `StateLayer` /
  `STATE_LAYER`; the compositing is exact source-over baked into the fill so
  it rides the color transition.
- **Press feedback** (`Element::press_scale`, `Style::scale`, `PRESS_SCALE` =
  0.97): a tactile shrink while pressed, applied as a paint-time transform
  about the control's center — it animates and never disturbs layout or
  hit-testing.
- **Motion families**: `EASE_DECELERATE` (entrances) and `EASE_ACCELERATE`
  (exits) alongside `EASE_STANDARD`; `MotionDuration::Micro` (100 ms) and
  `MotionDuration::exit_ms` (exits ~25% quicker than the matching entrance).
- **Focus-ring spec** (shadcn v4): a keyboard-focused control swaps its border
  to the ring color and draws a soft 3px halo at 50% alpha flush outside it;
  `Element::invalid` recolors the ring to the danger hue. `FocusRing` /
  `FOCUS_RING` reworked to width 3 / offset 0 / alpha 0.5.
- **Control sizes**: `ControlSize` now spans a shared 24/32/36/40 height grid
  (`Xs`/`Sm`/`Md`/`Lg`) and resolves to a `ControlMetrics` bundle
  (height, padding, gap, font, icon) so rows of mixed controls align.

### Changed

- Neutral interactive surfaces (Ghost/Secondary buttons, menu/select/tree/
  date-picker/table/toast rows) style their states through the state layer
  instead of swapping to `element`. Solid brand buttons (Primary/Danger) keep
  their gamut-mapped ramp-step hover/press — a white veil would wash the
  accent out.
- Keyboard-driven state changes snap (no fade): a keyboard-focused control
  shows its ring and state layer instantly, since keyboard users move faster
  than a fade can keep up.
- Buttons gain press-scale and size-driven gap/icon metrics; `ControlSize::Sm`
  is now 32px (was 28) and `Lg` 40px (was 44); the default `Md` stays 36px.
- Disabled neutral controls fade their container through the state layer and
  dim their content to `text_disabled` (Material's 12%-container / 38%-content
  split, expressed with fenestra's tokens).

### Decided

- See ARCHITECTURE.md "0.12: the interaction release (Tier 2)" for the
  state-layer-vs-ramp split, the tree-model disabled-content decision, and the
  keyboard-snap rule.

## 0.11.0 — 2026-06-13

The craft release: structural sophistication on top of the OKLCH ramps.

### Added

- **Semantic element states**: `Theme::element` / `element_hover` /
  `element_active` (neutral steps 3/4/5, Radix's UI-element-fill model),
  plus `accent_active` and `StatusColors::solid_active` — pressed states
  one OKLCH-lightness notch below hover, mode-invariant. Kit interaction
  styling is now scale arithmetic rather than hand-picked steps.
- **Alpha twins**: `Theme::neutral_alpha` / `accent_alpha` — translucent
  twins of each ramp (the smallest alpha that composites over `bg` back
  to the solid step) for overlays and state layers that must read over
  any surface, not just `bg`.
- **APCA-validated themes**: `apca::lc` / `lc_abs` / `meets` (APCA-W3
  `0.98G-4g` lightness contrast) and `Theme::validate_contrast`, which
  checks every text/background role pair against role-tiered Lc floors.
  Every built-in theme and shipped Look is asserted legible in headless
  tests — a guarantee no CSS framework can make.
- **Layered, hued elevation**: shadows take the surface hue at low chroma
  instead of flat black (`Theme::shadow_tint`); a new `ShadowToken::Xl`
  three-layer overlay shadow (modals); a 1px inset top highlight on solid
  buttons (`Style::highlight_top` / `Element::highlight_top`).
- **Typography from a formula**: letter spacing follows Inter's
  dynamic-metrics tracking curve at the actual font size (`tracking_em`),
  and tabular figures are one call — `Style::tabular` / `Element::tabular`
  — applied to numeric kit widgets (stat cards, tables, chart labels).

### Changed

- Pressed Primary and Danger buttons use the new `accent_active` /
  `danger.solid_active` (a deeper, richer press) instead of reusing a
  text-role color as a fill; selected rows in tree/table use the named
  `accent_bg`.
- Modal overlays use the deeper `ShadowToken::Xl`.

### Decided

- Line height stays the hand-tuned per-size scale: it already curves
  smaller-looser / larger-tighter more aggressively than a naive linear
  fit, and `Base = 24px` is the line box virtualization is pinned to.
  APCA validation covers text pairs only (it scores text, not borders).
  See ARCHITECTURE.md.

## 0.10.0 — 2026-06-13

Performance honesty.

### Added

- **Variable-height virtualization**: `virtual_list_variable(count,
  estimated_height, builder)` (kit) and `.virtual_rows_variable(..)`
  (core) — rows place from a prefix-sum height index seeded with an
  estimate; measured heights feed back so offsets, scrollbar geometry,
  and the total self-correct as the user scrolls.
- **Clean-frame memoization**: the windowed runner re-presents the
  cached scene when nothing changed since the last paint (expose,
  un-occlude, timer redraws) — idle apps cost ~zero CPU. Animation,
  input, focus, resize, scale, and resume all invalidate; headless
  paths never memoize, so goldens always exercise the full pipeline.
- **Performance chapter** in the book; `BENCHMARKS.md` refreshed at
  0.10.0.

### Fixed

- A programmatic `scroll_to` far past the end of a fixed-height
  virtual list realizes the last page immediately instead of an empty
  window for one frame.

### Decided

- Subtree scene caching is deferred until per-subtree resolve purity
  can be tracked (stale pixels are the one failure mode a
  verification-first framework cannot ship); vello sparse-strips
  (`vello_cpu`/`vello_hybrid`) assessed as watch-don't-move with
  explicit migrate-when conditions. Both in ARCHITECTURE.md.


## 0.9.0 — 2026-06-12

Text grows up; looks arrive.

### Added

- **Selectable static text**: `.selectable()` — drag/word/line
  selection with Cmd/Ctrl+C, browser semantics, highlight painted in
  the selection color, range exposed headlessly.
- **`fenestra-markdown`** (new crate): CommonMark as native elements —
  headings, inline styling via rich-text spans, code panels, lists,
  blockquotes, rules, clickable links emitting their URL.
- **`fenestra-looks`** (new crate): packaged design languages —
  product, editorial (Playfair/duotone), terminal (JetBrains Mono) —
  golden-locked identities, OFL typefaces vendored. Registered faces
  now win for every family role.
- **Motion**: `Transition::spring()` (closed-form damped response,
  geometry overshoots, colors clamp), `.enter(transition)` fade-ins on
  first appearance, theme/color crossfades proven end to end.
- **`date_picker`** (#6), select multi-char type-ahead via the new
  `on_type_ahead` core handler (#5), tooltip flip-above (#4),
  `badge_dot` (#8), `progress_indeterminate` (#3), toast enter
  animation (#2), 14 more Lucide icons (#1).
- **Emoji status resolved** (#11): color emoji render through system
  fonts (proof test); VS16 caveat documented.


## 0.8.0 — 2026-06-12

Trusted, formally.

### Added

- **cargo-deny** in CI: license allowlist, registry pinning, ban rules,
  yanked-crate denial (alongside the existing cargo-audit job).
- **Fuzzing**: three libFuzzer targets over the public API (theme-file
  parsing, layout/paint totality on arbitrary trees, the text-input
  pipeline), weekly and on demand.
- **MSRV policy**: `rust-version = "1.88"` declared everywhere and
  enforced by a CI job; minor releases may raise it (recorded here).
- **Perf regression gates**: release-mode ceiling tests on the macOS
  reference runner (counter/dashboard/virtual-100k scale).
- **Coverage floor**: fenestra-core line coverage ≥ 45% enforced in CI
  (measured baseline 47.28%; ratchets up).
- **Release provenance attestations**: .crate files attached to GitHub
  releases with build-provenance attestations
  (`gh attestation verify`).
- **SECURITY.md** + GitHub private vulnerability reporting enabled.
- Book: **Trust and security** page tying it together.


## 0.7.0 — 2026-06-12

Ecosystem seams.

### Added

- **Embedded mode**: `Embedded` runs a fenestra app inside a
  caller-owned wgpu world — your event loop, device, surface, and
  frame pacing. Renders on your device, composites onto any target
  view with premultiplied-alpha blending (transparent clear = floating
  UI layer), `EventResponse {consumed, repaint}` arbitration,
  `texture_view()` for custom compositing, `frame()` for semantic
  queries. wgpu/winit/vello re-exported for version-matched
  integration. `examples/embedded.rs` is a full host app.
- **`fenestra-charts`** (new crate): sparkline, line chart, bar chart —
  and the reference third-party widget crate (fenestra-core only,
  theme tokens, golden-tested, panic-free on hostile data).
- **Widget-crate guide**: a book chapter with the authoring contract.
- **Theme files**: `ThemeSpec` recipes ⇄ JSON (`{"mode": "dark",
  "accent_hue": 265.0}`), resolving through the stock builders; typos
  fail loudly.
- **Kit v2**: `split_pane` (draggable divider, app-owned fraction),
  `tree_view` (disclosure + selection + arrow-key collapse/expand),
  `command_palette` (modal filter launcher, Enter runs first match),
  `data_table` (sortable headers, row selection — Elm-pure, app sorts).
- **Per-window themes**: `App::theme_for(key)`, consulted by the
  windowed runner.


## 0.6.0 — 2026-06-12

Text is real.

### Added

- **Selection depth**: double-click selects the word, triple-click the
  line, shift-click extends from the caret (pointer modifiers now flow
  through the new `InputEvent::Modifiers`; shift-click was previously
  dead code), drag-select verified under follow-scroll.
- **Undo/redo**: Cmd/Ctrl+Z, Shift+Cmd/Ctrl+Z, Ctrl+Y — per-field
  QUndoStack semantics: coalesced typing/deleting runs; boundaries on
  caret moves, clicks, paste, cut, and programmatic value changes;
  bounded history; selection restored; emitted through `on_input` so
  the app stays the source of truth.
- **Rich text**: `rich_text([span("…").weight(..).color(..)
  .size_px(..).family(..).italic(), …])` — one wrapped paragraph with
  ranged styles, per-span paint brushes, single accessible label.
- **Bidi/RTL**: mixed-direction shaping verified total on embedded
  fonts; RTL system-font fallback pixel-proven (macOS-gated, like the
  CJK proof).
- **A11y state**: `.live()` live regions (AccessKit `Live::Polite`;
  toasts mark themselves), and text inputs expose caret/selection
  byte ranges headlessly via `AccessNode::selection`.
- Harness verbs `triple_click` / `shift_click` (+ scenario verbs),
  `SyntheticEvent::Modifiers`.

### Notes

- The full screen-reader text protocol (per-run inline text boxes)
  remains out of scope; field-level value/caret/selection are exposed.


## 0.5.0 — 2026-06-12

The verification release: nobody else combines deterministic pixels on
the production renderer, semantic queries over the accessibility tree,
and Elm message assertions in one harness. Informed by a four-strand
survey of SwiftUI/Compose/Flutter, Testing Library/Playwright/
Storybook, Qt/WPF/Avalonia/GTK, and egui/ImGui/iced/Slint/GPUI — see
the book's new Influences section.

### Added

- **Semantic queries**: `by::role(..).name(..)`, `by::label`,
  `by::value`, `by::id` (+ `_contains` forms) over the accessibility
  tree; strict `get` (zero or several matches panic with the tree in
  the message), `query` (Option), `get_all`, machine-facing `try_get`.
- **`Harness`** — drive an app headlessly at three assertion levels:
  structure (queries), behavior (`take_messages()` — every message the
  UI emitted), pixels (`render()`, only when asked). Verbs: click,
  right/double click, hover, type_text, key, tab, focus, drag,
  drop_file, wheel; explicit `pump(ms)` clock. `render_app` is now a
  thin wrapper over it (one dispatch path; 0.4 goldens unchanged).
- **Multi-window headless**: the harness reconciles `App::windows()`;
  `activate_window(key)` scopes verbs, `render_window(key)` renders any
  window at its own size.
- **JSON scenarios**: `run_scenario` — semantic targets, verbs,
  asserts (exists/absent/count/value/windows), named PNG shots;
  unknown fields are loud parse errors; failures carry the step index
  and the accessibility tree.
- **Golden failure artifacts**: `<name>.diff.png` (offending pixels in
  red over the dimmed golden) and `<name>.side.png`
  (golden | actual | diff) beside the existing `.actual.png`; panic
  message carries counts, budget, and the worst pixel. Stale artifacts
  clean up on pass.
- **Headless inspector**: `Frame::debug_tree()` (kind, #key, rect,
  flags, semantics, `src=file:line` via `#[track_caller]` — zero proc
  macros) and `Frame::access_yaml()` (verbatim Playwright aria-snapshot
  grammar). `AccessNode` now carries the user key.
- **Heterogeneous children**: `.children((text(..), button(..)))` —
  tuples up to 12 mix kit builders and elements with no
  `Element::from`; iterator form unchanged.
- **Property tests** (dev-only proptest): layout/paint totality over
  arbitrary trees, Tab-order permutation, per-frame id uniqueness.
- Book: rewritten verification chapter, new **Determinism contract**
  page, **Influences** section; AGENTS.md teaches the harness as the
  primary loop.

### Changed

- `App` is blanket-implemented for `&mut A` (harnesses can borrow an
  app the caller still owns).

## 0.4.0 — 2026-06-12

Apps feel native.

### Added

- **Multi-window** (#10): `App::windows()` declares the open set as
  `WindowDesc {key, title, size, on_close}` — new keys open, removed
  keys close, titles live-update, exactly like modal state.
  `App::view_for(key)` routes per-window views; each window keeps its
  own focus/scroll/editor state and accessibility tree while app state
  stays shared. The OS close button emits `on_close` (interceptable).
  Native only; defaults preserve single-window apps. `examples/windows.rs`.
- **Right-click and double-click**: `.on_right_click(msg)`,
  `.on_double_click(msg)` (0.4 s window; single clicks still fire).
- **Drag-and-drop**: OS file drops via `.on_file_drop(|path| ..)`
  (pointer-position hit with tree-order fallback); internal drags via
  `.drag_source("payload")` + `.on_drop(|payload| ..)`.
- **Programmatic focus**: `.autofocus()` focuses an element when it
  newly appears (dialogs, search fields) without stealing focus on
  rebuilds.
- **Scrolling depth**: `.stick_to_bottom()` (chat-log pin, released by
  scrolling up), `FrameState::scroll_to(id, offset)` (absolute;
  `f32::MAX` = bottom), and keyboard paging — PageUp/PageDown/Home/End
  target the scroll container nearest the focused element when the
  element itself doesn't consume the key.
- **IME candidate-window positioning**: the runner anchors the OS
  candidate window to the caret (every window), derived from paint with
  no extra layout pass.
- **Kit menus**: `menu` (styled action panel), `dropdown_menu`,
  `context_menu` (pins at the right-click position via
  `Overlay::context()` / `OverlayPlacement::Pointer`), `popover`, and
  `combobox` (filtering text input + pickable listbox, Elm-pure).
- **Window polish** (#7, #9): `WindowOptions::{with_min_size,
  with_resizable, maximized, fullscreen, with_icon, with_font}` — the
  last registers custom faces for windowed apps (the poster example now
  opens in a window by default).

### Changed

- `fenestra-core` internals: `input::paint` returns the caret rect;
  `dispatch` handles `RightDown`/`RightUp`/`FileDrop` and records drag
  payloads. Additive for users of the public API.

## 0.3.0 — 2026-06-12

### Added

- **WebAssembly/WebGPU support**: the windowed runner compiles to
  wasm32-unknown-unknown (async surface setup, browser-paced frames,
  canvas auto-append); the interactive demo and the mdBook deploy to
  GitHub Pages on every push.
- **Virtualized lists**: `Element::virtual_rows` / kit `virtual_list` —
  only the scrolled-into-view window materializes (100k rows ≈ 0.09 ms a
  frame); handlers on materialized rows dispatch like any other element.
- **Editorial design language**: `Fonts::register` loads custom faces
  under `FamilyRole::Display`/`Serif`; text gains `.size_px`, `.tracking`,
  `.leading`, `.family`; `Theme::duotone` builds atmospheric fields;
  `render_element_with` renders through caller-provided fonts. Proven by
  the golden-tested poster example (Playfair Display, OFL).
- Windows CI (DX12 WARP), a benchmarks page with measured numbers, the
  mdBook guide, AGENTS.md + llms.txt, CONTRIBUTING + issue templates, an
  rfd file-dialog example, and a pixel test proving CJK fallback through
  system fonts.

### Changed

- `click_msg_of` now takes the frame and state, so accessibility clicks
  resolve virtual rows.
- Headless modules (`render_element`, `render_app`, testing, clipboard,
  AccessKit adapter) are native-only; the wasm build exposes the windowed
  runner.


## 0.2.0 — 2026-06-11

### Added

- `Element::map`: convert every message a subtree emits, so components
  written around their own message type compose into any parent.
- Command channel: `App::init` receives a cloneable, thread-safe
  `Proxy<Msg>`; the windowed runner wakes and repaints on proxied
  messages, and headless `render_app` drains them deterministically.
- Image element: `image_rgba8(width, height, pixels)`, stretched to the
  element rect and clipped to the corner radius.
- Multiline text area: `raw_text_area` / kit `text_area` — wrapping,
  Enter-as-newline, line-wise arrow movement, auto-growing height.
- Toasts: kit `toast_stack` with per-toast dismiss, pinned via the new
  `OverlayPlacement::TopRight` / `Overlay::toasts()`.
- Lucide icon subset: 24 icons vendored from lucide-static 1.17.0 (ISC),
  `icons::lucide::*` plus `lucide::all()`.
- Keyframe timelines: `Keyframes::new(..).stop(..)` looping style
  animation sampled from the frame clock; reduced motion pins the first
  stop.
- AccessKit: `Semantics` roles on every interactive kit widget,
  `.semantics()`/`.label()` builders, headless `Frame::access_tree()`,
  and an accesskit_winit adapter in the windowed runner (tree pushed per
  frame; Click/Focus action requests honored).

### Fixed

- `.opacity` now wraps the element's own shadows, fill, and border (CSS
  group semantics); previously only children faded.
- `select` no longer emits an out-of-range index on Home with empty
  options.
- Control characters arriving as `Key::Char` are filtered from text
  inputs, matching the commit and paste paths.
- IME preedit cursor offsets are clamped before reaching parley.
- Scroll state is garbage-collected like other retained state.
- Headless render sizes clamp to the device-supported range instead of
  panicking on zero or oversized requests.
- Lost wgpu surfaces rebuild instead of panicking.
- `Ramp::step` clamps out-of-range steps instead of panicking.

### Security / CI

- GitHub Actions pinned to commit SHAs; workflow token read-only; weekly
  `cargo audit` job; `unsafe_code = "forbid"` workspace-wide.

## 0.1.0 — 2026-06-10

Initial release: milestones M0–M7 — element IR and theme generation
(OKLCH ramps from one accent hue), parley text with embedded Inter,
taffy flexbox/grid layout with scrolling, interactivity (hover/active/
focus, transitions), single-line text input with clipboard and IME,
overlays (menus, tooltips, modals), the themed widget kit, headless
PNG rendering with golden tests, and the dashboard example.
