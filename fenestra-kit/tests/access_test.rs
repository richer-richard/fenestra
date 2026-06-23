//! The accessibility projection: kit widgets expose roles, names, states,
//! and values through `Frame::access_tree`, headlessly.

use fenestra_core::{AccessNode, Element, Fonts, FrameState, Semantics, Theme, build_frame, col};
use fenestra_kit::{
    button, checkbox, meter, multi_select, progress, progress_indeterminate, slider, spin_button,
    text_input,
};

fn find<'a>(node: &'a AccessNode, pred: &impl Fn(&AccessNode) -> bool) -> Option<&'a AccessNode> {
    if pred(node) {
        return Some(node);
    }
    node.children.iter().find_map(|c| find(c, pred))
}

#[test]
fn value_widgets_expose_roles_values_and_bounds() {
    let view: Element<()> = col().p(16.0).items_start().gap(8.0).children([
        Element::from(meter(75.0, 0.0, 100.0).label("Disk")),
        Element::from(spin_button("3").range(3.0, 0.0, 10.0).label("Quantity")),
        progress(0.6),
        progress_indeterminate(),
    ]);
    let theme = Theme::light();
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    let frame = build_frame(&view, &theme, &mut fonts, &mut state, (400.0, 260.0), 1.0);
    let tree = frame.access_tree();

    let m = find(&tree, &|n| {
        matches!(n.semantics, Some(Semantics::Meter { .. }))
    })
    .expect("the meter is exposed");
    assert!(matches!(
        m.semantics,
        Some(Semantics::Meter {
            value,
            min,
            max,
        }) if (value - 75.0).abs() < 1e-4 && min == 0.0 && max == 100.0
    ));
    assert_eq!(m.value.as_deref(), Some("75%"));
    assert_eq!(m.label.as_deref(), Some("Disk"));

    let sb = find(&tree, &|n| {
        matches!(n.semantics, Some(Semantics::Spinbutton { .. }))
    })
    .expect("the spin button exposes a spinbutton role");
    assert!(matches!(
        sb.semantics,
        Some(Semantics::Spinbutton { value, min, max }) if (value - 3.0).abs() < 1e-4 && min == 0.0 && max == 10.0
    ));
    assert!(sb.focusable, "a spinbutton is keyboard focusable");
    assert_eq!(sb.label.as_deref(), Some("Quantity"));

    let p = find(&tree, &|n| {
        matches!(n.semantics, Some(Semantics::ProgressBar { value: Some(_) }))
    })
    .expect("the determinate progress bar is exposed");
    assert!(matches!(
        p.semantics,
        Some(Semantics::ProgressBar { value: Some(v) }) if (v - 0.6).abs() < 1e-4
    ));
    assert_eq!(p.value.as_deref(), Some("60%"));

    assert!(
        find(&tree, &|n| matches!(
            n.semantics,
            Some(Semantics::ProgressBar { value: None })
        ))
        .is_some(),
        "the indeterminate progress bar exposes a value-less progressbar role"
    );
}

#[test]
fn multi_select_chips_are_checkboxes_with_state() {
    let view: Element<()> =
        col().children([Element::from(multi_select([0, 2], ["Rust", "Go", "Zig"]))]);
    let theme = Theme::light();
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    let frame = build_frame(&view, &theme, &mut fonts, &mut state, (400.0, 120.0), 1.0);
    let tree = frame.access_tree();

    // The selected "Rust" chip is a checked, focusable checkbox.
    let rust = find(&tree, &|n| {
        matches!(n.semantics, Some(Semantics::Checkbox { checked: true, .. }))
            && n.label.as_deref() == Some("Rust")
    })
    .expect("a checked chip for Rust");
    assert!(rust.focusable, "chips are keyboard focusable");
    // The unselected "Go" chip is an unchecked checkbox.
    assert!(
        find(&tree, &|n| {
            matches!(
                n.semantics,
                Some(Semantics::Checkbox { checked: false, .. })
            ) && n.label.as_deref() == Some("Go")
        })
        .is_some(),
        "an unchecked chip for Go"
    );
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
            matches!(n.semantics, Some(Semantics::Checkbox { checked: true, .. }))
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
