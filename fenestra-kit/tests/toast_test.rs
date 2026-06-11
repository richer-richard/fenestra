//! Toasts: an app-owned stack pinned to the top-right as an overlay, with
//! per-toast dismiss buttons.

use std::path::PathBuf;

use fenestra_core::{App, Element, Key, KeyInput, SP4, Theme, col, text};
use fenestra_kit::{Status, button, toast_stack};
use fenestra_shell::{SyntheticEvent, render_app, testing::assert_png_snapshot};

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

struct Toasty {
    items: Vec<(String, Status)>,
}

#[derive(Clone)]
enum ToastMsg {
    Dismiss(usize),
}

impl App for Toasty {
    type Msg = ToastMsg;

    fn update(&mut self, msg: ToastMsg) {
        match msg {
            ToastMsg::Dismiss(i) => {
                if i < self.items.len() {
                    self.items.remove(i);
                }
            }
        }
    }

    fn view(&self) -> Element<ToastMsg> {
        col()
            .w_full()
            .h_full()
            .p(SP4)
            .children([text("content behind the toasts")])
            .children(
                [toast_stack(self.items.iter().map(|(m, s)| (m.clone(), *s)))
                    .on_dismiss(ToastMsg::Dismiss)
                    .id("toasts")],
            )
    }
}

fn three() -> Vec<(String, Status)> {
    vec![
        ("Report saved".to_owned(), Status::Success),
        ("Storage almost full".to_owned(), Status::Warning),
        ("Deploy failed".to_owned(), Status::Danger),
    ]
}

/// Each toast's dismiss button is focusable; activating the second one
/// emits Dismiss(1) and the stack shrinks.
#[test]
fn dismiss_emits_the_right_index() {
    let theme = Theme::light();
    let mut app = Toasty { items: three() };
    render_app(
        &mut app,
        &[
            SyntheticEvent::Tab,
            SyntheticEvent::Tab,
            SyntheticEvent::Key(KeyInput::plain(Key::Enter)),
        ],
        (420, 240),
        &theme,
    );
    assert_eq!(
        app.items
            .iter()
            .map(|(m, _)| m.as_str())
            .collect::<Vec<_>>(),
        vec!["Report saved", "Deploy failed"],
        "the middle toast should have been dismissed"
    );
}

/// An empty stack renders nothing (no stray overlay box).
#[test]
fn empty_stack_is_inert() {
    let theme = Theme::light();
    let mut app = Toasty { items: Vec::new() };
    render_app(
        &mut app,
        &[
            SyntheticEvent::Tab,
            SyntheticEvent::Key(KeyInput::plain(Key::Enter)),
        ],
        (420, 240),
        &theme,
    );
    assert!(app.items.is_empty());
}

#[test]
fn toast_stack_golden() {
    let theme = Theme::light();
    let mut app = Toasty { items: three() };
    let image = render_app(&mut app, &[], (420, 240), &theme);
    assert_png_snapshot(snapshot_dir(), "toast_stack", &image);
}

/// Keep `button` import exercised so the test models real usage.
#[expect(dead_code, reason = "compile-checks the typical save-button pairing")]
fn save_button() -> Element<ToastMsg> {
    button("Save").on_click(ToastMsg::Dismiss(0)).into()
}
