//! Form-field goldens: a required field with help text, a plain field, and an
//! invalid field showing a danger-toned error message. Light + dark.

use std::path::PathBuf;

use fenestra_core::{Element, SP5, SP6, Theme, col};
use fenestra_kit::{field, text_input};
use fenestra_shell::{render_element, testing::assert_png_snapshot};

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

const SIZE: (u32, u32) = (360, 320);

fn view(theme: &Theme) -> Element<()> {
    let email: Element<()> = field("Email")
        .required(true)
        .help("We'll never share it.")
        .child(text_input("ada@example.com"))
        .into();
    let username: Element<()> = field("Username").child(text_input("ada")).into();
    let password: Element<()> = field("Password")
        .error("Must be at least 8 characters.")
        .child(text_input("123").invalid(true))
        .into();
    col()
        .p(SP6)
        .gap(SP5)
        .bg(theme.bg)
        .children([email, username, password])
}

#[test]
fn field_light() {
    let theme = Theme::light();
    let image = render_element(view(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "field_light", &image);
}

#[test]
fn field_dark() {
    let theme = Theme::dark();
    let image = render_element(view(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "field_dark", &image);
}
