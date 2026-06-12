//! The IME caret area: painting a frame with a focused editor records the
//! caret rect so the runner can position the composition popup.

use fenestra_core::{
    Element, Fonts, FrameState, InputEvent, Theme, build_frame, col, dispatch, raw_input,
};

#[test]
fn painting_a_focused_editor_records_the_caret_area() {
    let theme = Theme::light();
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    state.reduced_motion = true;
    let view: Element<()> = col().p(8.0).children([raw_input("hello", "").w(160.0)]);

    let frame = build_frame(&view, &theme, &mut fonts, &mut state, (200.0, 60.0), 1.0);
    let _ = dispatch(&view, &frame, &mut state, &mut fonts, InputEvent::Tab);

    // Rebuild with focus applied, then paint: the caret area appears.
    let frame = build_frame(&view, &theme, &mut fonts, &mut state, (200.0, 60.0), 1.0);
    let _scene = frame.paint(&mut fonts, &mut state);

    let caret = state
        .ime_caret()
        .expect("focused editor exposes a caret area");
    assert!(
        caret.x0 >= 8.0 && caret.x1 <= 168.0,
        "caret sits inside the input, got {caret:?}"
    );
    assert!(caret.height() > 6.0, "caret has line height, got {caret:?}");

    // Without focus, painting records nothing.
    let mut blurred = FrameState::new();
    blurred.reduced_motion = true;
    let frame = build_frame(&view, &theme, &mut fonts, &mut blurred, (200.0, 60.0), 1.0);
    let _scene = frame.paint(&mut fonts, &mut blurred);
    assert!(blurred.ime_caret().is_none());
}
