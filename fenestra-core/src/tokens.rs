//! Mode-independent design tokens: spacing, radii, typography, and motion.
//! These numbers are the spec; kit widgets and examples never hardcode them.

/// Spacing constants on a 4px grid, in logical pixels.
pub const SP0: f32 = 0.0;
/// 2px.
pub const SP0_5: f32 = 2.0;
/// 4px.
pub const SP1: f32 = 4.0;
/// 8px.
pub const SP2: f32 = 8.0;
/// 12px.
pub const SP3: f32 = 12.0;
/// 16px.
pub const SP4: f32 = 16.0;
/// 20px.
pub const SP5: f32 = 20.0;
/// 24px.
pub const SP6: f32 = 24.0;
/// 32px.
pub const SP8: f32 = 32.0;
/// 40px.
pub const SP10: f32 = 40.0;
/// 48px.
pub const SP12: f32 = 48.0;
/// 64px.
pub const SP16: f32 = 64.0;

/// The default reading measure, in CSS `ch` units (1ch = the advance of the
/// digit `'0'`; see [`crate::Length::Ch`]). Set to 52 so a proportional body
/// face renders ~66 characters per line — the classic optimum for sustained
/// reading (the comfortable band is 45–75). The value is below 66 on purpose:
/// `'0'` is wider than the average glyph, so a column of N `ch` holds somewhat
/// more than N real characters; 52ch lands the rendered line near 66. A wider
/// face fits fewer characters per line, a narrower one more. Prose containers
/// cap their width here via [`crate::Style::measure`].
pub const MEASURE_CH: f32 = 52.0;

/// Small radius (badges, small chips): 6px.
pub const R_SM: f32 = 6.0;
/// Medium radius (controls: buttons, inputs): 10px.
pub const R_MD: f32 = 10.0;
/// Large radius (cards): 14px.
pub const R_LG: f32 = 14.0;
/// Extra-large radius (modals): 20px.
pub const R_XL: f32 = 20.0;
/// Fully-rounded (pills, avatars); clamped to half the box size at paint.
pub const R_FULL: f32 = f32::INFINITY;

/// A corner-radius family derived from one base measure, so a theme can be
/// tighter or rounder from a single knob. The ratios (0.6 / 1.0 / 1.4 / 2.0 ×
/// base) are fenestra's: a base of `10` reproduces [`R_SM`] / [`R_MD`] /
/// [`R_LG`] / [`R_XL`] exactly, which is why the kit's constants and the
/// default scale agree.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RadiusScale {
    /// Small radius (badges, chips).
    pub sm: f32,
    /// Medium radius (controls).
    pub md: f32,
    /// Large radius (cards).
    pub lg: f32,
    /// Extra-large radius (modals).
    pub xl: f32,
}

impl RadiusScale {
    /// The family derived from a base radius: `0.6 / 1.0 / 1.4 / 2.0 × base`.
    #[must_use]
    pub fn from_base(base: f32) -> Self {
        let b = base.max(0.0);
        Self {
            sm: b * 0.6,
            md: b,
            lg: b * 1.4,
            xl: b * 2.0,
        }
    }

    /// A tight family for sharp / minimal "tech" chrome — corners read crisp and
    /// near-square: `sm 1 / md 2 / lg 3 / xl 4`. Set it on a theme with
    /// [`Theme::with_radius`](crate::Theme::with_radius) to un-round the whole
    /// kit at once; pills and avatars ([`R_FULL`]) stay round regardless. For a
    /// fully-square look, use `RadiusScale::from_base(0.0)`.
    #[must_use]
    pub fn sharp() -> Self {
        Self {
            sm: 1.0,
            md: 2.0,
            lg: 3.0,
            xl: 4.0,
        }
    }

    /// A rounder, friendlier family (base `16`): `sm 9.6 / md 16 / lg 22.4 / xl 32`
    /// — for soft, consumer/whiteboard-class tools.
    #[must_use]
    pub fn soft() -> Self {
        Self::from_base(16.0)
    }
}

impl Default for RadiusScale {
    /// The stock family (base `R_MD` = 10): `R_SM` / `R_MD` / `R_LG` / `R_XL`.
    fn default() -> Self {
        Self::from_base(R_MD)
    }
}

/// How resting, same-plane surfaces (cards) convey separation. Floating
/// surfaces — menus, popovers, modals, tooltips — always cast a shadow; this
/// only governs cards that rest in the page. Set on a theme with
/// [`Theme::with_elevation`](crate::Theme::with_elevation).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Elevation {
    /// Resting cards cast a subtle tinted shadow — the stock look.
    #[default]
    Shadowed,
    /// Resting cards lean on a border + surface tone-step instead of a shadow
    /// — sharper, and the honest choice in dark mode where shadows barely
    /// register. Shadows stay reserved for surfaces that truly float. Pairs
    /// naturally with [`RadiusScale::sharp`].
    Flat,
}

