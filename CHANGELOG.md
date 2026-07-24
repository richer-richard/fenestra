# Changelog

## Unreleased

The two crash classes left open by the 2026-07-24 adversarial review are
closed: runaway-deep element trees and GPU-environment failures now fail as
pointed, catchable errors instead of process aborts and panics.

A follow-up solo ultra-review of this branch hardened the new surfaces
further (all regression-tested):

- **A2UI correctness:** `Icon.name` is dynamic (bindings resolve — the
  official task-card example now resolves `/priorityIcon` instead of
  stringifying the binding); one canonical path joiner backs reads,
  template item scopes, and write paths (absolute template paths inside a
  collection scope no longer corrupt into `//` pointers, and values always
  read back from where their two-way binding writes); action messages
  carry `source_id` so hosts can populate the client→server
  `sourceComponentId`; every literal-valued input control (CheckBox,
  Slider, ChoicePicker) stays interactive through local edits; a
  ChoicePicker selection accepts a bound string as well as a list.
- **A2UI robustness:** unknown message types skip with a per-surface note
  instead of hard-failing the stream (the envelope no longer denies
  unknown fields); a known component with malformed fields degrades to a
  placeholder + note while its siblings render; data-model writes support
  RFC 6901 `-` append, and a write that cannot apply records a note;
  present-but-mistyped bool/number bindings note the type error.
- **Effect layer:** one shared `fenestra_core::apply_cmd` now executes
  effects for the runner, `Embedded`, and the harness — semantics can no
  longer fork — and follow-up commands run FIFO (sibling message chains
  no longer invert).
- **Emitter/CLI:** out-of-range percent sizes warn on emit (they clamp at
  parse time, so silence would break the zero-warnings ⇒ identical
  re-render contract); `fenestra a2ui` gained `--theme`; `render_a2ui`
  maps the catalog once instead of twice.

### Added

- **`agent_dashboard`: the dogfood flagship example.** A live dashboard
  over a Claude Code session log: the agent's tool-call feed
  (virtualized), output-token sparkline, per-tool bar chart, and summary
  stats, tailing the newest session every two seconds while it runs. One
  app exercises the new effect layer (`Cmd::task` parse off-thread,
  `Sub::every` live tail, `init_cmd`), the native menu bar, fenestra-charts,
  and virtualization — and `-- shot` renders both themes headlessly from a
  bundled fixture (the README hero pipeline).
- **Declarative native menus.** `App::menu()` returns a `MenuSpec` —
  message-emitting items, accelerators, separators, nested submenus,
  enabled flags — reconciled after every update by structural fingerprint
  and dispatched through the ordinary update path. Attaches natively on
  macOS (muda's safe path, app menu + Quit provided); Windows/Linux keep
  the kit's in-window `menubar` (the honest platform story is in
  ARCHITECTURE.md). New `native_menu` example. Tray icons are designed
  and deferred (same reconcile pattern; see ARCHITECTURE.md).
- **`fenestra-a2ui`: a native Rust renderer for A2UI v0.9** — the open
  Agent-to-UI standard (a2ui.org) where agents send declarative JSON
  surfaces and clients render them natively. Full basic-catalog coverage
  (18 components), the protocol's message fold, JSON Pointer bindings with
  template scopes, two-way input binding, action events (with
  `sendDataModel`), and the client-side function library — with a
  fidelity-or-report contract (placeholders + notes for network media,
  unknown components, and the documented gaps) and hostile-input
  hardening (exact reference-cycle detection, template caps). Verified
  against eleven vendored official gallery examples, two pinned goldens,
  and behavioral tests. Integrated end to end: `fenestra a2ui <stream>`
  in the CLI and a `render_a2ui` MCP tool returning the same typed access
  tree as `render_ui` — fenestra is the A2UI client whose output an agent
  can verify headlessly, deterministically, in CI.
- **Hi-DPI headless rendering.** `render_element_scaled` /
  `try_render_element_scaled` render at a device scale factor through the
  same two-pass pipeline — text rasterizes at physical resolution and
  frosted glass keeps its real backdrop blur (scaled renders used to fall
  back to the flat tint; `render_element_over` and fenestra-motion's
  sampling inherit the upgrade). Scale-1.0 output stays byte-identical to
  every existing golden; a 2× golden pins retina rendering. Agents can now
  see retina-only regressions headlessly.
- **The effect layer: fenestra finally has an async story.**
  `App::update_with` optionally returns a `Cmd<Msg>` — `Cmd::task`
  (blocking work on a worker thread), `Cmd::future` (runtime-agnostic
  futures), `Cmd::msg`, `Cmd::batch`, and `Cmd::map` for Elm-style
  component composition — executed by every runner (windowed, web,
  `Embedded`) and delivered back as messages. `App::subscriptions`
  declares recurring effects (`Sub::every` timers) reconciled by key after
  every update, exactly like secondary windows; `App::init_cmd` covers
  startup loads. Fully non-breaking: existing apps implement `update` and
  never see any of it. The differentiated half: **effects are
  deterministic under the test harness** — `Harness::run_effects()`
  resolves queued tasks/futures synchronously in FIFO order, and
  `Sub::every` ticks fire on the harness's explicit clock
  (`pump(1000ms)` with a 300ms timer delivers exactly the ticks at
  300/600/900), so effectful apps stay pixel- and message-verifiable in
  CI with no races and no wall clock. New `http_fetch` example shows the
  blessed pattern (blocking `ureq` GET inside `Cmd::task`); the `clock`
  example now uses a subscription instead of a hand-rolled thread.
- **Charts and markdown are now authorable in fenestra/1.** Four new nodes:
  `sparkline` (inline trend line), `line_chart` (single- or multi-series
  with optional axes, markers, labels, titles), `bar_chart` (optional axes
  and printed values), and `markdown` (CommonMark + GFM as native
  elements; `on_link` fires one inert intent). Hostile inputs degrade with
  path-pointed errors: series truncate at 10k points, multi-series at the
  10-color palette, bars at the item cap, tick requests clamp. The JSON
  Schema and vocabulary gained the nodes automatically (the drift guard
  enforces all three stay in lockstep), and a golden pins the rendering.
- **The JSON grammar reaches layout parity with the builders.** `w`/`h`/
  `min_*`/`max_*` accept `"NN%"` and `"full"` beside px numbers, and new
  `grow` (bool or factor), `shrink`, `wrap`, and `scroll`
  (`"x"|"y"|"both"|"hidden"`) style fields cover the flex vocabulary real
  UIs are built from. (`Element` gained matching `grow_by`/`shrink`
  builders.)
