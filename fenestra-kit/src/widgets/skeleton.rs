//! Loading placeholders ([`skeleton`], [`skeleton_text`], [`skeleton_circle`]).
//!
//! A skeleton mirrors the shape of content that has not loaded yet: size it
//! like the real thing so the swap-in causes no layout shift. The fill pulses
//! gently (opacity), and pins to a flat fill under reduced motion, so headless
//! renders stay deterministic. Reach for a skeleton when the layout is known
//! and the wait is ≥~1s (lists, cards, dashboards); reach for [`spinner`] when
//! the result has no previewable shape.
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

use fenestra_core::{Element, Keyframes, Length, R_FULL, SP2, Theme, col, div};

/// One pulse cycle, ms. A slow breath that reads as "working" without nagging.
const PULSE_MS: f32 = 1600.0;

/// Applies the resting neutral fill and the shared opacity pulse to a shape.
/// The fill is the translucent neutral twin so it reads as a faint veil over
/// any surface — a white card in light mode, an elevated card in dark — rather
/// than vanishing into a same-tone background.
fn pulse<Msg>(shape: Element<Msg>) -> Element<Msg> {
    shape
        .themed(|t: &Theme, s| s.bg(t.neutral_alpha.step(4)))
        .keyframes(
            Keyframes::new(PULSE_MS)
                .stop(0.0, |s| s.opacity(1.0))
                .stop(0.5, |s| s.opacity(0.45))
                .stop(1.0, |s| s.opacity(1.0)),
        )
}

/// A rectangular loading placeholder of the given size (logical px, or any
/// [`Length`]). Size it to mirror the content it stands in for.
pub fn skeleton<Msg>(width: impl Into<Length>, height: impl Into<Length>) -> Element<Msg> {
    pulse(
        div()
            .w(width)
            .h(height)
            .shrink0()
            .themed(|t: &Theme, s| s.rounded(t.radius.sm)),
    )
}

/// A circular loading placeholder (avatars, icons), diameter `d` logical px.
pub fn skeleton_circle<Msg>(d: f32) -> Element<Msg> {
    pulse(div().w(d).h(d).shrink0().rounded(R_FULL))
}

/// A stack of `lines` text-line placeholders. The last line is 60% width to
/// mimic a ragged paragraph end; one line yields a single full-width bar.
pub fn skeleton_text<Msg>(lines: usize) -> Element<Msg> {
    let n = lines.max(1);
    col().gap(SP2).w_full().children((0..n).map(move |i| {
        let w = if i + 1 == n && n > 1 {
            Length::Pct(60.0)
        } else {
            Length::Pct(100.0)
        };
        pulse(
            div()
                .h(10.0)
                .w(w)
                .themed(|t: &Theme, s| s.rounded(t.radius.sm)),
        )
    }))
}
