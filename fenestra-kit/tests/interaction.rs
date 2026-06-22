//! M4 acceptance: headless pointer interaction emits messages and the
//! hover/active/focus states match their goldens; the switch animates.

use std::path::PathBuf;

use fenestra_core::{App, Element, Key, KeyInput, SP3, SP4, TextSize, Theme, col, row, text};
use fenestra_kit::{
    ButtonVariant, ControlSize, button, checkbox, radio, range_slider, slider, switch,
};
use fenestra_shell::{SyntheticEvent, render_app, testing::assert_png_snapshot};

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

// ---------------------------------------------------------------- counter

struct Counter {
    n: i64,
}

#[derive(Clone)]
enum CounterMsg {
    Inc,
}

impl App for Counter {
    type Msg = CounterMsg;

    fn update(&mut self, msg: CounterMsg) {
        match msg {
            CounterMsg::Inc => self.n += 1,
        }
    }

    fn view(&self) -> Element<CounterMsg> {
        // p(16) + Md button (36 tall): the button rect is x 16.., y 16..52.
        col()
            .p(SP4)
            .items_start()
            .gap(SP3)
            .children([button("Increment").on_click(CounterMsg::Inc)])
            .children([text(format!("count: {}", self.n)).size(TextSize::Sm)])
    }
}

/// Pointer onto the button, press, release: the message fires exactly once
/// and only when the release happens over the element.
#[test]
fn click_emits_message() {
    let theme = Theme::light();
    let mut app = Counter { n: 0 };
    render_app(
        &mut app,
        &[
            SyntheticEvent::MouseMove { x: 50.0, y: 34.0 },
            SyntheticEvent::MouseDown,
            SyntheticEvent::MouseUp,
        ],
        (260, 110),
        &theme,
    );
    assert_eq!(app.n, 1, "click = press + release on the same element");

    // Press on the button, drag off, release: no click.
    let mut app = Counter { n: 0 };
    render_app(
        &mut app,
        &[
            SyntheticEvent::MouseMove { x: 50.0, y: 34.0 },
            SyntheticEvent::MouseDown,
            SyntheticEvent::MouseMove { x: 200.0, y: 100.0 },
            SyntheticEvent::MouseUp,
        ],
        (260, 110),
        &theme,
    );
    assert_eq!(app.n, 0, "release off the element must not click");
}

/// Enter and Space activate the focused button; Tab moves focus.
#[test]
fn keyboard_activates_focused_button() {
    let theme = Theme::light();
    let mut app = Counter { n: 0 };
    render_app(
        &mut app,
        &[
            SyntheticEvent::Tab,
            SyntheticEvent::Key(KeyInput::plain(Key::Enter)),
            SyntheticEvent::Key(KeyInput::plain(Key::Space)),
        ],
        (260, 110),
        &theme,
    );
    assert_eq!(app.n, 2, "Enter and Space each activate the focused button");
}

#[test]
fn button_hover_golden() {
    let theme = Theme::light();
    let mut app = Counter { n: 0 };
    let image = render_app(
        &mut app,
        &[SyntheticEvent::MouseMove { x: 50.0, y: 34.0 }],
        (260, 110),
        &theme,
    );
    assert_png_snapshot(snapshot_dir(), "button_hover", &image);
}

#[test]
fn button_active_golden() {
    let theme = Theme::light();
    let mut app = Counter { n: 0 };
    let image = render_app(
        &mut app,
        &[
            SyntheticEvent::MouseMove { x: 50.0, y: 34.0 },
            SyntheticEvent::MouseDown,
        ],
        (260, 110),
        &theme,
    );
    assert_png_snapshot(snapshot_dir(), "button_active", &image);
}

#[test]
fn button_focus_ring_golden() {
    let theme = Theme::light();
    let mut app = Counter { n: 0 };
    let image = render_app(&mut app, &[SyntheticEvent::Tab], (260, 110), &theme);
    assert_png_snapshot(snapshot_dir(), "button_focus_ring", &image);
}

// ---------------------------------------------------------------- slider

struct Volume {
    value: f32,
}

#[derive(Clone)]
enum VolumeMsg {
    Set(f32),
}

impl App for Volume {
    type Msg = VolumeMsg;

    fn update(&mut self, msg: VolumeMsg) {
        match msg {
            VolumeMsg::Set(v) => self.value = v,
        }
    }

