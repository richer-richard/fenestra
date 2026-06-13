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
    /// Let layout decide.
    #[default]
    Auto,
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
}

impl From<Color> for Paint {
    fn from(c: Color) -> Self {
        Self::Solid(c)
    }
}

/// A uniform border: width and color (v1; per-side borders are out of scope).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Border {
    /// Stroke width in logical pixels.
    pub width: f32,
    /// Stroke color.
    pub color: Color,
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
    /// Tabular (fixed-width) figures via the `tnum` OpenType feature, so
    /// digits align in columns. For tables, timers, and numeric data.
    pub tabular_nums: bool,
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
    /// Per-corner radii.
    pub corner_radius: CornerRadius,
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
            corner_radius: CornerRadius::default(),
            shadow_token: None,
            shadows: Vec::new(),
            highlight_top: None,
            opacity: 1.0,
            scale: 1.0,
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

    /// Uniform border.
    pub fn border(mut self, width: f32, color: Color) -> Self {
        self.border = Some(Border { width, color });
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

    /// Tabular (fixed-width) numerals — digits align in columns. For tables,
    /// timers, charts, and any numeric data that updates in place.
    pub fn tabular(mut self) -> Self {
        self.text.tabular_nums = true;
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
