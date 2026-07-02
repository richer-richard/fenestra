//! Declarative motion graphics for fenestra: a composition is a pure function
//! of integer frame number, sampled into fenestra element trees and rendered
//! through the existing headless pipeline. Built for agents that author a
//! timeline, render frames, inspect them, and assert on them — no human eye
//! in the loop required.
//!
//! The determinism contract: `sample(frame)` depends only on
//! `(composition, frame)` — no wall clock, no accumulated state, no
//! frame-to-frame dependence — so any frame renders standalone, in any order,
//! in parallel, with identical results.

mod clip;
mod composition;
mod easing;
mod sample;
mod timeline;

pub use clip::{Anchor, AnyTrack, Clip, Prop, ResolvedProps};
pub use composition::Composition;
pub use easing::{
    EASE_CRISP, EASE_EDITORIAL, EASE_POP, Ease, Spring, ease_in, ease_in_out, ease_out, hold,
    linear, spring,
};
pub use sample::{ResolvedClip, SampledScene};
pub use timeline::{ColorSpace, FrameRange, Frames, Key, Track, TrackValue, key};
