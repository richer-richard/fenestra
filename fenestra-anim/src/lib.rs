//! Frame-pure keyframe animation math: typed tracks, CSS cubic-bezier and
//! closed-form spring easing, and an exact rational timebase.
//!
//! This crate has zero dependency on any fenestra crate, GPU API, or text
//! layout engine — it is pure math over `f32`/`u64`, usable headless by any
//! frame-based or tick-based sampler (fenestra's own interactive transition
//! engine, its offline `fenestra-motion` sampler, or an unrelated project
//! like an audio engine sequencing automation ticks).
//!
//! `Color` interpolation is deliberately not shipped here: perceptual
//! (Oklab) color mixing needs a color-management dependency and a concrete
//! color type this crate has no other reason to carry. A consumer that
//! wants a color track builds it directly on [`sorted_keys`] and [`locate`]
//! — the same segment-lookup and easing evaluation [`Track`] uses — with
//! its own color-space-aware value combination in place of [`Interpolate`].
//! See `fenestra-motion`'s `ColorTrack` for a worked example.

mod bezier;
mod easing;
mod interpolate;
mod rational;
mod spring;
mod timeline;
mod track;

pub use bezier::CubicBezier;
pub use easing::{Ease, Spring, ease_in, ease_in_out, ease_out, hold, linear, spring};
pub use interpolate::Interpolate;
pub use rational::{Rounding, mul_div};
pub use spring::SpringSpec;
pub use timeline::{FrameRange, Frames};
pub use track::{Key, Located, Track, key, locate, sorted_keys};
