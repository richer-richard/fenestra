//! The value contract a [`Track`](crate::Track) samples through.

/// A value a track can interpolate between two keyframes at eased progress
/// `t`. `t` is the *eased* progress and may leave `0..=1` under overshoot
/// easing (a bezier or spring curve that overshoots its target) — geometry
/// extrapolates past the target and back, matching typical UI-animation
/// behavior for lengths and offsets.
///
/// This crate ships impls for plain numeric types only. A color type wants
/// perceptual (Oklab) mixing, which needs a color-management dependency this
/// crate deliberately doesn't carry — implement `Interpolate` for your own
/// color type in the crate that already depends on one.
pub trait Interpolate: Copy {
    /// Interpolates `a → b` at eased progress `t`.
    fn interpolate(a: Self, b: Self, t: f32) -> Self;
}

impl Interpolate for f32 {
    fn interpolate(a: Self, b: Self, t: f32) -> Self {
        a + (b - a) * t
    }
}

/// A 2-component numeric pair (e.g. non-uniform scale, a 2D offset),
/// interpolated componentwise.
impl Interpolate for (f32, f32) {
    fn interpolate(a: Self, b: Self, t: f32) -> Self {
        (f32::interpolate(a.0, b.0, t), f32::interpolate(a.1, b.1, t))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f32_interpolates_linearly_and_extrapolates_past_1() {
        assert_eq!(f32::interpolate(0.0, 10.0, 0.5), 5.0);
        assert_eq!(
            f32::interpolate(0.0, 10.0, 1.5),
            15.0,
            "overshoot extrapolates"
        );
        assert_eq!(
            f32::interpolate(0.0, 10.0, -0.5),
            -5.0,
            "undershoot extrapolates"
        );
    }

    #[test]
    fn tuple_interpolates_componentwise() {
        assert_eq!(
            <(f32, f32)>::interpolate((0.0, 10.0), (10.0, 0.0), 0.25),
            (2.5, 7.5)
        );
    }
}
