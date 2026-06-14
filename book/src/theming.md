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

## Web-grade by default: `derive`

When you don't want to think in ramps at all, derive the whole palette from
three inputs — Linear's model, on fenestra's OKLCH scales:

```rust,ignore
let theme = Theme::derive(
    BaseField { hue: 80.0, chroma: 2.5 }, // warm paper field
    40.0,                                  // terracotta accent hue
    Contrast::High,                        // crisp ink-on-paper
    Mode::Light,
);
```

`base` is the neutral field (its hue and how far it departs from gray),
`accent_hue` the brand hue, and `contrast` (`Low` / `Standard` / `High`) scales
every step's lightness distance from the page background. `from_accent` and
`duotone` are special cases — `derive` at `Standard` contrast reproduces them
exactly — and every contrast level still clears the APCA floors, so derivation
never ships an illegible theme. Recipes carry it too:
`{"mode":"light","derive":{"base_hue":80,"base_chroma":2.5,"accent_hue":40,"contrast":"high"}}`.

A matching corner-radius family comes from one knob:
`RadiusScale::from_base(8.0)` yields `{sm, md, lg, xl}` at fenestra's ratios
(0.6 / 1.0 / 1.4 / 2.0 ×); the default base (10) reproduces `R_SM`…`R_XL`.

## The 12-step scale

Each ramp follows the Radix model, so styling is arithmetic rather than
art. The neutral steps carry names:

| step | role | step | role |
|------|------|------|------|
| 1 `bg` | app background | 7 `border_strong` | strong border |
| 2 `surface` | subtle surface | 9 `text_subtle` | low-emphasis text |
| 3 `element` | element fill | 11 `text_muted` | secondary text |
| 4 `element_hover` | hovered fill | 12 `text` | primary text |
| 5 `element_active` | pressed fill | | |
| 6 `border` | border | | |

Interaction is "+1 step": a control rests on `element`, hovers to
`element_hover`, presses to `element_active`. Solid accents do the same —
`accent` → `accent_hover` → `accent_active` (one OKLCH-lightness notch
darker), and each status color carries `solid` / `solid_hover` /
`solid_active`. Pressed colors are mode-invariant, so a button feels the
same in light and dark.

Every ramp also has a translucent **alpha twin** (`neutral_alpha`,
`accent_alpha`) — the same step rendered as the lowest-alpha color that
composites over `bg` back to the solid value. Use a twin where a tint must
read correctly over an arbitrary surface, not just the page background.

## Provably legible: APCA

Because fenestra resolves every color at construction, it can *prove* a
theme is readable — something no CSS framework can do:

```rust,ignore
let theme = Theme::from_accent(262.0, Mode::Dark);
assert!(theme.validate_contrast().is_ok()); // every text pair clears its floor
```

