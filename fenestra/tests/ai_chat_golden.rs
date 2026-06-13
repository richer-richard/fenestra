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
