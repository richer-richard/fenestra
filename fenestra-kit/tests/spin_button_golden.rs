//! Spin button goldens: a normal stepper, one gated at its minimum (− dimmed),
//! one gated at its maximum (+ dimmed), and one with an app-formatted value.
//! Light + dark — verifies the bordered group, the dividers, and the disabled
//! step buttons.

use std::path::PathBuf;

use fenestra_core::{Element, SP4, SP6, Theme, col};
use fenestra_kit::spin_button;
use fenestra_shell::{render_element, testing::assert_png_snapshot};

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

const SIZE: (u32, u32) = (240, 240);

fn view(theme: &Theme) -> Element<()> {
    let normal: Element<()> = spin_button("3")
        .label("Quantity")
        .on_decrement(())
        .on_increment(())
        .into();
    let at_min: Element<()> = spin_button("0")
        .label("Guests")
        .on_increment(())
        .can_decrement(false)
        .into();
    let at_max: Element<()> = spin_button("10")
        .label("Seats")
        .on_decrement(())
        .can_increment(false)
        .into();
    let money: Element<()> = spin_button("$5.00")
        .label("Price")
        .on_decrement(())
        .on_increment(())
        .into();
    col()
        .p(SP6)
        .gap(SP4)
        .bg(theme.bg)
        .children([normal, at_min, at_max, money])
}

#[test]
fn spin_button_light() {
    let theme = Theme::light();
    let image = render_element(view(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "spin_button_light", &image);
}

#[test]
fn spin_button_dark() {
    let theme = Theme::dark();
    let image = render_element(view(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "spin_button_dark", &image);
}
