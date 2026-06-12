# Changelog

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