    fn view(&self) -> Element<VolumeMsg> {
        col()
            .p(SP4)
            .items_start()
            .children([slider(self.value).step(0.05).on_change(VolumeMsg::Set)])
    }
}

struct Temp {
    value: f32,
}

#[derive(Clone)]
enum TempMsg {
    Set(f32),
}

impl App for Temp {
    type Msg = TempMsg;
    fn update(&mut self, msg: TempMsg) {
        let TempMsg::Set(v) = msg;
        self.value = v;
    }
    fn view(&self) -> Element<TempMsg> {
        col().p(SP4).items_start().children([slider(self.value)
            .range(0.0, 100.0)
            .step(10.0)
            .on_change(TempMsg::Set)])
    }
}

struct PriceRange {
    low: f32,
    high: f32,
}

#[derive(Clone)]
enum RangeMsg {
    Set(f32, f32),
}

impl App for PriceRange {
    type Msg = RangeMsg;
    fn update(&mut self, msg: RangeMsg) {
        let RangeMsg::Set(l, h) = msg;
        self.low = l;
        self.high = h;
    }
    fn view(&self) -> Element<RangeMsg> {
        col()
            .p(SP4)
            .items_start()
            .children([range_slider(self.low, self.high)
                .range(0.0, 100.0)
                .step(10.0)
                .on_change(RangeMsg::Set)])
    }
}

#[test]
fn slider_custom_domain_steps_in_real_units() {
    let theme = Theme::light();
    let mut app = Temp { value: 50.0 };
    render_app(
        &mut app,
        &[
            SyntheticEvent::Tab,
            SyntheticEvent::Key(KeyInput::plain(Key::ArrowRight)),
        ],
        (260, 80),
        &theme,
    );
    assert!(
        (app.value - 60.0).abs() < 1e-3,
        "ArrowRight steps by 10 in a 0..100 domain, got {}",
        app.value
    );
    let mut app = Temp { value: 50.0 };
    render_app(
        &mut app,
        &[
            SyntheticEvent::Tab,
            SyntheticEvent::Key(KeyInput::plain(Key::End)),
        ],
        (260, 80),
        &theme,
    );
    assert!(
        (app.value - 100.0).abs() < 1e-3,
        "End snaps to max, got {}",
        app.value
    );
}

#[test]
fn range_slider_thumbs_move_independently_and_cannot_cross() {
    let theme = Theme::light();
    // First tab stop = low thumb; ArrowRight raises low by one step.
    let mut app = PriceRange {
        low: 20.0,
        high: 80.0,
    };
    render_app(
        &mut app,
        &[
            SyntheticEvent::Tab,
            SyntheticEvent::Key(KeyInput::plain(Key::ArrowRight)),
        ],
        (260, 80),
        &theme,
    );
    assert!(
        (app.low - 30.0).abs() < 1e-3,
        "low thumb +10 -> 30, got {}",
        app.low
    );
    assert!(
        (app.high - 80.0).abs() < 1e-3,
        "high unchanged, got {}",
        app.high
    );

    // The low thumb cannot pass the high thumb: End on low clamps to high.
    let mut app = PriceRange {
        low: 20.0,
        high: 50.0,
    };
    render_app(
        &mut app,
        &[
            SyntheticEvent::Tab,
            SyntheticEvent::Key(KeyInput::plain(Key::End)),
        ],
        (260, 80),
        &theme,
    );
    assert!(
        (app.low - 50.0).abs() < 1e-3,
        "low clamps to high (50), got {}",
        app.low
    );

    // Second tab stop = high thumb; ArrowLeft lowers high by one step.
    let mut app = PriceRange {
        low: 20.0,
        high: 80.0,
    };
    render_app(
        &mut app,
        &[
            SyntheticEvent::Tab,
            SyntheticEvent::Tab,
            SyntheticEvent::Key(KeyInput::plain(Key::ArrowLeft)),
        ],
        (260, 80),
        &theme,
    );
    assert!(
        (app.high - 70.0).abs() < 1e-3,
        "high thumb -10 -> 70, got {}",
        app.high
    );
}

#[test]
fn slider_variants_golden() {
    // The new visuals: step marks under a single slider, and a two-thumb range
    // with the fill spanning between the thumbs.
    let theme = Theme::light();
    let scene = col::<()>()
        .p(SP4)
        .gap(SP4)
        .items_start()
        .bg(theme.bg)
        .children((
            slider(0.4).marks(true),
            range_slider(20.0, 70.0)
                .range(0.0, 100.0)
                .step(10.0)
                .marks(true),
        ));
    let image = fenestra_shell::render_element(scene, &theme, (240, 96));
    assert_png_snapshot(snapshot_dir(), "slider_variants", &image);
}