- **The Element→JSON emitter closes the round-trip** (deferred #9):
  `fenestra_describe::emit::{emit_description, emit_element}` serialize a
  builder-authored tree back to fenestra/1. The contract is
  fidelity-or-report: for the JSON-expressible subset (layout, text,
  literal styles, click intents) the emitted document re-parses and
  renders byte-identical pixels with zero warnings — pinned by round-trip
  tests — and every feature with no JSON projection (theme/hover style
  closures, vector paths, virtual rows, exotic style fields) is reported
  path-pointed, never dropped silently. Colors emit as concrete OKLCH;
  `Element` gained read accessors (`kind`, `children_ref`, `key`,
  `click_msg`, `access_label`, `is_stack`, `has_dynamic_style`,
  `has_generated_content`) for tooling.
- **`FENESTRA_CPU`: vello's CPU compute pipeline as a first-class switch**
  (`fenestra_shell::CPU_ENV`). The renderer's compute stages run as native
  Rust, leaving the adapter only upload/copy work — the key to software
  adapters whose compute rasterization crashes (Windows WARP) or diverges.
  Honored by headless rendering and the live window alike; adapter
  *selection* was already env-steerable end to end via wgpu's
  `WGPU_BACKEND`/`WGPU_ADAPTER_NAME`. On the reference platform the CPU
  pipeline reproduces the Metal goldens within the default 0.2% budget.
  Windows CI gains an informational full-suite render step on WARP under
  this switch — the path to Windows pixels becoming a required gate.
  (vello_cpu itself was evaluated and deferred with evidence — see
  ARCHITECTURE.md: it is a second paint backend today, not a fallback
  flag.)

### Fixed

- The web runner now installs a real clipboard: in-app copy/paste works
  and copy-out reaches the system clipboard via `navigator.clipboard`
  (paste-in from other apps remains the documented gap — see
  ARCHITECTURE.md's web-parity entry, which also settles the AccessKit
  and glass asterisks precisely).
- Markdown task-list checkboxes rendered as tofu in deterministic headless
  output (the U+2610/U+2611 ballot boxes are outside embedded Inter's
  coverage); they now render as `[x]` / `[ ]`.

- **Deep element trees no longer abort the process.** Recursion in
  build/layout/paint used to hit a raw stack overflow (an uncatchable abort)
  a few hundred levels deep — data-dependent for any recursive component
  (file trees, comment threads). The tree contract is now the documented
  `MAX_TREE_DEPTH` (48): `build_frame` pre-scans iteratively (detection
  works at any depth), content generated mid-walk (virtual rows, container
  queries) is checked during the walk, `Element::map` enforces the same cap,
  and the panic message points at the offending builder call site. Dropping
  is depth-unlimited: `Element` child storage now drops iteratively, so even
  a 100k-deep tree unwinds safely. The cap is measured, not estimated: an
  at-cap tree builds within a 2 MiB thread stack (the `std::thread`/tokio
  blocking default) in unoptimized debug builds, pinned by a regression test
  on exactly that stack size.
- **The live window reports GPU failures instead of panicking.** Window
  creation, surface creation, vello renderer construction, mid-run render
  errors, surface-texture validation, and device polls all funnel into new
  `ShellError` variants with actionable messages (what to install in a
  VM/CI); `run_app`/`run_static`/`run_scene` return them. The recoverable
  surface states (lost/outdated/occluded/timeout) keep their in-place
  recovery. On the web, where `run_app` has already returned, fatal shell
  errors log to the browser console instead of throwing an opaque exception.
- **The headless API promises — and now has — an error channel.**
  `try_render_element`, `try_render_element_with`,
  `try_render_element_with_state`, and `Harness::try_new` return
  `ShellError` values; the panicking wrappers delegate to them and carry the
  same actionable message ("install mesa-vulkan-drivers…"). The
  render/verify engine (`fenestra-render`) and the MCP server surface
  GPU-environment failures as typed errors by design, not by
  `spawn_blocking` panic-catch. Regression-tested against a real
  zero-adapter environment.

## 0.40.0 — 2026-07-10

A hardening + correctness pass from an adversarial self-review — agent-reachable DoS
vectors closed, three retained-state/correctness bugs fixed, the headless
verification envelope documented — followed by a community-usefulness batch:
the JSON authoring grammar now reaches the whole kit, `fenestra preview` gives
authors a live-reload window, filmstrips let agents watch motion play, and the
kit gains an OKLCH color picker. Additive; every prior golden is byte-identical.
A subsequent recall-focused code review (`/code-review 17`) found and fixed a
further P0, two P1s, and several lower-severity issues — see Fixed and Security
below.

### Added

- **The fenestra/1 grammar now reaches the whole kit.** A new `image` node
  (base64 PNG through a dependency-free strict RFC 4648 decoder; dimension
  limits enforced *before* pixel allocation against decompression bombs; alt
  text required) and 14 previously code-only widgets become authorable:
  `field`, `split_pane`, `combobox`, `multi_select`, `tag_input`,
  `date_picker`, `tree`, `toast`, `data_table`, `virtual_list`, `popover`,
  `dropdown_menu`, `command_palette`, and `color_picker`. Every handler stays
  an inert intent string or a scalar state write; the three kit capabilities
  that need computed per-event payloads (data_table column resize/reorder,
  keyboard-cursor navigation, calendar grid nav) are deliberately not
  authorable and say so in their node docs. List/table inputs clamp
  (`MAX_LIST_ITEMS` 1000, `MAX_TABLE_COLUMNS` 128), path-pointed, never
  panicking.
- **`fenestra preview <file>`: a live-reload window for authoring `fenestra/1`
  JSON.** Edit and save; the window re-renders (200ms mtime/len poll, no new
  dependency). A parse error never crashes or blanks the window — the last
  description that loaded cleanly keeps rendering under a themed, path-pointed
  error panel, and a broken very-first load shows the panel alone. Runtime
  state survives a reload best-effort (bindings the edit still declares keep
  their values; new ones seed; removed ones drop).
- **`Harness::film` + `fenestra film` + the `film_ui` MCP tool: agents can
  watch motion play.** `film(frames, interval_ms)` captures a render sequence
  across the harness's deterministic clock (call `set_reduced_motion(false)`
  first — determinism comes from the clock, not from suppressing animation);
  `testing::assert_filmstrip_snapshot` locks a captioned strip as a golden
  through the existing snapshot machinery, and the public non-panicking
  `testing::filmstrip_image` composes strips outside tests. The CLI and MCP
  surfaces drive optional steps *first* — with motion already on, so a
  step-triggered transition is still in flight at capture — and report the
  actual frame count / interval / scale used after clamping
  (`MAX_FILM_FRAMES` 64, `MAX_FILM_INTERVAL_MS` 60s), never just the request.
  MCP grows to thirteen tools.
- **OKLCH color picker (`fenestra_kit::color_picker`).** A generated
  lightness×chroma pad, hue and checkerboard-backed alpha strips, a swatch,
  and a forgiving hex/`oklch()` text entry — Elm-pure like every kit widget,
  keyboard-operable on every channel (the 2D pad exposes independent
  Lightness/Chroma focus targets per the RangeSlider two-thumb precedent),
  with an honest gamut-mapped indicator built only on core's existing
  `oklch`/`oklch_of`. Text parsing rejects `nan`/`inf` literals explicitly and
  never commits an unparseable edit. A proptest sweep over the full `f32`
  domain caught a `rem_euclid` hue-wrap boundary bug before it shipped.
- **Screenshot masks on every compare surface.** `match-png` gains a
  repeatable `--mask x,y,w,h` flag and the `match_screenshot` MCP tool an
  optional `masks` array — validated (finite, non-negative) before any disk
  I/O; the scenario path already threaded masks, now the ad-hoc surfaces do
  too.

- **`fenestra-anim`: a new standalone leaf crate for keyframe animation math**,
  published independently at `0.1.0`. Extracted from `fenestra-core` (`CubicBezier`,
  `SpringSpec` — both still re-exported at their original `fenestra-core` paths, public
  API unchanged) and `fenestra-motion` (`Frames`/`FrameRange`, the `Ease`/`Spring`
  easing set, `Track<T: Interpolate>`/`Key<T>`), plus a new `mul_div(a, b, c, Rounding)
  -> u64` exact rational timebase primitive. Zero dependency on any fenestra crate,
  wgpu, vello, parley, taffy, or winit — usable by any frame/tick-based sampler,
  including outside this workspace. See ARCHITECTURE.md for the extraction rationale,
  the `Interpolate`/`Color` design (Oklab mixing stays in `fenestra-motion`'s new
  `ColorTrack`, not the leaf crate), and two bugs a property-test sweep over the full
  input domain caught before either shipped: a `u128` overflow in `mul_div`'s `Round`
  mode, and an under-converged `CubicBezier::eval` residual on skewed control points
  outside fenestra's own shipped curve set (fixed by raising Newton iterations 8 → 16).

- **`fenestra-motion`: frame-pure motion graphics.** A new workspace crate: a
  composition is a pure function of integer frame number — typed keyframe tracks
  (hold-at-ends, per-segment easing: CSS beziers shared with the interactive engine,
  closed-form damped springs with initial velocity, named curves crisp/editorial/pop),
  clips with spans/z/anchors, and `Clip::dynamic(|frame| …)` for data-driven trees.
  Frames rasterize through the existing headless pipeline (new
  `fenestra_shell::render_element_over`: caller background — transparent renders
  straight alpha — and preview scale factors); sinks: parallel PNG sequences (rayon +
  order-restoring writer), an ffmpeg rawvideo pipe (PNG paths never need ffmpeg), and
  contact sheets with frame numbers burned in. A versioned RON/JSON data form embeds
  the `fenestra/1` node vocabulary; the `motion` CLI renders/probes/lints/sheets it.
  Verification layer: pre-raster prop/bbox probes, temporal lints (undeclared jumps
  vs declared `.cut()`s, monotonicity, settling), auto-selected sentinel frames with
  golden coverage for the three shipped demos (lower third, per-word stagger, chart
  race over `fenestra-charts`). Core gains additive shared seams:
  `SpringSpec::step(v0, t)`, public `Style::paint_affine`, and `Style.scale_xy` +
  `Style.transform_origin` (hit-testing follows via the shared matrix). The
  determinism contract is layered and CI-tested: sampling is exactly deterministic;
  renders agree within the GPU's ±1 LSB antialiasing noise in-process; cross-machine
  reproduction uses the golden tolerance harness.

### Fixed

