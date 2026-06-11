//! The accessibility projection: kit widgets expose roles, names, states,
//! and values through `Frame::access_tree`, headlessly.

use fenestra_core::{AccessNode, Element, Fonts, FrameState, Semantics, Theme, build_frame, col};
use fenestra_kit::{button, checkbox, slider, text_input};

fn find<'a>(node: &'a AccessNode, pred: &impl Fn(&AccessNode) -> bool) -> Option<&'a AccessNode> {
    if pred(node) {
        return Some(node);
    }
    node.children.iter().find_map(|c| find(c, pred))
}

#[test]
fn widgets_expose_semantics() {
    let view: Element<()> = col().p(16.0).items_start().gap(8.0).children([
        Element::from(button("Save")),
        Element::from(checkbox(true).label("Notify")),
        Element::from(slider(0.4)),
        Element::from(text_input("hello").placeholder("Search…")),
    ]);
    let theme = Theme::light();
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    let frame = build_frame(&view, &theme, &mut fonts, &mut state, (400.0, 240.0), 1.0);
    let tree = frame.access_tree();

    let save = find(&tree, &|n| {
        matches!(n.semantics, Some(Semantics::Button)) && n.label.as_deref() == Some("Save")
    })
    .expect("the button is exposed with its label");
    assert!(save.focusable, "buttons are focusable");
    assert!(save.rect.width() > 0.0, "exposed nodes carry layout rects");

    assert!(
        find(&tree, &|n| {
            matches!(n.semantics, Some(Semantics::Checkbox { checked: true }))
                && n.label.as_deref() == Some("Notify")
        })
        .is_some(),
        "the checkbox exposes its checked state and label"
    );

    let s = find(&tree, &|n| {
        matches!(n.semantics, Some(Semantics::Slider { .. }))
    })
    .expect("the slider is exposed");
    if let Some(Semantics::Slider { value, min, max }) = s.semantics {
        assert!((value - 0.4).abs() < 1e-4);
        assert_eq!((min, max), (0.0, 1.0));
    }

    let input = find(&tree, &|n| {
        matches!(n.semantics, Some(Semantics::TextInput { multiline: false }))
    })
    .expect("the input is exposed");
    assert_eq!(input.value.as_deref(), Some("hello"));

    assert!(
        find(&tree, &|n| {
            matches!(n.semantics, Some(Semantics::Label)) && n.label.as_deref() == Some("Save")
        })
        .is_some(),
        "text leaves project as labels with their content"
    );
}
