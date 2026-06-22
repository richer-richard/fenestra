//! The fully-typed style IR. Three groups: layout (maps 1:1 onto taffy),
//! paint, and text. No CSS strings anywhere; every property autocompletes.

use peniko::Color;

use crate::tokens::{FamilyRole, TextSize, Weight};

/// A length value for sizes and flex basis.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Length {
    /// Logical pixels.
    Px(f32),
    /// Percent of the parent, 0.0..=100.0.
    Pct(f32),
    /// A reading measure in CSS `ch` units: 1ch is the advance width of the
    /// digit `'0'` in the element's own resolved text style. Used to cap a
    /// prose column near the optimal line length (the default
    /// [`MEASURE_CH`](crate::MEASURE_CH) is calibrated for ~66 characters)
    /// independent of window width. Resolved to `Px` during layout, where font
    /// metrics are available; treat it as `Auto` if it ever reaches taffy
    /// unresolved.
    Ch(f32),
    /// Let layout decide.
    #[default]
    Auto,
}

impl Length {
    /// Resolves a `Ch(n)` to `Px(n * ch_px)`; other variants pass through.
    pub(crate) fn resolved(self, ch_px: f32) -> Length {
        match self {
            Length::Ch(n) => Length::Px(n * ch_px),
            other => other,
        }
    }
}

/// Display mode of a box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Display {
    /// Flexbox container (the default).
    #[default]
    Flex,
    /// Grid container.
    Grid,
    /// Removed from layout entirely.
    None,
}

/// Main axis direction of a flex container.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Direction {
    /// Left to right.
    #[default]
    Row,
    /// Top to bottom.
    Column,
}

/// Cross-axis alignment of children.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlignItems {
    /// Stretch to fill the cross axis (CSS default).
    #[default]
    Stretch,
    /// Pack toward the start.
    Start,
    /// Center.
    Center,
    /// Pack toward the end.
    End,
    /// Align children on their first text baseline (rows only). Boxes
    /// without text use their bottom edge, like CSS synthesized baselines.
    Baseline,
}

/// Main-axis distribution of children.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JustifyContent {
    /// Pack toward the start (the default).
    #[default]
    Start,
    /// Center.
    Center,
    /// Pack toward the end.
    End,
    /// Distribute with space between items.
    SpaceBetween,
}

/// Multi-line content alignment (flex wrap / grid).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlignContent {
    /// Pack lines toward the start.
    #[default]
    Start,
    /// Center lines.
    Center,
    /// Pack lines toward the end.
    End,
    /// Stretch lines to fill.
    Stretch,
    /// Distribute with space between lines.
    SpaceBetween,
}

/// Positioning scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Position {
    /// Normal flow; inset offsets visually only.
    #[default]
    Relative,
    /// Out of flow, positioned against the nearest relative ancestor.
    Absolute,
}

/// Overflow behavior per axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Overflow {
    /// Content may paint outside the box.
    #[default]
    Visible,
    /// Content is clipped to the box.
    Hidden,
    /// Content is clipped and the box scrolls (M3).
    Scroll,
}

/// Per-edge values (padding, margin, inset).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Edges {
    /// Top edge.
    pub top: f32,
    /// Right edge.
    pub right: f32,
    /// Bottom edge.
    pub bottom: f32,
    /// Left edge.
    pub left: f32,
}

impl Edges {
    /// The same value on all four edges.
    pub const fn all(v: f32) -> Self {
        Self {
            top: v,
            right: v,
            bottom: v,
            left: v,
        }
    }
}

/// Optional per-edge offsets for positioned elements.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Inset {
    /// Offset from the top.
    pub top: Option<f32>,
    /// Offset from the right.
    pub right: Option<f32>,
    /// Offset from the bottom.
    pub bottom: Option<f32>,
    /// Offset from the left.
    pub left: Option<f32>,
}

/// A grid track size: fixed logical pixels or a fraction of free space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Track {
    /// Fixed logical pixels.
    Px(f32),
    /// Fraction of remaining space (CSS `fr`).
    Fr(f32),
}

/// Grid item placement on one axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GridPlace {
    /// 1-based start line; `None` for auto.
    pub start: Option<i16>,
    /// Number of tracks to span; `None` for 1 (or auto).
    pub span: Option<u16>,
}

/// A gradient color stop: offset 0.0..=1.0 and a color.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GradientStop {
    /// Position along the gradient, 0.0..=1.0.
    pub offset: f32,
    /// Color at this stop.
    pub color: Color,
}

/// Background paint.
#[derive(Debug, Clone, PartialEq)]
pub enum Paint {
    /// A solid color.
    Solid(Color),
    /// A linear gradient at a CSS-style angle in degrees: 0 points up,
    /// 90 points right. Endpoints are computed from the element's rect.
    LinearGradient {
        /// CSS-style angle in degrees.
        angle_deg: f32,
        /// Color stops, offsets 0.0..=1.0.
        stops: Vec<GradientStop>,
    },
    /// A radial gradient centered at `center` (unit coordinates within the
    /// element rect) reaching `radius` times half the rect diagonal.
    RadialGradient {
        /// Center in unit coordinates (0.5, 0.5 is the middle).
        center: (f32, f32),
        /// Radius as a multiple of half the rect's larger side.
        radius: f32,
        /// Color stops, offsets 0.0..=1.0.
        stops: Vec<GradientStop>,
    },
    /// A conic (sweep) gradient centered at `center` (unit coordinates within
    /// the element rect), sweeping the stops once around the full circle.
    ConicGradient {
        /// Center in unit coordinates (0.5, 0.5 is the middle).
        center: (f32, f32),
        /// Color stops, offsets 0.0..=1.0 mapped around the sweep.
        stops: Vec<GradientStop>,
    },
}

impl From<Color> for Paint {
    fn from(c: Color) -> Self {
        Self::Solid(c)
    }
}

/// Expands OKLCH-interpolated color stops between anchor colors. Each adjacent
/// anchor pair is walked in `steps` sub-segments through OKLCH (shortest hue
/// arc, achromatic-endpoint handling, gamut-clamped — the exact OKLCH lerp the
/// transition engine animates colors along), so the rendered ramp stays
/// perceptually even with no desaturated "gray dead-zone" through the middle of
/// a wide-hue transition. The vello renderer interpolates the returned stops in
/// sRGB, but they sit densely on the OKLCH curve, so the on-screen ramp tracks
/// it.
/// Anchors are `(offset, color)` with offsets in `0.0..=1.0`; they are sorted
/// ascending and the endpoints are preserved exactly. Colors must come from
/// theme tokens or [`oklch`](crate::oklch) / [`oklch_of`](crate::oklch_of) —
/// never a raw hex literal.
///
/// Edge cases: empty anchors yield `vec![]`; a single anchor yields one stop at
/// its offset; `steps == 0` yields the (sorted) anchors verbatim, un-interpolated.
#[must_use]
pub fn oklch_stops(anchors: &[(f32, Color)], steps: usize) -> Vec<GradientStop> {
    if anchors.is_empty() {
        return Vec::new();
    }
    let mut sorted = anchors.to_vec();
    sorted.sort_by(|a, b| a.0.total_cmp(&b.0));
    // A single anchor (or no sub-segments requested) has nothing to walk
    // through; emit the anchors verbatim.
    if sorted.len() == 1 || steps == 0 {
        return sorted
            .iter()
            .map(|&(offset, color)| GradientStop { offset, color })
            .collect();
    }
    let mut out = Vec::with_capacity((sorted.len() - 1) * steps + 1);
    for (seg, pair) in sorted.windows(2).enumerate() {
        let (o0, c0) = pair[0];
        let (o1, c1) = pair[1];
        // The shared boundary stop is emitted once: skip j == 0 on every
        // segment after the first.
        let first = usize::from(seg != 0);
        for j in first..=steps {
            #[expect(clippy::cast_precision_loss, reason = "gradient step counts are tiny")]
            let t = j as f32 / steps as f32;
            out.push(GradientStop {
                offset: o0 + (o1 - o0) * t,
                color: crate::anim::lerp_color(c0, c1, t),
            });
        }
    }
    out
}

