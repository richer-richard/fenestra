//! M6 acceptance: the full gallery is green in both themes; the modal traps
//! focus; the select navigates by keyboard.

use std::path::PathBuf;

use fenestra_core::{
    App, BaseField, Contrast, Element, Elevation, Key, KeyInput, Mode, RadiusScale, SP4, Theme,
    col, text,
};
use fenestra_kit::{
    button, console_showcase, gallery_controls, gallery_display, gallery_feedback, modal, select,
    text_input,
};
use fenestra_shell::{SyntheticEvent, render_app, render_element, testing::assert_png_snapshot};

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

#[test]
fn gallery_controls_light() {
    let theme = Theme::light();
    let image = render_element(gallery_controls(&theme), &theme, (688, 900));
    assert_png_snapshot(snapshot_dir(), "gallery_controls_light", &image);
}

#[test]
fn gallery_controls_dark() {
    let theme = Theme::dark();
    let image = render_element(gallery_controls(&theme), &theme, (688, 900));
    assert_png_snapshot(snapshot_dir(), "gallery_controls_dark", &image);
}

#[test]
fn gallery_display_light() {
    let theme = Theme::light();
    let image = render_element(gallery_display(&theme), &theme, (760, 1190));
    assert_png_snapshot(snapshot_dir(), "gallery_display_light", &image);
}

#[test]
fn gallery_display_dark() {
    let theme = Theme::dark();
    let image = render_element(gallery_display(&theme), &theme, (760, 1190));
    assert_png_snapshot(snapshot_dir(), "gallery_display_dark", &image);
}

#[test]
fn gallery_feedback_light() {
    let theme = Theme::light();
    let image = render_element(gallery_feedback(&theme), &theme, (688, 820));
    assert_png_snapshot(snapshot_dir(), "gallery_feedback_light", &image);
}

#[test]
fn gallery_feedback_dark() {
    let theme = Theme::dark();
    let image = render_element(gallery_feedback(&theme), &theme, (688, 820));
    assert_png_snapshot(snapshot_dir(), "gallery_feedback_dark", &image);
}

/// The slate + lime, sharp + flat "console" look (the design-range showcase).
fn console_theme(mode: Mode) -> Theme {
    Theme::derive(
        BaseField {
            hue: 250.0,
            chroma: 1.5,
        },
        130.0,
        Contrast::High,
        mode,
    )
    .with_radius(RadiusScale::sharp())
    .with_elevation(Elevation::Flat)
}

#[test]
fn console_showcase_dark() {
    let theme = console_theme(Mode::Dark);
    let image = render_element(console_showcase(&theme), &theme, (1200, 760));
    assert_png_snapshot(snapshot_dir(), "console_showcase_dark", &image);
}

#[test]
fn console_showcase_light() {
    let theme = console_theme(Mode::Light);
    let image = render_element(console_showcase(&theme), &theme, (1200, 760));
    assert_png_snapshot(snapshot_dir(), "console_showcase_light", &image);
}

// ----------------------------------------------------------------- select

struct Picker {
    selected: usize,
}

#[derive(Clone)]
enum PickMsg {
    Pick(usize),
}

impl App for Picker {
    type Msg = PickMsg;

    fn update(&mut self, msg: PickMsg) {
        match msg {
            PickMsg::Pick(i) => self.selected = i,
        }
    }

    fn view(&self) -> Element<PickMsg> {
        col().p(SP4).items_start().children([Element::from(
            select(self.selected, ["Apple", "Banana", "Cherry", "Apricot"])
                .on_change(PickMsg::Pick)
                .id("fruit"),
        )])
    }
}

/// Arrows step the value, Home/End jump, and first-letter type-ahead
/// scans forward with wrap-around.
#[test]
fn select_keyboard_navigation() {
    let theme = Theme::light();
    let mut app = Picker { selected: 0 };
    render_app(
        &mut app,
        &[
            SyntheticEvent::Tab,
            SyntheticEvent::Key(KeyInput::plain(Key::ArrowDown)),
            SyntheticEvent::Key(KeyInput::plain(Key::ArrowDown)),
            SyntheticEvent::Key(KeyInput::plain(Key::ArrowUp)),
        ],
        (300, 300),
        &theme,
    );
    assert_eq!(app.selected, 1, "down, down, up from 0 lands on 1");

    // Type-ahead: 'a' scans forward from Banana -> Apricot, then wraps to
    // Apple.
    render_app(
        &mut app,
        &[
            SyntheticEvent::Tab,
            SyntheticEvent::Key(KeyInput::plain(Key::Char('a'))),
        ],
        (300, 300),
        &theme,
    );
    assert_eq!(app.selected, 3, "type-ahead jumps to Apricot");
    render_app(
        &mut app,
        &[
            SyntheticEvent::Tab,
            SyntheticEvent::Key(KeyInput::plain(Key::Char('a'))),
        ],
        (300, 300),
        &theme,
    );
    assert_eq!(app.selected, 0, "type-ahead wraps to Apple");
}