/// The typographic scale. Sizes are logical px.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TextSize {
    /// 12px.
    Xs,
    /// 14px.
    Sm,
    /// 16px (body default).
    #[default]
    Base,
    /// 20px.
    Lg,
    /// 25px.
    Xl,
    /// 31px.
    Xl2,
    /// 39px.
    Xl3,
}

impl TextSize {
    /// Font size in logical pixels.
    pub const fn px(self) -> f32 {
        match self {
            Self::Xs => 12.0,
            Self::Sm => 14.0,
            Self::Base => 16.0,
            Self::Lg => 20.0,
            Self::Xl => 25.0,
            Self::Xl2 => 31.0,
            Self::Xl3 => 39.0,
        }
    }

    /// Line height as a multiple of the font size.
    pub const fn line_height(self) -> f32 {
        match self {
            Self::Xs | Self::Sm | Self::Base => 1.5,
            Self::Lg => 1.4,
            Self::Xl | Self::Xl2 => 1.25,
            Self::Xl3 => 1.15,
        }
    }

    /// Letter spacing in em, from Inter's dynamic-metrics tracking curve
    /// ([`tracking_em`]). Multiply by `px()` for logical pixels.
    pub fn letter_spacing(self) -> f32 {
        tracking_em(self.px())
    }
}

/// Optical tracking (letter spacing) in em for a font size in logical px,
/// from Inter's published dynamic-metrics formula
/// `-0.0223 + 0.185·e^(-0.1745·px)`: a hair positive at caption sizes,
/// tightening smoothly as text grows. Applies to any size, including
/// free-form display sizes, instead of a handful of hand-set steps.
#[must_use]
pub fn tracking_em(px: f32) -> f32 {
    -0.0223 + 0.185 * (-0.1745 * px).exp()
}

/// Font weights shipped with the embedded Inter family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Weight {
    /// 400: body text.
    #[default]
    Regular,
    /// 500: labels and buttons.
    Medium,
    /// 600: headings.
    Semibold,
}

impl Weight {
    /// Numeric OpenType weight.
    pub const fn value(self) -> f32 {
        match self {
            Self::Regular => 400.0,
            Self::Medium => 500.0,
            Self::Semibold => 600.0,
        }
    }
}

/// Font family roles resolved through fontique.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FamilyRole {
    /// Inter, with system fallback.
    #[default]
    Sans,
    /// SF Mono / Cascadia Code / JetBrains Mono / monospace fallback list.
    Mono,
    /// A display face registered via `Fonts::register` (falls back to Sans
    /// until one is registered). Editorial headlines. High-contrast "display"
    /// cuts — Playfair and other Didones — are drawn for large sizes; their
    /// thin strokes shimmer or drop out below ~24px, so keep this role to
    /// headings and decks and set body text in [`Sans`](Self::Sans) or a
    /// *text* [`Serif`](Self::Serif).
    Display,
    /// A serif face registered via `Fonts::register` (falls back to Sans). For
    /// reading prose register a *text*-optical serif here and keep runs at
    /// ≥20px with generous leading; a Didone *display* face (e.g. Playfair)
    /// works as a large pull-quote but bands below ~18–20px, so it belongs
    /// under [`Display`](Self::Display), not as small body text.
    Serif,
}

/// Shadow elevation tokens. Resolved to concrete layered shadows by the
/// theme (dark mode multiplies alphas by 1.6). Ordered by depth
/// (`Xs < Sm < Md < Lg < Xl`, in declaration order) so elevation roles can be
/// compared — e.g. a floating surface's shadow must out-rank a resting one's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ShadowToken {
    /// Single hairline shadow.
    Xs,
    /// Card shadow; pairs with a 1px `border_subtle` (the signature look).
    Sm,
    /// Raised controls and popovers.
    Md,
    /// Menus and dropdowns.
    Lg,
    /// Deep overlays: modals and dialogs (a three-layer ramp).
    Xl,
}

/// Motion duration tokens, in milliseconds. Interaction feedback lives in the
/// 100–200ms band; larger surfaces take longer. Exits are quicker than the
/// matching entrance (see [`MotionDuration::exit_ms`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MotionDuration {
    /// 100ms: the smallest interaction feedback (press, state-layer fade).
    Micro,
    /// 120ms: hover color and background changes.
    Fast,
    /// 200ms: most state changes, overlay enter.
    #[default]
    Base,
    /// 300ms: large surfaces.
    Slow,
}

impl MotionDuration {
    /// Duration in milliseconds.
    pub const fn ms(self) -> f32 {
        match self {
            Self::Micro => 100.0,
            Self::Fast => 120.0,
            Self::Base => 200.0,
            Self::Slow => 300.0,
        }
    }