/// Spaces `colors` evenly across `0.0..=1.0` as `(offset, color)` anchors. One
/// color anchors at 0.0; the empty list yields no anchors.
fn even_anchors(colors: impl IntoIterator<Item = Color>) -> Vec<(f32, Color)> {
    let colors: Vec<Color> = colors.into_iter().collect();
    let last = colors.len().saturating_sub(1);
    if last == 0 {
        return colors.into_iter().map(|c| (0.0, c)).collect();
    }
    colors
        .into_iter()
        .enumerate()
        .map(|(i, c)| {
            #[expect(clippy::cast_precision_loss, reason = "color counts are tiny")]
            let offset = i as f32 / last as f32;
            (offset, c)
        })
        .collect()
}

/// A gradient needs at least two colors to interpolate; fewer collapses to a
/// solid fill (the lone color, or fully transparent for none) so the painter
/// never receives a degenerate one-or-zero-stop list.
fn degenerate_solid(colors: &[Color]) -> Option<Paint> {
    match colors {
        [] => Some(Paint::Solid(Color::new([0.0, 0.0, 0.0, 0.0]))),
        [c] => Some(Paint::Solid(*c)),
        _ => None,
    }
}

/// A linear [`Paint`] whose `colors` are spaced evenly across `0.0..=1.0` and
/// expanded into a perceptually smooth OKLCH ramp ([`oklch_stops`] with
/// [`GRADIENT_STEPS`](crate::GRADIENT_STEPS)). `angle_deg` is CSS-style (0 up,
/// 90 right). Reads from tokens, e.g.
/// `bg(linear_gradient(135.0, [t.accent, t.accent_text]))`. Fewer than two
/// colors collapse to a solid fill (one color, or transparent for none).
#[must_use]
pub fn linear_gradient(angle_deg: f32, colors: impl IntoIterator<Item = Color>) -> Paint {
    let colors: Vec<Color> = colors.into_iter().collect();
    degenerate_solid(&colors).unwrap_or_else(|| Paint::LinearGradient {
        angle_deg,
        stops: oklch_stops(&even_anchors(colors), crate::tokens::GRADIENT_STEPS),
    })
}

/// A radial [`Paint`] (see [`Paint::RadialGradient`] for `center` / `radius`)
/// whose `colors` are spaced evenly across `0.0..=1.0` and expanded into an
/// OKLCH ramp ([`oklch_stops`] with [`GRADIENT_STEPS`](crate::GRADIENT_STEPS)).
/// Fewer than two colors collapse to a solid fill (one color, or transparent).
#[must_use]
pub fn radial_gradient(
    center: (f32, f32),
    radius: f32,
    colors: impl IntoIterator<Item = Color>,
) -> Paint {
    let colors: Vec<Color> = colors.into_iter().collect();
    degenerate_solid(&colors).unwrap_or_else(|| Paint::RadialGradient {
        center,
        radius,
        stops: oklch_stops(&even_anchors(colors), crate::tokens::GRADIENT_STEPS),
    })
}

/// A conic (sweep) [`Paint`] (see [`Paint::ConicGradient`]) centered at `center`
/// (unit coordinates within the element rect), sweeping `colors` once around the
/// full circle as a smooth OKLCH ramp ([`oklch_stops`] with
/// [`GRADIENT_STEPS`](crate::GRADIENT_STEPS)). Fewer than two colors collapse to
/// a solid fill. Reads from theme tokens — never a raw hex literal.
#[must_use]
pub fn conic_gradient(center: (f32, f32), colors: impl IntoIterator<Item = Color>) -> Paint {
    let colors: Vec<Color> = colors.into_iter().collect();
    degenerate_solid(&colors).unwrap_or_else(|| Paint::ConicGradient {
        center,
        stops: oklch_stops(&even_anchors(colors), crate::tokens::GRADIENT_STEPS),
    })
}

/// A border stroke: a width and color. Apply it to every edge with
/// [`Style::border`], or to a single edge with [`Style::border_top`] and
/// friends (carried per-edge by [`EdgeBorders`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Border {
    /// Stroke width in logical pixels.
    pub width: f32,
    /// Stroke color.
    pub color: Color,
}

/// Per-edge border strokes (top/right/bottom/left), each optional and
/// independent of the uniform [`Border`]. Drawn as straight hairlines with
/// square corners — for a rounded full edge use [`Style::border`]. Lets
/// hairline-divided layouts (a header's bottom rule, a left accent rail, a
/// table's ruled rows) skip manual 1px divider children.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct EdgeBorders {
    /// Top edge stroke.
    pub top: Option<Border>,
    /// Right edge stroke.
    pub right: Option<Border>,
    /// Bottom edge stroke.
    pub bottom: Option<Border>,
    /// Left edge stroke.
    pub left: Option<Border>,
}

/// Per-corner radii in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CornerRadius {
    /// Top-left.
    pub tl: f32,
    /// Top-right.
    pub tr: f32,
    /// Bottom-right.
    pub br: f32,
    /// Bottom-left.
    pub bl: f32,
}

impl CornerRadius {
    /// The same radius on all corners.
    pub const fn all(r: f32) -> Self {
        Self {
            tl: r,
            tr: r,
            br: r,
            bl: r,
        }
    }
}

/// One drop shadow layer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shadow {
    /// Horizontal offset.
    pub dx: f32,
    /// Vertical offset.
    pub dy: f32,
    /// Blur radius (CSS semantics: gaussian std dev = blur / 2).
    pub blur: f32,
    /// Outset applied to the shadow rect before blurring.
    pub spread: f32,
    /// Shadow color (usually black at low alpha).
    pub color: Color,
}

/// Horizontal text alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TextAlign {
    /// Align to the start (left in LTR).
    #[default]
    Start,
    /// Center.
    Center,
    /// Align to the end.
    End,
}

/// How text is broken into lines once shaping has wrapped it at the
/// available width. parley line-breaks greedily (each line is filled as
/// full as it can be); these modes refine that result by re-wrapping at a
/// narrower width that the framework searches for. Part of the layout
/// cache key (so a mode flip is never cached away).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TextWrap {
    /// Greedy line breaking — parley's native behavior. The default; the
    /// only mode that costs nothing.
    #[default]
    Normal,
    /// Even line lengths (CSS `text-wrap: balance`). After the greedy wrap
    /// yields N lines, the smallest wrap width still yielding N lines is
    /// found by binary search, so lines come out evenly filled instead of
    /// `[full, full, short]`. For short, high-impact text — headings,
    /// titles, pull quotes — not body copy. Line count is preserved.
    Balance,
    /// Avoid a stranded last word (CSS `text-wrap: pretty`). If the greedy
    /// wrap leaves the final line a single short word (an orphan), the wrap
    /// width is nudged down just enough to pull a second word onto the last
    /// line, without adding a line. Best-effort: when no such width exists,
    /// the greedy result is kept unchanged. For paragraphs.
    Pretty,
}

/// Figure (numeral) shape. Old-style figures have varying heights and
/// descenders that sit naturally in serif prose; lining figures are
/// uniform cap-height digits for data and UI. `Default` leaves the font's
/// own default figures untouched. Maps to the `onum`/`lnum` OpenType features.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FigureStyle {
    /// Leave the font's default figures.
    #[default]
    Default,
    /// Lining figures (`lnum`): uniform cap-height digits.
    Lining,
    /// Old-style / text figures (`onum`): ascending and descending digits.
    OldStyle,
}

/// Figure spacing. Proportional figures are individually spaced for prose;
/// tabular figures share one advance so columns of numbers align and values
/// that update in place don't jump. `Default` leaves the font's own spacing.
/// Maps to the `pnum`/`tnum` OpenType features.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum NumericSpacing {
    /// Leave the font's default spacing.
    #[default]
    Default,
    /// Proportional figures (`pnum`): naturally spaced for prose.
    Proportional,
    /// Tabular figures (`tnum`): fixed-width for aligned columns.
    Tabular,
}

