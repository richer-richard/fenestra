# The determinism contract

Verification is only as good as its reproducibility. This page states
exactly what fenestra guarantees about headless output, where the
boundaries are, and why — so you can decide what to stake on it.

## The guarantee

Given the same **element tree**, **theme**, **logical size**, **scale**,
and **font set**, a headless render produces the same pixels on the
same GPU class, run after run. Everything that could drift is pinned:

- **Fonts** — `Fonts::embedded()` ships three Inter faces inside the
  crate; text shaping and metrics cannot vary with the host system.
  (`Fonts::with_system()` deliberately trades this for CJK and emoji
  coverage — windowed apps want it, goldens should not.)
- **Scale** — headless renders at 1.0; no DPI surprises.
- **Motion** — reduced motion is forced; animations resolve to their
  end states. The harness clock is explicit: `pump(ms)` advances it,
  nothing else does, so mid-animation frames are reproducible at exact
  timestamps.
- **State** — a fresh `FrameState` (in-memory clipboard, no focus, no
  scroll) unless your test built some up on purpose.
- **Sizes** — clamped to the device texture range (≥1, typically
  ≤8192), so a wild request degrades predictably instead of failing.

## The boundaries (honest part)

- **Across GPU classes, pixels wobble.** Antialiased edge coverage is
  floating-point work; Metal, lavapipe, and other rasterizers disagree
  by a hair. The golden comparator absorbs this: 3/255 per channel,
  0.2% of pixels — and CI's software rasterizer widens the pixel budget
  via `FENESTRA_SNAPSHOT_BUDGET=0.006` without loosening the reference
  platform. Goldens in this repo are rendered on **macOS/Metal**; that
  is the reference. (Flutter reached the same conclusion and moved to a
  triage service; Slint gets exact pixels only on a reduced CPU
  renderer. fenestra keeps the production renderer and states budgets.)
- **Dark themes wobble more.** Low-luminance antialiasing amplifies
  rounding differences; the budget covers it, but expect dark goldens
  to sit closer to the threshold.
- **The structural layers do not wobble at all.** The accessibility
  tree, `debug_tree`, query results, and emitted messages are exact on
  every platform — prefer them when an assertion doesn't need pixels.
- **Wall-clock leaks are a bug.** If you find output that varies with
  time of day, machine speed, or run order, file it; determinism
  regressions are treated as breakage, not flake.

## What this buys

A UI test that fails only when the UI changed; PNGs an agent can diff
to *see* its own work; goldens that survive `git bisect`; and a CI
matrix where the one honest source of variance (the rasterizer) is
named and budgeted instead of fudged per-OS.