`validate_contrast` scores each text/background role pair with APCA
(`apca::lc`, the APCA-W3 `0.98G-4g` algorithm) against a role-tiered
lightness-contrast floor — primary text Lc 75 (the stock themes reach
90+), secondary text 55, control labels 60, colored component text 40 —
and returns the pairs that fall short. The built-in themes and every
shipped Look are asserted to pass in headless tests, and your own themes
can be too. (APCA scores text legibility, so borders and other non-text
contrast aren't checked.)

The role floors are fixed per tier, but two helpers go finer. APCA is the
draft WCAG-3 contrast method, and `apca::required_lc(size_px, weight)` exposes
its readability criterion as a function: small or thin text needs *more* Lc,
large or heavy text *less*. Feed it to `theme.contrast_ok(text, bg, size_px,
weight)` to prove a *specific* label legible at its real rendered size — not
just against a tier average. And `theme.text_on(bg)` returns the more-legible
ramp extreme — a theme-tinted neutral that reads strongly on any custom or
status surface (the `on_accent` rule generalized to colors the theme never
generated; on a hard mid-tone, where no color reads well, it lands at
secondary-text grade):

```rust,ignore
let theme = Theme::light();
// A 13px caption needs more contrast than 16px body text:
assert!(theme.contrast_ok(theme.text, theme.surface, 13.0, 400.0));
// Legible text for an arbitrary brand surface, picked from the ramp ends:
let label = theme.text_on(brand_color);
```

The role floors (75/60/55/40) are unchanged regression sentinels;
`required_lc` now anchors them to the same APCA scale (`required_lc(16, 400)`
*is* the primary-text floor, 75).

## Elevation

Shadows are layered (a tight contact shadow under a soft ambient one) and
tinted with the *surface hue* at low chroma rather than flat black, so an
editorial green field casts a green-black shadow. `ShadowToken` runs
`Xs`/`Sm`/`Md`/`Lg`/`Xl`; the `Xl` token is a three-layer overlay shadow
for modals. In dark mode, elevation lightens surfaces
(`elevated_surface(level)`) rather than relying on shadows. Solid controls
can carry a 1px inset top highlight (`.highlight_top(color)`) — the subtle
top sheen that reads as "raised."

Rather than re-typing radius + fill + border + shadow at each call site, a
`Surface` material bundles them per elevation *role* — `Card`, `Raised`,
`Popover`, `Menu`, `Modal`, `Thumb`, `Tooltip` — resolved against the theme.
`el.surface(Surface::Menu)` gives a floating panel its whole look in one call;
floating roles carry radius and shadow depth at or above the resting card by
construction, so every floating thing matches. The kit's cards, menus, modals,
toasts, tooltips, and slider thumbs all derive from this one table.

For an edge that sits *outside* the box, `.ring(width, color)` draws a crisp
band hugging the corner radius (the "ring, not border" look) — rendered as a
zero-blur spread shadow, so unlike `.border(..)` (an edge stroke) it never
covers content and recolors with zero layout cost. Reach for it for selection
and focus rings and sub-pixel hairlines; it stacks and composes with shadows.

## Typography

Letter spacing follows Inter's dynamic-metrics tracking curve at the
actual font size (positive at caption sizes, tightening as text grows), so
display sizes are tracked correctly without hand-tuning. Tabular figures
are one call — `.tabular()` — for tables, timers, and any numbers that
align in columns or update in place:

```rust,ignore
text(format!("{revenue:>10}")).tabular()
```

## Interaction, motion, and sizing tokens

Interaction is tokenized too, so the whole kit moves and reacts in one
language (the *Interactivity* chapter shows the builders). The **state layer**
(`STATE_LAYER`) is the veil a control lays over itself — hover 8%, focus and
press 12%, drag 16%. **Motion** lives in `MotionDuration` (100–300 ms) and the
Material easing curves `EASE_STANDARD` / `EASE_DECELERATE` / `EASE_ACCELERATE`;
`PRESS_SCALE` (0.97) is the pressed-control dip. The **focus ring** is
`FOCUS_RING` — a 3px halo at 0.5 alpha flush outside a ring-colored border,
recolored to the danger hue when a control is `.invalid(true)`.

Control sizes share a height grid so a row of mixed controls lines up:

| size | height | font | size | height | font |
|------|--------|------|------|--------|------|
| `Xs` | 24px | Xs | `Md` | 36px | Sm |
| `Sm` | 32px | Sm | `Lg` | 40px | Base |

`ControlSize::metrics()` resolves the full bundle — height, padding, gap, font,
icon edge — the kit's buttons, inputs, and selects build from.

One `Density` knob packs that grid tighter or looser:
`ControlSize::metrics_at(Density::Compact)` shrinks height/padding/gap/icon for
dense pro-tool UIs, `Spacious` loosens them, and `Comfortable` (the default) is
the table above — byte-identical to before density existed. Density scales
*spacing*, not *type*: the label font stays tied to the `ControlSize`, so text
never shrinks below its legible size. `density_showcase` renders all three side
by side.

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

## Theme files

Themes serialize as *recipes* — the few numbers a theme generates from,
not hundreds of resolved colors:

```json
{"mode": "dark", "duotone": {"neutral_hue": 152.0, "chroma": 6.0, "accent_hue": 72.0}}
```

`ThemeSpec::from_json(s)?.theme()` resolves through the same builders
(`Theme::dark`, `from_accent`, `duotone`); `spec.to_json()` writes one.
Unknown fields are errors, so a typo'd recipe fails loudly instead of
silently falling back. Recipes stay tiny, hand-editable, and stable
across fenestra versions.