/// A typed set of OpenType features applied to a text run. Orthogonal axes:
/// figure shape (`figures`) and figure spacing (`spacing`) compose freely
/// (e.g. tabular + old-style is valid), small caps, standard ligatures, and
/// fractions are independent toggles. The default enables nothing, leaving
/// every glyph at the font's own defaults. Built into a CSS
/// `font-feature-settings` string for parley; part of the layout cache key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct FontFeatures {
    /// Figure shape (`onum`/`lnum`).
    pub figures: FigureStyle,
    /// Figure spacing (`pnum`/`tnum`).
    pub spacing: NumericSpacing,
    /// Small capitals for lowercase letters (`smcp`).
    pub small_caps: bool,
    /// Standard ligatures (`liga`): `None` = font default, `Some(false)`
    /// disables, `Some(true)` forces on. Most fonts enable `liga` already.
    pub ligatures: Option<bool>,
    /// Common fractions (`frac`): turns `1/2` into a single fraction glyph.
    pub fractions: bool,
}

impl FontFeatures {
    /// The CSS `font-feature-settings` string for these features, or `None`
    /// when nothing is enabled. Tags are emitted in a fixed order so the
    /// output is deterministic: figures, spacing, small caps, ligatures,
    /// fractions — e.g. `"onum" 1, "tnum" 1`.
    pub(crate) fn feature_string(&self) -> Option<String> {
        let mut parts: Vec<&'static str> = Vec::new();
        match self.figures {
            FigureStyle::Default => {}
            FigureStyle::Lining => parts.push("\"lnum\" 1"),
            FigureStyle::OldStyle => parts.push("\"onum\" 1"),
        }
        match self.spacing {
            NumericSpacing::Default => {}
            NumericSpacing::Proportional => parts.push("\"pnum\" 1"),
            NumericSpacing::Tabular => parts.push("\"tnum\" 1"),
        }
        if self.small_caps {
            parts.push("\"smcp\" 1");
        }
        match self.ligatures {
            None => {}
            Some(true) => parts.push("\"liga\" 1"),
            Some(false) => parts.push("\"liga\" 0"),
        }
        if self.fractions {
            parts.push("\"frac\" 1");
        }
        if parts.is_empty() {
            None
        } else {
            Some(parts.join(", "))
        }
    }
}

/// How the `opsz` (optical size) variation axis of a *variable* font is set.
/// Optical-size masters are drawn for a size range: fine, high-contrast cuts
/// at large display sizes and sturdier, more open cuts at small text sizes, so
/// a face looks right across the scale instead of one master scaled up and
/// down. Maps to CSS `font-optical-sizing` / the `opsz` `font-variation-settings`
/// axis.
///
/// This only affects faces that carry an `opsz` axis (e.g. the bundled Fraunces
/// text serif); static faces — the embedded Inter, JetBrains Mono — have no
/// such axis, so it is a no-op for them. The default ([`OpticalSizing::Default`])
/// sets *no* variation, so every element renders byte-identically to before this
/// knob existed; opt in per element.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum OpticalSizing {
    /// Leave the font's own default `opsz` (emit no variation). The default,
    /// so existing output is unchanged.
    #[default]
    Default,
    /// `opsz` tracks the rendered size in px (CSS `font-optical-sizing: auto`):
    /// small text picks up the text-optical master, large sizes the display
    /// master. The everyday choice for a variable text face.
    Auto,
    /// Pin `opsz` to a fixed axis value (the font's `opsz` units, ≈ points),
    /// independent of the rendered size. For showing one optical master at any
    /// size (specimens, deliberate contrast).
    Fixed(f32),
}

impl OpticalSizing {
    /// The `opsz` axis value to apply for text rendered at `px` logical pixels,
    /// or `None` to leave the font's default (emit no `opsz` variation).
    /// [`Auto`](Self::Auto) returns `px`; [`Fixed`](Self::Fixed) its (clamped
    /// non-negative) value.
    #[must_use]
    pub fn opsz_at(self, px: f32) -> Option<f32> {
        match self {
            OpticalSizing::Default => None,
            OpticalSizing::Auto => Some(px.max(0.0)),
            OpticalSizing::Fixed(v) => Some(v.max(0.0)),
        }
    }
}

/// The text style group. `color`, `line_height`, and `letter_spacing`
/// default to the theme/text-size tokens when `None`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TextStyle {
    /// Size on the typographic scale.
    pub size: TextSize,
    /// Free-form size in logical px, overriding the scale token (editorial
    /// display sizes). Line height defaults to 1.25 when set.
    pub size_px: Option<f32>,
    /// Font weight.
    pub weight: Weight,
    /// Text color; defaults to the theme's `text` role.
    pub color: Option<Color>,
    /// Line height multiple; defaults to the size token's value.
    pub line_height: Option<f32>,
    /// Letter spacing in em; defaults to the size token's value.
    pub letter_spacing: Option<f32>,
    /// Family role (sans or mono).
    pub family: FamilyRole,
    /// Horizontal alignment within the text box.
    pub align: TextAlign,
    /// Maximum number of lines before truncation with an ellipsis.
    pub max_lines: Option<u32>,
    /// OpenType features applied to the run (figures, spacing, small caps,
    /// ligatures, fractions). Defaults to the font's own defaults.
    pub features: FontFeatures,
    /// Line-breaking refinement (balance / pretty). Defaults to greedy
    /// [`TextWrap::Normal`], which costs nothing; other modes do extra
    /// line-break passes inside shaping for this element only.
    pub wrap: TextWrap,
    /// Optical sizing: how the `opsz` variation axis of a variable font is set.
    /// Defaults to [`OpticalSizing::Default`] (no variation), so static faces
    /// and existing output are untouched.
    pub optical: OpticalSizing,
}

/// The complete style of an element: layout, paint, and text groups.
#[derive(Debug, Clone, PartialEq)]
pub struct Style {
    // -- layout --
    /// Display mode.
    pub display: Display,
    /// Flex main axis.
    pub direction: Direction,
    /// Whether flex children wrap.
    pub wrap: bool,
    /// Cross-axis alignment of children.
    pub align_items: AlignItems,
    /// Override of the parent's `align_items` for this element.
    pub align_self: Option<AlignItems>,
    /// Main-axis distribution of children.
    pub justify_content: JustifyContent,
    /// Multi-line alignment.
    pub align_content: AlignContent,
    /// Gap between children (both axes), logical px.
    pub gap: f32,
    /// Inner padding.
    pub padding: Edges,
    /// Outer margin.
    pub margin: Edges,
    /// Offsets for positioned elements.
    pub inset: Inset,
    /// Positioning scheme.
    pub position: Position,
    /// Preferred width.
    pub width: Length,
    /// Preferred height.
    pub height: Length,
    /// Minimum width.
    pub min_width: Length,
    /// Maximum width.
    pub max_width: Length,
    /// Minimum height.
    pub min_height: Length,
    /// Maximum height.
    pub max_height: Length,
    /// Flex grow factor.
    pub flex_grow: f32,
    /// Flex shrink factor.
    pub flex_shrink: f32,
    /// Flex basis.
    pub flex_basis: Length,
    /// Grid template columns (when `display` is `Grid`).
    pub grid_template_columns: Vec<Track>,
    /// Grid template rows.
    pub grid_template_rows: Vec<Track>,
    /// Column placement when inside a grid.
    pub grid_column: GridPlace,
    /// Row placement when inside a grid.
    pub grid_row: GridPlace,
    /// Horizontal overflow.
    pub overflow_x: Overflow,
    /// Vertical overflow.
    pub overflow_y: Overflow,

