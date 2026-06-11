# Introduction

fenestra is a pure-Rust native GUI framework: winit windowing, wgpu GPU
access, vello vector rendering, parley text shaping, taffy flexbox/grid
layout. No browser, no webview, no HTML/CSS parser — and two commitments
that shape everything else:

1. **Web-grade aesthetics by default.** OKLCH color ramps generated from
   accent hues, 4px-grid spacing, layered soft shadows, real typographic
   hierarchy, focus rings, 120-300ms easing. The widget kit looks like a
   polished web product out of the box, and an editorial design language
   (custom faces, free-form type, duotone fields) is a few calls away.
2. **You can see what you build — programmatically.** Rendering is a pure
   function of `(element tree, theme, size, scale)`, it runs without a
   display server, and it is deterministic. Humans get golden tests; AI
   agents get a build → render → look → verify loop ([AGENTS.md] is the
   agent manual).

[AGENTS.md]: https://github.com/richer-richard/fenestra/blob/main/AGENTS.md

Try the [live demo](https://richer-richard.github.io/fenestra/) — the
same code compiled to WebAssembly, rendering through WebGPU.

## Where things live

| Crate | Contents |
| --- | --- |
| `fenestra` | facade: `run()`, prelude, examples |
| `fenestra-core` | element IR, theme, layout, text, paint, input, accessibility |
| `fenestra-shell` | windowed runner + headless renderer and test harness |
| `fenestra-kit` | the themed widget kit |
