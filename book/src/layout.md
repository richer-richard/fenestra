# Layout, scrolling, virtualization

Layout is taffy: real CSS flexbox and grid semantics.

- `row()`/`col()` are flex containers; `.grow()`, `.shrink0()`, `.gap()`,
  alignment and justification work like the CSS you already know.
- `div().grid_cols(vec![Track::Fr(1.0); 4])` makes grids;
  `.grid_col(start, span)` places items.
- `.absolute()` positions against the nearest relative ancestor;
  `stack()` overlays children in one cell, painting in order.
- Text participates with real measurement (wrapping width in, wrapped
  height out) and true first-line baselines for `items_baseline()`.

## Scrolling

`.scroll_y()` clips and scrolls; wheel input routes to the deepest
scrollable that actually overflows. Scroll offsets persist per `.id(..)`
across rebuilds and clamp to the content range each frame. Scrollbars
fade in while scrolling and out after.

`.stick_to_bottom()` is the chat-log pattern: while the container sits
at its bottom edge, appended content keeps it pinned there; scrolling up
releases the pin; returning to the bottom re-pins.

Keyboard: PageUp/PageDown page the scroll container nearest the focused
element by 90% of its viewport; Home/End jump to its ends. Both defer to
the focused element first (text inputs keep Home/End for the caret).

In tests and headless runs, `FrameState::scroll_to(id, offset)` sets an
absolute offset (`f32::MAX` means "the bottom").

## Virtualization

For long lists, `virtual_list(count, row_height, |i| row_element)`
materializes only the visible window (plus overscan), with spacers
keeping scrollbar geometry exact. 100,000 rows cost ~0.09 ms per frame.
Rows are keyed by index, so their retained state stays put while the
window slides; handlers on rows dispatch normally. Constraints: fixed row
height, no overlays inside rows.

When row heights vary or are unknown,
`virtual_list_variable(count, estimated_height, |i| row_element)` places
rows from a prefix-sum height index seeded with your estimate; each
realized row feeds its measured height back, so offsets, the scrollbar,
and the total height self-correct as the user scrolls. Rows size
themselves — give each a real height (or content that has one). The
estimate only has to be in the right ballpark: it positions rows the
first time they appear, before measurement corrects them. See the
[performance chapter](performance.md) for the convergence model.
