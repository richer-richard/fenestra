//! Text-input adornment golden: a leading `$`, a trailing unit, and both at
//! once. The bordered input keeps its focus ring; the adornments sit inside the
//! field at each end and the text is padded clear of them. Light + dark.

use std::path::PathBuf;

use fenestra_core::{Element, SP5, Theme, Weight, col, text};
use fenestra_kit::text_input;
use fenestra_shell::{render_element, testing::assert_png_snapshot};

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

const SIZE: (u32, u32) = (320, 240);

fn adorn(s: &str) -> Element<()> {
    text(s.to_string())
        .weight(Weight::Medium)
        .themed(|t: &Theme, st| st.color(t.text_muted))
}

fn view(theme: &Theme) -> Element<()> {
    col().p(SP5).gap(SP5).bg(theme.bg).children([
        Element::from(text_input("1200").prefix(adorn("$")).width(240.0)),
        Element::from(text_input("12.5").suffix(adorn("kg")).width(240.0)),
        Element::from(
            text_input("9.99")
                .prefix(adorn("$"))
                .suffix(adorn("USD"))
                .width(240.0),
        ),
    ])
}

#[test]
fn adornments_light() {
    let theme = Theme::light();
    let image = render_element(view(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "adornments_light", &image);
}

#[test]
fn adornments_dark() {
    let theme = Theme::dark();
    let image = render_element(view(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "adornments_dark", &image);
}
