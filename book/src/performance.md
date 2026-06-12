# Performance

fenestra rebuilds the visible tree every frame — no diffing, no retained
widget graph. That stays honest because the work is small (fractions of a
millisecond for real screens; see `BENCHMARKS.md` in the repo) and
because the two places where rebuilding *would* hurt have dedicated
machinery: idle frames and long lists.

## Clean-frame memoization

The windowed runner keeps the last painted scene, keyed by
`(logical width, height, scale)`. When the OS asks for a redraw and
nothing has changed — the window was exposed, un-occluded, or a timer
fired — it re-presents that scene and skips build, layout, and paint
entirely.

"Changed" is tracked by a dirty flag, set by input events, app updates,
accessibility focus, hover refresh after scrolls, resize, scale change,
and resume. While anything is time-driven — a caret blinking, a spring
settling, a spinner, a tooltip waiting out its delay, a scrollbar fading
— `frame.animating` keeps the flag set, so memoization can never starve
an animation.

Two consequences worth knowing:

- An idle fenestra app costs approximately zero CPU per OS redraw.
- Headless rendering (`Harness`, `render_element`) never memoizes: tests
  always exercise the full pipeline, so a memoization bug cannot hide
  from goldens.

There is deliberately no caching *below* the frame level. Caching a
subtree's scene requires proving its paint is a pure function of its
retained inputs; that purity is not tracked per-subtree today, and a
wrong cache means stale pixels. The decision and its revisit conditions
are recorded in ARCHITECTURE.md.

## Virtualization

`virtual_list(count, row_height, |i| row)` materializes only the rows
overlapping the viewport (plus overscan), with spacer elements keeping
scroll geometry exact. Row count stops mattering: 100k rows lay out in
~0.09 ms.

`virtual_list_variable(count, estimated_height, |i| row)` handles rows
whose heights vary or are unknown up front. It keeps a prefix-sum index
of row heights, seeded with your estimate:

1. Rows are *placed* from the index — top spacer, realized rows, bottom
   spacer.
2. After layout, each realized row's measured height is recorded back
   into the index.
3. The next frame's placement uses the corrected sums.

So offsets, scrollbar geometry, and the total height converge to the
truth as the user scrolls; by the time a region has been seen once, its
geometry is exact. The estimate only positions rows the first time they
appear — it should be in the right ballpark (the typical row), not
precise. Sub-quarter-pixel measurement jitter is ignored, so the index
never thrashes.

Constraints, both variants: rows are keyed by index (retained state
follows the index, so prepending shifts identity), and overlays inside
rows are unsupported. Variable rows must size themselves — fixed heights
or content with intrinsic height.

## What to reach for, in order

1. **Nothing.** Measure first; `cargo run --release --example bench`
   shows what full rebuilds actually cost.
2. **Virtualize** any list that can grow past a few hundred rows.
3. **Check `animating`.** If your app never goes idle, something is
   keeping the flag set — an always-on spinner or an indeterminate
   progress bar costs a full pipeline pass per frame, on purpose.
