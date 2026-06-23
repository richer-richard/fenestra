//! Swipe recognition: a press, a fast flick past a small distance, and release
//! fire `on_swipe` with the dominant direction; a tiny movement is a tap, not a
//! swipe. All headless via the event dispatcher.

use fenestra_core::{
    Element, Fonts, FrameState, InputEvent, SwipeDir, Theme, build_frame, col, dispatch, div,
};

#[derive(Clone, Debug, PartialEq)]
enum Msg {
    Swiped(SwipeDir),
    Tapped,
}

/// A full-canvas card that recognizes swipes and also taps.
fn view() -> Element<Msg> {
    col().w(200.0).h(200.0).children([div()
        .id("card")
        .w(200.0)
        .h(200.0)
        .on_swipe(Msg::Swiped)
        .on_click(Msg::Tapped)])
}

fn drive(events: &[InputEvent]) -> Vec<Msg> {
    let theme = Theme::light();
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    state.reduced_motion = true;
    let v = view();
    let mut msgs = Vec::new();
    for ev in events {
        let frame = build_frame(&v, &theme, &mut fonts, &mut state, (200.0, 200.0), 1.0);
        msgs.extend(dispatch(&v, &frame, &mut state, &mut fonts, ev.clone()).msgs);
    }
    msgs
}

#[test]
fn horizontal_flick_is_a_right_swipe() {
    let msgs = drive(&[
        InputEvent::PointerMove { x: 30.0, y: 100.0 },
        InputEvent::PointerDown,
        InputEvent::PointerMove { x: 160.0, y: 106.0 },
        InputEvent::PointerUp,
    ]);
    assert!(
        msgs.contains(&Msg::Swiped(SwipeDir::Right)),
        "expected a right swipe, got {msgs:?}"
    );
}

#[test]
fn vertical_flick_is_an_up_swipe() {
    let msgs = drive(&[
        InputEvent::PointerMove { x: 100.0, y: 160.0 },
        InputEvent::PointerDown,
        InputEvent::PointerMove { x: 94.0, y: 30.0 },
        InputEvent::PointerUp,
    ]);
    assert!(
        msgs.contains(&Msg::Swiped(SwipeDir::Up)),
        "expected an up swipe, got {msgs:?}"
    );
}

#[test]
fn a_tiny_movement_is_a_tap_not_a_swipe() {
    let msgs = drive(&[
        InputEvent::PointerMove { x: 100.0, y: 100.0 },
        InputEvent::PointerDown,
        InputEvent::PointerMove { x: 107.0, y: 103.0 },
        InputEvent::PointerUp,
    ]);
    assert!(
        !msgs.iter().any(|m| matches!(m, Msg::Swiped(_))),
        "a 7px move must not swipe, got {msgs:?}"
    );
    assert!(msgs.contains(&Msg::Tapped), "it taps instead: {msgs:?}");
}
