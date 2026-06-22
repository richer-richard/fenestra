//! The 0.32 vocabulary widgets, driven semantically: segmented-control
//! selection + ARIA payload, kbd glyph mapping + accessible chord name, status
//! labels, and skeleton totality over edge inputs.

use std::path::PathBuf;

use fenestra_core::{App, Element, Key, KeyInput, Semantics, Theme, by, col, div, row};
use fenestra_kit::{
    Status, checkbox, kbd, kbd_raised, radio_group, segmented, skeleton, skeleton_circle,
    skeleton_text, status, tabs, wavy_progress,
};
use fenestra_shell::{Harness, render_element, testing::assert_png_snapshot};

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

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

// ---------------------------------------------------------------- tabs

#[derive(Default)]
struct Tabs {
    active: usize,
}

#[derive(Clone)]
enum TabMsg {
    Go(usize),
}

impl App for Tabs {
    type Msg = TabMsg;
    fn update(&mut self, msg: TabMsg) {
        let TabMsg::Go(i) = msg;
        self.active = i;
    }
    fn view(&self) -> Element<TabMsg> {
        col().p(8.0).children([tabs(
            self.active,
            ["Overview", "Activity", "Settings"],
            TabMsg::Go,
        )])
    }
}

#[test]
fn tabs_arrow_keys_move_the_active_tab() {
    // A tab strip is one tab stop; ←/→ move + activate (automatic activation),
    // Home/End jump to the ends.
    let mut h = Harness::new(Tabs::default(), Theme::light(), (480, 120));
    h.tab();
    h.key(KeyInput::plain(Key::ArrowRight));
    assert_eq!(h.app().active, 1);
    h.key(KeyInput::plain(Key::End));
    assert_eq!(h.app().active, 2);
    h.key(KeyInput::plain(Key::ArrowRight));
    assert_eq!(h.app().active, 2, "clamps at the last tab");
    h.key(KeyInput::plain(Key::Home));
    assert_eq!(h.app().active, 0);
}

// ---------------------------------------------------------------- radio group

#[derive(Default)]
struct RadioG {
    selected: usize,
}

#[derive(Clone)]
enum RgMsg {
    Pick(usize),
}

impl App for RadioG {
    type Msg = RgMsg;
    fn update(&mut self, msg: RgMsg) {
        let RgMsg::Pick(i) = msg;
        self.selected = i;
    }
    fn view(&self) -> Element<RgMsg> {
        col().p(8.0).children([radio_group(
            self.selected,
            ["Monthly", "Quarterly", "Annual"],
            RgMsg::Pick,
        )])
    }
}

#[test]
fn radio_group_arrows_move_and_wrap_the_selection() {
    // WAI-ARIA radio group: arrows move AND select, and the ends wrap.
    let mut h = Harness::new(RadioG::default(), Theme::light(), (300, 160));
    h.tab();
    h.key(KeyInput::plain(Key::ArrowDown));
    assert_eq!(h.app().selected, 1);
    h.key(KeyInput::plain(Key::ArrowUp));
    assert_eq!(h.app().selected, 0);
    h.key(KeyInput::plain(Key::ArrowUp));
    assert_eq!(
        h.app().selected,
        2,
        "ArrowUp wraps from the first to the last"
    );
    h.key(KeyInput::plain(Key::ArrowDown));
    assert_eq!(
        h.app().selected,
        0,
        "ArrowDown wraps from the last to the first"
    );
}

#[test]
fn segmented_arrow_keys_move_the_active_segment() {
    // The control is one tab stop; arrows roam the selection within it
    // (WAI-ARIA tablist keyboard model), Home/End jump to the ends.
    let mut h = Harness::new(Seg::default(), Theme::light(), (420, 120));
    h.tab(); // focus the segmented control
    h.key(KeyInput::plain(Key::ArrowRight));
    assert_eq!(h.app().active, 1, "ArrowRight advances the selection");
    h.key(KeyInput::plain(Key::ArrowRight));
    assert_eq!(h.app().active, 2);
    h.key(KeyInput::plain(Key::ArrowRight));
    assert_eq!(h.app().active, 2, "selection clamps at the last segment");
    h.key(KeyInput::plain(Key::ArrowLeft));
    assert_eq!(h.app().active, 1, "ArrowLeft retreats the selection");
    h.key(KeyInput::plain(Key::Home));
    assert_eq!(h.app().active, 0, "Home jumps to the first segment");
    h.key(KeyInput::plain(Key::End));
    assert_eq!(h.app().active, 2, "End jumps to the last segment");
}

// ---------------------------------------------------------------- checkbox

