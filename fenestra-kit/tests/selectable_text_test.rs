//! 0.9 selectable static text: drag/double/triple-click select,
//! Cmd/Ctrl+C copies — proven by pasting into an input (the clipboard
//! is asserted through public behavior, end to end).

use fenestra_core::{App, Element, InputEvent, Key, KeyInput, Theme, by, col, text};
use fenestra_kit::text_input;
use fenestra_shell::Harness;

#[derive(Default)]
struct Reader {
    pasted: String,
}

#[derive(Clone)]
struct Set(String);

impl App for Reader {
    type Msg = Set;

    fn update(&mut self, Set(s): Set) {
        self.pasted = s;
    }

    fn view(&self) -> Element<Set> {
        col().p(16.0).gap(12.0).items_start().children((
            text("copy this exact sentence").selectable().id("para"),
            text_input(&self.pasted)
                .width(260.0)
                .on_input(|s| Set(s.to_owned()))
                .id("sink"),
        ))
    }
}

fn copy_combo() -> KeyInput {
    let mut k = KeyInput::plain(Key::Char('c'));
    k.meta = true;
    k
}

fn paste_combo() -> KeyInput {
    let mut k = KeyInput::plain(Key::Char('v'));
    k.meta = true;
    k
}

#[test]
fn drag_select_then_copy_then_paste() {
    let mut h = Harness::new(Reader::default(), Theme::light(), (420, 160));
    let rect = h.get(&by::id("para")).rect;
    #[expect(clippy::cast_possible_truncation, reason = "test coords")]
    let (y, x0, x1) = (
        rect.center().y as f32,
        rect.x0 as f32 + 1.0,
        rect.x1 as f32 - 1.0,
    );
    // Drag across the whole sentence, copy, then paste into the input.
    h.input(InputEvent::PointerMove { x: x0, y });
    h.input(InputEvent::PointerDown);
    h.input(InputEvent::PointerMove { x: x1, y });
    h.input(InputEvent::PointerUp);
    h.key(copy_combo());

    h.tab(); // focus the input (the text itself is not focusable)
    h.key(paste_combo());
    assert_eq!(h.app().pasted, "copy this exact sentence");
}

#[test]
fn double_click_selects_one_word() {
    let mut h = Harness::new(Reader::default(), Theme::light(), (420, 160));
    h.double_click(&by::id("para")); // center lands inside the sentence
    h.key(copy_combo());
    h.tab();
    h.key(paste_combo());
    let pasted = h.app().pasted.clone();
    assert!(
        ["copy", "this", "exact", "sentence"].contains(&pasted.trim()),
        "double-click selected one word, got {pasted:?}"
    );
}

#[test]
fn triple_click_selects_the_line_and_exposes_selection() {
    let mut h = Harness::new(Reader::default(), Theme::light(), (420, 160));
    h.triple_click(&by::id("para"));
    // Headless a11y exposure, like inputs.
    let node = h.get(&by::id("para"));
    assert_eq!(
        node.selection,
        Some((0, "copy this exact sentence".len())),
        "the whole line is selected"
    );
    // Clicking elsewhere clears it.
    h.click(&by::id("sink"));
    let node = h.get(&by::id("para"));
    assert_eq!(node.selection, None);
}

#[test]
fn selection_highlight_renders() {
    let mut h = Harness::new(Reader::default(), Theme::light(), (420, 160));
    h.triple_click(&by::id("para"));
    let image = h.render();
    fenestra_shell::testing::assert_png_snapshot(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("snapshots"),
        "static_selection",
        &image,
    );
}
