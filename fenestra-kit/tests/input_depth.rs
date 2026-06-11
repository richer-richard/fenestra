//! 0.4 input depth: right-click and double-click dispatch.

use fenestra_core::Theme;
use fenestra_core::{App, Element, col, div, row};
use fenestra_shell::{SyntheticEvent, render_app};

struct Clicks {
    clicks: u32,
    doubles: u32,
    rights: u32,
}

#[derive(Clone)]
enum Msg {
    Click,
    Double,
    Right,
}

impl App for Clicks {
    type Msg = Msg;

    fn update(&mut self, msg: Msg) {
        match msg {
            Msg::Click => self.clicks += 1,
            Msg::Double => self.doubles += 1,
            Msg::Right => self.rights += 1,
        }
    }

    fn view(&self) -> Element<Msg> {
        // Two 50x30 targets side by side at the origin.
        col().children([row().children([
            div()
                .w(50.0)
                .h(30.0)
                .id("a")
                .on_click(Msg::Click)
                .on_double_click(Msg::Double)
                .on_right_click(Msg::Right),
            div().w(50.0).h(30.0).id("b").on_click(Msg::Click),
        ])])
    }
}

fn app() -> Clicks {
    Clicks {
        clicks: 0,
        doubles: 0,
        rights: 0,
    }
}

#[test]
fn right_click_emits() {
    let mut app = app();
    render_app(
        &mut app,
        &[
            SyntheticEvent::MouseMove { x: 25.0, y: 15.0 },
            SyntheticEvent::RightDown,
            SyntheticEvent::RightUp,
        ],
        (200, 100),
        &Theme::light(),
    );
    assert_eq!(app.rights, 1, "right press over the element fires once");
    assert_eq!(app.clicks, 0, "right click is not a left click");
}

#[test]
fn double_click_emits_on_same_element() {
    let mut app = app();
    render_app(
        &mut app,
        &[
            SyntheticEvent::MouseMove { x: 25.0, y: 15.0 },
            SyntheticEvent::MouseDown,
            SyntheticEvent::MouseUp,
            SyntheticEvent::MouseDown,
            SyntheticEvent::MouseUp,
        ],
        (200, 100),
        &Theme::light(),
    );
    assert_eq!(app.clicks, 2, "both single clicks still fire");
    assert_eq!(app.doubles, 1, "the second click within the window doubles");
}

#[test]
fn clicks_on_different_elements_do_not_double() {
    let mut app = app();
    render_app(
        &mut app,
        &[
            SyntheticEvent::MouseMove { x: 25.0, y: 15.0 },
            SyntheticEvent::MouseDown,
            SyntheticEvent::MouseUp,
            SyntheticEvent::MouseMove { x: 75.0, y: 15.0 },
            SyntheticEvent::MouseDown,
            SyntheticEvent::MouseUp,
        ],
        (200, 100),
        &Theme::light(),
    );
    assert_eq!(app.doubles, 0);
}
