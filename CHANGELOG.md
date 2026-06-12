# Changelog

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
