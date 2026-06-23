//! RTL layout mirroring and Dynamic Type, headless: `Theme::rtl()` flips the
//! realized geometry horizontally, and `Theme::with_text_scale` grows every
//! resolved font size.

use fenestra_core::{Fonts, Frame, FrameState, Theme, build_frame, by, col, div, row, text};

fn frame_with(view: &fenestra_core::Element<()>, theme: Theme, size: (f32, f32)) -> Frame {
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    build_frame(view, &theme, &mut fonts, &mut state, size, 1.0)
}

fn x0(frame: &Frame, id: &str) -> f64 {
    frame.get(&by::id(id)).rect.x0
}
fn w(frame: &Frame, id: &str) -> f64 {
    frame.get(&by::id(id)).rect.width()
}

/// Under RTL the first child of a row mirrors to the right edge, and the row's
/// visual order reverses — while widths and the vertical axis are preserved.
#[test]
fn rtl_mirrors_a_row() {
    let view = row().items_start().children([
        div::<()>().id("first").w(40.0).h(40.0),
        div::<()>().id("second").w(40.0).h(40.0),
    ]);

    // LTR: first sits at the left, second to its right.
    let ltr = frame_with(&view, Theme::light(), (200.0, 100.0));
    assert!(
        x0(&ltr, "first") < 1.0,
        "ltr first x0 = {}",
        x0(&ltr, "first")
    );
    assert!(x0(&ltr, "first") < x0(&ltr, "second"));

    // RTL: the same row mirrors about the 200px canvas. `first` [0,40] → [160,200].
    let rtl = frame_with(&view, Theme::light().rtl(), (200.0, 100.0));
    assert!(
        x0(&rtl, "first") > x0(&rtl, "second"),
        "first ({}) mirrors to the right of second ({})",
        x0(&rtl, "first"),
        x0(&rtl, "second")
    );
    assert!(
        (x0(&rtl, "first") - 160.0).abs() < 1.0,
        "first mirrors to x0=160, got {}",
        x0(&rtl, "first")
    );
    assert!(
        (w(&rtl, "first") - 40.0).abs() < 1.0,
        "width is preserved under mirror"
    );
}

/// `Theme::with_text_scale(2.0)` roughly doubles a line of text's height; the
/// stock scale (1.0) is unchanged.
#[test]
fn dynamic_type_scales_text() {
    let view = col().items_start().children([text("Hello").id("t")]);
    let h1 = frame_with(&view, Theme::light(), (300.0, 200.0))
        .get(&by::id("t"))
        .rect
        .height();
    let h2 = frame_with(&view, Theme::light().with_text_scale(2.0), (300.0, 200.0))
        .get(&by::id("t"))
        .rect
        .height();
    assert!(
        h2 > h1 * 1.6,
        "2x Dynamic Type should grow line height ~2x: h1={h1}, h2={h2}"
    );
}
