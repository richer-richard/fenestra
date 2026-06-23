//! Backdrop-blur + foreground-filter eyeball goldens.
//!
//! These prove the two-pass renderer produces a *real* blur, not a flat tint:
//!
//! * `glass_blur_*` floats a [`glass_surface`] over sharp vertical accent-ramp
//!   stripes. A tint alone would leave the stripes crisp through the pane; the
//!   CPU backdrop blur smears them into soft vertical bands — unmistakable.
//! * `element_filter_blur` puts two identical chip panels side by side and
//!   blurs the right one's *own* content via `element_filter`.
//!
//! The blur is a deterministic integer box blur, so these goldens are stable
//! across the macOS/Metal reference and the Linux/lavapipe CI rasterizer (the
//! blur only shrinks the small antialiasing differences between them).

use std::path::PathBuf;

use fenestra_core::{
    Element, ElementFilter, R_LG, SP2, SP3, SP4, SP6, TextSize, Theme, Weight, col, row, stack,
    text,
};
use fenestra_kit::{glass_panel, glass_surface};
use fenestra_shell::{render_element, testing::assert_png_snapshot};

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

const SIZE: (u32, u32) = (640, 420);

/// Sharp vertical stripes from the accent ramp — deterministic high-frequency
/// content the backdrop blur visibly softens behind the pane.
fn striped_backdrop<Msg>(t: &Theme) -> Element<Msg> {
    row().w_full().h_full().children(
        (0..20_usize)
            .map(|i| col().w(32.0).h_full().bg(t.accents.step(2 + i % 9)))
            .collect::<Vec<_>>(),
    )
}

/// A frosted pane (the [`glass_surface`] kit helper) over the stripes.
fn glass_over_stripes<Msg: 'static>(t: &Theme) -> Element<Msg> {
    stack().w_full().h_full().children([
        striped_backdrop(t),
        glass_surface([
            text("Frosted")
                .size(TextSize::Xl2)
                .weight(Weight::Semibold)
                .color(t.text),
            text("real backdrop blur — not a tint")
                .size(TextSize::Sm)
                .color(t.text_muted),
        ])
        .absolute()
        .top(140.0)
        .left(170.0)
        .w(300.0)
        .h(140.0)
        .p(SP6)
        .gap(SP2),
    ])
}

/// Two identical chip panels; the right one's own content is blurred with a
/// foreground `element_filter`.
fn element_filter_demo<Msg: 'static>(t: &Theme) -> Element<Msg> {
    let chip = |label: &str, fill: fenestra_core::Color| {
        row()
            .items_center()
            .px(SP3)
            .h(28.0)
            .rounded_full()
            .bg(fill)
            .child(
                text(label.to_owned())
                    .size(TextSize::Xs)
                    .weight(Weight::Semibold)
                    .color(t.text_on(fill)),
            )
    };
    let panel = || {
        col()
            .w(220.0)
            .p(SP4)
            .gap(SP3)
            .rounded(R_LG)
            .bg(t.surface_raised)
            .border(1.0, t.border_subtle)
            .children([
                text("Panel")
                    .size(TextSize::Base)
                    .weight(Weight::Semibold)
                    .color(t.text),
                row().gap(SP2).children([
                    chip("Danger", t.danger.solid),
                    chip("Success", t.success.solid),
                ]),
            ])
    };
    row()
        .w_full()
        .h_full()
        .items_center()
        .justify_center()
        .gap(SP6)
        .p(SP6)
        .bg(t.surface)
        .children([panel(), panel().element_filter(ElementFilter::Blur(5.0))])
}

#[test]
fn glass_blur_light() {
    let theme = Theme::light();
    let image = render_element(glass_over_stripes::<()>(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "glass_blur_light", &image);
}

#[test]
fn glass_blur_dark() {
    let theme = Theme::dark();
    let image = render_element(glass_over_stripes::<()>(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "glass_blur_dark", &image);
}

#[test]
fn element_filter_blur() {
    let theme = Theme::light();
    let image = render_element(element_filter_demo::<()>(&theme), &theme, (560, 220));
    assert_png_snapshot(snapshot_dir(), "element_filter_blur", &image);
}

/// Smoke: a glass frame (via the `glass_panel` helper) renders through the
/// two-pass pipeline without panicking, at the requested size.
#[test]
fn glass_frame_renders_without_panic() {
    let theme = Theme::dark();
    let view = stack().w_full().h_full().children([
        striped_backdrop::<()>(&theme),
        glass_panel([text("ok").color(theme.text)])
            .absolute()
            .top(40.0)
            .left(40.0)
            .w(160.0)
            .h(80.0),
    ]);
    let image = render_element(view, &theme, (240, 180));
    assert_eq!(image.dimensions(), (240, 180));
}
