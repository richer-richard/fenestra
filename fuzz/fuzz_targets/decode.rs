//! Decodes a layout-fuzz artifact: prints the Plan and reproduces.
use arbitrary::{Arbitrary, Unstructured};

include!("layout_plan.rs");

fn main() {
    let path = std::env::args().nth(1).expect("artifact path");
    let bytes = std::fs::read(path).expect("read artifact");
    let u = Unstructured::new(&bytes);
    let (plan, w, h) = <(Plan, f32, f32)>::arbitrary_take_rest(u).expect("decode");
    println!("viewport: {w} x {h}");
    println!("{plan:#?}");
    let mut seq = 0;
    let tree = materialize(&plan, 0, &mut seq);
    let mut fonts = fenestra_core::Fonts::embedded();
    let mut state = fenestra_core::FrameState::new();
    state.reduced_motion = true;
    let (w, h) = (
        if w.is_finite() { w.clamp(0.0, 4096.0) } else { 512.0 },
        if h.is_finite() { h.clamp(0.0, 4096.0) } else { 512.0 },
    );
    let frame = fenestra_core::build_frame(
        &tree,
        &fenestra_core::Theme::dark(),
        &mut fonts,
        &mut state,
        (w, h),
        1.0,
    );
    let _scene = frame.paint(&mut fonts, &mut state);
    println!("no panic");
}
