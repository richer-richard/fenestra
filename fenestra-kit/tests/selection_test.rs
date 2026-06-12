//! 0.6 selection depth: double-click selects the word, triple-click the
//! line, shift-click extends, and drag-select replaces — all asserted
//! through the app's value (Elm: the app owns the text).

use fenestra_core::{App, Element, InputEvent, Key, KeyInput, Theme, by, col};
use fenestra_kit::text_input;
use fenestra_shell::Harness;

#[derive(Default)]
struct Form {
    value: String,
}

#[derive(Clone)]
struct Set(String);

impl App for Form {
    type Msg = Set;

    fn update(&mut self, Set(s): Set) {
        self.value = s;
    }

    fn view(&self) -> Element<Set> {
        col().p(16.0).items_start().children([Element::from(
            text_input(&self.value)
                .width(220.0)
                .on_input(|s| Set(s.to_owned()))
                .id("field"),
        )])
    }
}

fn harness_with(value: &str) -> Harness<Form> {
    let mut h = Harness::new(
        Form {
            value: value.to_owned(),
        },
        Theme::light(),
        (400, 200),
    );
    h.tab(); // focus the field
    h
}

#[test]
fn double_click_selects_the_word_under_the_pointer() {
    let mut h = harness_with("hello world");
    h.double_click(&by::id("field"));
    h.type_text("X");
    // Typing replaced the selected word — never a plain append.
    assert_ne!(h.app().value, "hello worldX");
    assert_eq!(h.app().value, "hello X");
}

#[test]
fn triple_click_selects_the_line() {
    let mut h = harness_with("hello world");
    h.triple_click(&by::id("field"));
    h.type_text("X");
    assert_eq!(h.app().value, "X");
}

#[test]
fn shift_click_extends_from_the_caret() {
    let mut h = harness_with("hello world");
    h.key(KeyInput::plain(Key::Home)); // caret to 0
    h.shift_click(&by::id("field")); // extend to the pointer (past the end)
    h.type_text("X");
    assert_eq!(h.app().value, "X");
}

#[test]
fn drag_across_the_text_selects_it() {
    let mut h = harness_with("hello world");
    let rect = h.get(&by::id("field")).rect;
    #[expect(clippy::cast_possible_truncation, reason = "test coords")]
    let (y, x0, x1) = (
        rect.center().y as f32,
        (rect.x0 + 4.0) as f32,
        (rect.x1 - 4.0) as f32,
    );
    h.input(InputEvent::PointerMove { x: x0, y });
    h.input(InputEvent::PointerDown);
    h.input(InputEvent::PointerMove { x: x1, y });
    h.input(InputEvent::PointerUp);
    h.type_text("X");
    assert_eq!(h.app().value, "X");
}
