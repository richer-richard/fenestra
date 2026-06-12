//! Performance regression gates: generous absolute ceilings that catch
//! catastrophic regressions (10x), not noise. `#[ignore]`d locally —
//! CI runs them with `--ignored` on the macOS runner. Real numbers
//! live in BENCHMARKS.md; these ceilings are ~20x those numbers so
//! shared-runner variance never flakes.

use std::time::Instant;

use fenestra_core::{Element, Fonts, FrameState, Theme, build_frame, col, div, row, text};

fn median_build_ms(view: impl Fn() -> Element<()>, size: (f32, f32), runs: usize) -> f64 {
    let mut fonts = Fonts::embedded();
    let theme = Theme::light();
    let mut times: Vec<f64> = (0..runs)
        .map(|_| {
            let mut state = FrameState::new();
            let tree = view();
            let start = Instant::now();
            let frame = build_frame(&tree, &theme, &mut fonts, &mut state, size, 1.0);
            let _scene = frame.paint(&mut fonts, &mut state);
            start.elapsed().as_secs_f64() * 1000.0
        })
        .collect();
    times.sort_by(f64::total_cmp);
    times[times.len() / 2]
}

#[test]
#[ignore = "perf gate: run with --ignored in CI"]
fn counter_scale_stays_under_ceiling() {
    let ms = median_build_ms(
        || {
            col().p(24.0).gap(16.0).children([
                text("42"),
                row().gap(8.0).children([
                    div().w(120.0).h(36.0).focusable(true),
                    div().w(120.0).h(36.0).focusable(true),
                ]),
            ])
        },
        (480.0, 320.0),
        20,
    );
    assert!(ms < 5.0, "counter-scale frame took {ms:.3} ms (ceiling 5)");
}

#[test]
#[ignore = "perf gate: run with --ignored in CI"]
fn dashboard_scale_stays_under_ceiling() {
    let ms = median_build_ms(
        || {
            col().p(16.0).gap(8.0).children(
                (0..120)
                    .map(|i| {
                        row().gap(8.0).h(28.0).shrink0().children([
                            text(format!("row {i}")),
                            div().w(80.0).h(20.0),
                            text("status"),
                        ])
                    })
                    .collect::<Vec<_>>(),
            )
        },
        (900.0, 700.0),
        10,
    );
    assert!(
        ms < 12.0,
        "dashboard-scale frame took {ms:.3} ms (ceiling 12)"
    );
}

#[test]
#[ignore = "perf gate: run with --ignored in CI"]
fn virtual_100k_stays_under_ceiling() {
    let ms = median_build_ms(
        || {
            col().h(600.0).children([col()
                .h(600.0)
                .scroll_y()
                .id("v")
                .children([div().virtual_rows(100_000, 24.0, |i| {
                    row().h(24.0).children([text(format!("row {i}"))])
                })])])
        },
        (800.0, 600.0),
        10,
    );
    assert!(ms < 4.0, "virtual 100k frame took {ms:.3} ms (ceiling 4)");
}