    // -- paint --
    /// Background paint.
    pub fill: Option<Paint>,
    /// Uniform border.
    pub border: Option<Border>,
    /// Per-edge border strokes, independent of the uniform [`border`](Self::border).
    pub side_borders: EdgeBorders,
    /// Per-corner radii.
    pub corner_radius: CornerRadius,
    /// Continuous-curvature corner smoothing, `0.0..=1.0` (Figma's "corner
    /// smoothing"). `0.0` (the default) draws exact circular arcs, so every
    /// element renders byte-identically to before this knob existed. As it
    /// rises toward `1.0` the corners blend toward a fuller superellipse (an
    /// Apple-style squircle): the curve hugs each straight edge longer and
    /// turns more gradually, removing the curvature discontinuity ("kink")
    /// where the edge meets a circular arc. Opt in per element; the painter
    /// clamps to `0.0..=1.0`. Fill, border, and clip share one path so they
    /// stay aligned. Structural, not animated: it is never lerped, so a target
    /// state's smoothing simply wins.
    pub corner_smoothing: f32,
    /// A shadow elevation token, expanded against the theme at resolution.
    pub shadow_token: Option<crate::tokens::ShadowToken>,
    /// Concrete drop shadow layers, painted bottom-up. Filled from
    /// `shadow_token` during style resolution; may also be set directly.
    pub shadows: Vec<Shadow>,
    /// A 1px inset highlight along the top inner edge (CSS `inset 0 1px 0`):
    /// the cheapest "raised, crafted" signal on a solid control. Painted over
    /// the fill, clipped to the corner radius. Usually white at low alpha.
    pub highlight_top: Option<Color>,
    /// Opacity 0.0..=1.0 applied to the whole subtree.
    pub opacity: f32,
    /// Uniform scale applied at paint time about the element's center
    /// (1.0 = no transform). Pressed controls dip to [`crate::tokens::PRESS_SCALE`];
    /// it never affects layout or hit-testing, and it animates. Spring
    /// transitions may carry it past the target for a tactile overshoot.
    pub scale: f32,
    /// Paint-time translation in logical px `(x, y)` — never affects layout or
    /// hit-testing; animatable.
    pub translate: (f32, f32),
    /// Paint-time rotation in degrees about the element center; animatable.
    pub rotate: f32,
    /// Paint-time skew in degrees `(x, y)` about the element center; animatable.
    pub skew: (f32, f32),
    /// Clip children to the (rounded) bounds.
    pub clip: bool,
    /// Draw progress of path elements, 0.0..=1.0 (animatable; this is how
    /// check marks draw on).
    pub path_trim: f32,

    // -- text --
    /// Text properties (used by text elements; inherited defaults elsewhere).
    pub text: TextStyle,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            display: Display::default(),
            direction: Direction::default(),
            wrap: false,
            align_items: AlignItems::default(),
            align_self: None,
            justify_content: JustifyContent::default(),
            align_content: AlignContent::default(),
            gap: 0.0,
            padding: Edges::default(),
            margin: Edges::default(),
            inset: Inset::default(),
            position: Position::default(),
            width: Length::Auto,
            height: Length::Auto,
            min_width: Length::Auto,
            max_width: Length::Auto,
            min_height: Length::Auto,
            max_height: Length::Auto,
            flex_grow: 0.0,
            flex_shrink: 1.0,
            flex_basis: Length::Auto,
            grid_template_columns: Vec::new(),
            grid_template_rows: Vec::new(),
            grid_column: GridPlace::default(),
            grid_row: GridPlace::default(),
            overflow_x: Overflow::Visible,
            overflow_y: Overflow::Visible,
            fill: None,
            border: None,
            side_borders: EdgeBorders::default(),
            corner_radius: CornerRadius::default(),
            corner_smoothing: 0.0,
            shadow_token: None,
            shadows: Vec::new(),
            highlight_top: None,
            opacity: 1.0,
            scale: 1.0,
            translate: (0.0, 0.0),
            rotate: 0.0,
            skew: (0.0, 0.0),
            clip: false,
            path_trim: 1.0,
            text: TextStyle::default(),
        }
    }
}

/// A theme-aware partial style overlay: interaction variants and kit
/// widgets' deferred base styling both use this shape.
pub type ThemedFn = Box<dyn Fn(&crate::theme::Theme, Style) -> Style>;

/// Spring parameters for physical motion (see [`Transition::spring`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpringSpec {
    /// Stiffness (ω² scale): higher = snappier. 170 is gentle, 380 brisk.
    pub stiffness: f32,
    /// Damping: lower overshoots more. Critical damping ≈ 2·√stiffness.
    pub damping: f32,
}

/// Declares which properties animate between style states, and how.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transition {
    /// Animate colors (fill, border, text), lerped in OKLCH.
    pub colors: bool,
    /// Animate opacity.
    pub opacity: bool,
    /// Animate lengths (sizes, padding, radii).
    pub lengths: bool,
    /// Animate position offsets.
    pub offsets: bool,
    /// Animate shadow alpha.
    pub shadows: bool,
    /// Duration in milliseconds.
    pub duration_ms: f32,
    /// Easing curve.
    pub easing: crate::tokens::CubicBezier,
    /// Physical spring response instead of the duration+curve pair.
    /// Lengths and offsets may overshoot; colors, opacity, and shadows
    /// clamp at the target (extrapolated colors aren't colors).
    pub spring: Option<SpringSpec>,
}

/// A looping keyframe timeline: style stops at fractional times across one
/// period, sampled from the frame clock every frame. Built for ambient
/// motion (pulses, shimmers, breathing); one-shot state changes belong to
/// [`Transition`]. With reduced motion the first stop is pinned, keeping
/// headless renders deterministic.
pub struct Keyframes {
    pub(crate) stops: Vec<(f32, ThemedFn)>,
    pub(crate) duration_ms: f32,
    pub(crate) easing: crate::tokens::CubicBezier,
}

impl Keyframes {
    /// A timeline lasting `duration_ms` per cycle (looped).
    pub fn new(duration_ms: f32) -> Self {
        Self {
            stops: Vec::new(),
            duration_ms,
            easing: crate::tokens::EASE_STANDARD,
        }
    }

    /// Adds a stop at fraction `at` (clamped to 0..=1) transforming the
    /// element's resolved base style. For a seamless loop, make the styles
    /// at 0 and 1 match.
    pub fn stop(self, at: f32, f: impl Fn(Style) -> Style + 'static) -> Self {
        self.themed_stop(at, move |_, s| f(s))
    }

    /// A theme-aware stop, for keyframes that color through tokens.
    pub fn themed_stop(
        mut self,
        at: f32,
        f: impl Fn(&crate::theme::Theme, Style) -> Style + 'static,
    ) -> Self {
        self.stops.push((at.clamp(0.0, 1.0), Box::new(f)));
        self.stops.sort_by(|a, b| a.0.total_cmp(&b.0));
        self
    }

    /// Per-segment easing (standard ease by default).
    pub fn ease(mut self, easing: crate::tokens::CubicBezier) -> Self {
        self.easing = easing;
        self
    }
}

impl Transition {
    /// The standard hover transition: colors and shadow alpha over the Fast
    /// duration with standard easing.
    pub fn colors() -> Self {
        Self {
            colors: true,
            opacity: false,
            lengths: false,
            offsets: false,
            shadows: true,
            duration_ms: crate::tokens::MotionDuration::Fast.ms(),
            easing: crate::tokens::EASE_STANDARD,
            spring: None,
        }
    }

    /// Animate every animatable property over the Base duration.
    pub fn all() -> Self {
        Self {
            colors: true,
            opacity: true,
            lengths: true,
            offsets: true,
            shadows: true,
            duration_ms: crate::tokens::MotionDuration::Base.ms(),
            easing: crate::tokens::EASE_STANDARD,
            spring: None,
        }
    }

