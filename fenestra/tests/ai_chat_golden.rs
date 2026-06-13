//! The AI-chat showcase: "the GUI framework AI agents can see," wearing the
//! warm-editorial (Claude-like) look — derived cream/terracotta field, serif
//! prose, the human in a bubble and the assistant flat, a streaming caret and
//! a thinking shimmer. Golden-locked in reduced motion (caret/shimmer pinned
//! to their first keyframe).

use std::path::PathBuf;

use fenestra::prelude::*;
use fenestra::shell::{render_element_with, testing::assert_png_snapshot};

#[test]
fn ai_chat_golden() {
    let mut fonts = Fonts::embedded();
    // Serif prose voice (upright Playfair under the Serif role) — what the
    // warm-editorial Look registers.
    assert!(fonts.register(
        FamilyRole::Serif,
        include_bytes!("../examples/assets/poster/PlayfairDisplay.ttf").to_vec(),
    ));
    // The warm-editorial field: warm paper, terracotta accent, crisp contrast.
    let theme = Theme::derive(
        BaseField {
            hue: 80.0,
            chroma: 2.5,
        },
        40.0,
        Contrast::High,
        Mode::Light,
    );
    let image = render_element_with(ai_chat::<()>(&theme), &theme, (900, 640), &mut fonts);
    assert_png_snapshot(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots"),
        "ai_chat",
        &image,
    );
}

/// Finds the first node whose label contains `needle`, depth-first.
fn find_label<'a>(node: &'a AccessNode, needle: &str) -> Option<&'a AccessNode> {
    if node.label.as_deref().is_some_and(|l| l.contains(needle)) {
        return Some(node);
    }
    node.children.iter().find_map(|c| find_label(c, needle))
}

#[test]
fn ai_chat_column_is_metric_derived() {
    // The reading column is capped at the default measure resolved against its
    // own 20px prose, not a literal 768px. This guard uses embedded fonts (no
    // serif registered), so the Serif role falls back to Inter: 1ch ≈ 12.6px
    // ('0' at 20px), and MEASURE_CH (52) ≈ 655px — the metric-derived reading
    // column. The full-width assistant prose fills it exactly.
    let mut fonts = Fonts::embedded();
    let theme = Theme::light();
    let mut state = FrameState::new();
    state.reduced_motion = true;
    let el = ai_chat::<()>(&theme);
    let frame = build_frame(&el, &theme, &mut fonts, &mut state, (900.0, 640.0), 1.0);
    let tree = frame.access_tree();
    let prose = find_label(&tree, "Ask me anything").expect("assistant prose leaf");
    let width = prose.rect.width();
    assert!(
        (600.0..=720.0).contains(&width),
        "reading-column prose width {width} should be metric-derived (~655px), not 768px",
    );
}
