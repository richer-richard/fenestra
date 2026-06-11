//! Keyframe timelines sample from the frame clock and respect reduced
//! motion, verified through real pixels.

use fenestra_core::{Color, Element, FrameState, Keyframes, Theme, col, div};
use fenestra_shell::render_element_with_state;

fn pulsing_square() -> Element<()> {
    col()
        .p(8.0)
        .children([div().w(40.0).h(40.0).bg(Color::BLACK).keyframes(
            Keyframes::new(1000.0)
                .stop(0.0, |s| s.opacity(1.0))
                .stop(0.5, |s| s.opacity(0.2))
                .stop(1.0, |s| s.opacity(1.0)),
        )])
}

#[test]
fn keyframes_sample_from_the_clock() {
    let theme = Theme::light();

    let mut at_zero = FrameState::new();
    at_zero.tick(0.0);
    let a = render_element_with_state(pulsing_square(), &theme, (60, 60), &mut at_zero);
    assert!(
        a.get_pixel(28, 28)[0] < 40,
        "phase 0 is the opaque stop, got {:?}",
        a.get_pixel(28, 28)
    );

    let mut at_half = FrameState::new();
    at_half.tick(0.5);
    let b = render_element_with_state(pulsing_square(), &theme, (60, 60), &mut at_half);
    assert!(
        b.get_pixel(28, 28)[0] > 150,
        "phase 0.5 is the 20%-opacity stop over a light background, got {:?}",
        b.get_pixel(28, 28)
    );
}

#[test]
fn reduced_motion_pins_the_first_stop() {
    let theme = Theme::light();
    let mut state = FrameState::new();
    state.reduced_motion = true;
    state.tick(0.5);
    let image = render_element_with_state(pulsing_square(), &theme, (60, 60), &mut state);
    assert!(
        image.get_pixel(28, 28)[0] < 40,
        "reduced motion renders the first stop, got {:?}",
        image.get_pixel(28, 28)
    );
}
