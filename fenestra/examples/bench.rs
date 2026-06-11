//! Honest numbers: measures the CPU side of the pipeline (view build +
//! style resolution + taffy layout + vello scene build) and the full
//! headless pipeline including the GPU render and readback. Run with
//! `cargo run --release --example bench`; BENCHMARKS.md records results.

use std::time::Instant;

use fenestra::prelude::*;
use fenestra::shell::render_element;
use fenestra_core::{build_frame, row as core_row};

fn time_ms(iterations: u32, mut f: impl FnMut()) -> (f64, f64) {
    // Warmup.
    for _ in 0..3 {
        f();
    }
    let mut best = f64::MAX;
    let start = Instant::now();
    for _ in 0..iterations {
        let t = Instant::now();
        f();
        best = best.min(t.elapsed().as_secs_f64() * 1000.0);
    }
    let mean = start.elapsed().as_secs_f64() * 1000.0 / f64::from(iterations);
    (mean, best)
}

fn cpu_frame(name: &str, size: (f32, f32), view: impl Fn() -> Element<()>) {
    let theme = Theme::light();
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    state.reduced_motion = true;
    let (mean, best) = time_ms(100, || {
        let el = view();
        let frame = build_frame(&el, &theme, &mut fonts, &mut state, size, 1.0);
        let scene = frame.paint(&mut fonts, &mut state);
        std::hint::black_box(scene);
    });
    println!(
        "{name:<34} {mean:>7.3} ms mean   {best:>7.3} ms best   (build+layout+paint, 100 iters)"
    );
}

fn gpu_frame(name: &str, size: (u32, u32), view: impl Fn() -> Element<()>) {
    let theme = Theme::light();
    let (mean, best) = time_ms(20, || {
        let image = render_element(view(), &theme, size);
        std::hint::black_box(image);
    });
    println!(
        "{name:<34} {mean:>7.3} ms mean   {best:>7.3} ms best   (full pipeline + readback, 20 iters)"
    );
}

fn small_view() -> Element<()> {
    col().p(SP6).gap(SP4).items_center().children([
        text("42").size(TextSize::Xl2).weight(Weight::Semibold),
        core_row().gap(SP3).children([
            Element::from(button("Decrement").variant(ButtonVariant::Secondary)),
            Element::from(button("Increment")),
        ]),
    ])
}

fn main() {
    println!("fenestra bench — release build, embedded fonts, scale 1.0\n");
    let theme = Theme::light();

    cpu_frame("counter (tiny)", (320.0, 160.0), small_view);
    cpu_frame("gallery_controls (medium)", (688.0, 900.0), {
        let theme = theme.clone();
        move || gallery_controls(&theme)
    });
    cpu_frame("gallery_display (large)", (760.0, 1190.0), {
        let theme = theme.clone();
        move || gallery_display(&theme)
    });
    cpu_frame("virtual_list 100k rows (1120x720)", (1120.0, 720.0), || {
        col()
            .w(1120.0)
            .h(720.0)
            .children([virtual_list(100_000, 36.0, |i| {
                core_row()
                    .items_center()
                    .px(SP3)
                    .children([text(format!("Row {i}"))])
            })
            .id("bench-list")])
    });

    println!();
    gpu_frame("gallery_display (760x1190, GPU)", (760, 1190), {
        let theme = theme.clone();
        move || gallery_display(&theme)
    });
    gpu_frame("counter (320x160, GPU)", (320, 160), small_view);
}