    /// Every property on a brisk spring with a touch of overshoot
    /// (stiffness 380, damping 26). Lengths and offsets carry the
    /// bounce; colors clamp at the target.
    pub fn spring() -> Self {
        Self {
            spring: Some(SpringSpec {
                stiffness: 380.0,
                damping: 26.0,
            }),
            ..Self::all()
        }
    }

    /// Overrides the spring parameters (and switches to spring motion).
    pub fn with_spring(mut self, stiffness: f32, damping: f32) -> Self {
        self.spring = Some(SpringSpec { stiffness, damping });
        self
    }

    /// Overrides the duration with a token.
    pub fn duration(mut self, d: crate::tokens::MotionDuration) -> Self {
        self.duration_ms = d.ms();
        self
    }

    /// Overrides the duration in milliseconds.
    pub fn duration_ms(mut self, ms: f32) -> Self {
        self.duration_ms = ms;
        self
    }

    /// Enables or disables length animation (sizes, padding, radii, trim).
    pub fn lengths(mut self, on: bool) -> Self {
        self.lengths = on;
        self
    }

    /// Enables or disables offset animation (inset).
    pub fn offsets(mut self, on: bool) -> Self {
        self.offsets = on;
        self
    }

    /// Enables or disables opacity animation.
    pub fn opacity(mut self, on: bool) -> Self {
        self.opacity = on;
        self
    }

    /// Overrides the easing curve.
    pub fn easing(mut self, e: crate::tokens::CubicBezier) -> Self {
        self.easing = e;
        self
    }
}

impl From<f32> for Length {
    /// Raw `f32` means logical pixels.
    fn from(v: f32) -> Self {
        Self::Px(v)
    }
}

impl Style {
    // -- layout: padding --

    /// Padding on all edges.
    pub fn p(mut self, v: f32) -> Self {
        self.padding = Edges::all(v);
        self
    }

    /// Horizontal padding (left and right).
    pub fn px(mut self, v: f32) -> Self {
        self.padding.left = v;
        self.padding.right = v;
        self
    }

    /// Vertical padding (top and bottom).
    pub fn py(mut self, v: f32) -> Self {
        self.padding.top = v;
        self.padding.bottom = v;
        self
    }

    /// Top padding.
    pub fn pt(mut self, v: f32) -> Self {
        self.padding.top = v;
        self
    }

    /// Right padding.
    pub fn pr(mut self, v: f32) -> Self {
        self.padding.right = v;
        self
    }

    /// Bottom padding.
    pub fn pb(mut self, v: f32) -> Self {
        self.padding.bottom = v;
        self
    }

    /// Left padding.
    pub fn pl(mut self, v: f32) -> Self {
        self.padding.left = v;
        self
    }

    // -- layout: margin --

    /// Margin on all edges.
    pub fn m(mut self, v: f32) -> Self {
        self.margin = Edges::all(v);
        self
    }

    /// Horizontal margin.
    pub fn mx(mut self, v: f32) -> Self {
        self.margin.left = v;
        self.margin.right = v;
        self
    }

    /// Vertical margin.
    pub fn my(mut self, v: f32) -> Self {
        self.margin.top = v;
        self.margin.bottom = v;
        self
    }

    /// Top margin.
    pub fn mt(mut self, v: f32) -> Self {
        self.margin.top = v;
        self
    }

    /// Right margin.
    pub fn mr(mut self, v: f32) -> Self {
        self.margin.right = v;
        self
    }

    /// Bottom margin.
    pub fn mb(mut self, v: f32) -> Self {
        self.margin.bottom = v;
        self
    }

    /// Left margin.
    pub fn ml(mut self, v: f32) -> Self {
        self.margin.left = v;
        self
    }

    // -- layout: size and flex --

    /// Gap between children, both axes.
    pub fn gap(mut self, v: f32) -> Self {
        self.gap = v;
        self
    }

    /// Preferred width. Raw `f32` means logical px; use `Length::Pct`/`Auto`.
    pub fn w(mut self, v: impl Into<Length>) -> Self {
        self.width = v.into();
        self
    }

    /// Preferred height.
    pub fn h(mut self, v: impl Into<Length>) -> Self {
        self.height = v.into();
        self
    }

    /// Minimum width.
    pub fn min_w(mut self, v: impl Into<Length>) -> Self {
        self.min_width = v.into();
        self
    }

    /// Maximum width.
    pub fn max_w(mut self, v: impl Into<Length>) -> Self {
        self.max_width = v.into();
        self
    }

    /// Minimum height.
    pub fn min_h(mut self, v: impl Into<Length>) -> Self {
        self.min_height = v.into();
        self
    }

    /// Maximum height.
    pub fn max_h(mut self, v: impl Into<Length>) -> Self {
        self.max_height = v.into();
        self
    }

    /// Width 100%.
    pub fn w_full(mut self) -> Self {
        self.width = Length::Pct(100.0);
        self
    }

    /// Height 100%.
    pub fn h_full(mut self) -> Self {
        self.height = Length::Pct(100.0);
        self
    }

    /// Caps this element's width at a reading measure of `chars` `ch` units
    /// (a `ch`-based `max-width`). 1ch is the advance of `'0'` in this
    /// element's resolved text style, so the column is `chars` `'0'`-widths
    /// wide regardless of how wide the window is — a proportional face fits
    /// somewhat *more* than `chars` real glyphs per line (the default
    /// [`MEASURE_CH`](crate::MEASURE_CH) is tuned for ~66 characters). Set the
    /// element's text `size` and `family` to the prose it wraps so the measure
    /// tracks the real text.
    pub fn measure(mut self, chars: f32) -> Self {
        self.max_width = Length::Ch(chars);
        self
    }

    /// Preferred width in `ch` units (see [`Length::Ch`]).
    pub fn w_ch(mut self, chars: f32) -> Self {
        self.width = Length::Ch(chars);
        self
    }

    /// Minimum width in `ch` units.
    pub fn min_w_ch(mut self, chars: f32) -> Self {
        self.min_width = Length::Ch(chars);
        self
    }

    /// Maximum width in `ch` units (alias of [`Style::measure`] for symmetry).
    pub fn max_w_ch(mut self, chars: f32) -> Self {
        self.max_width = Length::Ch(chars);
        self
    }

    /// Flex grow 1.
    pub fn grow(mut self) -> Self {
        self.flex_grow = 1.0;
        self
    }

    /// Flex shrink 0.
    pub fn shrink0(mut self) -> Self {
        self.flex_shrink = 0.0;
        self
    }

    // -- layout: alignment --

    /// Align children to the cross-axis start.
    pub fn items_start(mut self) -> Self {
        self.align_items = AlignItems::Start;
        self
    }

    /// Center children on the cross axis.
    pub fn items_center(mut self) -> Self {
        self.align_items = AlignItems::Center;
        self
    }

    /// Align children to the cross-axis end.
    pub fn items_end(mut self) -> Self {
        self.align_items = AlignItems::End;
        self
    }

    /// Align children on their first text baseline (rows only).
    pub fn items_baseline(mut self) -> Self {
        self.align_items = AlignItems::Baseline;
        self
    }

    /// Override the parent's cross-axis alignment for this element alone,
    /// packing it toward the cross-axis start (so it hugs its content instead
    /// of stretching).
    pub fn self_start(mut self) -> Self {
        self.align_self = Some(AlignItems::Start);
        self
    }

    /// Override the parent's cross-axis alignment for this element alone,
    /// centering it on the cross axis.
    pub fn self_center(mut self) -> Self {
        self.align_self = Some(AlignItems::Center);
        self
    }

    /// Override the parent's cross-axis alignment for this element alone,
    /// packing it toward the cross-axis end.
    pub fn self_end(mut self) -> Self {
        self.align_self = Some(AlignItems::End);
        self
    }

    /// Override the parent's cross-axis alignment for this element alone,
    /// stretching it to fill the cross axis.
    pub fn self_stretch(mut self) -> Self {
        self.align_self = Some(AlignItems::Stretch);
        self
    }

