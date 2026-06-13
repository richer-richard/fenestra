//! OpenType font-feature specimen golden. Registers Playfair Display under
//! the Serif role (it carries `onum`/`lnum`/`smcp`/`liga`/`frac`); figure
//! spacing (`tnum`/`pnum`) renders on the Sans role (embedded Inter). Light
//! theme only — features are theme-independent.

use std::path::PathBuf;

use fenestra::prelude::*;
use fenestra::shell::{render_element_with, testing::assert_png_snapshot};

const SIZE: (u32, u32) = (760, 720);

#[test]
fn font_features_golden() {
    let mut fonts = Fonts::embedded();
    // Serif role carries the figure-shape, small-caps, and fraction features
    // the specimen demonstrates; Inter (Sans) carries tabular/proportional.
    assert!(fonts.register(
        FamilyRole::Serif,
        include_bytes!("../examples/assets/poster/PlayfairDisplay.ttf").to_vec(),
    ));
    let theme = Theme::light();
    let image = render_element_with(
        font_feature_specimen::<()>(&theme),
        &theme,
        SIZE,
        &mut fonts,
    );
    assert_png_snapshot(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots"),
        "font_features",
        &image,
    );
}
