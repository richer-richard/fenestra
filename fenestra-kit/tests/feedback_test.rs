//! The 0.32 vocabulary widgets, driven semantically: segmented-control
//! selection + ARIA payload, kbd glyph mapping + accessible chord name, status
//! labels, and skeleton totality over edge inputs.

use fenestra_core::{App, Element, Semantics, Theme, by, col};
use fenestra_kit::{
    Status, kbd, segmented, skeleton, skeleton_circle, skeleton_text, status, wavy_progress,
};
use fenestra_shell::Harness;

/// A harness over a single static (inert) element.
struct Static(fn() -> Element<()>);

impl App for Static {
    type Msg = ();
    fn update(&mut self, (): ()) {}
    fn view(&self) -> Element<()> {
        col().p(8.0).children([(self.0)()])
    }
}

fn show(build: fn() -> Element<()>) -> Harness<Static> {
    Harness::new(Static(build), Theme::light(), (320, 160))
}

// ---------------------------------------------------------------- segmented

#[derive(Default)]
struct Seg {
    active: usize,
}

#[derive(Clone)]
enum SegMsg {
    Pick(usize),
}

impl App for Seg {
    type Msg = SegMsg;
    fn update(&mut self, msg: SegMsg) {
        let SegMsg::Pick(i) = msg;
        self.active = i;
    }
    fn view(&self) -> Element<SegMsg> {
        col().p(8.0).children([segmented(
            self.active,
            ["List", "Board", "Calendar"],
            SegMsg::Pick,
        )])
    }
}

#[test]
fn segmented_selects_by_click_and_marks_active_segment() {
    let mut h = Harness::new(Seg::default(), Theme::light(), (420, 120));
    // "List" (index 0) starts selected — its semantics payload says so.
    let list = h
        .query(&by::role(Semantics::Tab { selected: false }).name("List"))
        .expect("List segment present");
    assert_eq!(list.semantics, Some(Semantics::Tab { selected: true }));

    // Clicking "Board" emits on_select(1); the host echoes it and Board
    // becomes the selected segment.
    h.click(&by::role(Semantics::Tab { selected: false }).name("Board"));
    assert_eq!(h.app().active, 1);
    let board = h
        .query(&by::role(Semantics::Tab { selected: false }).name("Board"))
        .expect("Board segment present");
    assert_eq!(board.semantics, Some(Semantics::Tab { selected: true }));
}

// ---------------------------------------------------------------- kbd

#[test]
fn kbd_maps_modifiers_to_glyphs_and_names_the_chord() {
    // The chord is one accessible unit named with the ⌘ glyph (queried by role
    // + name, since its glyph text projects as a child Label — the same shape
    // a `button` has).
    let h = show(|| kbd(["cmd", "K"]));
    assert!(h.query(&by::role(Semantics::Image).name("⌘ K")).is_some());
}

#[test]
fn kbd_keeps_obscure_keys_readable() {
    // Esc renders as a short word (not the obscure ⎋ glyph); Enter keeps ↵.
    let h = show(|| kbd(["shift", "esc"]));
    assert!(h.query(&by::role(Semantics::Image).name("⇧ Esc")).is_some());
    let h2 = show(|| kbd(["enter"]));
    assert!(h2.query(&by::role(Semantics::Image).name("↵")).is_some());
}

// ---------------------------------------------------------------- status

#[test]
fn status_indicator_exposes_its_label() {
    let h = show(|| status("Operational", Status::Success).into());
    assert!(h.query(&by::label("Operational")).is_some());
}

#[test]
fn live_status_renders_without_panicking() {
    // The pulsing ring must not break headless rendering (reduced motion pins
    // the keyframe), and the label still reads.
    let mut h = show(|| status("Deploying", Status::Accent).live(true).into());
    let _ = h.render();
    assert!(h.query(&by::label("Deploying")).is_some());
}

// ---------------------------------------------------------------- skeleton

#[test]
fn skeletons_are_total_over_edge_inputs() {
    // Zero lines clamps to one; large counts and zero sizes never panic.
    let builds: [fn() -> Element<()>; 5] = [
        || skeleton_text(0),
        || skeleton_text(40),
        || skeleton(0.0, 0.0),
        || skeleton_circle(0.0),
        || skeleton_circle(64.0),
    ];
    for build in builds {
        let mut h = show(build);
        let _ = h.render();
    }
}

#[test]
fn wavy_progress_clamps_and_is_total() {
    // Fractions clamp to 0..=1 and a zero width is floored, so a hostile value
    // never panics the sine-path build.
    let builds: [fn() -> Element<()>; 5] = [
        || wavy_progress(0.0, 240.0),
        || wavy_progress(1.0, 240.0),
        || wavy_progress(-1.0, 240.0),
        || wavy_progress(2.0, 240.0),
        || wavy_progress(0.5, 0.0),
    ];
    for build in builds {
        let mut h = show(build);
        let _ = h.render();
    }
}
