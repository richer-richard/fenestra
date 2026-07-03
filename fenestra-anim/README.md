# fenestra-anim

Frame-pure keyframe animation math: typed tracks, CSS cubic-bezier and
closed-form spring easing, and an exact rational timebase.

This crate has **zero dependency on any fenestra crate, GPU API, or text
layout engine** — it is pure math over `f32`/`u64`. It exists so the same
easing and keyframe math can back more than one sampler: fenestra's own
interactive transition engine, its offline `fenestra-motion` frame sampler,
and any unrelated project that needs to sample keyframed values off an
integer clock (e.g. an audio engine sequencing automation ticks).

## What's here

- **`CubicBezier`** — CSS `cubic-bezier(x1, y1, x2, y2)` easing, solved with
  Newton iteration to a documented `1e-5` accuracy bound.
- **`SpringSpec`** — a closed-form damped spring step response (underdamped
  and critically damped analytic solutions). No numeric integration, no
  state: any instant is sampled directly.
- **`Ease` / `Spring`** — the per-segment easing a keyframe eases into:
  linear, hold, a bezier curve, or a launched spring — plus the
  `linear` / `hold` / `ease_in` / `ease_out` / `ease_in_out` / `spring`
  constructors.
- **`Frames` / `FrameRange`** — integer frames as ground truth. Seconds are
  derived per frame (`frame / fps`), never accumulated.
- **`Track<T: Interpolate>` / `Key<T>`** — a typed keyframe track: holds
  before the first key and after the last, returns key values exactly on
  their frames, eases each segment by its leading key's `Ease`.
- **`mul_div(a, b, c, Rounding) -> u64`** — an exact rational timebase
  primitive: `a * b / c` through a `u128` intermediate, rounding mode
  chosen explicitly (`Floor` / `Ceil` / `Round`). Converting a tick count
  between two rates each call recomputes the exact rational value from
  scratch, so it never accumulates drift the way a running float
  accumulator would.

## What's deliberately not here

**Color interpolation.** Perceptual (Oklab) color mixing needs a
color-management dependency and a concrete color type this crate has no
other reason to carry. A consumer that wants a color track builds it
directly on `sorted_keys` and `locate` — the same segment-lookup and easing
evaluation `Track` itself uses — with its own color-space-aware value
combination in place of `Interpolate`. `fenestra-motion`'s `ColorTrack` is a
worked example: it reuses this crate's bookkeeping and easing math, and
implements Oklab-default / sRGB-opt-in mixing on top, entirely in the
consuming crate.

## Example

```rust
use fenestra_anim::{Frames, Track, ease_out, key};

let opacity = Track::new([
    key(0, 0.0f32).ease(ease_out()),
    key(30, 1.0),
]);

assert_eq!(opacity.sample(Frames(0), 60), 0.0);
assert_eq!(opacity.sample(Frames(30), 60), 1.0);
```

## `serde`

Enable the `serde` feature for `Serialize`/`Deserialize` where the crate's
types support it — off by default, since most consumers (including
fenestra-core's interactive engine) never serialize a curve or spring.

## MSRV

Matches the fenestra workspace: Rust 1.88.