- **Second-pass polish from the same code review (P2s + a convention).** The
  color picker's amber badge no longer claims "Out of gamut — showing the
  nearest displayable color" (the picker only ever holds a displayable color, so
  nothing is substituted) — it now reads "At the sRGB gamut edge …", the honest
  meaning. `date_picker` now rejects a day past its month's length (April 31,
  Feb 30) with a path-pointed error instead of silently selecting nothing.
  `image_node` decodes with `into_rgba8` (consuming) so the native and RGBA8
  buffers aren't both resident — the transient peak stays near the committed
  budget. `fenestra preview` detects changes by content hash (a same-length
  rewrite within a coarse mtime tick is no longer missed) and its poll thread
  now stops when the `PreviewApp` is dropped (no leak when embedded). The
  color-picker gallery example seeds from `theme.accent` rather than a raw
  `oklch()` literal (the kit's "colors only through theme tokens" rule).
- **`fenestra-motion`: a 10-angle multi-agent review found and fixed 15 issues**
  before the crate's PR merged — none shipped. Two reproduced live before the
  fix: a hostile document (`start: u64::MAX`-class span) panicked with an
  integer overflow through every CLI subcommand, and a chatty ffmpeg encoder
  (>64KB to stderr before reading stdin) deadlocked the frame writer. Also: a
  semi-transparent composition background was composited twice (darkened);
  non-finite (`NaN`/`inf`) keyframe *values* were accepted and silently passed
  every temporal lint (only easing parameters were checked); `to_ron`/`to_json`
  serialized the stale load-time document after builder mutations, dropping
  edits silently; frame counts were unbounded end to end (CLI `--frames` and
  document `duration`); `verify::monotone` didn't clamp to the clip span like
  every other sampler, reporting phantom violations past `span.end`; exit
  animations still pivoted about the hardcoded rect center instead of the
  element's own `transform_origin` — the one paint site this PR's
  `Style::paint_affine` unification missed; the ffmpeg video sink cloned every
  frame's full pixel buffer; data-form clips deep-cloned their content tree
  every frame instead of once; and a color track couldn't animate to
  `"transparent"` (only the background field could). Full account in
  ARCHITECTURE.md. Every fix landed with a test that failed first; all gates
  green, the full `fenestra-kit` golden corpus and motion's own sentinel
  goldens byte/tolerance-compared clean.
- **`fenestra-motion`: `motion lint` now runs all three temporal lints, not just
  one.** `verify::monotone` and `verify::settled` existed as tested library functions
  but were unreachable from the CLI; only `verify::discontinuities` was wired to
  `motion lint`. Added `--monotone <clip_id>:<prop>:<direction>` (with `--frames a..b`)
  and `--settled-after <frame>` flags; problems from all three checks merge into one
  report under the existing exit-code convention. CLI integration tests now also run
  `probe`/`lint`/`sheet` against the actual shipped `lower_third.ron` example, not only
  a synthetic fixture (`title_stagger`/`chart_race` are code-only demos with no RON
  form to drive through the CLI — `chart_race` fundamentally can't, since
  `Clip::dynamic` doesn't serialize; a regression test pins that this is a real
  limitation, not an oversight).
- **Agent-reachable DoS vectors clamped.** Each grid template axis is bounded so its
  realized track total — summed across every `repeat(...)` and single-track entry —
  stays ≤ 1024 where it realizes into taffy (a total near 32767 allocated ~1 GB;
  ≥32768 overflowed taffy's i16 grid coordinates); a per-`repeat()` clamp alone was
  not enough, since many small clamped repeats can still sum past the ceiling, or an
  oversized fragment can escape a count already clamped to 1 — the budget is now
  enforced once, axis-wide, at the template-to-taffy boundary. `pagination` clamps
  `siblings` (≤50, the window driver) with saturating window math (a `siblings` ≈ 2e9
  input built a multi-billion-cell strip; `count` is floored at 1 but otherwise
  uncapped — the strip never materializes more than a small window regardless of page
  count, so `count` carried no allocation risk, and capping it too just broke
  legitimate large pagers); and scenario `tab` / `shift_tab` repeats are bounded to
  4096 (a `u32::MAX` repeat re-derived the whole tree billions of times, hanging the
  MCP worker). All reachable from pure `fenestra/1` JSON through every frame-building
  tool; clamp-over-panic at the API boundary.
- **Hit-testing follows paint-time transforms.** A `translate`/`rotate`/`skew`/`scale`'d
  interactive element painted in one place but activated at its old layout slot;
  `walk_hit` now inverts the same affine `paint_node` draws under (shared
  `node_transform`), so the activatable region matches the painted one — restoring the
  invariant "what you hit-test is exactly what you painted." Paint output is unchanged.
- **Duplicate `WidgetId`s are caught.** Two elements sharing an `.id("…")` (or a
  non-unique keyed-list key) realize the same id and silently share `FrameState`;
  `build_frame` now `debug_assert!`s frame-wide id uniqueness (compiled out of release).
- **`virtual_heights` no longer leaks.** The one retained `FrameState` map never
  garbage-collected is now frame-stamped and GC'd like scroll/anim/editor state.

### Security

- **A second, adversarial code review of the batch closed a P0 and two P1s
  (agent-reachable), each with a regression test.** (1) *P0* — the aggregate
  image-decode budget was reset to its full value inside `virtual_list`'s lazy
  per-row render closure, so on the *paint* path each materialized row decoded
  with an independent full budget; a tiny `row_height` collapses the virtual
  window onto every row at once, so a `virtual_list` of large images could
  decode the whole list simultaneously and OOM, defeating the very cap this
  batch introduced. A per-frame budget (`fenestra_core::frame_epoch`) now bounds
  all rows a frame materializes; over-budget rows fail decode before allocation
  and degrade to spacers. (2) *P1* — an image decoded on every rebuild (`view()`
  runs per frame), so an `interact {"tab": 4096}` step or an animated preview
  re-ran the full decode thousands of times (a hang, contradicting `film_ui`'s
  "degrades instead of hanging"); an LRU decode cache (capped at
  `MAX_TOTAL_IMAGE_BYTES`) now decodes each payload once and shares the blob.
  (3) *P1* — `Step::Drag`'s `to` selector skipped strict resolution and panicked
  in `Frame::get` on a miss instead of returning `EngineError::Step`; it now
  resolves like every other target. The `image_budget_security` suite only
  exercised the eager parse pass, so it was green with the P0 present — a
  render-path test (real `build_frame`) now covers it.
- **Two newly published RUSTSEC advisories against locked transitive versions,
  cleared with minimal patch bumps.** `RUSTSEC-2026-0204` (`crossbeam-epoch`,
  reached through `rayon`) and `RUSTSEC-2026-0190` (`anyhow`) both had fixed
  releases already out; bumping to `crossbeam-epoch 0.9.20` and `anyhow 1.0.103`
  cleared both with no API surface change (`Cargo.lock` only).
- **Three unfixable transitive RUSTSEC advisories are documented and ignored, not
  silenced.** `quick-xml 0.39.4` (RUSTSEC-2026-0194, RUSTSEC-2026-0195, both HIGH) reaches
  the workspace through winit's Wayland backend and `zbus_xml`'s AT-SPI stack; no
  `wayland-scanner` release yet accepts `quick-xml >= 0.41`. `ttf-parser 0.25.1`
  (RUSTSEC-2026-0192, unmaintained) reaches it through winit's optional Wayland Adwaita
  decorations; neither `ab_glyph` nor `sctk-adwaita` has published since 2025-09. All
  three are dated, commented, and scoped in `.cargo/audit.toml` and `deny.toml`, with the
  dependency chains and revisit triggers recorded in ARCHITECTURE.md. Dropping Wayland
  support to close them outright was considered and rejected as a platform regression out
  of proportion to two upstream-blocked advisories.

### Documentation

- **The headless verification envelope is stated plainly** (README, llms.txt): headless
  is a deterministic subset of the live window — embedded Latin/Inter fonts (real
  mono/CJK/emoji/RTL and the OS clipboard only in a window), reduced motion, one
  reference GPU backend, and the full Liquid-Glass optics (backdrop blur, edge lensing,
  adaptive tint) headless-only (live keeps tint + rim + sheen); the web target compiles
  out AccessKit/clipboard/glass.

## 0.39.0 — 2026-06-29

The truthful-verification pass — make the typed access tree and the accessibility
gate tell an agent the *whole* truth (the agent-native verification moat), make
the visuals it can now verify real, and give the format a machine-checkable
grammar. Three new MCP tools (twelve total), new typed access-tree fields, new CLI
subcommands, three new scenario expectations, and a formal JSON Schema for the
description format; everything is additive.

### Added

- **Numeric / range state in the access tree.** `AccessNodeDto` carries `value_now`
  / `value_min` / `value_max` (slider, spinbutton, meter, progressbar) and `mixed`
  (tri-state checkbox), projected from the widget semantics — so an agent reads a
  control's value off the typed tree instead of regexing the aria string.
- **Selector matching by state.** Semantic selectors gain `checked` / `selected` /
  `invalid` / `value_gte` / `value_lte`, so "the selected tab", "no checkbox
  checked", or "sliders ≥ 0.5" resolve in one query.
- **Keyboard focus order.** `inspect::focus_order` returns the refs a Tab cycle
  visits, in order, honoring a modal focus trap — exposed as the `focus_order` MCP
  tool, a `fenestra focus-order` CLI subcommand, and a scenario `expect.focus_order`.
- **Layout report.** `inspect::layout_report` flags interactive targets below the
  24px minimum hit size (WCAG 2.5.8) and signal-bearing nodes rendered off the
  window — exposed as the `check_layout` MCP tool, a `fenestra layout` subcommand,
  and a scenario `expect.layout`.
- **Strict accessibility.** `A11yReport.text_contrast_failures` lists every text
  node failing the strict per-node APCA floor even when the theme verdict is
  legible; gated opt-in via scenario `expect.a11y_strict` and CLI `check --strict`.
  `LegibilityDto.bg_uniform` flags when a reported background is a sampled gradient
  field rather than a single color.
- **Charts speak their data.** Every `fenestra-charts` chart derives a data-driven
  accessible description (series/point count, min/max, trend, categories, pie slice
  %) so charts are headlessly verifiable; opt-in `.x_title()` / `.y_title()` axis
  titles (byte-identical when unset).
- **Markdown code highlighting.** Fenced code blocks get a language chip and
  theme-token syntax highlighting (rust / js / ts / python / json / sh / sql) via a
  dependency-free lexer; unknown or untagged blocks stay plain mono. Footnote
  bodies now keep inline bold / italic / code.
- **Formal JSON Schema for the description format.** `describe::format::description_schema`
  derives a machine-checkable input grammar from the format types (externally-tagged
  nodes, `deny_unknown_fields`, the untagged colour/track unions all expressed
  precisely), exposed as the `describe_schema` MCP tool and a `fenestra schema` CLI
  subcommand — the formal complement to the prose `describe_vocabulary`. A drift
  guard keeps it covering exactly the nodes the vocabulary advertises.
- **Flagship `verify` example.** `cargo run --example verify` authors a sign-in card
  as fenestra/1 JSON and runs the whole verification battery headlessly with no
  display and no GPU (aria snapshot, access-tree roles, query, a11y, focus order,
  layout, then drive-and-assert via interact) — the "author in JSON, verify offline"
  proof, doubling as a smoke gate.

### Fixed

- **Legibility over gradients.** `Frame::legibility` measured text against the
  nearest *solid* ancestor, silently mis-reporting any text on an authored gradient
  (a false pass/fail) — exactly the framework's most ambitious surfaces. It now
  samples the gradient field densely (interpolated) and reports the worst-contrast
  point; `TextLegibility.bg_uniform` marks a sampled field.
- **MCP errors are path-pointed.** `render_ui` / `interact` parse failures come back
  located (via `serde_path_to_error`), matching the `validate` tool.
- **Icon authoring drift.** The describe parser derives its authorable icon set from
  the kit's vendored registry (`lucide::by_name` / `lucide::names`), so the parser,
  the grammar, and the kit can't drift apart; the four newly-vendored icons and the
  canonical `house` / `triangle-alert` / `trash-2` names are authorable, with
  `home` / `alert-triangle` / `trash` kept as back-compat aliases.
- Doc drift: `Element::semantics` was undocumented; `Theme::corner_smoothing` said
  "per-corner"; `StackedBarChart::w` had a pasted doc; the ARCHITECTURE "discovery
  deferred" note was stale.

### Notes

- Highlighted markdown code uses per-token elements (the span color is concrete and
  the markdown builder has no theme at build time), so selection in a *highlighted*
  block is per-token — the same tradeoff as inline links; unknown / untagged blocks
  keep whole-block selection. Multi-line block comments / strings highlight per line
  (highlight-grade, not a full grammar).
- `layout_report.offscreen` measures against the window — exact for the authored
  format (no scroll viewports); a builder-built frame with a scroll container would
  over-report content scrolled below the fold.

## 0.38.0 — 2026-06-26

The Liquid Glass optics pass — a multi-agent research survey of Apple Liquid
Glass (WWDC25 / iOS 26 / macOS Tahoe 26), Material 3 Expressive, Microsoft Fluent
(Acrylic/Mica), and visionOS, distilled to the distinctive, deterministic,
golden-lockable gaps fenestra could close on vello. Every piece is orthogonal and
default-off, so all non-glass elements and opaque `Surface` roles stay
byte-identical; `Surface::Glass` lights them up with no API change.

### Added

- **Specular edge rim.** `SpecularEdge { light_deg, intensity, shade }` (new
  `Style::specular_edge` / `Element::specular_edge`, default `None`) paints a
  hairline (~1.5px) linear-gradient stroke just inside a pane's silhouette — a
  shaded underside grading through clear to a bright white top — so the perimeter
  reads as a lit bevel (a lens), not a flat outline. `SpecularEdge::glass()` is the
  stock recipe (top light, intensity `0.6`, shade `0.18`).
- **Directional body sheen.** `Sheen { light_deg, top, bottom }` (new
  `Style::sheen` / `Element::sheen`, default `None`) washes a soft linear gradient
  over the face — white at the lit corner, through clear, to a faint shade at the
  far corner — so a translucent pane reads as lit glass instead of a uniform tint.
  `Sheen::glass()` is a 135° rake (`top` `0.12`, `bottom` `0.06`).
- **Edge lensing (refraction) — the rounded-rim "crown."** The headless
  backdrop-blur pass now bends the blurred backdrop into the rim: within a bevel
  band along the rounded-rect SDF, each pixel resamples from further inside along
  the corner normal, so straight backdrop lines compress and curve through the
  edge. Pure IEEE-754 `f32` (no FMA fusion), so it stays bit-stable across
  macOS/Metal and Linux/lavapipe like the box blur, and the interior beyond the
  band is byte-identical. *Headless only* — the live single-pass window keeps the
  flat tint (the standing glass envelope).
- **Backdrop-adaptive vibrancy.** `AdaptiveTint { pivot, gain }` (new
  `Style::adaptive_tint` / `Element::adaptive_tint`, default `None`) shifts a glass
  tint's OKLCH lightness by the mean luminance of the frosted backdrop behind it —
  brighter over dark content, darker over light — so the pane holds a stable contrast
  against whatever floats behind it (Fluent Acrylic's "luminosity clamp" reduced to a
  single mean-luminance shift; Apple Liquid Glass does the same tonal adaptation).
  Measured in the core painter at paint time, where the composited backdrop image is
  already in hand, so no luminance plumbing is needed; the mean is summed in integer
  channels, so it stays bit-stable across macOS/Metal and Linux/lavapipe. *Headless
  only* — the single-pass live window has no backdrop image and keeps the flat tint.
  `AdaptiveTint::glass()` is the stock recipe (pivot `0.55`, gain `0.20`).
- **`Surface::Glass` wears them all for free.** The rim, sheen, and adaptive tint are
  derived from the role's `Material` in `SurfaceBundle::apply`, so the stock frosted
  pane gains them with no new call; new `tokens::GLASS_LIGHT_DEG` (`0` = top) is the
  shared light direction.
- **Flagship example `liquid_glass`.** `cargo run --example liquid_glass` floats a
  *vibrant* 0.5-alpha glass pane (glassier than the legibility-first stock
  `Surface::Glass` at 0.82α) over bold accent stripes, so the rim, sheen, and
  lensing all read at once. Output is gitignored.
- **Author the glass aesthetic in JSON (`fenestra-describe`).** The agent-facing
  description format now mirrors the glass/material vocabulary: a `surface` role
  (`glass` | `card` | `popover` | `menu` | `modal` | `tooltip` | `thumb`) applies a
  whole material in one token, and `corner_smoothing`, `backdrop_blur`,
  `specular_edge`, `sheen`, and `adaptive_tint` — each a `"glass"` preset string or a
  structured object — compose a custom pane. So an agent can now author fenestra's
  signature Liquid Glass surface *and* verify it headlessly through the same render
  path the builders use — closing the gap between the web-grade visuals and the
  agent-native authoring half of the moat. Authoring-only round-trip, default-off,
  with `"glass"` presets that resolve to the exact core `::glass()` recipes.
- **The whole visual layer is authorable in JSON.** Completing the above, the
  description format now also expresses a custom translucent `material` background (a
  `Material` vibrancy tint + its backdrop blur — the glassier-than-stock escape
  hatch), per-corner `corners` / `rounded_full` (pill chips), paint-time `translate`
  / `rotate` / `skew`, and a foreground `element_filter` (`blur` / `brightness` /
  `saturate`). A new golden authors fenestra's signature Liquid Glass surface
  *entirely in JSON* — frosted material, specular rim, body sheen, adaptive tint, and
  pill chips over a striped backdrop — and renders it headlessly: an agent authors the
  headline visual *and* verifies it, no Rust touched.
- **Headless interaction-state verification.** The agent-facing access tree
  (`AccessNodeDto`) now exposes `selection` (the caret / selected `[start, end]`
  range, collapsed = caret) and `live` (polite live regions), so an agent can drive
  input through `interact` and assert the caret position or a live-region
  announcement straight off the returned tree — no pixels. Surfacing it also fixed a
  real a11y gap: a `status` with `live: true` drew the sonar ring but never marked
  itself an aria-live region; it now does (no pixel change).
- **The style vocabulary is self-documenting.** `describe_vocabulary` now advertises
  the full `style` grammar — ~35 properties (spacing, sizing, paint, border / radius /
  shadow, the glass / material vocabulary, transforms, filter, typography, grid), each
  with an example — and the closed `enums` (surface / shadow / align / justify /
  text_align / variant / status / side / kind / glass presets), generated from
  registries and pinned by coherence tests so the advertised grammar can't drift from
  the parser. An agent can now *discover* everything the prior passes made authorable,
  completing the author → verify → discover loop.

## 0.37.0 — 2026-06-25

The "unlock the defaults" pass — quality features that were built but shipped
off by default are now on (or finally reachable), each verified to keep the
stock goldens byte-identical except where the improvement is the point.

### Added

- **Optical sizing on by default.** `Theme::optical_sizing` (default
  `OpticalSizing::Auto`, the CSS default) drives a variable font's `opsz` axis
  to the rendered size kit-wide; `Style::optical` now defaults to a new
  `OpticalSizing::Inherit` that follows the theme. A no-op on the static stock
  faces (verified byte-identical), so only variable text faces change — e.g. the
  `warm_editorial` look's Fraunces now tracks its optical master at body size.
  `Theme::with_optical_sizing` opts the whole kit out; `Style::optical(Default)`
  opts one element out.
- **Reduced-motion now honored on real windows.** The live window reads the OS
  "Reduce Motion" setting (macOS / Linux-GNOME / Windows, via each platform's CLI
  so `unsafe_code = forbid` holds) and re-reads it on focus, so a user who asked
  for reduced motion gets it (WCAG 2.3.3) — previously this engaged only in
  headless/tests.
- **Toast motion.** Toasts swipe-to-dismiss (`on_dismiss` now fires on a flick),
  animate out (fade + shrink + drop), and the survivors FLIP up into the gap.
- **FLIP layout animation, finally wired.** Tag-input chips and inline
  `data_table` rows carry stable keys and `animate_layout`, so removing a chip or
  re-sorting a table glides instead of jumping.
- **Runnable `looks` example.** `cargo run --example looks` renders all six
  design languages (each with its own faces) — the showcase the aesthetic range
  was missing; the `gallery` example now uses `fenestra_looks::console`.

### Changed

- **Tabular figures for numeric widgets.** Spin-button values, meter percentages,
  pagination, stepper numbers, and calendar days use tabular (`tnum`) figures so
  digits stay column-aligned and don't jitter — matching `data_table`/`stat_card`.
- **Anchored pop overlays rise as they fade in.** Menus, select/combobox
  dropdowns, tooltips, submenu flyouts, and context menus now rise 8px on enter
  (the motion modals and drawers already had), snapping under reduced motion.
- **Tactile press feedback on more controls.** The select trigger, multi-select
  chips, and the tag-input remove button gain the button-style press-shrink.
- **Markdown body paragraphs use pretty wrapping** (no stranded one-word last
  line); headings were already balanced.

## 0.36.0 — 2026-06-24

### Added

- **Apple-style squircle corners, kit-wide.** Every rounded surface now draws
  continuous-curvature corners (a superellipse "squircle") instead of a plain
  circular arc, removing the curvature kink where a straight edge meets a
  circular corner and giving the whole UI a softer, web-grade silhouette.
  - The corners are built from **true cubic Béziers**, fitted to the
    superellipse quadrant with `kurbo::fit_to_bezpath` (no flattened polyline,
    no magic constants). Fill, border, clip, focus ring, and image clip all
    share the one silhouette.
  - `Theme::corner_smoothing` (default `DEFAULT_CORNER_SMOOTHING` = `0.6`) is the
    single knob that re-curves the kit, mirroring `Theme::with_radius`. Set it
    with `Theme::with_corner_smoothing`, or override per element with
    `Style::corner_smoothing` / `Element::corner_smoothing` (`0.0` restores
    exact circular arcs). Square boxes, and pills/avatars (an `R_FULL` radius),
    are left perfectly circular by construction.

## 0.35.0 — 2026-06-24

The maturity pass: responsive grid follow-up, forms depth, internationalization,
and navigation/gestures — each taken to a complete, tested, web-grade feature.

### Added

- **Navigation router, swipe gestures, and verified web/wasm.**
  - `Nav<Route>`: an Elm-native navigation stack — `push` / `pop` / `replace` /
    `pop_to` / `pop_to_root` / `reset`, plus `current` / `can_pop` / `depth` /
    `stack`. The app holds it in its model and matches `current()` in the view; it
    is never empty (the root never pops), so the current screen is always valid.
  - `Element::on_swipe(|dir| …)`: a swipe (flick) recognizer — a press plus a fast
    drag past a small threshold fires with the dominant `SwipeDir`
    (`Up`/`Down`/`Left`/`Right`). Built on the existing press/capture path; a tiny
    move stays a tap.
  - **Web/wasm is first-class, now verified end to end.** All four crates build
    for `wasm32-unknown-unknown`, and the existing `pages.yml` workflow ships the
    `web_demo` example to GitHub Pages over WebGPU. The router, gestures, i18n,
    RTL, and forms work added this pass are all wasm-clean.
- **RTL mirroring, Dynamic Type, and i18n.** Three locale/accessibility knobs on
  the theme, plus a new i18n module.
  - `Theme::rtl()` / `with_direction(WritingDir)` mirrors the realized layout
    horizontally (a leading element lands on the right, a row's order reverses)
    and flips `start`/`end` text alignment to the opposite edge. Applied as a
    final geometry pass, so widths, container queries, and motion deltas are
    unchanged — and LTR is byte-identical.
  - `Theme::with_text_scale(f32)` — Dynamic Type — multiplies every resolved font
    size (clamped 0.5–3.0); 1.0 is byte-identical.
  - New `Locale` (RTL detection + locale-aware integer/decimal grouping, e.g.
    `1,234.50` vs `1.234,50`) and `Catalog` (key → message with `{name}`
    interpolation and key fallback). No ICU/heavy data. `Locale::direction()`
    feeds straight into `Theme::with_direction`.
- **Forms maturity: value semantics, adornments, multi-select, deeper validation.**
  - New ARIA roles surfaced through both the headless access tree and the live
    AccessKit write-path: `meter`, `spinbutton`, and `progressbar` (carrying
    numeric value/min/max). `meter`, `spin_button`, `progress`, and
    `progress_indeterminate` now expose them, so they are verifiable headlessly.
  - `spin_button` is a real ARIA spinbutton: `.range(value, min, max)` publishes
    the numeric value, and ↑/↓ step it from the keyboard.
  - Text-input adornments — `text_input(..).prefix(x)` / `.suffix(x)` — place an
    icon or unit inside the field at either end (the focus ring stays on the
    input; the text is padded clear of the adornment).
  - New `multi_select`: a toggleable chip set with checkbox semantics
    (Space/Enter toggles, selected chips accent-fill with a check) — the
    fixed-option-set complement to `tag_input` (free text) and `select` (single).
  - Validation engine deepened: `Constraint::Step { step, base }` (value on a
    `base + k·step` grid) and `validate_all` (every failing message, not just the
    first). New `Element::value(..)` sets a non-input control's accessible value
    (ARIA `valuetext`).
- **Named grid lines + `grid-template-areas`.** The second responsive-grid layer.
  Lay out by name: `grid_template_areas(["header header", "nav main", "footer
  footer"])` on a container plus `grid_area("main")` on a child place the
  holy-grail in two lines; or name the lines (`grid_col_names(["sidebar", …])`) and
  place a span with `grid_col_lines("sidebar", "content")` (likewise `grid_row_*`).
  fenestra resolves names to numeric taffy lines itself, deterministically: areas
  validate as rectangles (a non-rectangular name falls back to numeric placement,
  never a panic), and areas without explicit tracks imply an `auto` grid shaped to
  the map. Authorable in JSON (`grid_template_areas`, `grid_area`, `grid_col_lines`/
  `grid_row_lines`, `grid_col_names`/`grid_row_names`). Every prior grid golden is
  byte-identical.
- **Real frosted-glass backdrop blur.** `Surface::Glass` now genuinely blurs the
  content *behind* it (its `Material.blur_radius`, reserved since 0.22, is live) —
  a floating pane reads as frosted glass over live content, not a flat tint. It is
  an opt-in two-pass CPU pipeline: render with glass panes skipped, read the pixels
  back, blur the region behind each pane with a deterministic integer box blur
  (bit-identical on Metal and lavapipe), then composite the frost under the vibrancy
  tint. A frame with no glass renders in a single pass exactly as before (every
  prior golden byte-identical). Also adds `Element::element_filter(Blur/Brightness/
  Saturate)` (foreground filter of an element's own content) on the same machinery,
  the raw `Element::backdrop_blur(px)` builder, and `kit::{glass_surface, glass_panel}`
  one-call frosted panes. Realized in headless rendering (the golden source of truth);
  the live window currently falls back to the tint-only look.
- **Constraints-aware layout: window breakpoints + container queries.** Two opt-in
  tiers for layout that reacts to *size*. Tier 1: `App::view_at(key, size)` hands the
  window's logical size to the view, so an app can switch layout on width breakpoints
  (defaults to `view_for` → `view`, so existing apps are untouched). Tier 2:
  `responsive(|avail| -> Element)` is a **container query** — the closure rebuilds a
  subtree from the container's *own* measured size (reusing the motion system's
  per-node rect record), converging one frame after a resize like CSS container
  queries, with no layout cycles. `responsive_hinted` seeds the first frame to skip the
  flash. New `Breakpoint`/`Breakpoints` give Tailwind-style thresholds
  (`Breakpoint::at(width)`, `Breakpoints::is_md(width)`, …) for either tier, and
  `Harness::resize` drives both headlessly. Every prior golden is byte-identical.
- **Motion completion: FLIP layout animation + exit animations.** `.animate_layout()`
  makes an element *slide* when layout moves it (FLIP / shared-element): its measured
  rect is compared frame-to-frame and, on a real move, it paints from the old position
  and springs to the new — reorder a keyed list or resize a sibling and rows glide
  instead of jumping (needs a stable `.id`; identity is the `WidgetId`). `.exit(...)` /
  `.exit_to(opacity, scale, dx, dy)` animate an element *out* when it leaves the tree:
  a paint-only "ghost" snapshot lingers and fades/scales/slides to its targets, then is
  dropped — the counterpart of `.enter()`. A ghost faithfully replays the transform it
  last painted with, so a node removed mid-FLIP-slide (or carrying a static transform)
  exits from where it actually was. Both ride the existing transition engine and are
  **inert under reduced motion** (FLIP snaps, exits are immediate) — every prior golden
  is byte-identical.
- **`data_table` is feature-complete.** On top of the existing sort + multi-select:
  row **virtualization** (only the visible window materializes — 100k rows cost the
  same as 100), a **sticky header** (pinned above the scrolling body), column
  **resize** (`.column_widths` + drag handles + `.on_resize`/`.on_resize_end`),
  column **reorder** (`.column_order` + header drag-and-drop + `.on_reorder`),
  column **pin/freeze** (`.pinned_left`/`.pinned_right`, frozen during horizontal
  scroll via `position: sticky`), and a per-column **filter** row (`.filter` +
  `.on_filter`). Elm-pure throughout — the app owns widths/order/filter and emits
  Msgs. New core primitive `Element::on_drag_end(msg)` (fires on release after a
  drag) powers the resize lifecycle.
- **Horizontal scrolling + `position: sticky`.** Scroll state is now 2D: new
  `.scroll_x()` / `.scroll_xy()` builders, an `offset_x` axis with its own clamp
  and scrollbar, and wheel `dx` routing (each axis routes to the nearest scroller
  *on that axis*, so a horizontal pane nested in a vertical one each get their own
  delta). `position: sticky` arrives via `.sticky_top/bottom/left/right(px)`: a
  sticky element pins to its scroll viewport's content box once scrolled past the
  threshold (top/left win on conflict, per CSS), painting and hit-testing above
  its siblings. This is the core primitive sticky table headers and frozen
  columns build on.
- **`aria-invalid` is verifiable.** A control's danger-ring `invalid` state now
  surfaces in the access tree: `AccessNode` / `AccessNodeDto` gain an `invalid`
  field, the aria snapshot emits `[invalid]`, and the describe `text_input` /
  `text_area` vocab gains `invalid`. So an agent can author an invalid control and
  a scenario can assert `- textbox [invalid]` through the verify loop — validity
  is provable, not just visual. Serializes skip-if-false (prior snapshots
  unchanged).

## 0.34.0 — 2026-06-23

A verification + layout pass: the agent verify loop is closed end-to-end, grids
gain the responsive CSS track vocabulary (in builders and JSON), and forms get a
constraint-validation engine. Pure-additive — every prior golden is byte-identical.

### Added

- **The verify loop is closed: unified scenario verification.** A `Scenario` — a
  description, optional interaction steps, and a bundle of expectations — runs in
  one pass and returns a single `VerifyReport` (overall `ok` + a per-check
  breakdown). Expectations: `emitted` author intents, `a11y` (legible + every
  control named), `aria` snapshot match, `screenshot` baseline diff, and `queries`
  (selector → match count). The screenshot check compares the **post-interaction**
  pixels — so "after this click, the screen looks like this baseline" is now
  verifiable, not just the static render.
- **`fenestra verify <scenario>`** drives the steps, asserts every expectation, and
  signals one verdict through the exit code (`0` ok · `1` a check failed · `3` a
  setup/IO error). `--bless` (re)writes the screenshot baseline from the current
  render (capture once, then verify); `--out` writes the diff PNG on a mismatch.
- **`run_scenario`** is a ninth `fenestra-mcp` tool: the same unified verify over
  the MCP boundary, returning the structured report plus a preview (the diff on a
  screenshot miss, else the final render). A failing verdict is a normal result.
- **`fenestra-describe::inspect`** gains frame-level primitives so verification can
  read a post-interaction frame, not only a static description: `frame_a11y`,
  `match_aria_text`, and `query_tree`. `fenestra-render` gains public `diff_images`
  (compare an already-rendered image to a baseline) and the `scenario` module
  (`Scenario` / `verify` / `bless`). All additive — existing APIs are unchanged.
- **Responsive grid tracks.** Grid templates now speak the responsive CSS
  vocabulary: `Track` gains `Auto` / `MinContent` / `MaxContent` / `FitContent(px)`
  / `MinMax(min, max)`, and a new `GridTemplate` adds `repeat(...)` including
  `auto-fit` / `auto-fill`. `grid_cols` / `grid_rows` are generic over
  `Into<GridTemplate>`, so plain `Track`s still work and
  `grid_cols([GridTemplate::auto_fit_minmax(240.0)])` gives a reflowing column
  count with no breakpoints. New `responsive_grid(min_col, children)` kit helper.
  Authorable in JSON too: the describe `style` block gains `grid_cols` / `grid_rows`
  (track strings like `"1fr"` / `"200px"`, or `minmax` / `fit_content` / `repeat`
  objects), so a responsive grid is describable and verifiable through the scenario
  loop. (Named grid lines + `grid-template-areas` are a tracked follow-up.)
- **Form constraint validation.** A pure engine in `fenestra-kit::validation`:
  `Constraint` (`Required` / `MinLen` / `MaxLen` / `Min` / `Max` / `Email` /
  `Integer` / `Number`) + `validate(value, &[Constraint]) -> Validity` (valid +
  the first failing message), plus `Field::validity(&v)` to show it. Elm-pure —
  the app validates in `view` and wires `.invalid(..)` + the field error. Regex
  `pattern` is intentionally out (the widget crate stays `regex`-free).

## 0.33.0 — 2026-06-22

A craft pass that deepens the 0.32 vocabulary widgets from first-cut MVPs into
their full, advanced forms. The pre-existing widget goldens are byte-identical;
only the new widgets' own behavior and the feedback showcase changed.

### Changed

- **The segmented control now slides.** The active thumb is a single
  absolutely-positioned element that *travels* to the selected segment on a
  spatial spring (it cross-faded before). Segments are equal width — sized to the
  longest label, or pin the total with `.width` — and the builder gains `.size`
  (Sm/Md/Lg) and `.disabled`. `segmented(..)` now returns a `Segmented` builder
  (call `.into()` where an `Element` is needed directly).
- **Skeletons shimmer.** Blocks and circles run a left-to-right highlight sweep
  (a translucent band gliding across the neutral base, clipped to the shape)
  instead of a flat pulse; text lines keep the quieter opacity pulse. Both stay
  deterministic under reduced motion.
- **Wavy progress is the real Material 3 indicator.** The wave's amplitude tapers
  to flat at its leading edge and as it nears completion, with a small gap before
  the remaining track; `.amplitude` and `.wavelength` are tunable.
  `wavy_progress(..)` now returns a `WavyProgress` builder.
- **Live status indicators glow.** The pulsing ring reads as a soft halo around
  the dot for realtime / online states, visible even in a static frame.

### Added

- **`kbd_raised`** — a chunky 3D keycap variant (a raised surface with a thick
  bottom lip) alongside the flat-chip `kbd`, for documentation and onboarding.

## 0.32.0 — 2026-06-22

Adds the universal modern primitives that premium apps ship — chosen from a
five-strand survey of contemporary design systems (Linear/Raycast, Vercel Geist
+ Radix, shadcn / Tailwind v4 / Base UI, Material 3 Expressive, Apple HIG).
Pure-additive: every existing golden is byte-identical; this release only adds
new widgets and a new showcase scene.

### Added

- **Segmented control** (`segmented`) — a compact, single-select view/option
  switcher: a contained track with a raised, cross-fading thumb behind the
  active segment. Elm-pure (active index in, `on_select(index)` out), with ARIA
  tab semantics per segment.
- **Skeleton loaders** (`skeleton`, `skeleton_text`, `skeleton_circle`) — the
  content-shaped loading placeholder. A gentle opacity pulse that pins flat under
  reduced motion; the fill is the translucent neutral twin, so it reads on any
  surface (a white card in light mode, an elevated card in dark) instead of
  vanishing into a same-tone background.
- **Status indicator** (`status`) — a semantic dot plus a label, with an optional
  `.live()` pulsing "sonar" ring for realtime / online / recording states. The
  dot is decorative; the label carries the meaning.
- **Keyboard key-caps** (`kbd`) — flat-chip shortcut hints (`kbd(["cmd", "K"])`
  → ⌘ K) that map modifier names to glyphs and keep obscure keys readable
  (Esc/Tab as words). The whole chord exposes one accessible label.
- **Wavy progress** (`wavy_progress`) — the Material 3 Expressive determinate
  bar, drawn as an accent-stroked sine wave over a flat track: a pure-vector
  parametric path, static so headless renders stay deterministic.
- **Feedback showcase** (`gallery_feedback`) — a new headless golden scene
  (light + dark) covering all of the above; `cargo run --example gallery`
  now also renders `gallery/feedback_{light,dark}.png`.

## 0.31.0 — 2026-06-19

Closes the two deferred phase-2 increments — declarative state and the MCP output
contract — and hardens the boundary from an adversarial review. Additive
throughout; every existing golden is byte-identical.

### Added

- **Declarative state (the Elm wall is gone).** A `Description` may carry a root
  `state` map, and a widget may `bind` a state key. The framework owns the
  transition — a bound checkbox/switch toggles, a bound input echoes typed text,
  a bound slider updates its number — with no logic in the JSON. `interact` now
  returns the post-interaction `state`. Unbound handlers still emit inert intent
  strings. `describe_vocabulary` advertises `bind` on every bindable widget.
- **MCP `outputSchema`.** `query_ui`, `check_a11y`, `match_aria_snapshot`, and
  `describe_vocabulary` return a typed result with a formal `outputSchema`
  (derived from the describe DTOs, which now derive `schemars::JsonSchema`), so a
  client knows the result shape before calling.
- **Full-resolution render as a `resource_link`.** Visual tools attach the
  full-res PNG as an MCP `resource_link` (a `file://` URI) next to the inline
  downscaled preview, instead of a bare path in the structured result. Per-process
  temp files are bounded to the last 64 renders.

### Fixed

- **Text layout no longer hangs on a pathological font size.** A non-finite or
  enormous `size_px` (`∞`, `NaN`, `f32::MAX`) made the line breaker spin forever
  on wrapping text; `fenestra-core` now clamps font size to a finite range, so
  every app is protected.
- **`validate()` rejects out-of-range style numbers.** A non-finite dimension or
  border width, an out-of-range `size_px`, or an out-of-gamut `oklch` (lightness
  outside `0..=1`, negative chroma, or a non-finite component) is now a
  path-pointed error instead of silently rendering garbage.

## 0.30.0 — 2026-06-19

### Added

- **Description format**: button `variant` (primary / secondary / ghost / danger)
  and slider `step` — additive optional fields, mapped to the kit builders and
  surfaced in `describe_vocabulary`.

The `fenestra-describe` parser's libFuzzer target was run (1.9M executions, no
panics on hostile JSON). Declarative state (value echo / the Elm wall), MCP
`outputSchema`, and a full-resolution `resource_link` are deferred to focused
phase-2 increments — see ARCHITECTURE.md for the designs and rationale.

## 0.29.1 — 2026-06-19

A serialized boundary for describing and verifying UIs as JSON — with a CLI and
an MCP server — plus the per-text-node legibility primitives they read. Additive
throughout: every existing golden is byte-identical.

### Added

- **`fenestra-describe`** — a serde `Description` (a schema-tagged `"fenestra/1"`,
  strict `deny_unknown_fields` JSON mirror of an element tree) that parses to the
  same `Element` the builders produce. Containers, text, and the interactive
  widgets; colors by theme role name or an `oklch` escape hatch; handlers are
  inert intent strings. Plus the windowless structural engine: a typed access
  tree, semantic `query` (with nearest-candidates on a miss), `aria_snapshot` +
  `match_aria` (partial / strict / regex), `check_a11y`, path-pointed `validate`,
  and a self-coherent `describe_vocabulary`.
- **`fenestra-render`** — the `fenestra` binary: `render`, `query`, `interact`,
  `check`, `match-aria`, `match-png`, `vocabulary`, and `validate` subcommands,
  reading a description from a path or stdin and emitting JSON (`cargo install
  fenestra-render`).
- **`fenestra-mcp`** — a Model Context Protocol server (over stdio) exposing the
  same eight operations as tools, so an AI assistant can build, render, query,
  and assert native UIs (`cargo install fenestra-mcp`).
- **`Frame::legibility`** (`fenestra-core`): per-text-node APCA `Lc` and WCAG 2
  contrast measured against the floor for each rendered size, with
  `apca::wcag2_ratio` / `apca::wcag2_passes` and a public `Semantics::aria_role`.

## 0.28.0 — 2026-06-16

Typography, density, and optical polish — four threads, each opt-in and
defaulting to a true no-op, so every existing golden is byte-identical.

### Added

- **Optical sizing (`OpticalSizing`)**: drives a variable font's `opsz` axis.
  `.optical_auto()` tracks the rendered size (CSS `font-optical-sizing: auto`),
  `.optical(OpticalSizing::Fixed(n))` pins one optical master, and the default
  emits no variation (static faces and existing output unchanged). On `Style`
  and `Element`; threaded through shaping, the layout cache key, and editors.
- **Bundled text serif — Fraunces** (`fenestra-looks`): a variable text-optical
  serif (`opsz` 9–144, `wght` 100–900; upright + true italic, SIL OFL). It is
  the `warm_editorial` Look's `Serif` role — a real text serif for prose — with
  Playfair Display kept for display headlines. New `optical_sizing` golden.
- **Widget density (`.density(Density)`)**: `button`, `icon_button`,
  `text_input`, and `select` take `Compact` / `Comfortable` (default) /
  `Spacious`, packing the shared height grid tighter or looser while the label
  font stays legible. `Comfortable` is byte-identical to before.
- **Optical icon correction**: `.optical_overshoot()` scales a round/pointed
  path icon so it reads the same visual size as square neighbors, and
  `.optical_center()` seats an asymmetric glyph on its centroid (the play-button
  nudge). Opt-in per icon (uncorrected paths render identically). New
  `optical_overshoot` golden; `optical_play` now uses the builder.

### Changed

- **`warm_editorial`** body serif is now Fraunces (was Playfair, a display
  face); display headlines stay Playfair. Only the `look_warm_editorial` golden
  moves.
- **Command palette** derives its panel from `Surface::Menu` instead of a
  hand-rolled recipe (one source of truth; tracks the radius knob). Corners rise
  `R_MD`→`R_LG`; new `command_palette` golden locks it.
- **Markdown code blocks** read the theme radius token (`radius.sm`) instead of
  a hardcoded `6.0`, so a sharp/soft theme re-rounds them (byte-identical at the
  default).

### Fixed

- **Editors clear a toggled-off OpenType feature or `opsz` axis** instead of
  leaving the prior property stuck on a persistent editor (the 0.16 known
  limitation) — `apply_style` is now insert-or-remove.

## 0.27.1 — 2026-06-16

### Added

- **`console_showcase`** (`fenestra-kit`): the sharp/minimal "observability
  console" scene as a reusable, golden-tested showcase — slate + a single lime
  accent, hairline rules instead of cards, mono tabular numerals. Rendered to
  `gallery/console_{light,dark}.png` by the `gallery` example.

### Changed

- **README**: the Design-range section drops the "Year 8 / Evolution"
  study-guide poster and features the sharp console (light + dark) as the
  opposite end of the range from the soft default dashboard. (README image URLs
  were made absolute in 0.27.0 so they render on crates.io.)

## 0.27.0 — 2026-06-15

Beautiful by default — the design system advertises its range up front: a
curated non-blue Look, and one-knob radius and elevation that the whole kit
reads.

### Added

- **`Theme::radius` (`RadiusScale`) + `Theme::with_radius`**: a corner-radius
  family the entire kit resolves from — buttons, inputs, selects, data tables,
  cards, menus, modals, tooltips, and concentric menu items. `RadiusScale::sharp()`
  (1–4px, crisp tech chrome) and `RadiusScale::soft()` join `from_base`; the
  default reproduces `R_SM`…`R_XL` exactly, so the stock look is unchanged.
- **`Theme::elevation` (`Elevation::{Shadowed, Flat}`) + `Theme::with_elevation`**:
  `Flat` separates resting `Card`/`Raised` surfaces with a border + surface
  tone-step instead of a shadow (dark-mode-honest, sharper); floating roles keep
  their shadow. Default `Shadowed`.
- **`console` Look** in `fenestra-looks` (and `all()`): a cool-slate + electric-
  lime, sans-body, sharp + flat "observability console" voice — APCA-passing,
  golden-locked.
- **Per-side borders**: `border_top/right/bottom/left(width, color)` on `Style`
  and `Element` (an `EdgeBorders` field) — straight hairline edges for ruled
  layouts, with no manual divider children. Default none, so goldens are
  unchanged.

### Changed

- **`effects::mesh` is ordered-dithered** (4×4 Bayer, ±0.5 LSB) before 8-bit
  output, so smooth gradient ramps don't band without a grain overlay.
- **Editorial type guidance**: `FamilyRole::Display`/`Serif` and the editorial
  Looks now document that Didone display faces (Playfair) are headline-only and
  body prose wants a *text* serif at ≥20px (or the sans).

### Decided

- See ARCHITECTURE.md "0.27: beautiful by default" — radius/elevation as theme
  knobs the kit reads (defaults preserve every golden); per-side borders as
  painter strokes (no layout change); mesh dither always-on. Deferred with
  rationale: variable-font `opsz` (needs variation-axis plumbing through the
  font stack), true multi-line drop-caps (need text-exclusion layout — raised
  initials already work via `rich_text` spans), and vendoring a text-optical
  serif asset.

## 0.26.0 — 2026-06-14

Generated effect fields — the "bespoke" end of the design system as
deterministic, token-colored RGBA8 textures.

### Added

- **`effects::mesh(width, height, &[MeshPoint])`**: a multi-point mesh gradient
  (the Stripe "liquid light" field) — every pixel an inverse-distance blend of
  the color points in OKLab, so it stays vivid through the middle with no gray
  dead-zone.
- **`effects::grain(width, height, seed, intensity)`**: fine film grain from a
  seeded PRNG (deterministic), to break up banding and add a tactile paper
  texture. Both return RGBA8 buffers for `image_rgba8`; an `effects_showcase`
  golden renders the mesh field with a grain overlay.

### Decided

- See ARCHITECTURE.md "0.26: effect nodes" — generated textures (pure,
  deterministic, golden-locked) rather than a live shader; colors are theme
  tokens; the third effect-family member, a scroll-edge fade, needs no new
  primitive (a `linear_gradient` surface→transparent is the fade).

## 0.25.0 — 2026-06-14

Optical-adjustment helpers — the small geometric corrections that make shapes
*look* right even though they measure "wrong".

### Added

- **`optical::CIRCLE_OVERSHOOT`** (~1.1284) + **`optical::overshoot(size)`**: a
  circle must be ~12.84% larger than a square to read as the same visual size;
  scale a round icon's diameter against adjacent squares with this.
- **`optical::centroid(vertices)`**: a polygon's visual-mass center. Center an
  asymmetric shape (a play triangle) on its centroid, not its bounding box, so
  it looks centered — the classic play-button nudge. An `optical_play` golden
  shows bbox-centered (left-heavy) vs centroid-centered (centered) side by side.

### Decided

- See ARCHITECTURE.md "0.25: optical adjustments" — math helpers (constant +
  centroid) plus a demonstrative golden; the correction is applied by the caller
  (no painter change), so every existing golden is byte-identical.

## 0.24.0 — 2026-06-14

A composited ring border — the "ring, not border" primitive (Geist): a crisp
band just outside the box that hugs the corner radius.

### Added

- **`Style::ring(width, color)`** / **`Element::ring(width, color)`**: a
  `width`-px ring rendered as a zero-blur spread shadow, sitting just outside
  the element and hugging its corner radius. Unlike `.border()` (an edge
  stroke), it never covers the element's content or children and recolors with
  zero layout cost — ideal for selection/emphasis rings and sub-pixel hairlines.
  Composes with shadow tokens (paints on top of any drop shadow) and stacks
  (call it more than once). Generalizes the `ChromeElevation` hairline ring to
  any surface; a `ring_showcase` golden demonstrates border vs ring.

### Decided

- See ARCHITECTURE.md "0.24: composited ring border" — why the outer,
  corner-hugging, content-safe ring complements the edge `border`, and that it
  rides the existing shadow-layer machinery (no new paint primitive, opt-in, so
  every existing golden is byte-identical).

## 0.23.0 — 2026-06-14

A `Density` knob that packs the control grid tighter or looser from one value —
the Linear/pro-tool density toggle.

### Added

- **`Density`** (`Compact` / `Comfortable` / `Spacious`, Comfortable default) +
  **`ControlSize::metrics_at(Density)`**: scales control height, padding, gap,
  and icon together. `Comfortable` is byte-identical to the prior
  `ControlSize::metrics()`, so the kit is unchanged unless you opt in; `Compact`
  tightens, `Spacious` loosens. The label font stays tied to the `ControlSize`
  across every density — density scales *spacing*, not *type*, so control text
  never shrinks below its legible size.
- **`density_showcase()`** (kit) + a golden: the same controls at all three
  densities side by side.

### Decided

- See ARCHITECTURE.md "0.23: density mode" — Comfortable == today's metrics
  (existing widget goldens byte-identical), the clean per-size Compact/Spacious
  tables (not a raw multiplier), and the spacing-not-type decision (font tied to
  `ControlSize` for legibility).

## 0.22.0 — 2026-06-14

A translucent "glass" material — a frosted pane that reads as floating glass
over the content behind it (Apple materials / Linear & Raycast command
palettes), wired into the Surface system.

### Added

- **`Material`** (`fill_alpha` / `blur_radius` / `saturation`) and the
  **`Surface::Glass`** role: a translucent, vibrancy-tinted fill resolved
  against the theme via `Material::tint` (raise OKLCH chroma to re-saturate,
  then apply alpha — never a raw color). `Material::popover()` is the
  command-palette recipe. Carried by `SurfaceBundle` (the `material` field);
  opaque roles set `material: None` and render byte-identically to before.
- **`glass_showcase()`** (kit) + a light/dark golden: a frosted command palette
  over a vivid accent-gradient backdrop, with the backdrop card visibly
  modulated through the pane. Text on the glass is proven legible at its role
  floors (primary `text` ≥ 75, secondary `text_muted` ≥ 55) over the gradient
  endpoints, in both modes.

### Scope

- `blur_radius` is **stored but not yet rendered**: vello 0.9 has no
  backdrop-filter, so the shipped look is a translucent vibrancy fill (no live
  backdrop blur). A true multi-pass backdrop blur is recorded as a renderer
  milestone. See ARCHITECTURE.md "0.22" for the feasibility assessment and the
  deferred-blur decision.

## 0.21.0 — 2026-06-14

Size/weight-aware APCA contrast and a `text_on(surface)` legibility helper.
Pure additive logic in `fenestra-core` — the fixed role floors, every theme
snapshot, and all goldens are unchanged.

### Added

- **`apca::required_lc(size_px, weight)`** — APCA's readability criterion as a
  function instead of a fixed floor: the minimum Lc magnitude that text of a
  given logical size and OpenType weight needs to read fluently. Monotonically
  decreasing in both axes (heavier weight maps to a larger effective px via
  `eff = px·(weight/400)^0.5`), calibrated to the APCA "in a nutshell" anchors
  (14px/400 → ~90, 16px/400 → 75, 24px/400 → 60, 36px → ~45) and clamped to a
  `[15, 108]` range. Inputs are clamped (`size_px ≥ 1`, `weight ∈ 1..=1000`),
  so out-of-range values are safe.
- **`Theme::contrast_ok(text, bg, size_px, weight)`** — the size/weight-aware
  companion to `validate_contrast`'s fixed role floors: proves a *specific*
  label legible at its real rendered size by checking `lc_abs(text, bg) >=
  required_lc(size_px, weight)`.
- **`Theme::text_on(bg)`** — the strongest legible neutral text color for any
  background, generalizing the `on_accent` rule to custom and status surfaces:
  returns whichever ramp extreme (`neutrals.step(1)` paper / `step(12)` ink)
  wins APCA Lc on `bg`, always theme-tinted (never raw white/black). Ties break
  toward the ink.

### Scope

- The role floors (75/60/55/40) are unchanged regression sentinels;
  `required_lc` now anchors them to the same APCA scale, with the load-bearing
  identity `PRIMARY_TEXT_MIN == required_lc(16px, 400)` asserted literally. See
  ARCHITECTURE.md "Size/weight-aware APCA + `text_on` (0.21)" for the tie-in and
  the two recorded framing deviations.

## 0.20.0 — 2026-06-14

Concentric corner radii and opt-in continuous-curvature (squircle) corners.
Both default to a true no-op, so every existing golden is byte-identical — the
only new pixels are one demonstration golden.

### Added

- **`SurfaceRadius::inner(inset)`** (`max(0, outer - inset)`) — the concentric
  rule for nesting a rounded child inside a rounded surface: the child's radius
  is the parent's outer radius minus the padding between them, so corners share
  a center and the inner corner never bulges. Menu and select items derive their
  radius from the panel via this accessor (one `SP1` token for both pad and
  radius).
- **`Style::corner_smoothing`** / **`Element::corner_smoothing(f)`** — Figma-style
  continuous-curvature corner smoothing, `0.0..=1.0` (clamped). `0.0` (default)
  draws exact circular arcs; higher values blend toward a fuller superellipse
  (Apple-style squircle) that hugs each straight edge longer and turns into the
  corner with no curvature kink. Fill, border, and clip share one path.

### Changed

- Menu and select item radii now derive from
  `Surface::Menu.bundle().radius.inner(SP1)` instead of a hand-typed `R_LG - 4.0`,
  so they track the panel radius automatically. The value is unchanged (`10` =
  `R_MD`), so all goldens are byte-identical.

### Scope

- `corner_smoothing` reshapes fill, border, and clip only; shadows, the focus
  ring, and image clips stay circular this phase (no shipped widget opts in).
  See ARCHITECTURE.md "0.20: concentric radii + continuous-curvature (squircle)
  corners" for the recorded scoping decision and the superellipse construction.

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
