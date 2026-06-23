//! `on_drag_end` fires once when a captured drag releases — the pointer-up
//! half of the `on_drag` lifecycle (column-resize commit relies on it). A
//! plain click on a click-only element must never trigger it. All headless.

use fenestra_core::{
    Element, Fonts, FrameState, InputEvent, Theme, build_frame, col, dispatch, div,
};

#[derive(Clone, Debug, PartialEq)]
enum Msg {
    Moved,
    DragEnded,
    Clicked,
    NeverFires,
}

/// A draggable square (0..100) beside a click-only button (100..200). The
/// button carries an `on_drag_end` sentinel that must stay silent — proving
/// the gesture is gated on `on_drag`, not merely on the handler being set.
fn view() -> Element<Msg> {
    col()
        .w(200.0)
        .h(100.0)
        .children([fenestra_core::row().w(200.0).h(100.0).children([
            div()
                .id("drag")
                .w(100.0)
                .h(100.0)
                .on_drag(|_, _| Some(Msg::Moved))
                .on_drag_end(Msg::DragEnded),
            div()
                .id("btn")
                .w(100.0)
                .h(100.0)
                .on_click(Msg::Clicked)
                .on_drag_end(Msg::NeverFires),
        ])])
}

fn drive(events: &[InputEvent]) -> Vec<Msg> {
    let theme = Theme::light();
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    state.reduced_motion = true;
    let v = view();
    let mut msgs = Vec::new();
    for ev in events {
        let frame = build_frame(&v, &theme, &mut fonts, &mut state, (200.0, 100.0), 1.0);
        msgs.extend(dispatch(&v, &frame, &mut state, &mut fonts, ev.clone()).msgs);
    }
    msgs
}

#[test]
fn drag_end_fires_after_a_real_drag() {
    let msgs = drive(&[
        InputEvent::PointerMove { x: 40.0, y: 50.0 },
        InputEvent::PointerDown,
        InputEvent::PointerMove { x: 70.0, y: 50.0 },
        InputEvent::PointerUp,
    ]);
    // The press and the move both drive `on_drag`; the release commits.
    assert_eq!(
        msgs,
        vec![Msg::Moved, Msg::Moved, Msg::DragEnded],
        "drag end commits exactly once, last, after the drag"
    );
}

#[test]
fn drag_end_does_not_fire_on_a_plain_click() {
    // Click squarely on the click-only button: it owns an `on_drag_end`
    // sentinel but no `on_drag`, so only the click is delivered.
    let msgs = drive(&[
        InputEvent::PointerMove { x: 150.0, y: 50.0 },
        InputEvent::PointerDown,
        InputEvent::PointerUp,
    ]);
    assert_eq!(msgs, vec![Msg::Clicked], "a plain click never ends a drag");
    assert!(
        !msgs.contains(&Msg::NeverFires),
        "the sentinel must stay silent"
    );
}

#[test]
fn zero_distance_press_on_a_draggable_still_commits() {
    // Pressing and releasing a draggable without moving is a zero-distance
    // drag: it still commits on release (resize lifecycles clear their
    // active column here, so a stray press can never leave one stuck).
    let msgs = drive(&[
        InputEvent::PointerMove { x: 40.0, y: 50.0 },
        InputEvent::PointerDown,
        InputEvent::PointerUp,
    ]);
    assert_eq!(
        msgs,
        vec![Msg::Moved, Msg::DragEnded],
        "press fires on_drag, release commits on_drag_end"
    );
}
