//! M5 acceptance: headless typing, selection with Shift+arrows, copy and
//! paste through the (in-memory) clipboard, asserting the final string and
//! goldens for every visual state.

use std::path::PathBuf;

use fenestra_core::{App, Element, Key, KeyInput, SP3, SP4, Theme, col};
use fenestra_kit::text_input;
use fenestra_shell::{SyntheticEvent, render_app, testing::assert_png_snapshot};

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

struct Form {
    value: String,
}

#[derive(Clone)]
enum Msg {
    Edit(String),
}

impl App for Form {
    type Msg = Msg;

    fn update(&mut self, msg: Msg) {
        match msg {
            Msg::Edit(v) => self.value = v,
        }
    }

    fn view(&self) -> Element<Msg> {
        // p(16): the input occupies x 16..236, y 16..52.
        col().p(SP4).items_start().children([text_input(&self.value)
            .placeholder("Type here…")
            .on_input(Msg::Edit)])
    }
}

fn shift(key: Key) -> SyntheticEvent {
    SyntheticEvent::Key(KeyInput {
        key,
        shift: true,
        ctrl: false,
        alt: false,
        meta: false,
    })
}

fn ctrl(c: char) -> SyntheticEvent {
    SyntheticEvent::Key(KeyInput {
        key: Key::Char(c),
        shift: false,
        ctrl: true,
        alt: false,
        meta: false,
    })
}

const SIZE: (u32, u32) = (270, 70);
const CLICK: SyntheticEvent = SyntheticEvent::MouseMove { x: 60.0, y: 34.0 };

/// Type, select with Shift+arrows, copy, move, paste: the final value
/// reflects every editing operation.
#[test]
fn type_select_copy_paste() {
    let theme = Theme::light();
    let mut app = Form {
        value: String::new(),
    };
    let image = render_app(
        &mut app,
        &[
            CLICK,
            SyntheticEvent::MouseDown,
            SyntheticEvent::MouseUp,
            SyntheticEvent::Text("hello world".into()),
            // Select "world" with Shift+ArrowLeft x5.
            shift(Key::ArrowLeft),
            shift(Key::ArrowLeft),
            shift(Key::ArrowLeft),
            shift(Key::ArrowLeft),
            shift(Key::ArrowLeft),
            ctrl('c'),
            SyntheticEvent::Key(KeyInput::plain(Key::End)),
            SyntheticEvent::Text(" ".into()),
            ctrl('v'),
        ],
        SIZE,
        &theme,
    );
    assert_eq!(app.value, "hello world world");
    assert_png_snapshot(snapshot_dir(), "input_after_paste", &image);
}

/// Cut removes the selection and Home/word-jumps move the caret.
#[test]
fn cut_and_home() {
    let theme = Theme::light();
    let mut app = Form {
        value: String::new(),
    };
    render_app(
        &mut app,
        &[
            CLICK,
            SyntheticEvent::MouseDown,
            SyntheticEvent::MouseUp,
            SyntheticEvent::Text("abcdef".into()),
            shift(Key::ArrowLeft),
            shift(Key::ArrowLeft),
            ctrl('x'),
            SyntheticEvent::Key(KeyInput::plain(Key::Home)),
            ctrl('v'),
        ],
        SIZE,
        &theme,
    );
    assert_eq!(app.value, "efabcd");
}

/// Backspace, select-all replace, and word deletion.
#[test]
fn editing_operations() {
    let theme = Theme::light();
    let mut app = Form {
        value: String::new(),
    };
    render_app(
        &mut app,
        &[
            CLICK,
            SyntheticEvent::MouseDown,
            SyntheticEvent::MouseUp,
            SyntheticEvent::Text("draft".into()),
            SyntheticEvent::Key(KeyInput::plain(Key::Backspace)),
            ctrl('a'),
            SyntheticEvent::Text("final words".into()),
            SyntheticEvent::Key(KeyInput {
                key: Key::Backspace,
                shift: false,
                ctrl: false,
                alt: true,
                meta: false,
            }),
        ],
        SIZE,
        &theme,
    );
    assert_eq!(app.value, "final ");
}

// ------------------------------------------------------------ state goldens

#[test]
fn input_states_golden() {
    let theme = Theme::light();
    let states: Element<()> = col().p(SP4).gap(SP3).items_start().bg(theme.bg).children([
        text_input("").placeholder("Placeholder…").id("empty"),
        text_input("Filled value").id("filled"),
        text_input("Invalid value").invalid(true).id("invalid"),
        text_input("Disabled").disabled(true).id("disabled"),
    ]);
    let image = fenestra_shell::render_element(states, &theme, (270, 200));
    assert_png_snapshot(snapshot_dir(), "input_states_light", &image);
}

#[test]
fn input_states_dark_golden() {
    let theme = Theme::dark();
    let states: Element<()> = col().p(SP4).gap(SP3).items_start().bg(theme.bg).children([
        text_input("").placeholder("Placeholder…").id("empty"),
        text_input("Filled value").id("filled"),
        text_input("Invalid value").invalid(true).id("invalid"),
        text_input("Disabled").disabled(true).id("disabled"),
    ]);
    let image = fenestra_shell::render_element(states, &theme, (270, 200));
    assert_png_snapshot(snapshot_dir(), "input_states_dark", &image);
}

#[test]
fn input_hover_golden() {
    let theme = Theme::light();
    let mut app = Form {
        value: "Hover me".into(),
    };
    let image = render_app(&mut app, &[CLICK], SIZE, &theme);
    assert_png_snapshot(snapshot_dir(), "input_hover", &image);
}

/// Focused input with a selection: accent border, caret, selection tint.
#[test]
fn input_focus_selection_golden() {
    let theme = Theme::light();
    let mut app = Form {
        value: String::new(),
    };
    let image = render_app(
        &mut app,
        &[
            CLICK,
            SyntheticEvent::MouseDown,
            SyntheticEvent::MouseUp,
            SyntheticEvent::Text("selected text".into()),
            shift(Key::ArrowLeft),
            shift(Key::ArrowLeft),
            shift(Key::ArrowLeft),
            shift(Key::ArrowLeft),
        ],
        SIZE,
        &theme,
    );
    assert_eq!(app.value, "selected text");
    assert_png_snapshot(snapshot_dir(), "input_focus_selection", &image);
}
