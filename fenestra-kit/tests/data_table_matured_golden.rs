//! Goldens for the matured `data_table`: a virtualized table scrolled into its
//! middle (the sticky header holding while rows scroll under it), columns at
//! explicit pixel widths with resize handles, a filter row with one column
//! filtered, and a reordered display order. Light + dark for each.

use std::path::PathBuf;

use fenestra_core::{Element, Fonts, FrameState, SP6, Theme, WidgetId, build_frame, by, col};
use fenestra_kit::data_table;
use fenestra_shell::{render_element, render_element_with_state, testing::assert_png_snapshot};

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

/// 100-row dataset for the virtualized scroll golden.
fn crew_rows() -> Vec<Vec<String>> {
    let roles = ["Officer", "Captain", "Navigator", "Engineer", "Medic"];
    (0..100)
        .map(|i| {
            vec![
                format!("{i:03}"),
                format!("Crew {i}"),
                roles[i % roles.len()].to_owned(),
                format!("{}", (i * 37) % 400),
            ]
        })
        .collect()
}

fn virtual_view(theme: &Theme) -> Element<()> {
    let table: Element<()> = data_table(["#", "Name", "Role", "Commits"], crew_rows())
        .id("crew")
        .sort(0, true)
        .selected(Some(54))
        .sticky_header(true)
        .on_sort(|_| ())
        .into();
    col()
        .p(SP6)
        .bg(theme.bg)
        .child(col().w_full().h(220.0).child(table))
}

/// Logical size of a pixel size (both dims fit in u16, so the conversion is
/// lossless and clippy-clean).
fn logical(size: (u32, u32)) -> (f32, f32) {
    let to_f = |v: u32| f32::from(u16::try_from(v).expect("dimension fits in u16"));
    (to_f(size.0), to_f(size.1))
}

/// Builds once to find the body's stable id so the state can be pre-scrolled.
fn body_id(view: &Element<()>, size: (u32, u32)) -> WidgetId {
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    let frame = build_frame(
        view,
        &Theme::light(),
        &mut fonts,
        &mut state,
        logical(size),
        1.0,
    );
    frame.get(&by::id("dt-body-crew")).id
}

fn render_virtual(theme: &Theme, size: (u32, u32)) -> image::RgbaImage {
    let id = body_id(&virtual_view(theme), size);
    let mut state = FrameState::new();
    state.reduced_motion = true;
    // Scroll to ~row 50 so the window sits squarely in the middle of the list,
    // with the sticky header still pinned above it.
    state.scroll_to(id, 50.0 * 34.0);
    render_element_with_state(virtual_view(theme), theme, size, &mut state)
}

/// A compact table at explicit pixel widths (resize handles wired).
fn resize_view(theme: &Theme) -> Element<()> {
    let table: Element<()> = data_table(
        ["Name", "Role", "Commits"],
        vec![
            vec!["Ripley".into(), "Warrant Officer".into(), "128".into()],
            vec!["Dallas".into(), "Captain".into(), "97".into()],
            vec!["Lambert".into(), "Navigator".into(), "64".into()],
            vec!["Kane".into(), "Executive Officer".into(), "42".into()],
        ],
    )
    .id("resize")
    .sort(2, false)
    .column_widths([150.0, 220.0, 110.0])
    .on_sort(|_| ())
    .on_resize(|_, _| ())
    .on_resize_end(())
    .into();
    col().p(SP6).bg(theme.bg).children([table])
}

/// A filter row with the first column filtered to "rip".
fn filter_view(theme: &Theme) -> Element<()> {
    let table: Element<()> = data_table(
        ["Name", "Role", "Commits"],
        vec![
            vec!["Ripley".into(), "Warrant Officer".into(), "128".into()],
            vec!["Ripper".into(), "Gunner".into(), "73".into()],
        ],
    )
    .id("filter")
    .filter(["rip".into(), String::new(), String::new()])
    .on_filter(|_, _| ())
    .into();
    col().p(SP6).bg(theme.bg).children([table])
}

/// A reordered display order: Commits, Name, Role.
fn reorder_view(theme: &Theme) -> Element<()> {
    let table: Element<()> = data_table(
        ["Name", "Role", "Commits"],
        vec![
            vec!["Ripley".into(), "Warrant Officer".into(), "128".into()],
            vec!["Dallas".into(), "Captain".into(), "97".into()],
        ],
    )
    .id("reorder")
    .column_widths([160.0, 220.0, 110.0])
    .column_order([2, 0, 1])
    .sort(2, false)
    .on_sort(|_| ())
    .on_reorder(|_, _| ())
    .into();
    col().p(SP6).bg(theme.bg).children([table])
}

const TALL: (u32, u32) = (560, 320);
const WIDE: (u32, u32) = (560, 260);

#[test]
fn data_table_virtual_light() {
    let theme = Theme::light();
    let image = render_virtual(&theme, TALL);
    assert_png_snapshot(snapshot_dir(), "data_table_virtual_light", &image);
}

#[test]
fn data_table_virtual_dark() {
    let theme = Theme::dark();
    let image = render_virtual(&theme, TALL);
    assert_png_snapshot(snapshot_dir(), "data_table_virtual_dark", &image);
}

#[test]
fn data_table_resize_light() {
    let theme = Theme::light();
    let image = render_element(resize_view(&theme), &theme, WIDE);
    assert_png_snapshot(snapshot_dir(), "data_table_resize_light", &image);
}

#[test]
fn data_table_resize_dark() {
    let theme = Theme::dark();
    let image = render_element(resize_view(&theme), &theme, WIDE);
    assert_png_snapshot(snapshot_dir(), "data_table_resize_dark", &image);
}

#[test]
fn data_table_filter_light() {
    let theme = Theme::light();
    let image = render_element(filter_view(&theme), &theme, WIDE);
    assert_png_snapshot(snapshot_dir(), "data_table_filter_light", &image);
}

#[test]
fn data_table_filter_dark() {
    let theme = Theme::dark();
    let image = render_element(filter_view(&theme), &theme, WIDE);
    assert_png_snapshot(snapshot_dir(), "data_table_filter_dark", &image);
}

#[test]
fn data_table_reorder_light() {
    let theme = Theme::light();
    let image = render_element(reorder_view(&theme), &theme, WIDE);
    assert_png_snapshot(snapshot_dir(), "data_table_reorder_light", &image);
}

#[test]
fn data_table_reorder_dark() {
    let theme = Theme::dark();
    let image = render_element(reorder_view(&theme), &theme, WIDE);
    assert_png_snapshot(snapshot_dir(), "data_table_reorder_dark", &image);
}
