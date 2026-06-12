//! 0.4 kit: context menus (right-click + app-owned open flag) and the
//! combobox — driven through the 0.5 semantic harness instead of
//! coordinates: find widgets the way users do.

use fenestra_core::{App, Element, Key, KeyInput, Semantics, Theme, by, col, div};
use fenestra_kit::{combobox, context_menu};
use fenestra_shell::Harness;

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
    let mut h = Harness::new(Files::default(), Theme::light(), (400, 300));
    assert!(h.query(&by::id("ctx")).is_none(), "menu starts closed");

    h.right_click(&by::id("target"));
    assert!(h.query(&by::id("ctx")).is_some(), "right-click opened it");

    h.click(&by::role(Semantics::Button).name("Rename"));
    assert_eq!(h.app().picked, Some("rename"));
    assert!(!h.app().menu_open, "the app closed the menu on pick");
    assert!(h.query(&by::id("ctx")).is_none());

    // The harness logged the emitted messages (open + pick).
    let msgs = h.take_messages();
    assert!(matches!(msgs.first(), Some(FileMsg::OpenMenu)));
    assert!(matches!(msgs.last(), Some(FileMsg::Pick("rename"))));
}

#[test]
fn escape_asks_the_menu_to_close() {
    let mut h = Harness::new(Files::default(), Theme::light(), (400, 300));
    h.right_click(&by::id("target"));
    h.key(KeyInput::plain(Key::Escape));
    assert!(!h.app().menu_open);
    assert!(h.app().picked.is_none());
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
    let mut h = Harness::new(Langs::default(), Theme::light(), (400, 300));
    h.tab();
    h.type_text("ru");
    // Typing filtered the listbox down to the matching options.
    assert!(
        h.query(&by::role(Semantics::Button).name("Python"))
            .is_none()
    );
    h.click(&by::role(Semantics::Button).name("Rust"));
    assert_eq!(h.app().value, "Rust", "picking writes the option back");
    assert_eq!(h.app().picked.as_deref(), Some("Rust"));
    assert!(!h.app().open);
    // The input now exposes the picked value to queries too.
    assert!(h.query(&by::value("Rust")).is_some());
}
