//! Combobox + command-palette keyboard tests. The goldens lock the visual: each
//! open option list with one row carrying the keyboard cursor (the accent veil),
//! light + dark per widget. The harness tests then drive the actual keyboard —
//! Up/Down step the cursor (clamped at the ends) and Enter picks/runs the
//! highlighted row, not merely the first.

use std::path::PathBuf;

use fenestra_core::{App, Element, Key, KeyInput, SP6, Theme, col};
use fenestra_kit::{combobox, command_palette};
use fenestra_shell::{Harness, render_element, testing::assert_png_snapshot};

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

const COMBOBOX_SIZE: (u32, u32) = (360, 340);
const PALETTE_SIZE: (u32, u32) = (560, 380);

fn combobox_view(theme: &Theme) -> Element<()> {
    // Open over the full option set with the second row (Ruby) as the cursor.
    let cb: Element<()> = combobox("", true, ["Rust", "Ruby", "Python", "Go", "Zig"])
        .width(240.0)
        .placeholder("Language…")
        .highlighted(Some(1))
        .on_input(|_| ())
        .on_pick(|_| ())
        .on_navigate(|_| ())
        .on_close(())
        .id("lang")
        .into();
    col().p(SP6).bg(theme.bg).children([cb])
}

fn palette_view() -> Element<()> {
    // The modal launcher with the second command highlighted as the cursor.
    command_palette(
        "",
        true,
        [
            ("Open file…", ()),
            ("Go to line…", ()),
            ("Toggle theme", ()),
            ("Close window", ()),
        ],
    )
    .highlighted(Some(1))
    .on_input(|_| ())
    .on_navigate(|_| ())
    .on_close(())
    .into()
}

#[test]
fn combobox_open_light() {
    let theme = Theme::light();
    let image = render_element(combobox_view(&theme), &theme, COMBOBOX_SIZE);
    assert_png_snapshot(snapshot_dir(), "combobox_open_light", &image);
}

#[test]
fn combobox_open_dark() {
    let theme = Theme::dark();
    let image = render_element(combobox_view(&theme), &theme, COMBOBOX_SIZE);
    assert_png_snapshot(snapshot_dir(), "combobox_open_dark", &image);
}

#[test]
fn command_palette_open_light() {
    let theme = Theme::light();
    let image = render_element(palette_view(), &theme, PALETTE_SIZE);
    assert_png_snapshot(snapshot_dir(), "command_palette_open_light", &image);
}

#[test]
fn command_palette_open_dark() {
    let theme = Theme::dark();
    let image = render_element(palette_view(), &theme, PALETTE_SIZE);
    assert_png_snapshot(snapshot_dir(), "command_palette_open_dark", &image);
}

// ----------------------------------------------------- combobox keyboard

#[derive(Clone)]
enum ComboMsg {
    Type(String),
    Navigate(usize),
    Pick(String),
    Close,
}

#[derive(Default)]
struct ComboApp {
    value: String,
    open: bool,
    highlight: usize,
    picked: Option<String>,
}

impl App for ComboApp {
    type Msg = ComboMsg;

    fn update(&mut self, msg: ComboMsg) {
        match msg {
            ComboMsg::Type(s) => {
                self.open = !s.is_empty();
                self.value = s;
                self.highlight = 0;
            }
            ComboMsg::Navigate(i) => self.highlight = i,
            ComboMsg::Pick(s) => {
                self.value.clone_from(&s);
                self.picked = Some(s);
                self.open = false;
            }
            ComboMsg::Close => self.open = false,
        }
    }

    fn view(&self) -> Element<ComboMsg> {
        col().p(16.0).items_start().children([combobox(
            &self.value,
            self.open,
            ["Rust", "Ruby", "Python"],
        )
        .highlighted(Some(self.highlight))
        .on_input(ComboMsg::Type)
        .on_navigate(ComboMsg::Navigate)
        .on_pick(ComboMsg::Pick)
        .on_close(ComboMsg::Close)
        .id("lang")])
    }
}

#[test]
fn combobox_arrows_clamp_and_enter_picks_the_cursor() {
    let app = ComboApp {
        open: true,
        ..ComboApp::default()
    };
    let mut h = Harness::new(app, Theme::light(), (400, 360));
    h.tab(); // focus the input (the options are not tab stops)

    // Up at the top is a no-op (clamps), Down walks to the last and clamps.
    h.key(KeyInput::plain(Key::ArrowUp));
    assert_eq!(h.app().highlight, 0, "ArrowUp clamps at the first option");
    h.key(KeyInput::plain(Key::ArrowDown));
    assert_eq!(h.app().highlight, 1);
    h.key(KeyInput::plain(Key::ArrowDown));
    assert_eq!(h.app().highlight, 2);
    h.key(KeyInput::plain(Key::ArrowDown));
    assert_eq!(h.app().highlight, 2, "ArrowDown clamps at the last option");

    // Enter picks the highlighted option (index 2), not the first.
    h.key(KeyInput::plain(Key::Enter));
    assert_eq!(h.app().picked.as_deref(), Some("Python"));
    assert!(!h.app().open, "picking closes the listbox");
}

// ------------------------------------------------ command-palette keyboard

#[derive(Clone)]
enum PalMsg {
    Type(String),
    Navigate(usize),
    Run(&'static str),
    Close,
}

#[derive(Default)]
struct PalApp {
    query: String,
    open: bool,
    highlight: usize,
    ran: Option<&'static str>,
}

impl App for PalApp {
    type Msg = PalMsg;

    fn update(&mut self, msg: PalMsg) {
        match msg {
            PalMsg::Type(s) => {
                self.query = s;
                self.highlight = 0;
            }
            PalMsg::Navigate(i) => self.highlight = i,
            PalMsg::Run(name) => {
                self.ran = Some(name);
                self.open = false;
            }
            PalMsg::Close => self.open = false,
        }
    }

    fn view(&self) -> Element<PalMsg> {
        command_palette(
            &self.query,
            self.open,
            [
                ("Open file…", PalMsg::Run("open")),
                ("Go to line…", PalMsg::Run("goto")),
                ("Toggle theme", PalMsg::Run("toggle")),
            ],
        )
        .highlighted(Some(self.highlight))
        .on_input(PalMsg::Type)
        .on_navigate(PalMsg::Navigate)
        .on_close(PalMsg::Close)
        .into()
    }
}

#[test]
fn palette_arrows_move_cursor_and_enter_runs_it() {
    let app = PalApp {
        open: true,
        ..PalApp::default()
    };
    let mut h = Harness::new(app, Theme::light(), (560, 400));
    h.tab(); // focus the autofocused query input

    h.key(KeyInput::plain(Key::ArrowDown));
    assert_eq!(h.app().highlight, 1);
    h.key(KeyInput::plain(Key::ArrowDown));
    assert_eq!(h.app().highlight, 2);

    // Enter runs the highlighted command (index 2 = "Toggle theme").
    h.key(KeyInput::plain(Key::Enter));
    assert_eq!(h.app().ran, Some("toggle"));
    assert!(!h.app().open, "running a command closes the palette");
}
