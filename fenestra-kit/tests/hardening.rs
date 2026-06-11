//! Robustness regressions from the security/hardening audit: widget
//! callbacks must never emit values the host cannot use safely.

use fenestra_core::{App, Element, Key, KeyInput, SP4, Theme, col};
use fenestra_kit::{select, text_input};
use fenestra_shell::{SyntheticEvent, render_app};

struct PickEmpty {
    picked: Option<usize>,
}

#[derive(Clone)]
enum PickMsg {
    Pick(usize),
}

impl App for PickEmpty {
    type Msg = PickMsg;

    fn update(&mut self, msg: PickMsg) {
        match msg {
            PickMsg::Pick(i) => self.picked = Some(i),
        }
    }

    fn view(&self) -> Element<PickMsg> {
        col()
            .p(SP4)
            .items_start()
            .children([select(0, Vec::<String>::new())
                .on_change(PickMsg::Pick)
                .id("empty-select")])
    }
}

struct OrgName {
    value: String,
}

#[derive(Clone)]
enum NameMsg {
    Set(String),
}

impl App for OrgName {
    type Msg = NameMsg;

    fn update(&mut self, msg: NameMsg) {
        match msg {
            NameMsg::Set(s) => self.value = s,
        }
    }

    fn view(&self) -> Element<NameMsg> {
        col()
            .p(SP4)
            .items_start()
            .children([text_input(&self.value).on_input(NameMsg::Set).id("name")])
    }
}

/// Control characters arriving as `Key::Char` (the keyboard path) must be
/// filtered exactly like the text-commit and paste paths already are: a
/// single-line input must never contain `\r`, `\n`, `\t`, or DEL.
#[test]
fn control_characters_never_enter_text_input() {
    let theme = Theme::light();
    let mut app = OrgName {
        value: String::new(),
    };
    render_app(
        &mut app,
        &[
            SyntheticEvent::Tab,
            SyntheticEvent::Key(KeyInput::plain(Key::Char('\r'))),
            SyntheticEvent::Key(KeyInput::plain(Key::Char('\n'))),
            SyntheticEvent::Key(KeyInput::plain(Key::Char('\t'))),
            SyntheticEvent::Key(KeyInput::plain(Key::Char('\u{7f}'))),
            SyntheticEvent::Text("ok".into()),
        ],
        (300, 100),
        &theme,
    );
    assert_eq!(
        app.value, "ok",
        "control characters must be filtered from keyboard input"
    );
}

/// A select with zero options must never emit an index: the documented
/// contract is that `on_change` receives a valid index into `options`, and
/// hosts index into their data with it.
#[test]
fn empty_select_never_emits_an_index() {
    let theme = Theme::light();
    let mut app = PickEmpty { picked: None };
    render_app(
        &mut app,
        &[
            SyntheticEvent::Tab,
            SyntheticEvent::Key(KeyInput::plain(Key::Home)),
            SyntheticEvent::Key(KeyInput::plain(Key::End)),
            SyntheticEvent::Key(KeyInput::plain(Key::ArrowDown)),
            SyntheticEvent::Key(KeyInput::plain(Key::ArrowUp)),
            SyntheticEvent::Key(KeyInput::plain(Key::Char('a'))),
        ],
        (300, 100),
        &theme,
    );
    assert_eq!(
        app.picked, None,
        "an empty select emitted an option index that does not exist"
    );
}
