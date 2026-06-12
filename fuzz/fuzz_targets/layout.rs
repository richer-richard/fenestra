//! Hostile element trees: any shape, any styling, any viewport — build
//! and paint never panic. The fuzz twin of the proptest totality
//! property, with arbitrary-driven structure.

#![no_main]

use arbitrary::Arbitrary;
use fenestra_core::{Fonts, FrameState, Theme, build_frame};
use libfuzzer_sys::fuzz_target;

include!("layout_plan.rs");

fuzz_target!(|input: (Plan, f32, f32)| {
    let (plan, w, h) = input;
    let mut seq = 0;
    let tree = materialize(&plan, 0, &mut seq);
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    state.reduced_motion = true;
    // Clamp the viewport to something a GPU could host; hostile sizes
    // beyond that are the runner's clamp's job, not layout's.
    let (w, h) = (
        if w.is_finite() { w.clamp(0.0, 4096.0) } else { 512.0 },
        if h.is_finite() { h.clamp(0.0, 4096.0) } else { 512.0 },
    );
    let frame = build_frame(&tree, &Theme::dark(), &mut fonts, &mut state, (w, h), 1.0);
    let _scene = frame.paint(&mut fonts, &mut state);
});
