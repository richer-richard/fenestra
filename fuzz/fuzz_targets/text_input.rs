//! Hostile editing: arbitrary text commits and key chords against a
//! focused input never panic, and the editor's value stays valid UTF-8
//! (it is a String, so this asserts the pipeline never slices bytes).

#![no_main]

use arbitrary::Arbitrary;
use fenestra_core::{
    Element, Fonts, FrameState, InputEvent, Key, KeyInput, Theme, build_frame, col, dispatch,
    raw_input,
};
use libfuzzer_sys::fuzz_target;

#[derive(Arbitrary, Debug)]
enum Ev {
    Text(String),
    Chord { key: u8, shift: bool, ctrl: bool, alt: bool, meta: bool },
    ClickAt { x: u16, y: u16 },
    Tab,
}

fn key_of(byte: u8) -> Key {
    match byte % 14 {
        0 => Key::Enter,
        1 => Key::Space,
        2 => Key::Escape,
        3 => Key::ArrowLeft,
        4 => Key::ArrowRight,
        5 => Key::ArrowUp,
        6 => Key::ArrowDown,
        7 => Key::Home,
        8 => Key::End,
        9 => Key::Backspace,
        10 => Key::Delete,
        11 => Key::PageUp,
        12 => Key::PageDown,
        _ => Key::Char(char::from(byte)),
    }
}

fuzz_target!(|events: Vec<Ev>| {
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    state.reduced_motion = true;
    let mut value = String::new();
    for ev in events.into_iter().take(48) {
        let view: Element<String> = col().p(8.0).children([raw_input(&value, "fuzz")
            .on_input(|s| s.to_owned())
            .id("f")]);
        let frame = build_frame(&view, &Theme::light(), &mut fonts, &mut state, (300.0, 100.0), 1.0);
        let input = match ev {
            Ev::Text(t) => InputEvent::Text(t),
            Ev::Chord { key, shift, ctrl, alt, meta } => InputEvent::Key(KeyInput {
                key: key_of(key),
                shift,
                ctrl,
                alt,
                meta,
            }),
            Ev::ClickAt { x, y } => InputEvent::PointerMove {
                x: f32::from(x % 320),
                y: f32::from(y % 120),
            },
            Ev::Tab => InputEvent::Tab,
        };
        let result = dispatch(&view, &frame, &mut state, &mut fonts, input);
        if let Some(new_value) = result.msgs.into_iter().last() {
            value = new_value;
        }
    }
});