#[test]
fn checkbox_indeterminate_projects_mixed() {
    let h = show(|| {
        checkbox(false)
            .indeterminate(true)
            .label("Select all")
            .into()
    });
    let node = h
        .query(
            &by::role(Semantics::Checkbox {
                checked: false,
                mixed: false,
            })
            .name("Select all"),
        )
        .expect("indeterminate checkbox present");
    assert_eq!(
        node.semantics,
        Some(Semantics::Checkbox {
            checked: false,
            mixed: true
        })
    );
}

#[test]
fn checkbox_states_golden() {
    // Off / on / indeterminate (the dash) — the new tri-state visual.
    let theme = Theme::light();
    let scene = col::<()>()
        .p(8.0)
        .gap(8.0)
        .items_start()
        .bg(theme.bg)
        .children((
            checkbox(false).label("Off"),
            checkbox(true).label("On"),
            checkbox(false).indeterminate(true).label("Mixed"),
        ));
    let image = render_element(scene, &theme, (160, 110));
    assert_png_snapshot(snapshot_dir(), "checkbox_states", &image);
}

// ---------------------------------------------------------------- per-corner radius

#[test]
fn per_corner_radius_golden() {
    // Top-only rounding (tabs/sheets), and an explicit four-corner set.
    let theme = Theme::light();
    let scene = col::<()>().p(8.0).gap(8.0).bg(theme.bg).children((
        div::<()>()
            .w(88.0)
            .h(36.0)
            .rounded_t(16.0)
            .themed(|t: &Theme, s| s.bg(t.accent)),
        div::<()>()
            .w(88.0)
            .h(36.0)
            .corners(2.0, 16.0, 2.0, 16.0)
            .themed(|t: &Theme, s| s.bg(t.accent)),
    ));
    let image = render_element(scene, &theme, (120, 104));
    assert_png_snapshot(snapshot_dir(), "per_corner_radius", &image);
}

// ---------------------------------------------------------------- transforms

#[test]
fn transforms_golden() {
    // rotate / translate / skew as paint-time transforms (no layout shift).
    let theme = Theme::light();
    let scene = row::<()>()
        .p(14.0)
        .gap(18.0)
        .items_center()
        .bg(theme.bg)
        .children((
            div::<()>()
                .w(44.0)
                .h(44.0)
                .rounded(8.0)
                .rotate(30.0)
                .themed(|t: &Theme, s| s.bg(t.accent)),
            div::<()>()
                .w(44.0)
                .h(44.0)
                .rounded(8.0)
                .translate(0.0, 10.0)
                .themed(|t: &Theme, s| s.bg(t.accent)),
            div::<()>()
                .w(44.0)
                .h(44.0)
                .rounded(8.0)
                .skew(18.0, 0.0)
                .themed(|t: &Theme, s| s.bg(t.accent)),
        ));
    let image = render_element(scene, &theme, (210, 78));
    assert_png_snapshot(snapshot_dir(), "transforms", &image);
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
fn kbd_raised_exposes_the_same_chord_label() {
    let h = show(|| kbd_raised(["esc"]));
    assert!(h.query(&by::role(Semantics::Image).name("Esc")).is_some());
}

#[derive(Default)]
struct SegDisabled {
    picks: usize,
}

impl App for SegDisabled {
    type Msg = ();
    fn update(&mut self, (): ()) {
        self.picks += 1;
    }
    fn view(&self) -> Element<()> {
        col()
            .p(8.0)
            .children([segmented(0, ["On", "Off"], |_| ()).disabled(true)])
    }
}

#[test]
fn disabled_segmented_is_present_but_not_clickable() {
    let mut h = Harness::new(SegDisabled::default(), Theme::light(), (300, 100));
    // The segment still exposes Tab semantics for assistive tech...
    assert!(
        h.query(&by::role(Semantics::Tab { selected: false }).name("Off"))
            .is_some()
    );
    // ...but a disabled control carries no click handler, so nothing fires.
    h.click(&by::role(Semantics::Tab { selected: false }).name("Off"));
    assert_eq!(h.app().picks, 0);
}

#[test]
fn wavy_progress_clamps_and_is_total() {
    // Fractions clamp to 0..=1 and a zero width is floored, so a hostile value
    // never panics the sine-path build.
    let builds: [fn() -> Element<()>; 5] = [
        || wavy_progress(0.0, 240.0).into(),
        || wavy_progress(1.0, 240.0).into(),
        || wavy_progress(-1.0, 240.0).into(),
        || wavy_progress(2.0, 240.0).into(),
        || wavy_progress(0.5, 0.0).into(),
    ];
    for build in builds {
        let mut h = show(build);
        let _ = h.render();
    }
}