    /// Exit duration: an element leaving should clear ~25% quicker than it
    /// arrived (Material 3), so dismissals feel crisp rather than draggy.
    pub const fn exit_ms(self) -> f32 {
        self.ms() * 0.75
    }
}

/// A cubic bezier easing curve `(x1, y1, x2, y2)`, CSS-style.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CubicBezier {
    /// First control point x.
    pub x1: f32,
    /// First control point y.
    pub y1: f32,
    /// Second control point x.
    pub x2: f32,
    /// Second control point y.
    pub y2: f32,
}

/// The Material 3 easing families. Use [`EASE_STANDARD`] for two-way state
/// changes (hover, press, color), [`EASE_DECELERATE`] for entrances (an
/// element flying in and settling), and [`EASE_ACCELERATE`] for exits (an
/// element leaving the screen). Entrances ease *out* (fast then gentle); exits
/// ease *in* (gentle then fast) so they clear quickly.
///
/// Standard easing for entrances and two-way state changes: (0.2, 0, 0, 1).
pub const EASE_STANDARD: CubicBezier = CubicBezier {
    x1: 0.2,
    y1: 0.0,
    x2: 0.0,
    y2: 1.0,
};

/// Decelerate easing for entrances (Material 3): (0, 0, 0.2, 1) — quick to
/// arrive, easing to rest. Pair with an entrance transition.
pub const EASE_DECELERATE: CubicBezier = CubicBezier {
    x1: 0.0,
    y1: 0.0,
    x2: 0.2,
    y2: 1.0,
};

/// Accelerate easing for exits (Material 3): (0.4, 0, 1, 1) — easing away,
/// then leaving briskly. Pair with [`MotionDuration::exit_ms`].
pub const EASE_ACCELERATE: CubicBezier = CubicBezier {
    x1: 0.4,
    y1: 0.0,
    x2: 1.0,
    y2: 1.0,
};

/// Exit easing; the historical name for [`EASE_ACCELERATE`].
pub const EASE_EXIT: CubicBezier = EASE_ACCELERATE;

/// Symmetric ease for camera moves and large surface transitions (CSS
/// `easeInOutCubic`): (0.65, 0, 0.35, 1). The canvas camera
/// ([`crate::canvas`]) eases zoom-to-fit and zoom-to-selection with it — the
/// curve Figma and tldraw use for canvas motion.
pub const EASE_IN_OUT_CUBIC: CubicBezier = CubicBezier {
    x1: 0.65,
    y1: 0.0,
    x2: 0.35,
    y2: 1.0,
};

/// The focus-ring spec (shadcn v4 model). On keyboard focus a control swaps
/// its border to the ring color and draws a soft halo `width` px wide at
/// `alpha`, `offset` px outside the border (0 = flush). The ring color is the
/// accent by default and the danger hue when the control is marked invalid.
/// Painted only when focus arrived via keyboard ([`crate`]'s `focus_visible`).
#[derive(Debug, Clone, Copy)]
pub struct FocusRing {
    /// Halo stroke width in logical px.
    pub width: f32,
    /// Gap between the element edge and the halo (0 = flush).
    pub offset: f32,
    /// Halo alpha applied to the ring color.
    pub alpha: f32,
}

/// The focus ring token: a 3px halo at 0.5 alpha, flush outside the border.
pub const FOCUS_RING: FocusRing = FocusRing {
    width: 3.0,
    offset: 0.0,
    alpha: 0.5,
};

/// Pressed controls scale to this factor for tactile press feedback (Material
/// 3 uses 0.96–0.97). Applied as a paint-time transform about the control's
/// center, so it never disturbs layout or hit-testing, and it animates through
/// the same transition as the press color.
pub const PRESS_SCALE: f32 = 0.97;

/// Sub-segments generated per anchor pair when expanding an OKLCH gradient
/// ([`linear_gradient`](crate::linear_gradient) /
/// [`radial_gradient`](crate::radial_gradient)). Calibrated so a full hue-arc
/// ramp shows no perceptible banding once vello resamples the stops into its
/// ~512-texel sRGB ramp LUT (≈32 texels per sub-segment at 16). A perceptual
/// target, not a hard spec number: raise it if a wide-hue ramp ever bands at a
/// sub-segment joint.
pub const GRADIENT_STEPS: usize = 16;

/// The Material state-layer recipe: a translucent veil of a control's *content*
/// color, laid over its container to signal interaction — one set of opacities
/// shared across the whole kit instead of per-widget hover colors. Hover is the
/// lightest; focus and press share a stronger value; an in-progress drag is
/// strongest. Disabled is expressed separately as a faint container plus dimmed
/// content rather than an overlay.
#[derive(Debug, Clone, Copy)]
pub struct StateLayer {
    /// Hovered (pointer over an idle control).
    pub hover: f32,
    /// Focused via keyboard.
    pub focus: f32,
    /// Pressed (pointer down).
    pub press: f32,
    /// Being dragged.
    pub drag: f32,
    /// Disabled container: the share of the content color blended into the
    /// resting surface so the control reads as inert.
    pub disabled_container: f32,
    /// Disabled content: the opacity a disabled label/icon is dimmed to.
    pub disabled_content: f32,
}

