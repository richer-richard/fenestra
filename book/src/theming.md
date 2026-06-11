# Theming and design languages

A `Theme` is a bag of resolved color tokens generated from hues:

```rust,ignore
let theme = Theme::from_accent(160.0, Mode::Dark); // teal accent
```

`from_accent` produces two 12-step OKLCH ramps (tinted neutrals + accent),
status palettes (danger/warning/success), text roles, borders, surfaces,
and shadows. Gamut mapping reduces chroma, never clips, so every hue is
safe.

Widgets defer their colors with `themed` closures, because `view()` has no
theme parameter:

```rust,ignore
div().themed(|t: &Theme, s| s.bg(t.surface).border(1.0, t.border))
```

Flip `Mode::Light`/`Mode::Dark` (or return a different theme from
`App::theme`) and everything follows.

## Beyond the SaaS look

`Theme::duotone(neutral_hue, neutral_chroma, accent_hue, mode)` builds
atmospheric fields — deep green, warm paper — instead of near-gray
neutrals. Custom faces register under font roles:

```rust,ignore
let mut fonts = Fonts::embedded();
fonts.register(FamilyRole::Display, playfair_bytes.to_vec());
text("Evolution").family(FamilyRole::Display).size_px(148.0);
```

The repository's `poster` example reproduces an editorial study-guide
cover this way — golden-tested like everything else. The point: design
languages are code, and beauty is testable.