/// Arrow keys step a focused slider; dragging maps the pointer to a value.
#[test]
fn slider_keys_and_drag() {
    let theme = Theme::light();
    let mut app = Volume { value: 0.4 };
    render_app(
        &mut app,
        &[
            SyntheticEvent::Tab,
            SyntheticEvent::Key(KeyInput::plain(Key::ArrowRight)),
            SyntheticEvent::Key(KeyInput::plain(Key::ArrowRight)),
            SyntheticEvent::Key(KeyInput::plain(Key::ArrowLeft)),
        ],
        (260, 80),
        &theme,
    );
    assert!(
        (app.value - 0.45).abs() < 1e-4,
        "two steps up, one down from 0.4 should be 0.45, got {}",
        app.value
    );

    // Drag: slider spans x 16..216 (200 wide); pointer at the far right.
    let mut app = Volume { value: 0.0 };
    render_app(
        &mut app,
        &[
            SyntheticEvent::MouseMove { x: 210.0, y: 26.0 },
            SyntheticEvent::MouseDown,
            SyntheticEvent::MouseUp,
        ],
        (260, 80),
        &theme,
    );
    assert!(
        app.value > 0.9,
        "press near the right end should set a high value, got {}",
        app.value
    );
}

#[test]
fn slider_page_keys_jump_a_larger_step() {
    let theme = Theme::light();
    // PageUp jumps 10 steps: 0.4 + 10*0.05 = 0.9.
    let mut app = Volume { value: 0.4 };
    render_app(
        &mut app,
        &[
            SyntheticEvent::Tab,
            SyntheticEvent::Key(KeyInput::plain(Key::PageUp)),
        ],
        (260, 80),
        &theme,
    );
    assert!(
        (app.value - 0.9).abs() < 1e-4,
        "PageUp jumps 10 steps to 0.9, got {}",
        app.value
    );

    // PageDown jumps 10 steps and clamps at 0.0.
    let mut app = Volume { value: 0.4 };
    render_app(
        &mut app,
        &[
            SyntheticEvent::Tab,
            SyntheticEvent::Key(KeyInput::plain(Key::PageDown)),
        ],
        (260, 80),
        &theme,
    );
    assert!(
        app.value.abs() < 1e-4,
        "PageDown clamps at 0.0, got {}",
        app.value
    );
}

// ------------------------------------------------------------- controls

/// All M4 widgets in both states, both themes: the visual corpus.
fn controls<Msg: Clone + 'static>(theme: &Theme) -> Element<Msg> {
    col().p(SP4).gap(SP3).items_start().bg(theme.bg).children([
        row().gap(SP3).children([
            button("Primary"),
            button("Secondary").variant(ButtonVariant::Secondary),
            button("Ghost").variant(ButtonVariant::Ghost),
            button("Danger").variant(ButtonVariant::Danger),
            button("Disabled").disabled(true),
        ]),
        row().gap(SP3).items_center().children([
            button("Small").size(ControlSize::Sm),
            button("Medium").size(ControlSize::Md),
            button("Large").size(ControlSize::Lg),
        ]),
        row().gap(SP3).items_center().children([
            checkbox(false).label("Unchecked"),
            checkbox(true).label("Checked"),
            checkbox(true).label("Disabled").disabled(true),
        ]),
        row().gap(SP3).items_center().children([
            switch(false).label("Off"),
            switch(true).label("On"),
            switch(true).label("Disabled").disabled(true),
        ]),
        row()
            .gap(SP3)
            .items_center()
            .children([radio(false).label("Other"), radio(true).label("Selected")]),
        row().gap(SP3).items_center().children([
            slider(0.0),
            slider(0.62),
            slider(1.0).disabled(true),
        ]),
    ])
}

#[test]
fn controls_light_golden() {
    let theme = Theme::light();
    let image = fenestra_shell::render_element(controls::<()>(&theme), &theme, (760, 320));
    assert_png_snapshot(snapshot_dir(), "controls_light", &image);
}

#[test]
fn controls_dark_golden() {
    let theme = Theme::dark();
    let image = fenestra_shell::render_element(controls::<()>(&theme), &theme, (760, 320));
    assert_png_snapshot(snapshot_dir(), "controls_dark", &image);
}
