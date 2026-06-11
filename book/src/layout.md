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

## Virtualization

For long lists, `virtual_list(count, row_height, |i| row_element)`
materializes only the visible window (plus overscan), with spacers
keeping scrollbar geometry exact. 100,000 rows cost ~0.09 ms per frame.
Rows are keyed by index, so their retained state stays put while the
window slides; handlers on rows dispatch normally. Constraints: fixed row
height, no overlays inside rows.
