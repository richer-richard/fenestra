# Looks

A *Look* is a complete design language — theme and typefaces bundled
into one value, applied in one call. The same app, five voices, from
the `fenestra-looks` crate:

- **product** — the stock voice: Inter, neutral surfaces, blue accent.
- **editorial** — print energy: Playfair Display headlines over a deep
  duotone field (the poster's language, packaged).
- **terminal** — instrument panel: JetBrains Mono everywhere, phosphor
  accent, built for dense tools.
- **warm-editorial** — warm paper and ink: a cream-and-terracotta field
  *derived* (`Theme::derive`) with Playfair serif prose under sans chrome.
- **playful** — a soft pastel canvas with a saturated magenta accent, for
  whiteboard-class, friendly tools. (Ships with the base sans; a hand-drawn
  display face is a planned addition.)

```rust,ignore
let look = fenestra_looks::editorial(Mode::Dark);
let fonts = look.fonts();           // embedded base + the look's faces
render_element_with(view, &look.theme, size, &mut fonts);
// Windowed: WindowOptions::titled("…").with_font(role, bytes) per face.
```

Each look is golden-locked from the same sample screen, so its
identity is pinned, not aspirational. Typefaces are vendored under
their OFL licenses.

Two mechanics make looks possible and are available to your own:
registered faces win for *every* family role (register under
`FamilyRole::Sans` and body text changes voice), and a family must
cover the weights you request — asking for Semibold in a family that
only ships Medium falls back out of the family entirely, so looks
bundle 400–700.

Build your own look: a `Theme` (or `ThemeSpec` recipe) plus
`(FamilyRole, bytes)` pairs is the whole format.