    /// Pack children toward the main-axis start.
    pub fn justify_start(mut self) -> Self {
        self.justify_content = JustifyContent::Start;
        self
    }

    /// Center children on the main axis.
    pub fn justify_center(mut self) -> Self {
        self.justify_content = JustifyContent::Center;
        self
    }

    /// Pack children toward the main-axis end.
    pub fn justify_end(mut self) -> Self {
        self.justify_content = JustifyContent::End;
        self
    }

    /// Distribute children with space between.
    pub fn justify_between(mut self) -> Self {
        self.justify_content = JustifyContent::SpaceBetween;
        self
    }

    /// Allow flex children to wrap.
    pub fn wrap(mut self) -> Self {
        self.wrap = true;
        self
    }

    // -- layout: position and overflow --

    /// Position absolutely against the nearest relative ancestor.
    pub fn absolute(mut self) -> Self {
        self.position = Position::Absolute;
        self
    }

    /// Offset from the top (positioned elements).
    pub fn top(mut self, v: f32) -> Self {
        self.inset.top = Some(v);
        self
    }

    /// Offset from the right.
    pub fn right(mut self, v: f32) -> Self {
        self.inset.right = Some(v);
        self
    }

    /// Offset from the bottom.
    pub fn bottom(mut self, v: f32) -> Self {
        self.inset.bottom = Some(v);
        self
    }

    /// Offset from the left.
    pub fn left(mut self, v: f32) -> Self {
        self.inset.left = Some(v);
        self
    }

    /// Clip children to the (rounded) bounds.
    pub fn overflow_hidden(mut self) -> Self {
        self.overflow_x = Overflow::Hidden;
        self.overflow_y = Overflow::Hidden;
        self.clip = true;
        self
    }

    /// Vertical scrolling with clipped content (scroll state lands in M3).
    pub fn scroll_y(mut self) -> Self {
        self.overflow_y = Overflow::Scroll;
        self.clip = true;
        self
    }

    // -- paint --

    /// Background fill: a solid color or gradient.
    pub fn bg(mut self, paint: impl Into<Paint>) -> Self {
        self.fill = Some(paint.into());
        self
    }

    /// Uniform border (a stroke on the element's edge).
    pub fn border(mut self, width: f32, color: Color) -> Self {
        self.border = Some(Border { width, color });
        self
    }

    /// A border stroke on just the top edge (straight hairline, square corners).
    pub fn border_top(mut self, width: f32, color: Color) -> Self {
        self.side_borders.top = Some(Border { width, color });
        self
    }

    /// A border stroke on just the right edge.
    pub fn border_right(mut self, width: f32, color: Color) -> Self {
        self.side_borders.right = Some(Border { width, color });
        self
    }

    /// A border stroke on just the bottom edge.
    pub fn border_bottom(mut self, width: f32, color: Color) -> Self {
        self.side_borders.bottom = Some(Border { width, color });
        self
    }

    /// A border stroke on just the left edge.
    pub fn border_left(mut self, width: f32, color: Color) -> Self {
        self.side_borders.left = Some(Border { width, color });
        self
    }

    /// A crisp `width`-px ring *just outside* the box, hugging the corner
    /// radius — the "ring, not border" look (Geist). Rendered as a zero-blur
    /// spread shadow, so unlike [`border`](Self::border) (an edge stroke) it
    /// sits outside the element, never covers its content or children, and
    /// recolors with zero layout cost — ideal for selection/emphasis rings and
    /// sub-pixel hairlines. Composes with shadow tokens (the ring paints on top
    /// of any drop shadow). Stack multiple rings by calling it more than once.
    pub fn ring(mut self, width: f32, color: Color) -> Self {
        self.shadows.push(Shadow {
            dx: 0.0,
            dy: 0.0,
            blur: 0.0,
            spread: width,
            color,
        });
        self
    }

    /// The same corner radius on all corners.
    pub fn rounded(mut self, r: f32) -> Self {
        self.corner_radius = CornerRadius::all(r);
        self
    }

    /// Fully-rounded corners (pill / circle).
    pub fn rounded_full(mut self) -> Self {
        self.corner_radius = CornerRadius::all(crate::tokens::R_FULL);
        self
    }

    /// Rounds the top two corners, leaving the others unchanged.
    pub fn rounded_t(mut self, r: f32) -> Self {
        self.corner_radius.tl = r;
        self.corner_radius.tr = r;
        self
    }

    /// Rounds the bottom two corners, leaving the others unchanged.
    pub fn rounded_b(mut self, r: f32) -> Self {
        self.corner_radius.br = r;
        self.corner_radius.bl = r;
        self
    }

    /// Rounds the left two corners, leaving the others unchanged.
    pub fn rounded_l(mut self, r: f32) -> Self {
        self.corner_radius.tl = r;
        self.corner_radius.bl = r;
        self
    }

    /// Rounds the right two corners, leaving the others unchanged.
    pub fn rounded_r(mut self, r: f32) -> Self {
        self.corner_radius.tr = r;
        self.corner_radius.br = r;
        self
    }

    /// Sets each corner radius independently: top-left, top-right,
    /// bottom-right, bottom-left (clockwise from the top-left).
    pub fn corners(mut self, tl: f32, tr: f32, br: f32, bl: f32) -> Self {
        self.corner_radius = CornerRadius { tl, tr, br, bl };
        self
    }

    /// Continuous-curvature corner smoothing, `0.0..=1.0` (see
    /// [`Style::corner_smoothing`]). `0.0` keeps exact circular arcs; higher
    /// values blend toward a fuller squircle. Clamped to `0.0..=1.0`.
    pub fn corner_smoothing(mut self, s: f32) -> Self {
        self.corner_smoothing = s.clamp(0.0, 1.0);
        self
    }

    /// A shadow elevation token, resolved against the theme at render time.
    pub fn shadow(mut self, token: crate::tokens::ShadowToken) -> Self {
        self.shadow_token = Some(token);
        self
    }

    /// A 1px inset highlight along the top inner edge — the subtle top sheen
    /// that makes a solid control read as raised. Usually a low-alpha white.
    pub fn highlight_top(mut self, color: Color) -> Self {
        self.highlight_top = Some(color);
        self
    }

    /// Subtree opacity 0.0..=1.0.
    pub fn opacity(mut self, v: f32) -> Self {
        self.opacity = v;
        self
    }

    /// Paint-time uniform scale about the element center (1.0 = none). Used
    /// for press feedback; never disturbs layout.
    pub fn scale(mut self, v: f32) -> Self {
        self.scale = v;
        self
    }

    /// Paint-time translation in logical px (never affects layout). Animatable.
    pub fn translate(mut self, x: f32, y: f32) -> Self {
        self.translate = (x, y);
        self
    }

    /// Paint-time rotation in degrees about the element center. Animatable.
    pub fn rotate(mut self, degrees: f32) -> Self {
        self.rotate = degrees;
        self
    }

    /// Paint-time skew in degrees `(x, y)` about the element center. Animatable.
    pub fn skew(mut self, x_degrees: f32, y_degrees: f32) -> Self {
        self.skew = (x_degrees, y_degrees);
        self
    }

    /// Draw progress for path elements (0 = nothing, 1 = full path).
    pub fn trim(mut self, v: f32) -> Self {
        self.path_trim = v.clamp(0.0, 1.0);
        self
    }

    // -- text --

    /// Text size on the typographic scale.
    /// Free-form text size in logical px (overrides the scale token).
    pub fn size_px(mut self, px: f32) -> Self {
        self.text.size_px = Some(px);
        self
    }

    /// Letter spacing in em (tracked-out editorial eyebrows etc.).
    pub fn tracking(mut self, em: f32) -> Self {
        self.text.letter_spacing = Some(em);
        self
    }

    /// Line height as a multiple of the font size.
    pub fn leading(mut self, multiple: f32) -> Self {
        self.text.line_height = Some(multiple);
        self
    }

