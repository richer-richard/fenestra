//! Hostile element trees: any shape, any styling, any viewport — build
//! and paint never panic. The fuzz twin of the proptest totality
//! property, with arbitrary-driven structure.

#![no_main]

use arbitrary::Arbitrary;
use fenestra_core::{
    Element, Fonts, FrameState, Overlay, Theme, build_frame, col, div, image_rgba8, row, text,
};
use libfuzzer_sys::fuzz_target;

#[derive(Arbitrary, Debug)]
enum Plan {
    Empty,
    Text(String),
    Image,
    Sized(f32, f32),
    Node {
        is_row: bool,
        pad: f32,
        gap: f32,
        width: Option<f32>,
        height: Option<f32>,
        grow: bool,
        wrap: bool,
        scroll: bool,
        overlay: bool,
        opacity: f32,
        children: Vec<Plan>,
    },
}

fn materialize(plan: &Plan, depth: usize, seq: &mut u32) -> Element<()> {
    if depth > 5 {
        return div();
    }
    match plan {
        Plan::Empty => div(),
        Plan::Text(s) => text(s.clone()),
        Plan::Image => image_rgba8(2, 2, vec![200; 16]),
        Plan::Sized(w, h) => div().w(*w).h(*h),
        Plan::Node {
            is_row,
            pad,
            gap,
            width,
            height,
            grow,
            wrap,
            scroll,
            overlay,
            opacity,
            children,
        } => {
            let mut el = if *is_row { row() } else { col() };
            el = el.p(*pad).gap(*gap).opacity(*opacity);
            if let Some(w) = width {
                el = el.w(*w);
            }
            if let Some(h) = height {
                el = el.h(*h);
            }
            if *grow {
                el = el.grow();
            }
            if *wrap {
                el = el.wrap();
            }
            if *scroll {
                *seq += 1;
                el = el.scroll_y().id(&format!("s{seq}"));
            }
            if *overlay {
                el = el.overlay(Overlay::modal());
            }
            el.children(
                children
                    .iter()
                    .take(6)
                    .map(|c| materialize(c, depth + 1, seq)),
            )
        }
    }
}

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
