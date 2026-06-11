# Images and icons

`image_rgba8(width, height, pixels)` shows straight-alpha RGBA8, stretched
to the element rect and clipped to the corner radius — `.rounded_full()`
turns a square avatar into a circle. Equality is blob identity, so
rebuilt views stay cheap. Decode files with the `image` crate and hand
fenestra the pixels.

Icons are vector paths painted in the resolved text color:

- `icons::{check, chevron_down, chevron_right, x, search, circle_dot}` —
  the 16px built-ins the kit itself uses.
- `icons::lucide::*` — 24 vendored [Lucide](https://lucide.dev) icons
  (ISC), stroked at 2px with round caps; `lucide::all()` iterates them.
- `path(bez, viewbox, stroke)` — bring your own `kurbo::BezPath`; `.trim`
  animates draw-on, `.spin` rotates.
