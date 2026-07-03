//! Named easing presets built on `fenestra-anim`'s generic `Ease`.

use fenestra_anim::{CubicBezier, Ease};

/// Crisp UI entrance: a strong ease-out that slows firmly into rest,
/// `cubic-bezier(0.16, 1, 0.3, 1)`.
pub const EASE_CRISP: Ease = Ease::Bezier(CubicBezier {
    x1: 0.16,
    y1: 1.0,
    x2: 0.3,
    y2: 1.0,
});

/// Editorial fade: a balanced ease-in-out for slow, hold-friendly moves,
/// `cubic-bezier(0.45, 0, 0.55, 1)`.
pub const EASE_EDITORIAL: Ease = Ease::Bezier(CubicBezier {
    x1: 0.45,
    y1: 0.0,
    x2: 0.55,
    y2: 1.0,
});

/// Playful overshoot pop (control y > 1): a little past the target, then
/// settles, `cubic-bezier(0.34, 1.56, 0.64, 1)`. Numeric tracks extrapolate
/// past their endpoint mid-segment; color tracks clamp.
pub const EASE_POP: Ease = Ease::Bezier(CubicBezier {
    x1: 0.34,
    y1: 1.56,
    x2: 0.64,
    y2: 1.0,
});
