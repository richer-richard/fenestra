# fenestra

[![CI](https://github.com/richer-richard/fenestra/actions/workflows/ci.yml/badge.svg)](https://github.com/richer-richard/fenestra/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/fenestra.svg)](https://crates.io/crates/fenestra)
[![docs.rs](https://img.shields.io/docsrs/fenestra)](https://docs.rs/fenestra)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

**The UI stack built for the agent loop**: describe a UI as JSON, render
it natively, verify it in CI — no compile step, no screenshot flakiness.
fenestra is a pure-Rust GUI framework whose headless renderer is
deterministic, so both humans and AI coding agents can *see* — and prove —
what they build. It also speaks [A2UI](https://a2ui.org), the open
Agent-to-UI standard, as its first native Rust renderer
([`fenestra-a2ui`](fenestra-a2ui)). The web-grade widget kit and design
system below are the proof of what that loop can produce.

**[▶ Try the live demo](https://richer-richard.github.io/fenestra/)** — the
dashboard and widget galleries running in your browser via WebGPU. No DOM,
no CSS: every pixel is vello on wgpu, the same code as the native window.
**[Read the book](https://richer-richard.github.io/fenestra/book/)** for
the guided tour.

| Light | Dark |
| --- | --- |
| ![agent-session dashboard, light theme](https://raw.githubusercontent.com/richer-richard/fenestra/main/gallery/agent_dashboard_light.png) | ![agent-session dashboard, dark theme](https://raw.githubusercontent.com/richer-richard/fenestra/main/gallery/agent_dashboard_dark.png) |

*The hero is a real tool — `examples/agent_dashboard.rs`, a live dashboard
over an AI coding session (virtualized feed, charts, live tail via the
effect layer). The SaaS-style widget showcase lives on as
`examples/dashboard.rs`:*

| Light | Dark |
| --- | --- |
| ![dashboard, light theme](https://raw.githubusercontent.com/richer-richard/fenestra/main/gallery/dashboard_light.png) | ![dashboard, dark theme](https://raw.githubusercontent.com/richer-richard/fenestra/main/gallery/dashboard_dark.png) |

No browser. No webview. No HTML or CSS parser. fenestra draws everything
itself with [vello] on wgpu, lays out with [taffy] (flexbox + grid), shapes
text with [parley], and ships a themed widget kit that looks like a polished
modern web app: layered soft shadows, OKLCH color ramps, real typographic
hierarchy, hover/focus transitions, and first-class light and dark themes.

[vello]: https://github.com/linebender/vello
[taffy]: https://github.com/DioxusLabs/taffy
[parley]: https://github.com/linebender/parley

## Quickstart

```rust
use fenestra::prelude::*;

struct Counter { n: i64 }

#[derive(Clone)]
enum Msg { Inc, Dec }

impl App for Counter {
    type Msg = Msg;

    fn update(&mut self, msg: Msg) {
        match msg { Msg::Inc => self.n += 1, Msg::Dec => self.n -= 1 }
    }

    fn view(&self) -> Element<Msg> {
        col().p(SP6).gap(SP4).items_center().children([
            text(self.n.to_string()).size(TextSize::Xl2).weight(Weight::Semibold),
            row().gap(SP3).children([
                button("Decrement").variant(ButtonVariant::Secondary).on_click(Msg::Dec),
                button("Increment").on_click(Msg::Inc),
            ]),
        ])
    }
}

fn main() { fenestra::run(Counter { n: 0 }, WindowOptions::titled("Counter")) }
```

`cargo add fenestra`, paste, `cargo run`. Or start from the template —
`cargo generate richer-richard/fenestra-template` — which includes a
headless UI test and CI. The whole view is rebuilt, laid out, and
repainted on every redraw — no diffing, no macros, everything
autocompletes.

## Agents can see what they build

Rendering `(element tree, theme, size)` to pixels is a pure function, and it
runs without a window or display server:

```rust
use fenestra::shell::{SyntheticEvent, render_app, render_element};

// A picture of any element tree:
let image = render_element(my_view(), &Theme::dark(), (800, 600));
image.save("preview.png")?;

// Or drive a full app with scripted input and look at the result:
let image = render_app(
    &mut app,
    &[
        SyntheticEvent::MouseMove { x: 50.0, y: 34.0 },
        SyntheticEvent::MouseDown,
        SyntheticEvent::MouseUp,
        SyntheticEvent::Text("hello".into()),
    ],
    (800, 600),
    &Theme::light(),
);
assert_eq!(app.value, "hello");
```

Headless rendering is deterministic (embedded fonts, fixed scale, reduced
motion), which makes pixel-exact golden tests practical — fenestra's own
widget kit is tested this way, on CI, with no GPU display attached.

The same pipeline backs a JSON authoring format, `fenestra/1`, for agents and
tools that don't want to compile Rust: describe a UI — now including an
`image` node and fourteen widgets that used to be code-only (data tables,
trees, popovers, command palettes, the OKLCH color picker, and more) — and
[`fenestra-describe`](fenestra-describe) parses it into the identical
`Element` tree the builders above produce. `fenestra render` renders it,
`fenestra preview <file>` opens a live-reload window that re-renders on
every save, and the [`fenestra-mcp`](fenestra-mcp) server exposes the whole
loop — render, query, interact, verify — as fourteen MCP tools (including `render_a2ui`). Motion is
watchable too, not just single frames: `Harness::film` (or `fenestra film`,
or the MCP `film_ui` tool) captures a sequence with real motion turned on and
composes it into one captioned filmstrip.

**The verification envelope, stated plainly.** A headless render is a
deliberate *subset* of the live window — that subset is what makes it
deterministic — so trust it accordingly. It uses the embedded fonts (Inter
covers Latin; the real monospace, CJK, emoji, and RTL faces come from the OS
and so appear only in a real window), forces reduced motion, and is
referenced against one GPU backend (macOS/Metal; Linux/lavapipe within a
wider tolerance). The full Liquid-Glass optics — backdrop blur, edge lensing,
adaptive vibrancy — render only in the headless/golden path; the live
single-pass window shows the translucent tint plus the specular rim and
sheen. Scale is no longer pinned: `render_element_scaled` runs the same two-pass
pipeline at any device scale, so retina-only regressions (hairlines, blur
radii) are verifiable headlessly too. So headless is the right oracle for
layout, semantics, color, and the large majority of pixels — but confirm
non-Latin/monospace text and full glass in a window. On the web target,
copy-out reaches the system clipboard (paste-in from other apps stays
in-app), the glass story equals the native live window (tint-only, like
every single-pass swapchain), and AccessKit awaits an upstream web
adapter — the precise ledger is in ARCHITECTURE.md.

**Working with an AI agent?** [AGENTS.md](AGENTS.md) is the manual for the
build → render → look → verify loop (and [llms.txt](llms.txt) for
context loaders).

## Philosophy: web aesthetics without the web platform

The web's *look* — soft elevation, tinted neutrals, OKLCH ramps, 4px-grid
spacing, focus rings, 120–300ms easing — is the best-tested visual language
in software. The web *platform* is a heavy way to get it. fenestra encodes
that language as typed Rust values: a `Theme` generated from one accent hue,
spacing/radius/shadow/motion tokens, and a builder vocabulary (`row()`,
`.p(SP4)`, `.rounded(R_MD)`, `.shadow(ShadowToken::Sm)`) small enough to
memorize and regular enough for rust-analyzer (or a language model) to
autocomplete. Every widget routes every color through the theme; flip one
`Mode` and the whole app is dark.

## The kit

Button, IconButton, Checkbox, Switch, Radio, Slider, Color Picker (OKLCH
lightness×chroma pad, hue/alpha strips, forgiving hex entry), SegmentedControl,
TextInput (parley editing, clipboard, IME), TextArea (multiline,
auto-growing), Select, Tooltip, Modal (focus trap + backdrop), Toasts, Tabs,
Card, StatCard, Badge, Avatar, StatusIndicator (with a live pulse), Kbd
key-caps, Skeleton loaders, Divider, Progress (including a Material-3
Expressive wavy bar), Spinner, Table, Callout, and a vendored Lucide icon
subset — every state, both themes:

| | |
| --- | --- |
| ![controls, light](https://raw.githubusercontent.com/richer-richard/fenestra/main/gallery/controls_light.png) | ![controls, dark](https://raw.githubusercontent.com/richer-richard/fenestra/main/gallery/controls_dark.png) |
| ![display widgets, light](https://raw.githubusercontent.com/richer-richard/fenestra/main/gallery/display_light.png) | ![display widgets, dark](https://raw.githubusercontent.com/richer-richard/fenestra/main/gallery/display_dark.png) |
| ![segmented control, status, skeletons, key-caps, wavy progress — light](https://raw.githubusercontent.com/richer-richard/fenestra/main/gallery/feedback_light.png) | ![segmented control, status, skeletons, key-caps, wavy progress — dark](https://raw.githubusercontent.com/richer-richard/fenestra/main/gallery/feedback_dark.png) |

Regenerate this corpus any time with `cargo run --example gallery` — it
renders headlessly.

## Motion

`fenestra-motion` renders frame-pure compositions headlessly — no live
window, no screen recorder — and the same pipeline is what `fenestra film`
and the MCP `film_ui` tool use to let an agent watch a transition play. A
`fenestra-charts` bar chart, rebuilt every frame from rank-sorted,
track-interpolated data:

![chart race motion demo](https://raw.githubusercontent.com/richer-richard/fenestra/main/gallery/chart_race_demo.gif)

`cargo run -p fenestra-motion --example chart_race -- --mp4` renders this
exact sequence — the lead changes hands partway through, verified
structurally in the example itself, not just eyeballed. Two more shipped
demos render the same way: a broadcast lower-third
(`examples/lower_third.rs`) and a per-word title stagger
(`examples/title_stagger.rs`).

## Workspace

| Crate | Role |
| --- | --- |
| `fenestra` | Facade: prelude, `run()`, examples |
| `fenestra-core` | Element IR, theme/tokens, layout, text, paint, input, transitions |
| `fenestra-shell` | winit + wgpu window runner and the headless renderer |
| `fenestra-kit` | The themed widget kit, built only on core's public API |
| `fenestra-charts` | Sparklines, line and bar charts — the reference third-party widget crate |
| `fenestra-markdown` | CommonMark rendered as native `fenestra` elements |
| `fenestra-looks` | Six ready-made design languages (product, editorial, terminal, console, warm-editorial, playful), applied in one call |
| `fenestra-describe` | Parses `fenestra/1` JSON into the same `Element` tree the builders produce |
| `fenestra-render` | The `fenestra` CLI: render, preview, film, query, verify, lint — from the command line |
| `fenestra-mcp` | MCP server exposing render, query, interact, and verify as thirteen tools to AI agents |
| `fenestra-motion` | Frame-pure motion graphics: timelines, headless frame/video rendering, temporal lints, the `motion` CLI |
| `fenestra-anim` | Keyframe animation math — easing, springs, an exact rational timebase |

`fenestra-anim` is versioned independently (0.1.x): a standalone leaf crate
with zero dependency on any fenestra crate, wgpu, vello, parley, taffy, or
winit, extracted from `fenestra-core` and `fenestra-motion` so any
frame/tick-based sampler — inside this workspace or out — can depend on the
animation math alone. `fenestra-mcp` is also versioned independently, so the
MCP server can ship on its own release cadence.

See [ARCHITECTURE.md](ARCHITECTURE.md) for how the pipeline, widget
identity, transitions, and overlays work — recorded decision-by-decision as
the framework was built — and [BENCHMARKS.md](BENCHMARKS.md) for honest
frame-cost numbers (a full screen rebuilds, lays out, and paints in ~0.3 ms;
100k-row lists virtualize to ~0.09 ms).

## Design range

The same framework, the same tokens — a different design language. The
`fenestra-looks` crate bundles six ready voices (product, editorial, terminal,
console, warm-editorial, playful — enumerate them with `all()`), and one knob
re-skins the whole kit: `Theme::with_radius(RadiusScale::sharp())` for
un-rounded tech chrome, `Theme::with_elevation(Elevation::Flat)` for
border-not-shadow surfaces, and `Theme::duotone` for atmospheric fields instead
of neutral grays (custom display faces register under font roles via
`Fonts::register`). The opposite end of the range from the soft default
dashboard above — a sharp, hairline-ruled **console**: slate with a single lime
accent and mono numerals, rendered headlessly and golden-tested.

| Light | Dark |
| --- | --- |
| ![sharp console, light theme](https://raw.githubusercontent.com/richer-richard/fenestra/main/gallery/console_light.png) | ![sharp console, dark theme](https://raw.githubusercontent.com/richer-richard/fenestra/main/gallery/console_dark.png) |

## Composition, commands, accessibility

Components written around their own message type compose with
`Element::map`. Background work flows in through `App::init`, which hands
the app a cloneable `Proxy<Msg>` — spawn a thread, send messages, the
window repaints (`examples/clock.rs`, `examples/toasts.rs`). Every widget
exposes its role, state, and name: headlessly via `Frame::access_tree()`
(assert your UI is labeled, in CI), and to real assistive technology
through AccessKit in the windowed runner. Ambient motion comes from
looping `Keyframes` timelines; images from `image_rgba8` (round avatars
via `.rounded_full()`).

## Status

fenestra is at 0.40.0, built and recorded decision-by-decision in
[ARCHITECTURE.md](ARCHITECTURE.md). Shipped: the full interactive widget kit
in light and dark themes; six ready-made design languages
(`fenestra-looks`); a frosted-glass material system; reference third-party
widget crates for charts and markdown; the `fenestra/1` JSON format
authoring the entire kit, parsed by `fenestra-describe` and rendered/verified
by the `fenestra` CLI and the `fenestra-mcp` server's thirteen tools; a
live-reload `fenestra preview` window for authoring; and `fenestra-motion`
for frame-pure motion graphics with its own temporal-lint verification and
filmstrip capture. Every change goes through the same gate before it merges —
`cargo fmt --check`, `clippy -D warnings`, the full test suite, and a
headless golden-PNG comparison on macOS/Metal and Linux/lavapipe — plus a
weekly `cargo audit` sweep in CI. Open work is tracked as a ranked list in
ARCHITECTURE.md's "Deferred" notes; the two largest remaining gaps are a JSON
authoring bridge for charts/markdown (they're Rust-only today) and hi-DPI
headless rendering (every headless build site currently pins scale 1.0).

## License

MIT or Apache-2.0, at your option. The embedded Inter font, the Playfair
Display faces (poster and editorial looks), the Fraunces variable text serif
(the `opsz`/optical-sizing serif in the warm-editorial look), and JetBrains
Mono (terminal look) are licensed under the SIL Open Font License 1.1; the
vendored Lucide icon path data is ISC (see `fenestra-kit/LICENSE-LUCIDE.txt`).
