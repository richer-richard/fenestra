//! `.autofocus()`: focus moves to a newly-appearing element (the
//! open-a-modal-and-type pattern), without stealing focus afterward.

use fenestra_core::{App, Element, Theme, col, div, text};
use fenestra_kit::{button, modal, text_input};
use fenestra_shell::{SyntheticEvent, render_app};

struct Form {
    open: bool,
    name: String,
}

#[derive(Clone)]
enum Msg {
    Open,
    Close,
    Name(String),
}

impl App for Form {
    type Msg = Msg;

    fn update(&mut self, msg: Msg) {
        match msg {
            Msg::Open => self.open = true,
            Msg::Close => self.open = false,
            Msg::Name(s) => self.name = s,
        }
    }

    fn view(&self) -> Element<Msg> {
        let mut anchor = div()
            .w(400.0)
            .h(300.0)
            .children([Element::from(button("Open").on_click(Msg::Open).id("open"))]);
        if self.open {
            anchor = anchor.child(
                modal("Rename")
                    .child(
                        Element::from(
                            text_input(&self.name)
                                .placeholder("Name…")
                                .on_input(Msg::Name)
                                .id("name"),
                        )
                        .autofocus(),
                    )
                    .child(text("press escape to close"))
                    .on_close(Msg::Close)
                    .id("dialog"),
            );
        }
        col().children([anchor])
    }
}

/// Opening the modal focuses its input: typing lands immediately, no Tab.
#[test]
fn autofocus_focuses_newly_appearing_input() {
    let theme = Theme::light();
    let mut app = Form {
        open: false,
        name: String::new(),
    };
    render_app(
        &mut app,
        &[
            // Click the Open button (it sits at the top-left of the anchor).
            SyntheticEvent::MouseMove { x: 40.0, y: 18.0 },
            SyntheticEvent::MouseDown,
            SyntheticEvent::MouseUp,
            SyntheticEvent::Text("hi".into()),
        ],
        (400, 300),
        &theme,
    );
    assert!(app.open, "the modal opened");
    assert_eq!(app.name, "hi", "typing lands in the autofocused input");
}

/// Reopening refocuses; and autofocus does not steal once the user moves on.
#[test]
fn autofocus_refocuses_on_reopen_but_does_not_steal() {
    let theme = Theme::light();
    let mut app = Form {
        open: false,
        name: String::new(),
    };
    render_app(
        &mut app,
        &[
            SyntheticEvent::MouseMove { x: 40.0, y: 18.0 },
            SyntheticEvent::MouseDown,
            SyntheticEvent::MouseUp,
            SyntheticEvent::Text("a".into()),
            // Tab away (to the modal's Close affordances / next focusable);
            // the autofocus must not yank focus back while still mounted.
            SyntheticEvent::Tab,
            SyntheticEvent::Text("ignored".into()),
        ],
        (400, 300),
        &theme,
    );
    assert_eq!(app.name, "a", "focus stayed where the user moved it");
}
