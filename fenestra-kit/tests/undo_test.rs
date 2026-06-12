//! 0.6 undo/redo: QUndoStack-style semantics — typing coalesces into
//! runs, caret moves break runs, redo clears on new edits — asserted
//! through the app value (undo emits `on_input`; the app stays the
//! source of truth).

use fenestra_core::{App, Element, Key, KeyInput, Theme, by, col};
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

fn focused() -> Harness<Form> {
    let mut h = Harness::new(Form::default(), Theme::light(), (400, 200));
    h.tab();
    h
}

fn undo() -> KeyInput {
    let mut k = KeyInput::plain(Key::Char('z'));
    k.meta = true;
    k
}

fn redo() -> KeyInput {
    let mut k = KeyInput::plain(Key::Char('z'));
    k.meta = true;
    k.shift = true;
    k
}

#[test]
fn typing_coalesces_into_one_undo_run() {
    let mut h = focused();
    for c in ["h", "e", "l", "l", "o"] {
        h.type_text(c);
    }
    assert_eq!(h.app().value, "hello");
    h.key(undo());
    assert_eq!(h.app().value, "", "one undo reverts the whole run");
}

#[test]
fn undo_then_redo_round_trips() {
    let mut h = focused();
    h.type_text("hello world");
    h.key(undo());
    assert_eq!(h.app().value, "");
    h.key(redo());
    assert_eq!(h.app().value, "hello world");
}

#[test]
fn caret_moves_break_coalescing() {
    let mut h = focused();
    h.type_text("hello");
    h.key(KeyInput::plain(Key::ArrowLeft));
    h.type_text("X");
    assert_eq!(h.app().value, "hellXo");
    h.key(undo());
    assert_eq!(h.app().value, "hello", "second run undone first");
    h.key(undo());
    assert_eq!(h.app().value, "", "first run undone second");
}

#[test]
fn deletes_coalesce_separately_from_inserts() {
    let mut h = focused();
    h.type_text("hello");
    h.key(KeyInput::plain(Key::Backspace));
    h.key(KeyInput::plain(Key::Backspace));
    assert_eq!(h.app().value, "hel");
    h.key(undo());
    assert_eq!(h.app().value, "hello", "the delete run undoes as one");
    h.key(undo());
    assert_eq!(h.app().value, "");
}

#[test]
fn new_edits_clear_the_redo_stack() {
    let mut h = focused();
    h.type_text("a");
    h.key(undo());
    assert_eq!(h.app().value, "");
    h.type_text("b");
    h.key(redo());
    assert_eq!(h.app().value, "b", "redo after a fresh edit is a no-op");
}

#[test]
fn ctrl_y_also_redoes() {
    let mut h = focused();
    h.type_text("a");
    h.key(undo());
    let mut y = KeyInput::plain(Key::Char('y'));
    y.ctrl = true;
    h.key(y);
    assert_eq!(h.app().value, "a");
}

#[test]
fn undo_restores_the_selection_too() {
    let mut h = focused();
    h.type_text("hello world");
    // Select the word and replace it; undo brings text AND selection
    // back, so redo-typing replaces the same word again.
    h.double_click(&by::id("field"));
    h.type_text("X");
    assert_eq!(h.app().value, "hello X");
    h.key(undo());
    assert_eq!(h.app().value, "hello world");
    h.type_text("Y");
    assert_eq!(h.app().value, "hello Y", "restored selection was replaced");
}
