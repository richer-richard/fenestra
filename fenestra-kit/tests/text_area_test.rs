//! Multiline text area: Enter inserts a newline, committed text keeps
//! newlines, and the box grows with its wrapped content.

use std::path::PathBuf;

use fenestra_core::{App, Element, Key, KeyInput, SP4, Theme, col};
use fenestra_kit::text_area;
use fenestra_shell::{SyntheticEvent, render_app, testing::assert_png_snapshot};

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

struct Notes {
    value: String,
}

#[derive(Clone)]
enum NotesMsg {
    Set(String),
}

impl App for Notes {
    type Msg = NotesMsg;

    fn update(&mut self, msg: NotesMsg) {
        match msg {
            NotesMsg::Set(s) => self.value = s,
        }
    }

    fn view(&self) -> Element<NotesMsg> {
        col().p(SP4).items_start().children([text_area(&self.value)
            .placeholder("Write something…")
            .width(260.0)
            .on_input(NotesMsg::Set)
            .id("notes")])
    }
}

/// Enter inserts a hard newline instead of being swallowed, and multiline
/// committed text keeps its newlines.
#[test]
fn enter_inserts_newline() {
    let theme = Theme::light();
    let mut app = Notes {
        value: String::new(),
    };
    render_app(
        &mut app,
        &[
            SyntheticEvent::Tab,
            SyntheticEvent::Text("hello".into()),
            SyntheticEvent::Key(KeyInput::plain(Key::Enter)),
            SyntheticEvent::Text("world".into()),
        ],
        (320, 200),
        &theme,
    );
    assert_eq!(app.value, "hello\nworld");

    // Committed text with embedded newlines (e.g. IME or paste) keeps them
    // in a text area...
    let mut app = Notes {
        value: String::new(),
    };
    render_app(
        &mut app,
        &[SyntheticEvent::Tab, SyntheticEvent::Text("a\nb".into())],
        (320, 200),
        &theme,
    );
    assert_eq!(app.value, "a\nb", "text areas keep committed newlines");
}

/// Long values wrap and the box grows; the empty state shows a placeholder.
#[test]
fn text_area_golden() {
    let theme = Theme::light();
    let mut filled = Notes {
        value: "The quick brown fox jumps over the lazy dog.\nSecond paragraph wraps when it \
                runs out of horizontal room."
            .to_owned(),
    };
    let image = render_app(&mut filled, &[], (320, 220), &theme);
    assert_png_snapshot(snapshot_dir(), "text_area_filled", &image);

    let mut empty = Notes {
        value: String::new(),
    };
    let image = render_app(&mut empty, &[], (320, 160), &theme);
    assert_png_snapshot(snapshot_dir(), "text_area_empty", &image);
}
