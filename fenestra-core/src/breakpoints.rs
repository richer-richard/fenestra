//! Width breakpoints for constraints-aware layout.
//!
//! Tailwind's logical-pixel thresholds (`sm` 640, `md` 768, `lg` 1024,
//! `xl` 1280, `2xl` 1536), exposed two ways: classify a width into a named
//! [`Breakpoint`] band with [`Breakpoint::at`], or ask a yes/no question with
//! the [`Breakpoints`] helpers ([`Breakpoints::up`] and friends). Pair either
//! with [`App::view_at`](crate::App::view_at) for window-size breakpoints, or
//! with [`responsive`](crate::responsive) for a container's own size.
//!
//! ```
//! use fenestra_core::{Breakpoint, Breakpoints};
//!
//! assert_eq!(Breakpoint::at(390.0), Breakpoint::Base); // phone
//! assert_eq!(Breakpoint::at(800.0), Breakpoint::Md);   // tablet
//! assert!(Breakpoints::up(1280.0, Breakpoint::Lg));     // desktop ≥ lg
//! assert!(Breakpoints::is_md(800.0));
//! ```

/// The named width bands from Tailwind, mobile-first (smallest to largest).
/// [`Self::at`] classifies a logical-pixel width into the largest band whose
/// minimum width it meets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Breakpoint {
    /// The unprefixed base band, below `sm` (`< 640px`).
    Base,
    /// `>= 640px`.
    Sm,
    /// `>= 768px`.
    Md,
    /// `>= 1024px`.
    Lg,
    /// `>= 1280px`.
    Xl,
    /// `>= 1536px` (Tailwind's `2xl`).
    Xxl,
}

impl Breakpoint {
    /// The minimum width (logical px) at which this band begins; [`Self::Base`]
    /// begins at `0.0`.
    pub const fn min_width(self) -> f32 {
        match self {
            Self::Base => 0.0,
            Self::Sm => 640.0,
            Self::Md => 768.0,
            Self::Lg => 1024.0,
            Self::Xl => 1280.0,
            Self::Xxl => 1536.0,
        }
    }

    /// Classifies a width into the largest band whose [`Self::min_width`] it
    /// meets: `639.0 → Base`, `640.0 → Sm`, `2000.0 → Xxl`. A negative or NaN
    /// width falls through to [`Self::Base`].
    pub fn at(width: f32) -> Self {
        if width >= Self::Xxl.min_width() {
            Self::Xxl
        } else if width >= Self::Xl.min_width() {
            Self::Xl
        } else if width >= Self::Lg.min_width() {
            Self::Lg
        } else if width >= Self::Md.min_width() {
            Self::Md
        } else if width >= Self::Sm.min_width() {
            Self::Sm
        } else {
            Self::Base
        }
    }
}

/// Yes/no width queries against the [`Breakpoint`] thresholds — the boolean
/// counterpart of [`Breakpoint::at`], for [`view_at`](crate::App::view_at) /
/// [`responsive`](crate::responsive) consumers that want a flag rather than a
/// band. A zero-sized namespace, never constructed.
pub struct Breakpoints;

impl Breakpoints {
    /// `width` is at or above `bp`'s minimum — Tailwind's `md:` ("md and up").
    pub fn up(width: f32, bp: Breakpoint) -> bool {
        width >= bp.min_width()
    }

    /// `width` is strictly below `bp`'s minimum — Tailwind's `max-md:`.
    pub fn down(width: f32, bp: Breakpoint) -> bool {
        width < bp.min_width()
    }

    /// `width` falls in exactly this band (`>= bp` and below the next one up).
    pub fn only(width: f32, bp: Breakpoint) -> bool {
        Breakpoint::at(width) == bp
    }

    /// At or above the `sm` threshold (`>= 640px`).
    pub fn is_sm(width: f32) -> bool {
        Self::up(width, Breakpoint::Sm)
    }

    /// At or above the `md` threshold (`>= 768px`).
    pub fn is_md(width: f32) -> bool {
        Self::up(width, Breakpoint::Md)
    }

    /// At or above the `lg` threshold (`>= 1024px`).
    pub fn is_lg(width: f32) -> bool {
        Self::up(width, Breakpoint::Lg)
    }

    /// At or above the `xl` threshold (`>= 1280px`).
    pub fn is_xl(width: f32) -> bool {
        Self::up(width, Breakpoint::Xl)
    }

    /// At or above the `2xl` threshold (`>= 1536px`).
    pub fn is_xxl(width: f32) -> bool {
        Self::up(width, Breakpoint::Xxl)
    }
}

#[cfg(test)]
mod tests {
    use super::{Breakpoint, Breakpoints};

    #[test]
    fn at_classifies_each_band_at_and_below_its_edge() {
        // Just below each edge stays in the lower band; exactly the edge enters.
        assert_eq!(Breakpoint::at(0.0), Breakpoint::Base);
        assert_eq!(Breakpoint::at(639.9), Breakpoint::Base);
        assert_eq!(Breakpoint::at(640.0), Breakpoint::Sm);
        assert_eq!(Breakpoint::at(767.9), Breakpoint::Sm);
        assert_eq!(Breakpoint::at(768.0), Breakpoint::Md);
        assert_eq!(Breakpoint::at(1024.0), Breakpoint::Lg);
        assert_eq!(Breakpoint::at(1280.0), Breakpoint::Xl);
        assert_eq!(Breakpoint::at(1535.9), Breakpoint::Xl);
        assert_eq!(Breakpoint::at(1536.0), Breakpoint::Xxl);
        assert_eq!(Breakpoint::at(4000.0), Breakpoint::Xxl);
    }

    #[test]
    fn degenerate_widths_fall_to_base() {
        assert_eq!(Breakpoint::at(-100.0), Breakpoint::Base);
        assert_eq!(Breakpoint::at(f32::NAN), Breakpoint::Base);
    }

    #[test]
    fn up_down_only_agree_with_at() {
        assert!(Breakpoints::up(768.0, Breakpoint::Md));
        assert!(!Breakpoints::up(767.0, Breakpoint::Md));
        assert!(Breakpoints::down(767.0, Breakpoint::Md));
        assert!(!Breakpoints::down(768.0, Breakpoint::Md));
        assert!(Breakpoints::only(800.0, Breakpoint::Md));
        assert!(!Breakpoints::only(1024.0, Breakpoint::Md));
        // The named shorthands mirror `up`.
        assert!(Breakpoints::is_md(768.0));
        assert!(!Breakpoints::is_lg(1023.0));
        assert!(Breakpoints::is_xxl(1536.0));
        // The bands are ordered, so `Ord` matches threshold order.
        assert!(Breakpoint::Base < Breakpoint::Sm);
        assert!(Breakpoint::Lg < Breakpoint::Xxl);
    }
}