/// Clicking the trigger opens the listbox; clicking an option selects it
/// and closes the menu.
#[test]
fn select_open_click_golden() {
    let theme = Theme::light();
    let mut app = Picker { selected: 1 };
    // Open the menu (trigger spans x 16..216, y 16..52).
    let image = render_app(
        &mut app,
        &[
            SyntheticEvent::MouseMove { x: 60.0, y: 34.0 },
            SyntheticEvent::MouseDown,
            SyntheticEvent::MouseUp,
        ],
        (300, 240),
        &theme,
    );
    assert_png_snapshot(snapshot_dir(), "select_open", &image);

    // The listbox sits 4px below the trigger; option 0 is ~ y 60..90.
    render_app(
        &mut app,
        &[
            SyntheticEvent::MouseMove { x: 60.0, y: 34.0 },
            SyntheticEvent::MouseDown,
            SyntheticEvent::MouseUp,
            SyntheticEvent::MouseMove { x: 60.0, y: 75.0 },
            SyntheticEvent::MouseDown,
            SyntheticEvent::MouseUp,
        ],
        (300, 240),
        &theme,
    );
    assert_eq!(app.selected, 0, "clicking the first option selects it");
}

// ----------------------------------------------------------------- modal

struct Dialog {
    open: bool,
    confirmed: bool,
}

#[derive(Clone)]
enum DialogMsg {
    Close,
    Confirm,
}

impl App for Dialog {
    type Msg = DialogMsg;

    fn update(&mut self, msg: DialogMsg) {
        match msg {
            DialogMsg::Close => self.open = false,
            DialogMsg::Confirm => self.confirmed = true,
        }
    }

    fn view(&self) -> Element<DialogMsg> {
        let mut root = col()
            .p(SP4)
            .items_start()
            .children([button("Outside button").id("outside")]);
        if self.open {
            root = root.child(
                modal("Confirm action")
                    .child(text("This cannot be undone."))
                    .child(fenestra_core::row().gap(SP4).children([
                        Element::from(text_input("").placeholder("Reason…").id("reason")),
                        Element::from(button("Confirm").on_click(DialogMsg::Confirm)),
                    ]))
                    .on_close(DialogMsg::Close)
                    .id("dialog"),
            );
        }
        root
    }
}

#[test]
fn modal_golden() {
    let theme = Theme::light();
    let mut app = Dialog {
        open: true,
        confirmed: false,
    };
    let image = render_app(&mut app, &[], (560, 360), &theme);
    assert_png_snapshot(snapshot_dir(), "modal_open", &image);
}

/// Tab cycles only through the modal's focusables while it is open; Esc
/// emits on_close.
#[test]
fn modal_focus_trap() {
    let theme = Theme::light();
    let mut app = Dialog {
        open: true,
        confirmed: false,
    };
    // Modal focusables: close icon-button, the text input, Confirm. Three
    // Tabs wrap back to the close button; a fourth lands on the input; the
    // outside button is never reachable, so Enter on each focus stop must
    // never hit it. Cycle: Tab x4 -> input (close -> input -> confirm ->
    // close -> input). Then Enter on the Confirm stop:
    render_app(
        &mut app,
        &[
            SyntheticEvent::Tab, // close
            SyntheticEvent::Tab, // input
            SyntheticEvent::Tab, // confirm
            SyntheticEvent::Key(KeyInput::plain(Key::Enter)),
        ],
        (560, 360),
        &theme,
    );
    assert!(app.confirmed, "third Tab stop is the Confirm button");
    assert!(app.open, "confirm does not close");

    // Esc asks the app to close.
    render_app(
        &mut app,
        &[SyntheticEvent::Key(KeyInput::plain(Key::Escape))],
        (560, 360),
        &theme,
    );
    assert!(!app.open, "Esc emits on_close");
}

/// Clicking the dimmed backdrop emits on_close.
#[test]
fn modal_backdrop_click_closes() {
    let theme = Theme::light();
    let mut app = Dialog {
        open: true,
        confirmed: false,
    };
    render_app(
        &mut app,
        &[
            SyntheticEvent::MouseMove { x: 8.0, y: 350.0 },
            SyntheticEvent::MouseDown,
            SyntheticEvent::MouseUp,
        ],
        (560, 360),
        &theme,
    );
    assert!(!app.open, "backdrop click emits on_close");
}
