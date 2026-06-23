//! Form validation golden: an invalid field (danger ring + the first failing
//! constraint's message) above a valid field (muted help) — the `validate` →
//! `Field::validity` → control `.invalid(..)` wiring, rendered. Light + dark.

use std::path::PathBuf;

use fenestra_core::{Element, SP6, Theme, col};
use fenestra_kit::validation::{Constraint, validate};
use fenestra_kit::{field, text_input};
use fenestra_shell::{render_element, testing::assert_png_snapshot};

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

const SIZE: (u32, u32) = (380, 230);

fn view(theme: &Theme) -> Element<()> {
    let bad = validate("nope", &[Constraint::Required, Constraint::Email]);
    let bad_input: Element<()> = text_input("nope").into();
    let invalid_field: Element<()> = field("Email")
        .required(true)
        .validity(&bad)
        .child(bad_input.invalid(!bad.valid))
        .into();

    let good = validate(
        "ada@example.com",
        &[Constraint::Required, Constraint::Email],
    );
    let valid_field: Element<()> = field("Confirm email")
        .help("Must match the address above.")
        .validity(&good)
        .child(text_input("ada@example.com"))
        .into();

    col()
        .p(SP6)
        .gap(SP6)
        .bg(theme.bg)
        .w(380.0)
        .children([invalid_field, valid_field])
}

#[test]
fn validation_field_light() {
    let theme = Theme::light();
    let image = render_element(view(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "validation_field_light", &image);
}

#[test]
fn validation_field_dark() {
    let theme = Theme::dark();
    let image = render_element(view(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "validation_field_dark", &image);
}
