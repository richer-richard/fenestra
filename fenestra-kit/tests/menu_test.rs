//! 0.4 kit: context menus (right-click + app-owned open flag) and the
//! combobox (filtering text input + pickable listbox).

use fenestra_core::{App, Element, Key, KeyInput, Theme, col, div};
use fenestra_kit::{combobox, context_menu};
use fenestra_shell::{SyntheticEvent, render_app};

// ------------------------------------------------------------ context menu

#[derive(Default)]
struct Files {
    menu_open: bool,
    picked: Option<&'static str>,
}

#[derive(Clone)]
enum FileMsg {
    OpenMenu,
    CloseMenu,
    Pick(&'static str),
}

impl App for Files {
    type Msg = FileMsg;

    fn update(&mut self, msg: FileMsg) {
        match msg {
            FileMsg::OpenMenu => self.menu_open = true,
            FileMsg::CloseMenu => self.menu_open = false,
            FileMsg::Pick(what) => {
                self.picked = Some(what);
                self.menu_open = false;
            }
        }
    }

    fn view(&self) -> Element<FileMsg> {
        let mut target = div()
            .w(200.0)
            .h(120.0)
            .id("target")
            .on_right_click(FileMsg::OpenMenu);
        if self.menu_open {
            target = target.child(
                context_menu([
                    ("Rename", FileMsg::Pick("rename")),
                    ("Delete", FileMsg::Pick("delete")),
                ])
                .on_close(FileMsg::CloseMenu)
                .id("ctx"),
            );
        }
        col().children([target])
    }
}

#[test]
fn right_click_menu_opens_at_pointer_and_picks() {
    let theme = Theme::light();
    let mut app = Files::default();
    render_app(
        &mut app,
        &[
            SyntheticEvent::MouseMove { x: 50.0, y: 50.0 },
            SyntheticEvent::RightDown,
            SyntheticEvent::RightUp,
            // First item: menu pinned at (52, 52); panel padding 4, rows
            // 30 tall — click inside the first row.
            SyntheticEvent::MouseMove { x: 90.0, y: 70.0 },
            SyntheticEvent::MouseDown,
            SyntheticEvent::MouseUp,
        ],
        (400, 300),
        &theme,
    );
    assert_eq!(app.picked, Some("rename"));
    assert!(!app.menu_open, "the app closed the menu on pick");
}

#[test]
fn escape_asks_the_menu_to_close() {
    let theme = Theme::light();
    let mut app = Files::default();
    render_app(
        &mut app,
        &[
            SyntheticEvent::MouseMove { x: 50.0, y: 50.0 },
            SyntheticEvent::RightDown,
            SyntheticEvent::Key(KeyInput::plain(Key::Escape)),
        ],
        (400, 300),
        &theme,
    );
    assert!(!app.menu_open);
    assert!(app.picked.is_none());
}

// ---------------------------------------------------------------- combobox

#[derive(Default)]
struct Langs {
    value: String,
    open: bool,
    picked: Option<String>,
}

#[derive(Clone)]
enum LangMsg {
    Type(String),
    Pick(String),
    Close,
}

impl App for Langs {
    type Msg = LangMsg;

    fn update(&mut self, msg: LangMsg) {
        match msg {
            LangMsg::Type(s) => {
                self.open = !s.is_empty();
                self.value = s;
            }
            LangMsg::Pick(s) => {
                self.value.clone_from(&s);
                self.picked = Some(s);
                self.open = false;
            }
            LangMsg::Close => self.open = false,
        }
    }

    fn view(&self) -> Element<LangMsg> {
        col().p(16.0).items_start().children([combobox(
            &self.value,
            self.open,
            ["Rust", "Ruby", "Python"],
        )
        .width(220.0)
        .on_input(LangMsg::Type)
        .on_pick(LangMsg::Pick)
        .on_close(LangMsg::Close)
        .id("lang")])
    }
}

#[test]
fn combobox_filters_and_picks() {
    let theme = Theme::light();
    let mut app = Langs::default();
    render_app(
        &mut app,
        &[
            SyntheticEvent::Tab,
            SyntheticEvent::Text("ru".into()),
            // Listbox sits below the input (y 16..52), gap 4, padding 4:
            // first option row spans roughly y 60..90.
            SyntheticEvent::MouseMove { x: 60.0, y: 72.0 },
            SyntheticEvent::MouseDown,
            SyntheticEvent::MouseUp,
        ],
        (400, 300),
        &theme,
    );
    assert_eq!(app.value, "Rust", "picking writes the option back");
    assert_eq!(app.picked.as_deref(), Some("Rust"));
    assert!(!app.open);
}
