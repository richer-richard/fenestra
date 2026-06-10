//! Theme stub for M0. Replaced by the full OKLCH token system in M1.

use peniko::Color;

/// Design tokens resolved for one color mode. M0 carries just enough to paint
/// the hello scene; M1 generates the full ramp set from an accent hue.
#[derive(Debug, Clone)]
pub struct Theme {
    /// Window background (will become neutral step 1).
    pub bg: Color,
    /// Raised surface color (cards).
    pub surface_raised: Color,
    /// Default border color.
    pub border: Color,
}

impl Theme {
    /// Placeholder light theme until `Theme::from_accent` lands in M1.
    pub fn light() -> Self {
        Self {
            bg: Color::from_rgb8(0xfa, 0xfa, 0xfb),
            surface_raised: Color::from_rgb8(0xff, 0xff, 0xff),
            border: Color::from_rgb8(0xe4, 0xe4, 0xe9),
        }
    }
}
