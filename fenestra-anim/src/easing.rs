//! Per-segment easing: linear, hold, CSS cubic-bezier curves, and the
//! closed-form damped spring.

use crate::bezier::CubicBezier;
use crate::spring::SpringSpec;

/// How a key eases into the segment that follows it.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Ease {
    /// Constant-rate interpolation (the default).
    Linear,
    /// Hold this key's value for the whole segment; the next key snaps.
    Hold,
    /// A CSS `cubic-bezier(x1, y1, x2, y2)` curve. Control-point y values
    /// outside `0..=1` overshoot: numeric tracks extrapolate past their
    /// endpoints mid-segment.
    Bezier(CubicBezier),
    /// A closed-form damped spring occupying the segment (see [`Spring`]).
    Spring(Spring),
}

/// A damped spring easing a segment: the analytic step response of
/// [`SpringSpec`], launched with `velocity` (progress units per second) at
/// the segment's first key and settling on the segment's end value (decay
/// envelope below 0.1%). A segment shorter than the settle time truncates
/// the tail: the next key still lands exactly.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Spring {
    /// Stiffness (ω² scale): higher = snappier. 170 is gentle, 380 brisk.
    pub stiffness: f32,
    /// Damping: lower overshoots more. Critical damping ≈ 2·√stiffness.
    pub damping: f32,
    /// Initial velocity in progress units per second (0 = released at rest).
    pub velocity: f32,
}

impl Spring {
    /// The spring's progress at `elapsed_secs` into its segment.
    pub(crate) fn progress(self, elapsed_secs: f32) -> f32 {
        let spec = SpringSpec {
            stiffness: self.stiffness,
            damping: self.damping,
        };
        spec.step(self.velocity, elapsed_secs).0
    }
}

impl From<CubicBezier> for Ease {
    fn from(curve: CubicBezier) -> Self {
        Self::Bezier(curve)
    }
}

impl From<Spring> for Ease {
    fn from(spring: Spring) -> Self {
        Self::Spring(spring)
    }
}

/// Constant-rate easing (the default for a bare [`key`](crate::key)).
pub fn linear() -> Ease {
    Ease::Linear
}

/// Hold the key's value for the whole segment; the next key snaps.
pub fn hold() -> Ease {
    Ease::Hold
}

/// CSS `ease-in` — starts slow, accelerates away. Use for exits.
pub fn ease_in() -> Ease {
    Ease::Bezier(CubicBezier {
        x1: 0.42,
        y1: 0.0,
        x2: 1.0,
        y2: 1.0,
    })
}

/// CSS `ease-out` — arrives fast, decelerates into place. Use for entrances.
pub fn ease_out() -> Ease {
    Ease::Bezier(CubicBezier {
        x1: 0.0,
        y1: 0.0,
        x2: 0.58,
        y2: 1.0,
    })
}

/// CSS `ease-in-out` — symmetric acceleration and deceleration.
pub fn ease_in_out() -> Ease {
    Ease::Bezier(CubicBezier {
        x1: 0.42,
        y1: 0.0,
        x2: 0.58,
        y2: 1.0,
    })
}

/// A closed-form damped spring released from rest. Chain
/// [`Spring::velocity`](Spring) via struct update or use [`Spring`] directly
/// for a launched start.
pub fn spring(stiffness: f32, damping: f32) -> Ease {
    Ease::Spring(Spring {
        stiffness,
        damping,
        velocity: 0.0,
    })
}
