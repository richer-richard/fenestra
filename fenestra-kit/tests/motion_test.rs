//! 0.9 motion: spring physics (overshoot then settle, observed through
//! real layout), enter fade-ins, and color crossfades on retarget (the
//! machinery theme switching rides) — all on the deterministic clock.

use fenestra_core::{App, Element, Theme, Transition, by, col, div, text};
use fenestra_shell::Harness;

// ---------------------------------------------------------------- spring

struct Slide {
    right: bool,
}

#[derive(Clone)]
struct Toggle;

impl App for Slide {
    type Msg = Toggle;

    fn update(&mut self, Toggle: Toggle) {
        self.right = !self.right;
    }

    fn view(&self) -> Element<Toggle> {
        let x = if self.right { 200.0 } else { 0.0 };
        col().w(400.0).h(100.0).children([div()
            .absolute()
            .left(x)
            .top(20.0)
            .w(40.0)
            .h(40.0)
            .id("puck")
            .transition(Transition::spring().with_spring(380.0, 18.0))])
    }
}

#[test]
fn springs_overshoot_then_settle() {
    let mut h = Harness::new(Slide { right: false }, Theme::light(), (400, 100));
    // Springs are real animation: enable motion (the harness defaults
    // to reduced motion for determinism).
    h.set_reduced_motion(false);
    h.update(Toggle);

    let mut max_x = f64::MIN;
    let mut samples = Vec::new();
    for _ in 0..120 {
        h.pump(16.0);
        let x = h.get(&by::id("puck")).rect.x0;
        max_x = max_x.max(x);
        samples.push(x);
    }
    let last = *samples.last().expect("samples");
    assert!(
        max_x > 205.0,
        "an underdamped spring overshoots the 200 target (peak {max_x:.1})"
    );
    assert!(
        (last - 200.0).abs() < 0.5,
        "and settles on it (ended at {last:.1})"
    );
}

// ----------------------------------------------------------------- enter

struct List {
    rows: Vec<&'static str>,
}

#[derive(Clone)]
struct Add;

impl App for List {
    type Msg = Add;

    fn update(&mut self, Add: Add) {
        self.rows.push("fresh row");
    }

    fn view(&self) -> Element<Add> {
        col().p(10.0).gap(6.0).children(self.rows.iter().map(|r| {
            div()
                .w(160.0)
                .h(24.0)
                .id(r)
                .enter(Transition::all().duration_ms(200.0))
                .themed(|t: &Theme, s| s.bg(t.accent))
                .children([text(*r)])
        }))
    }
}

#[test]
fn new_rows_fade_in() {
    let mut h = Harness::new(
        List {
            rows: vec!["old row"],
        },
        Theme::light(),
        (300, 200),
    );
    h.set_reduced_motion(false);
    // Settle the existing row's own enter animation first.
    h.pump(400.0);
    let settled = h.render();

    h.update(Add);
    h.pump(20.0); // 10% in: the new row is mostly transparent
    let early = h.render();
    h.pump(400.0); // fully in
    let done = h.render();

    let row_pixel = |img: &image::RgbaImage| {
        // Center of the second row: y = 10 + 24 + 6 + 12 = 52, x = 90.
        *img.get_pixel(90, 52)
    };
    let early_px = row_pixel(&early);
    let done_px = row_pixel(&done);
    assert_ne!(early_px, done_px, "the entering row is mid-fade at 10%");
    // And the settled first row never flickered.
    assert_eq!(*settled.get_pixel(90, 22), *done.get_pixel(90, 22));
}

// ------------------------------------------------- crossfade (theme swap)

struct Mood {
    alert: bool,
}

#[derive(Clone)]
struct Flip;

impl App for Mood {
    type Msg = Flip;

    fn update(&mut self, Flip: Flip) {
        self.alert = !self.alert;
    }

    fn view(&self) -> Element<Flip> {
        let alert = self.alert;
        col().p(10.0).children([div()
            .w(80.0)
            .h(80.0)
            .id("panel")
            .transition(Transition::colors().duration_ms(300.0))
            .themed(move |t: &Theme, s| {
                if alert {
                    s.bg(t.danger.solid)
                } else {
                    s.bg(t.accent)
                }
            })])
    }
}

#[test]
fn color_retargets_crossfade() {
    let mut h = Harness::new(Mood { alert: false }, Theme::light(), (200, 200));
    h.set_reduced_motion(false);
    h.pump(400.0);
    let before = *h.render().get_pixel(50, 50);

    h.update(Flip);
    h.pump(150.0); // halfway through the crossfade
    let mid = *h.render().get_pixel(50, 50);
    h.pump(600.0);
    let after = *h.render().get_pixel(50, 50);

    assert_ne!(before, after, "the two states differ");
    assert_ne!(mid, before, "midway is no longer the old color");
    assert_ne!(mid, after, "…and not yet the new one (a real crossfade)");
}