    /// Tabular (fixed-width) numerals (`tnum`) — digits align in columns. For
    /// tables, timers, charts, and any numeric data that updates in place.
    pub fn tabular(mut self) -> Self {
        self.text.features.spacing = NumericSpacing::Tabular;
        self
    }

    /// Proportional numerals — individually spaced for prose (`pnum`).
    pub fn proportional_nums(mut self) -> Self {
        self.text.features.spacing = NumericSpacing::Proportional;
        self
    }

    /// Old-style / text figures (`onum`): ascending and descending digits
    /// that sit naturally in serif prose.
    pub fn oldstyle_nums(mut self) -> Self {
        self.text.features.figures = FigureStyle::OldStyle;
        self
    }

    /// Lining figures (`lnum`): uniform cap-height digits for data and UI.
    pub fn lining_nums(mut self) -> Self {
        self.text.features.figures = FigureStyle::Lining;
        self
    }

    /// Render lowercase letters as small capitals (`smcp`).
    pub fn small_caps(mut self) -> Self {
        self.text.features.small_caps = true;
        self
    }

    /// Enable or disable standard ligatures (`liga`); most fonts default on.
    pub fn ligatures(mut self, on: bool) -> Self {
        self.text.features.ligatures = Some(on);
        self
    }

    /// Common fractions (`frac`): `1/2` becomes a single fraction glyph.
    pub fn fractions(mut self) -> Self {
        self.text.features.fractions = true;
        self
    }

    /// Font family role (Sans, Mono, or a registered Display/Serif face).
    pub fn family(mut self, family: crate::tokens::FamilyRole) -> Self {
        self.text.family = family;
        self
    }

    pub fn size(mut self, size: crate::tokens::TextSize) -> Self {
        self.text.size = size;
        self
    }

    /// Font weight.
    pub fn weight(mut self, weight: crate::tokens::Weight) -> Self {
        self.text.weight = weight;
        self
    }

    /// Text color (defaults to the theme `text` role).
    pub fn color(mut self, color: Color) -> Self {
        self.text.color = Some(color);
        self
    }

    /// Use the mono family role.
    pub fn mono(mut self) -> Self {
        self.text.family = crate::tokens::FamilyRole::Mono;
        self
    }

    /// Truncate to one line with an ellipsis.
    pub fn truncate(mut self) -> Self {
        self.text.max_lines = Some(1);
        self
    }

    /// Horizontal text alignment.
    pub fn text_align(mut self, align: TextAlign) -> Self {
        self.text.align = align;
        self
    }

    /// Balance line lengths for this text ([`TextWrap::Balance`]) — even lines
    /// instead of a full-then-short ragged break. For headings and titles.
    pub fn balance(mut self) -> Self {
        self.text.wrap = TextWrap::Balance;
        self
    }

    /// Avoid a stranded last word ([`TextWrap::Pretty`]) — best-effort for
    /// paragraphs; never adds a line and never makes the break worse.
    pub fn pretty(mut self) -> Self {
        self.text.wrap = TextWrap::Pretty;
        self
    }

    /// Sets the line-breaking mode explicitly ([`TextWrap`]).
    pub fn text_wrap(mut self, wrap: TextWrap) -> Self {
        self.text.wrap = wrap;
        self
    }

    /// Sets optical sizing explicitly ([`OpticalSizing`]) — how the `opsz`
    /// variation axis of a variable font is driven.
    pub fn optical(mut self, optical: OpticalSizing) -> Self {
        self.text.optical = optical;
        self
    }

    /// Tracks the `opsz` axis to the rendered size ([`OpticalSizing::Auto`],
    /// CSS `font-optical-sizing: auto`) — small text gets the text-optical
    /// master, large sizes the display master. A no-op on static faces.
    pub fn optical_auto(mut self) -> Self {
        self.text.optical = OpticalSizing::Auto;
        self
    }
}

impl Style {
    // -- grid --

    /// Grid template columns (switches display to grid).
    pub fn grid_cols(mut self, tracks: impl IntoIterator<Item = Track>) -> Self {
        self.display = Display::Grid;
        self.grid_template_columns = tracks.into_iter().collect();
        self
    }

    /// Grid template rows (switches display to grid).
    pub fn grid_rows(mut self, tracks: impl IntoIterator<Item = Track>) -> Self {
        self.display = Display::Grid;
        self.grid_template_rows = tracks.into_iter().collect();
        self
    }

    /// Places this element at a 1-based grid column, spanning `span` tracks.
    pub fn grid_col(mut self, start: i16, span: u16) -> Self {
        self.grid_column = GridPlace {
            start: Some(start),
            span: (span > 1).then_some(span),
        };
        self
    }

    /// Places this element at a 1-based grid row, spanning `span` tracks.
    pub fn grid_row(mut self, start: i16, span: u16) -> Self {
        self.grid_row = GridPlace {
            start: Some(start),
            span: (span > 1).then_some(span),
        };
        self
    }
}

impl Style {
    /// True if any size constraint is expressed in `ch` (needs font metrics
    /// to resolve before taffy runs).
    pub(crate) fn has_ch(&self) -> bool {
        matches!(self.width, Length::Ch(_))
            || matches!(self.min_width, Length::Ch(_))
            || matches!(self.max_width, Length::Ch(_))
            || matches!(self.height, Length::Ch(_))
            || matches!(self.min_height, Length::Ch(_))
            || matches!(self.max_height, Length::Ch(_))
            || matches!(self.flex_basis, Length::Ch(_))
    }

    /// Replaces every `Length::Ch(n)` size constraint with `Length::Px(n *
    /// ch_px)`, using the advance of `'0'` in this element's resolved text
    /// style. Called during `build` once font metrics are available.
    pub(crate) fn resolve_ch(&mut self, ch_px: f32) {
        self.width = self.width.resolved(ch_px);
        self.min_width = self.min_width.resolved(ch_px);
        self.max_width = self.max_width.resolved(ch_px);
        self.height = self.height.resolved(ch_px);
        self.min_height = self.min_height.resolved(ch_px);
        self.max_height = self.max_height.resolved(ch_px);
        self.flex_basis = self.flex_basis.resolved(ch_px);
    }
}

#[cfg(test)]
mod corner_smoothing_tests {
    use super::*;

    #[test]
    fn corner_smoothing_defaults_zero_and_clamps() {
        assert_eq!(Style::default().corner_smoothing, 0.0);
        assert_eq!(Style::default().corner_smoothing(0.6).corner_smoothing, 0.6);
        assert_eq!(Style::default().corner_smoothing(5.0).corner_smoothing, 1.0);
        assert_eq!(
            Style::default().corner_smoothing(-1.0).corner_smoothing,
            0.0
        );
    }
}

#[cfg(test)]
mod feature_tests {
    use super::*;

    /// The feature string of a style built with the given builders.
    fn fs(style: Style) -> Option<String> {
        style.text.features.feature_string()
    }

    #[test]
    fn default_features_emit_nothing() {
        assert_eq!(FontFeatures::default().feature_string(), None);
    }

    #[test]
    fn tabular_unchanged() {
        // Locks the exact prior `.tabular()` behavior (`"tnum" 1`), so every
        // existing golden that uses it stays byte-identical.
        assert_eq!(
            fs(Style::default().tabular()),
            Some("\"tnum\" 1".to_owned())
        );
    }

    #[test]
    fn oldstyle_and_smcp() {
        let s = fs(Style::default().oldstyle_nums().small_caps()).unwrap();
        assert!(s.contains("\"onum\" 1"), "{s}");
        assert!(s.contains("\"smcp\" 1"), "{s}");
        assert!(!s.contains("\"lnum\""), "{s}");
        assert!(!s.contains("\"tnum\""), "{s}");
        assert!(!s.contains("\"pnum\""), "{s}");
    }

    #[test]
    fn tnum_onum_mutually_consistent() {
        // Figure shape and figure spacing are orthogonal axes; both apply.
        let s = fs(Style::default().tabular().oldstyle_nums()).unwrap();
        assert!(s.contains("\"tnum\" 1"), "{s}");
        assert!(s.contains("\"onum\" 1"), "{s}");
    }

