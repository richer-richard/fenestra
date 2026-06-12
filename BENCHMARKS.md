# Benchmarks

Honest numbers from `cargo run --release --example bench`, so you can
reason about the full-rebuild architecture instead of guessing. Re-run it
on your machine; numbers below are one snapshot (refreshed at 0.8.0), not a promise.

**Machine:** Apple M3 Pro, 18 GB RAM, macOS (Metal), rustc 1.95.0.
**Setup:** release build, embedded fonts, scale 1.0, reduced motion.

## CPU per frame (view build + style resolution + taffy layout + vello scene)

This is the work fenestra repeats from scratch every frame — there is no
diffing by design.

| Scene | mean | best |
| --- | ---: | ---: |
| counter (tiny) | 0.042 ms | 0.031 ms |
| gallery_controls (medium, ~hundreds of nodes) | 0.395 ms | 0.262 ms |
| gallery_display (large, tables + icons) | 0.312 ms | 0.266 ms |
| virtual_list, 100,000 rows @ 1120×720 | 0.086 ms | 0.084 ms |

Read: rebuilding and laying out a real screen costs a fraction of a
millisecond, leaving the 16.6 ms frame budget essentially untouched. The
virtualized list makes row count irrelevant — 100k rows cost less than the
static gallery because only a screenful materializes.

## Full headless pipeline (CPU + GPU render + readback)

`render_element`: everything above plus the vello compute render and a
GPU→CPU copy of the full image. The readback is test-harness overhead a
windowed app never pays (it presents instead).

| Scene | mean | best |
| --- | ---: | ---: |
| gallery_display @ 760×1190 | 3.165 ms | 2.295 ms |
| counter @ 320×160 | 1.949 ms | 1.733 ms |

## Artifact sizes

| Artifact | size |
| --- | ---: |
| `dashboard` example, release binary (macOS arm64, unstripped) | 12.8 MB |
| web demo `.wasm` (release, before gzip) | 6.6 MB |

The binary carries wgpu + vello (shader pipelines) and three embedded
Inter faces; it is independent of app code size to first order.

## Honest caveats

- Full rebuild means per-frame cost scales with *visible* tree size.
  Virtualize long lists (`virtual_list`); a 10k-node static tree would be
  felt.
- The GPU numbers above include `wgpu` readback; presentation latency in a
  window is governed by the compositor and vsync, not by these numbers.
- Text layout is cached by (text, style, width); cold caches (first frame,
  size changes) pay shaping costs not visible in warm means.
- Software rasterizers (lavapipe, WARP) are an order of magnitude slower
  on the GPU stage; they exist for CI, not for users.
