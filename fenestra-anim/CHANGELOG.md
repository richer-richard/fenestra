# Changelog

All notable changes to `fenestra-anim` are documented here. Format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [0.1.0] - 2026-07-03

Initial extraction from `fenestra-core` (CSS cubic-bezier, closed-form
spring) and `fenestra-motion` (typed keyframe tracks, easing set), plus a
new exact rational timebase (`mul_div`).

### Added

- `CubicBezier`: CSS-style cubic-bezier easing, solved with Newton iteration
  to a documented 1e-5 accuracy bound.
- `SpringSpec`: a closed-form damped spring step response (underdamped and
  critically damped analytic solutions), no numeric integration or state.
- `Ease`, `Spring`, and the `linear` / `hold` / `ease_in` / `ease_out` /
  `ease_in_out` / `spring` constructors: the per-segment easing vocabulary a
  keyframe track eases through.
- `Frames` / `FrameRange`: the integer-frame ground truth a timebase is
  built on.
- `Interpolate`: the value contract a `Track` samples through, implemented
  here for `f32` and `(f32, f32)`. Color interpolation is deliberately not
  shipped — see the crate docs.
- `Key<T>`, `Track<T: Interpolate>`, `sorted_keys`, and `locate`: typed
  keyframe tracks and the segment-lookup/easing-evaluation machinery behind
  them, exposed separately so a consumer with a value type outside
  `Interpolate` (e.g. a perceptual color type) can reuse the bookkeeping.
- `mul_div(a, b, c, Rounding) -> u64`: an exact rational timebase primitive,
  computed through a `u128` intermediate. `Rounding::{Floor, Ceil, Round}`.