    #[test]
    fn ligatures_off_and_on() {
        assert!(
            fs(Style::default().ligatures(false))
                .unwrap()
                .contains("\"liga\" 0")
        );
        assert!(
            fs(Style::default().ligatures(true))
                .unwrap()
                .contains("\"liga\" 1")
        );
    }

    #[test]
    fn fractions_and_proportional() {
        assert!(
            fs(Style::default().fractions())
                .unwrap()
                .contains("\"frac\" 1")
        );
        assert!(
            fs(Style::default().proportional_nums())
                .unwrap()
                .contains("\"pnum\" 1")
        );
    }

    #[test]
    fn figure_axis_is_exclusive() {
        // The figure axis is one slot: the last builder wins.
        let style = Style::default().oldstyle_nums().lining_nums();
        assert_eq!(style.text.features.figures, FigureStyle::Lining);
        let s = fs(style).unwrap();
        assert!(s.contains("\"lnum\""), "{s}");
        assert!(!s.contains("\"onum\""), "{s}");
    }
}

#[cfg(test)]
mod gradient_tests {
    use super::*;
    use crate::oklch_of;
    use crate::theme::Theme;
    use crate::tokens::GRADIENT_STEPS;

    #[test]
    fn midpoint_keeps_chroma_no_gray_deadzone() {
        // A wide-hue transition (accent ~262° → warning ~80°): the naive sRGB
        // average of the two anchors collapses toward gray, but the OKLCH
        // midpoint stays vivid.
        let theme = Theme::light();
        let a = theme.accent;
        let b = theme.warning.solid;
        let stops = oklch_stops(&[(0.0, a), (1.0, b)], GRADIENT_STEPS);
        let mid = stops
            .iter()
            .min_by(|x, y| (x.offset - 0.5).abs().total_cmp(&(y.offset - 0.5).abs()))
            .unwrap();
        let ca = a.components;
        let cb = b.components;
        let srgb_mid = Color::new([
            (ca[0] + cb[0]) / 2.0,
            (ca[1] + cb[1]) / 2.0,
            (ca[2] + cb[2]) / 2.0,
            (ca[3] + cb[3]) / 2.0,
        ]);
        let c_oklch = oklch_of(mid.color)[1];
        let c_srgb = oklch_of(srgb_mid)[1];
        assert!(
            c_oklch > 1.5 * c_srgb,
            "OKLCH mid chroma {c_oklch} should far exceed sRGB mid chroma {c_srgb}"
        );
    }

    #[test]
    fn lightness_is_monotonic_across_stops() {
        // A9-style same-hue ramp (A7 L 0.725 → A10 L 0.545), fully in gamut:
        // lightness must never reverse (no dark bump mid-ramp). The epsilon
        // absorbs per-channel gamut-clamp noise.
        let theme = Theme::light();
        let a = theme.accents.step(7);
        let b = theme.accents.step(10);
        let stops = oklch_stops(&[(0.0, a), (1.0, b)], GRADIENT_STEPS);
        for w in stops.windows(2) {
            let l0 = oklch_of(w[0].color)[0];
            let l1 = oklch_of(w[1].color)[0];
            assert!(l1 <= l0 + 1e-3, "lightness rose mid-ramp: {l0} -> {l1}");
        }
    }

    #[test]
    fn offsets_sorted_and_span_anchors() {
        let theme = Theme::light();
        let a = theme.accent;
        let b = theme.warning.solid;
        let stops = oklch_stops(&[(0.0, a), (1.0, b)], 16);
        assert_eq!(stops.len(), 17);
        assert_eq!(stops.first().unwrap().offset, 0.0);
        assert_eq!(stops.last().unwrap().offset, 1.0);
        for w in stops.windows(2) {
            assert!(w[1].offset > w[0].offset, "offsets must strictly increase");
        }
        // Unsorted anchors must produce the identical result.
        let unsorted = oklch_stops(&[(1.0, b), (0.0, a)], 16);
        assert_eq!(unsorted, stops);
    }

    #[test]
    fn endpoints_are_exact() {
        // Pre-expansion must never shift the anchors themselves.
        let theme = Theme::light();
        let a = theme.accent;
        let b = theme.warning.solid;
        let stops = oklch_stops(&[(0.0, a), (1.0, b)], 16);
        assert_eq!(stops.first().unwrap().color.to_rgba8(), a.to_rgba8());
        assert_eq!(stops.last().unwrap().color.to_rgba8(), b.to_rgba8());
    }

    #[test]
    fn linear_gradient_even_spacing() {
        let theme = Theme::light();
        let (a, b, c) = (theme.accent, theme.warning.solid, theme.success.solid);
        let paint = linear_gradient(90.0, [a, b, c]);
        let Paint::LinearGradient { angle_deg, stops } = paint else {
            panic!("linear_gradient must build a LinearGradient");
        };
        assert_eq!(angle_deg, 90.0);
        // Three colors land evenly at 0.0 / 0.5 / 1.0, each carrying its anchor.
        for (off, color) in [(0.0, a), (0.5, b), (1.0, c)] {
            let found = stops
                .iter()
                .find(|s| (s.offset - off).abs() < 1e-4)
                .unwrap_or_else(|| panic!("no stop at offset {off}"));
            assert_eq!(
                found.color.to_rgba8(),
                color.to_rgba8(),
                "anchor color at offset {off}"
            );
        }
    }

    #[test]
    fn degenerate_inputs() {
        let theme = Theme::light();
        let a = theme.accent;
        let b = theme.warning.solid;
        // Empty anchors → empty.
        assert!(oklch_stops(&[], 16).is_empty());
        // Single anchor → one stop at its offset.
        let single = oklch_stops(&[(0.3, a)], 16);
        assert_eq!(single.len(), 1);
        assert_eq!(single[0].offset, 0.3);
        assert_eq!(single[0].color.to_rgba8(), a.to_rgba8());
        // steps == 0 → exactly the two endpoints, no interpolation.
        let zero_steps = oklch_stops(&[(0.0, a), (1.0, b)], 0);
        assert_eq!(zero_steps.len(), 2);
        assert_eq!(zero_steps[0].color.to_rgba8(), a.to_rgba8());
        assert_eq!(zero_steps[1].color.to_rgba8(), b.to_rgba8());
    }

    #[test]
    fn optical_sizing_resolves_opsz_per_mode() {
        // Default emits no axis at any size; Auto tracks the rendered px;
        // Fixed pins a value (clamped non-negative) regardless of size.
        assert_eq!(OpticalSizing::Default.opsz_at(16.0), None);
        assert_eq!(OpticalSizing::Default.opsz_at(48.0), None);
        assert_eq!(OpticalSizing::Auto.opsz_at(16.0), Some(16.0));
        assert_eq!(OpticalSizing::Auto.opsz_at(48.0), Some(48.0));
        assert_eq!(OpticalSizing::Fixed(72.0).opsz_at(16.0), Some(72.0));
        assert_eq!(OpticalSizing::Fixed(72.0).opsz_at(48.0), Some(72.0));
        // Negatives clamp to 0 (a valid axis floor, never a negative coord).
        assert_eq!(OpticalSizing::Fixed(-5.0).opsz_at(16.0), Some(0.0));
        // The default of the typed value is `Default` (no behavior change).
        assert_eq!(OpticalSizing::default(), OpticalSizing::Default);
    }

    #[test]
    fn optical_builders_set_the_axis() {
        // The ergonomic builders flow into the text style group.
        assert_eq!(Style::default().text.optical, OpticalSizing::Default);
        assert_eq!(
            Style::default().optical_auto().text.optical,
            OpticalSizing::Auto
        );
        assert_eq!(
            Style::default()
                .optical(OpticalSizing::Fixed(60.0))
                .text
                .optical,
            OpticalSizing::Fixed(60.0)
        );
    }
}
