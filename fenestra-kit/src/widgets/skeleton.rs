//! Loading placeholders ([`skeleton`], [`skeleton_text`], [`skeleton_circle`]).
//!
//! A skeleton mirrors the shape of content that has not loaded yet: size it
//! like the real thing so the swap-in causes no layout shift. Blocks and
//! circles run a left-to-right **shimmer** sweep (a translucent highlight band
//! gliding across a neutral base); text lines run a quieter opacity **pulse**.
//! Both pin to a flat fill under reduced motion, so headless renders stay
//! deterministic. Reach for a skeleton when the layout is known and the wait is
//! ≥~1s (lists, cards, dashboards); reach for [`spinner`] when the result has
//! no previewable shape.
//!
//! ```
//! use fenestra_kit::{skeleton, skeleton_circle, skeleton_text};
//!
//! let avatar: fenestra_core::Element<()> = skeleton_circle(32.0);
//! let lines: fenestra_core::Element<()> = skeleton_text(3);
//! let block: fenestra_core::Element<()> = skeleton(120.0, 16.0);
//! ```
//!
//! [`spinner`]: crate::spinner

use fenestra_core::{
    Element, Keyframes, Length, Mode, R_FULL, SP2, Theme, col, div, linear_gradient,
};

/// One shimmer sweep, ms.
const SWEEP_MS: f32 = 1400.0;
/// One opacity-pulse cycle, ms.
const PULSE_MS: f32 = 1600.0;

/// A shimmering placeholder of `w`×`h` logical px: a neutral base with a
/// translucent highlight band that sweeps left→right (slightly tilted, the tilt
/// is what reads as motion), clipped to the shape. `circle` rounds it fully.
fn shimmer<Msg>(w: f32, h: f32, circle: bool) -> Element<Msg> {
    let w = w.max(1.0);
    let h = h.max(1.0);
    // The highlight band is ~65% of the width and travels from just off the
    // left edge to just off the right, so the bright streak crosses fully.
    let band = (w * 0.65).max(20.0);

    let sweep = div::<Msg>()
        .absolute()
        .top(0.0)
        .h(h)
        .w(band)
        .themed(|t: &Theme, s| {
            // The highlight is a *lighter* neutral than the base in either mode
            // (ramps invert by mode, so the step is chosen per mode), kept low so
            // it whispers across rather than flashing.
            let hi = if t.mode == Mode::Dark {
                t.neutrals.step(9)
            } else {
                t.neutrals.step(1)
            };
            s.bg(linear_gradient(
                100.0,
                [hi.with_alpha(0.0), hi.with_alpha(0.85), hi.with_alpha(0.0)],
            ))
        })
        // The band travels off the right edge (fully invisible there) and wraps
        // back to the left, so the loop is seamless. The first stop is kept near
        // the left edge — its bright centre on-screen — so a reduced-motion /
        // headless frame shows the shimmer mid-sweep rather than a flat block.
        .keyframes(
            Keyframes::new(SWEEP_MS)
                .stop(0.0, move |s| s.left(w * 0.06))
                .stop(1.0, move |s| s.left(w)),
        );

    div::<Msg>()
        .w(w)
        .h(h)
        .shrink0()
        .overflow_hidden()
        .themed(move |t: &Theme, s| {
            let s = s.bg(t.neutral_alpha.step(4));
            if circle {
                s.rounded(R_FULL)
            } else {
                s.rounded(t.radius.sm)
            }
        })
        .children([sweep])
}

/// A rectangular loading placeholder of `width`×`height` logical px, with a
/// left-to-right shimmer sweep. Size it to mirror the content it stands in for.
pub fn skeleton<Msg>(width: f32, height: f32) -> Element<Msg> {
    shimmer(width, height, false)
}

/// A circular loading placeholder (avatars, icons), diameter `d` logical px,
/// with the same shimmer sweep.
pub fn skeleton_circle<Msg>(d: f32) -> Element<Msg> {
    shimmer(d, d, true)
}

/// A stack of `lines` text-line placeholders that pulse (a quieter cue than the
/// shimmer, right for runs of prose). The last line is 60% width to mimic a
/// ragged paragraph end; one line yields a single full-width bar.
pub fn skeleton_text<Msg>(lines: usize) -> Element<Msg> {
    let n = lines.max(1);
    col().gap(SP2).w_full().children((0..n).map(move |i| {
        let w = if i + 1 == n && n > 1 {
            Length::Pct(60.0)
        } else {
            Length::Pct(100.0)
        };
        div()
            .h(10.0)
            .w(w)
            .themed(|t: &Theme, s| s.bg(t.neutral_alpha.step(4)).rounded(t.radius.sm))
            .keyframes(
                Keyframes::new(PULSE_MS)
                    .stop(0.0, |s| s.opacity(1.0))
                    .stop(0.5, |s| s.opacity(0.45))
                    .stop(1.0, |s| s.opacity(1.0)),
            )
    }))
}
