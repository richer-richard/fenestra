//! Component composition via `Element::map`: a child component built around
//! its own message type embeds into a parent that wraps those messages.

use fenestra_core::{
    Element, Fonts, FrameState, InputEvent, Key, KeyInput, Theme, build_frame, col, dispatch, div,
    raw_input, row, text,
};

#[derive(Clone, Debug, PartialEq)]
enum ChildMsg {
    Clicked,
    Typed(String),
    Stepped(i32),
}

#[derive(Clone, Debug, PartialEq)]
enum ParentMsg {
    Left(ChildMsg),
    Right(ChildMsg),
}

/// A self-contained component speaking its own message type.
fn clicker(label: &str) -> Element<ChildMsg> {
    div()
        .w(50.0)
        .h(30.0)
        .id(label)
        .on_click(ChildMsg::Clicked)
        .child(text(label))
}

fn drive(view: &Element<ParentMsg>, events: &[InputEvent]) -> Vec<ParentMsg> {
    let theme = Theme::light();
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    state.reduced_motion = true;
    let mut msgs = Vec::new();
    for ev in events {
        let frame = build_frame(view, &theme, &mut fonts, &mut state, (300.0, 100.0), 1.0);
        msgs.extend(dispatch(view, &frame, &mut state, &mut fonts, ev.clone()).msgs);
    }
    msgs
}

/// Clicks on two mapped instances of the same component arrive wrapped in
/// each instance's own parent variant.
#[test]
fn map_wraps_click_messages_per_instance() {
    let view: Element<ParentMsg> = row().children([
        clicker("left").map(ParentMsg::Left),
        clicker("right").map(ParentMsg::Right),
    ]);
    let click_at = |x: f32| {
        [
            InputEvent::PointerMove { x, y: 15.0 },
            InputEvent::PointerDown,
            InputEvent::PointerUp,
        ]
    };
    assert_eq!(
        drive(&view, &click_at(25.0)),
        vec![ParentMsg::Left(ChildMsg::Clicked)]
    );
    assert_eq!(
        drive(&view, &click_at(75.0)),
        vec![ParentMsg::Right(ChildMsg::Clicked)]
    );
}

/// Closure-based handlers (`on_input`, `on_key`) map through too, including
/// on children below the mapped root.
#[test]
fn map_wraps_closure_handlers_and_recurses() {
    let editor: Element<ChildMsg> = col().children([
        raw_input("", "type here")
            .w(120.0)
            .on_input(|s| ChildMsg::Typed(s.to_owned())),
        div()
            .w(40.0)
            .h(20.0)
            .focusable(true)
            .on_key(|k| match k.key {
                Key::ArrowUp => Some(ChildMsg::Stepped(1)),
                Key::ArrowDown => Some(ChildMsg::Stepped(-1)),
                _ => None,
            }),
    ]);
    let view: Element<ParentMsg> = col().p(8.0).children([editor.map(ParentMsg::Left)]);

    let msgs = drive(
        &view,
        &[
            InputEvent::Tab,
            InputEvent::Text("hi".into()),
            InputEvent::Tab,
            InputEvent::Key(KeyInput::plain(Key::ArrowUp)),
        ],
    );
    assert_eq!(
        msgs,
        vec![
            ParentMsg::Left(ChildMsg::Typed("hi".into())),
            ParentMsg::Left(ChildMsg::Stepped(1)),
        ]
    );
}