/// The state-layer token.
pub const STATE_LAYER: StateLayer = StateLayer {
    hover: 0.08,
    focus: 0.12,
    press: 0.12,
    drag: 0.16,
    disabled_container: 0.12,
    disabled_content: 0.38,
};

impl CubicBezier {
    /// Evaluates the easing curve at progress `x` in 0..=1 (CSS
    /// `cubic-bezier` semantics): solves the parametric x-curve with Newton
    /// iteration, then returns the y value.
    pub fn eval(self, x: f32) -> f32 {
        if x <= 0.0 {
            return 0.0;
        }
        if x >= 1.0 {
            return 1.0;
        }
        let bez = |t: f32, p1: f32, p2: f32| {
            let u = 1.0 - t;
            3.0 * u * u * t * p1 + 3.0 * u * t * t * p2 + t * t * t
        };
        let dbez = |t: f32, p1: f32, p2: f32| {
            let u = 1.0 - t;
            3.0 * u * u * p1 + 6.0 * u * t * (p2 - p1) + 3.0 * t * t * (1.0 - p2)
        };
        let mut t = x;
        for _ in 0..8 {
            let err = bez(t, self.x1, self.x2) - x;
            let d = dbez(t, self.x1, self.x2);
            if d.abs() < 1e-6 {
                break;
            }
            t = (t - err / d).clamp(0.0, 1.0);
        }
        bez(t, self.y1, self.y2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracking_curve_tightens_with_size_toward_its_asymptote() {
        // Inter's curve: a hair positive at caption sizes, monotonically
        // tightening toward the -0.0223em asymptote as size grows.
        assert!(tracking_em(12.0) > tracking_em(16.0));
        assert!(tracking_em(16.0) > tracking_em(31.0));
        assert!((tracking_em(12.0) - 0.0005).abs() < 0.002);
        assert!((tracking_em(16.0) - -0.0110).abs() < 0.002);
        // Large sizes approach but never cross the asymptote.
        assert!(tracking_em(96.0) > -0.0223);
        assert!((tracking_em(96.0) - -0.0223).abs() < 0.0005);
    }

    #[test]
    fn text_size_tracking_matches_the_formula() {
        for size in [TextSize::Xs, TextSize::Base, TextSize::Xl3] {
            assert_eq!(size.letter_spacing(), tracking_em(size.px()));
        }
    }

    #[test]
    fn easing_families_have_the_right_curvature() {
        // Decelerate (enter) eases out: it is ahead of the diagonal early.
        assert!(EASE_DECELERATE.eval(0.25) > 0.25);
        // Accelerate (exit) eases in: it lags the diagonal early.
        assert!(EASE_ACCELERATE.eval(0.25) < 0.25);
        // Standard is a gentle ease that also leads at the start.
        assert!(EASE_STANDARD.eval(0.25) > 0.25);
        // Endpoints are pinned for every curve.
        for e in [EASE_STANDARD, EASE_DECELERATE, EASE_ACCELERATE] {
            assert_eq!(e.eval(0.0), 0.0);
            assert_eq!(e.eval(1.0), 1.0);
        }
    }

    #[test]
    fn exit_is_a_quarter_quicker_than_entrance() {
        for d in [
            MotionDuration::Micro,
            MotionDuration::Fast,
            MotionDuration::Base,
            MotionDuration::Slow,
        ] {
            assert!((d.exit_ms() - d.ms() * 0.75).abs() < 1e-3);
        }
        assert_eq!(MotionDuration::Micro.ms(), 100.0);
    }

    #[test]
    fn press_scale_is_a_subtle_shrink() {
        assert!((0.96..=0.97).contains(&PRESS_SCALE));
    }

    #[test]
    fn radius_scale_default_matches_the_constants() {
        // The default family (base R_MD) must reproduce the kit's constants, so
        // a theme on the default scale looks identical to one using R_SM..R_XL.
        let r = RadiusScale::default();
        assert_eq!(r.sm, R_SM);
        assert_eq!(r.md, R_MD);
        assert_eq!(r.lg, R_LG);
        assert_eq!(r.xl, R_XL);
        // A tighter base scales the whole family down proportionally.
        let tight = RadiusScale::from_base(5.0);
        assert_eq!(tight.md, 5.0);
        assert!(tight.sm < tight.md && tight.md < tight.lg && tight.lg < tight.xl);
    }
}
