//! The editorial poster: fenestra's proof that design range goes beyond
//! dashboards — custom display faces, duotone atmosphere, tracked type.

use std::path::PathBuf;

use fenestra::prelude::*;
use fenestra::shell::{render_element_with, testing::assert_png_snapshot};

#[test]
fn poster_golden() {
    let mut fonts = Fonts::embedded();
    assert!(fonts.register(
        FamilyRole::Display,
        include_bytes!("../examples/assets/poster/PlayfairDisplay.ttf").to_vec(),
    ));
    assert!(fonts.register(
        FamilyRole::Serif,
        include_bytes!("../examples/assets/poster/PlayfairDisplay-Italic.ttf").to_vec(),
    ));
    let theme = Theme::duotone(152.0, 6.0, 72.0, Mode::Dark);
    let image = render_element_with(poster::<()>(&theme), &theme, (1040, 1300), &mut fonts);
    assert_png_snapshot(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots"),
        "poster",
        &image,
    );
}
