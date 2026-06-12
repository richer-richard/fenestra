//! 0.6 accessibility state: live regions reach the tree (and toasts set
//! them automatically), and text inputs expose their caret/selection
//! headlessly.

use fenestra_core::{App, Element, Key, KeyInput, Theme, by, col, div, text};
use fenestra_kit::text_input;
use fenestra_shell::Harness;

#[derive(Default)]
struct Status {
    note: String,
    value: String,
}

#[derive(Clone)]
enum Msg {
    Set(String),
}

impl App for Status {
    type Msg = Msg;

    fn update(&mut self, Msg::Set(s): Msg) {
        self.value = s;
    }

    fn view(&self) -> Element<Msg> {
        col().p(16.0).gap(8.0).items_start().children((
            div()
                .live()
                .id("status")
                .children([text(self.note.clone())]),
            text_input(&self.value)
                .width(220.0)
                .on_input(|s| Msg::Set(s.to_owned()))
                .id("field"),
        ))
    }
}

#[test]
fn live_regions_reach_the_tree_and_the_yaml() {
    let mut h = Harness::new(Status::default(), Theme::light(), (400, 200));
    h.app_mut().note = "Saved!".to_owned();
    h.rebuild();
    let node = h.get(&by::id("status"));
    assert!(node.live, "the region is marked live");
    assert!(
        h.frame().access_yaml().contains("[live]"),
        "yaml shows it:\n{}",
        h.frame().access_yaml()
    );
}

#[test]
fn toasts_are_live_automatically() {
    use fenestra_core::{Fonts, FrameState, build_frame};
    let view: Element<()> = col().children([Element::from(fenestra_kit::toast_stack([(
        "Copied to clipboard",
        fenestra_kit::Status::Accent,
    )]))]);
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    let frame = build_frame(
        &view,
        &Theme::light(),
        &mut fonts,
        &mut state,
        (400.0, 200.0),
        1.0,
    );
    let toast = frame.get(&by::role(fenestra_core::Semantics::Alert));
    assert_eq!(toast.label.as_deref(), Some("Copied to clipboard"));
    assert!(toast.live, "toasts announce politely");
}

#[test]
fn inputs_expose_caret_and_selection() {
    let mut h = Harness::new(Status::default(), Theme::light(), (400, 200));
    h.tab();
    h.type_text("hello");
    // Collapsed selection = caret after the typed text.
    let node = h.get(&by::id("field"));
    assert_eq!(node.selection, Some((5, 5)), "caret sits at the end");

    // Select-all widens the exposed range to the whole value.
    let mut select_all = KeyInput::plain(Key::Char('a'));
    select_all.meta = true;
    h.key(select_all);
    let node = h.get(&by::id("field"));
    assert_eq!(node.selection, Some((0, 5)));

    // Home collapses it back to the start.
    h.key(KeyInput::plain(Key::Home));
    let node = h.get(&by::id("field"));
    assert_eq!(node.selection, Some((0, 0)));
}
