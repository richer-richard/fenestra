//! `text-wrap: balance` / `pretty` specimen golden. Each refinement is
//! stacked directly under its greedy (`Normal`) twin at the same width, so
//! the PNG shows ragged-vs-even for a heading and orphan-vs-pulled-down for
//! a paragraph. Light theme only — line breaking is color-independent.

use std::path::PathBuf;

use fenestra::prelude::*;
use fenestra::shell::{render_element, testing::assert_png_snapshot};

const SIZE: (u32, u32) = (420, 680);

/// A heading wide enough to wrap to several lines at the panel width, so
/// balance has something to even out.
const HEADING: &str = "Balanced headings keep their lines visually even and tidy";

/// A paragraph whose greedy wrap strands a single word on the last line at
/// the 300px column below (4 lines, last line one word); pretty pulls a
/// second word down without adding a line.
const PARAGRAPH: &str = "Typographers avoid leaving a single short word \
stranded alone on the final line of any well-set paragraph anywhere.";

/// The paragraph column width, chosen so the greedy break orphans the last
/// word on the macOS/Metal Inter metrics.
const PARA_W: f32 = 300.0;

/// A muted eyebrow label routed through the theme `text_muted` token.
fn eyebrow(s: &str) -> Element<()> {
    text(s)
        .size(TextSize::Sm)
        .themed(|t: &Theme, st| st.color(t.text_muted))
}

#[test]
fn text_wrap_golden() {
    let view: Element<()> = col().p(24.0).gap(12.0).w(380.0).items_start().children([
        eyebrow("heading · text-wrap: normal"),
        text(HEADING).size_px(28.0).weight(Weight::Semibold),
        eyebrow("heading · text-wrap: balance"),
        text(HEADING)
            .size_px(28.0)
            .weight(Weight::Semibold)
            .balance(),
        divider(),
        eyebrow("paragraph · text-wrap: normal"),
        text(PARAGRAPH).size(TextSize::Base).max_w(PARA_W),
        eyebrow("paragraph · text-wrap: pretty"),
        text(PARAGRAPH).size(TextSize::Base).max_w(PARA_W).pretty(),
    ]);
    let image = render_element(view, &Theme::light(), SIZE);
    assert_png_snapshot(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots"),
        "text_wrap",
        &image,
    );
}
