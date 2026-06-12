//! 0.6 rich text: one wrapped paragraph, per-span weight/color/size/
//! family/italic — pixel-locked; plus bidi/RTL verification (parley
//! shapes mixed-direction text; Arabic/Hebrew glyphs come from system
//! fallback like CJK).

use fenestra_core::{Element, Fonts, Theme, col, rich_text, span, text};
use fenestra_shell::render_element;
use fenestra_shell::testing::assert_png_snapshot;

fn snapshot_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("snapshots")
}

fn rich_view(theme: &Theme) -> Element<()> {
    col().p(16.0).gap(8.0).children([
        rich_text([
            span("Ship it "),
            span("boldly")
                .weight(fenestra_core::Weight::Semibold)
                .color(theme.accent),
            span(" — or "),
            span("italically").italic(),
            span(", even "),
            span("LARGE").size_px(26.0),
            span(" or "),
            span("mono").family(fenestra_core::FamilyRole::Mono),
            span("."),
        ]),
        rich_text([
            span("Wrapped paragraphs keep spans flowing together across lines, "),
            span("highlighted runs included")
                .color(theme.accent)
                .weight(fenestra_core::Weight::Medium),
            span(", with the base style inherited everywhere else."),
        ]),
    ])
}

#[test]
fn rich_text_paints_per_span_styles() {
    let theme = Theme::light();
    let image = render_element(rich_view(&theme), &theme, (420, 160));
    assert_png_snapshot(snapshot_dir(), "rich_text", &image);
}

#[test]
fn rich_text_is_one_accessible_label() {
    use fenestra_core::{FrameState, build_frame, by};
    let theme = Theme::light();
    let view: Element<()> = col().children([rich_text([span("Hello "), span("world").italic()])]);
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    let frame = build_frame(&view, &theme, &mut fonts, &mut state, (300.0, 100.0), 1.0);
    // Spans concatenate into one accessible name.
    assert!(frame.query(&by::label("Hello world")).is_some());
}

// ------------------------------------------------------------- bidi / RTL

/// Mixed-direction text shapes without panicking and lays out nonzero
/// even on embedded fonts (glyph coverage aside) — the shaping path is
/// total.
#[test]
fn bidi_shaping_is_total_on_embedded_fonts() {
    let theme = Theme::light();
    let view: Element<()> = col().p(12.0).children([
        text("From left ثم العربية then back שוב עברית and done."),
        rich_text([
            span("rich "),
            span("مرحبا").color(theme.accent),
            span(" mixed "),
            span("שלום").italic(),
        ]),
    ]);
    let image = render_element(view, &theme, (460, 90));
    assert_eq!(image.dimensions(), (460, 90));
}

/// RTL scripts shape through real system families (like the CJK
/// fallback proof) — macOS-gated: the CI reference platform ships
/// Arabic/Hebrew system fonts; other runners' inventories vary.
#[cfg(target_os = "macos")]
#[test]
fn system_fonts_provide_rtl_fallback() {
    let theme = Theme::light();
    let view = || -> Element<()> {
        col()
            .p(12.0)
            .children([text("peace שלום سلام done").size_px(22.0)])
    };
    let mut embedded = Fonts::embedded();
    let a = fenestra_shell::render_element_with(view(), &theme, (460, 60), &mut embedded);
    let mut system = Fonts::with_system();
    let b = fenestra_shell::render_element_with(view(), &theme, (460, 60), &mut system);

    let differing = a
        .pixels()
        .zip(b.pixels())
        .filter(|(pa, pb)| pa != pb)
        .count();
    assert!(
        differing > 500,
        "system fallback should change the RTL glyphs ({differing} pixels differ)"
    );
    let ink = b
        .pixels()
        .filter(|p| p.0[0] < 200 || p.0[1] < 200 || p.0[2] < 200)
        .count();
    assert!(
        ink > 300,
        "RTL text should render visible glyphs, got {ink}"
    );
}
