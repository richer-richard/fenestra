//! 0.4 drag-and-drop: OS file drops and the internal drag-source/drop
//! primitive.

use std::path::PathBuf;

use fenestra_core::{App, Element, Theme, col, div, row};
use fenestra_shell::{SyntheticEvent, render_app};

#[derive(Default)]
struct Dnd {
    dropped_file: Option<PathBuf>,
    moved: Option<String>,
}

#[derive(Clone)]
enum Msg {
    File(PathBuf),
    Moved(String),
}

impl App for Dnd {
    type Msg = Msg;

    fn update(&mut self, msg: Msg) {
        match msg {
            Msg::File(p) => self.dropped_file = Some(p),
            Msg::Moved(s) => self.moved = Some(s),
        }
    }

    fn view(&self) -> Element<Msg> {
        col().children([row().children([
            // A: a drag source (50x30 at x 0..50).
            div().w(50.0).h(30.0).id("a").drag_source("item-3"),
            // B: accepts drops and files (50x30 at x 50..100).
            div()
                .w(50.0)
                .h(30.0)
                .id("b")
                .on_drop(|payload| Some(Msg::Moved(payload.to_owned())))
                .on_file_drop(|p| Msg::File(p.to_path_buf())),
        ])])
    }
}

#[test]
fn file_drop_reaches_the_element_under_the_pointer() {
    let mut app = Dnd::default();
    render_app(
        &mut app,
        &[
            SyntheticEvent::MouseMove { x: 75.0, y: 15.0 },
            SyntheticEvent::FileDrop(PathBuf::from("/tmp/notes.txt")),
        ],
        (200, 100),
        &Theme::light(),
    );
    assert_eq!(
        app.dropped_file.as_deref(),
        Some(std::path::Path::new("/tmp/notes.txt"))
    );
}

#[test]
fn file_drop_with_no_hit_falls_back_to_the_first_handler() {
    let mut app = Dnd::default();
    render_app(
        &mut app,
        // No pointer position at all.
        &[SyntheticEvent::FileDrop(PathBuf::from("/tmp/x.csv"))],
        (200, 100),
        &Theme::light(),
    );
    assert!(app.dropped_file.is_some(), "fallback delivery still works");
}

#[test]
fn drag_from_source_drops_on_target() {
    let mut app = Dnd::default();
    render_app(
        &mut app,
        &[
            SyntheticEvent::MouseMove { x: 25.0, y: 15.0 },
            SyntheticEvent::MouseDown,
            SyntheticEvent::MouseMove { x: 75.0, y: 15.0 },
            SyntheticEvent::MouseUp,
        ],
        (200, 100),
        &Theme::light(),
    );
    assert_eq!(app.moved.as_deref(), Some("item-3"));
}

#[test]
fn release_outside_a_target_drops_nothing() {
    let mut app = Dnd::default();
    render_app(
        &mut app,
        &[
            SyntheticEvent::MouseMove { x: 25.0, y: 15.0 },
            SyntheticEvent::MouseDown,
            SyntheticEvent::MouseMove { x: 150.0, y: 80.0 },
            SyntheticEvent::MouseUp,
        ],
        (200, 100),
        &Theme::light(),
    );
    assert!(app.moved.is_none());
}
