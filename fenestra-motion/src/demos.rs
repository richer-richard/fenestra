//! The shipped demos — each stresses a different claim of the crate, each
//! is golden-pinned and structurally verified (see `tests/demos.rs`), and
//! each follows the video-first layout doctrine: one message per frame,
//! generous safe areas, layout slots over scattered absolutes.
//!
//! Like `fenestra_kit::demo`, these live in the library so tests, examples,
//! and downstream users share one definition.

use fenestra_anim::{Frames, Track, ease_in_out, key};
use fenestra_charts::BarChartAxes;
use fenestra_core::{Theme, Weight, text};

use crate::clip::{Clip, Prop};
use crate::composition::Composition;
use crate::easing::EASE_CRISP;

/// The broadcast lower third (the data-form flagship): transparent
/// background rendered with real alpha, a plate, an accent bar sweeping
/// open, and staggered type. Loaded from the shipped
/// `examples/lower_third.ron` — the same document the CLI walkthroughs use.
///
/// # Panics
/// Never in a shipped build: a test compiles the embedded document on
/// every CI run.
#[must_use]
pub fn lower_third() -> Composition {
    Composition::from_ron(include_str!("../examples/lower_third.ron"))
        .expect("the shipped lower_third.ron always compiles (CI-guarded)")
}

/// The words of the stagger title, in display order.
const STAGGER_WORDS: [&str; 4] = ["Built", "to", "be", "verified."];
/// Frames between word entrances.
const STAGGER_STEP: u64 = 6;
/// One word's entrance length in frames.
const ENTER: u64 = 16;

/// Per-word entrance stagger: one clip per word, offsets measured by
/// probing a layout pass (no hand-tuned pixel constants), entrances offset
/// by `STAGGER_STEP` frames — the manual form of the pattern a text
/// animator would generalize.
#[must_use]
pub fn title_stagger() -> Composition {
    let theme = Theme::dark();
    let word_el = |word: &'static str| {
        text(word)
            .size_px(96.0)
            .weight(Weight::Semibold)
            .themed(|t, s| s.color(t.text))
    };

    // Probe pass: measure each word's width from a throwaway composition —
    // deterministic (embedded fonts), so the layout is data, not guesswork.
    let mut probe = Composition::new(1280, 720, 60).theme(theme.clone());
    for word in STAGGER_WORDS {
        probe = probe.clip(
            Clip::new(word, 0..1)
                .anchor(crate::clip::Anchor::TopLeft)
                .element(move || word_el(word)),
        );
    }
    let scene = probe.sample(Frames(0));
    let widths: Vec<f64> = STAGGER_WORDS
        .iter()
        .map(|w| {
            scene
                .resolve(w)
                .expect("probed")
                .bbox
                .expect("laid out")
                .width()
        })
        .collect();

    // Word slots: a centered line with a fixed word gap.
    const GAP: f64 = 28.0;
    let total: f64 = widths.iter().sum::<f64>() + GAP * (STAGGER_WORDS.len() - 1) as f64;
    let mut cursor = -total / 2.0;

    let mut comp = Composition::new(1280, 720, 60)
        .duration(Frames(150))
        .background(theme.bg)
        .theme(theme);
    for (i, word) in STAGGER_WORDS.iter().enumerate() {
        let offset = cursor + widths[i] / 2.0;
        cursor += widths[i] + GAP;
        let start = i as u64 * STAGGER_STEP;
        #[expect(
            clippy::cast_possible_truncation,
            reason = "word offsets are a few hundred px"
        )]
        let clip = Clip::new(*word, start..150)
            .element(move || word_el(word))
            .animate(Prop::TranslateX, Track::new([key(0, offset as f32)]))
            .animate(
                Prop::Opacity,
                Track::new([key(0, 0.0f32).ease(EASE_CRISP), key(ENTER, 1.0)]),
            )
            .animate(
                Prop::TranslateY,
                Track::new([key(0, 28.0f32).ease(EASE_CRISP), key(ENTER, 0.0)]),
            );
        comp = comp.clip(clip);
    }
    comp
}

/// The chart-race data: each series is itself a [`Track`] — the same pure
/// interpolator the animation system uses, sampled per frame to rebuild the
/// chart. Curves differ per series so the lead changes hands.
fn race_series() -> Vec<(&'static str, Track<f32>)> {
    vec![
        (
            "Rust",
            Track::new([
                key(0, 14.0f32).ease(crate::easing::EASE_EDITORIAL),
                key(130, 88.0),
            ]),
        ),
        ("Go", Track::new([key(0, 56.0f32), key(130, 61.0)])),
        (
            "TypeScript",
            Track::new([key(0, 72.0f32).ease(ease_in_out()), key(130, 38.0)]),
        ),
        ("Python", Track::new([key(0, 41.0f32), key(130, 74.0)])),
    ]
}

/// The bar-chart race: a [`Clip::dynamic`] rebuilding a
/// [`fenestra_charts::BarChartAxes`] every frame from rank-sorted,
/// track-interpolated data — charts are pure functions of `(data, theme)`,
/// so the race is exactly as deterministic as the timeline driving it.
#[must_use]
pub fn chart_race() -> Composition {
    let theme = Theme::dark();
    let fps = 30;
    let series = race_series();
    let comp = Composition::new(1280, 720, fps)
        .duration(Frames(150))
        .background(theme.bg)
        .theme(theme);
    comp.clip(Clip::dynamic("race", 0..150, move |frame| {
        let mut bars: Vec<(&str, f32)> = series
            .iter()
            .map(|(name, track)| (*name, track.sample(frame, fps)))
            .collect();
        bars.sort_by(|a, b| b.1.total_cmp(&a.1));
        fenestra_core::col().gap(24.0).items_center().children([
            text("Mindshare, 2020 → 2026")
                .size_px(40.0)
                .weight(Weight::Semibold)
                .themed(|t, s| s.color(t.text)),
            BarChartAxes::new(bars)
                .show_values()
                .w(960.0)
                .h(480.0)
                .build(),
        ])
    }))
}

/// The label currently leading [`chart_race`] at `frame` — the structural
/// probe the demo's test asserts rank swaps with.
#[must_use]
pub fn chart_race_leader(comp: &Composition, frame: Frames) -> &'static str {
    race_series()
        .iter()
        .map(|(name, track)| (*name, track.sample(frame, comp.fps())))
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(name, _)| name)
        .expect("the race has series")
}
