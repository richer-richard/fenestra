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
    /// until one is registered). Editorial headlines.
    Display,
    /// A serif face registered via `Fonts::register` (falls back to Sans).
    Serif,
}

/// Shadow elevation tokens. Resolved to concrete layered shadows by the
/// theme (dark mode multiplies alphas by 1.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

/// Motion duration tokens, in milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MotionDuration {
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
            Self::Fast => 120.0,
            Self::Base => 200.0,
            Self::Slow => 300.0,
        }
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

/// Standard easing for entrances and state changes: (0.2, 0.0, 0.0, 1.0).
pub const EASE_STANDARD: CubicBezier = CubicBezier {
    x1: 0.2,
    y1: 0.0,
    x2: 0.0,
    y2: 1.0,
};

/// Exit easing: (0.4, 0.0, 1.0, 1.0).
pub const EASE_EXIT: CubicBezier = CubicBezier {
    x1: 0.4,
    y1: 0.0,
    x2: 1.0,
    y2: 1.0,
};

/// Focus ring geometry: a 2px ring in the accent color at 0.6 alpha, offset
/// 2px outside the border, with ring radius = element radius + 2. Painted
/// only when focus arrived via keyboard.
#[derive(Debug, Clone, Copy)]
pub struct FocusRing {
    /// Ring stroke width in logical px.
    pub width: f32,
    /// Gap between the element edge and the ring.
    pub offset: f32,
    /// Ring alpha applied to the accent color.
    pub alpha: f32,
}

/// The focus ring token.
pub const FOCUS_RING: FocusRing = FocusRing {
    width: 2.0,
    offset: 2.0,
    alpha: 0.6,
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
}
