//! APCA contrast floors are enforced on every shipped theme recipe. This is
//! the headless half of fenestra's "provably-legible themes": a regression
//! that darkens text or lightens a background fails CI, not a design review.

use fenestra_core::{Mode, Theme};

fn assert_legible(name: &str, t: &Theme) {
    if let Err(violations) = t.validate_contrast() {
        let lines: Vec<String> = violations.iter().map(ToString::to_string).collect();
        panic!("{name} fails its APCA floors:\n  {}", lines.join("\n  "));
    }
}

#[test]
fn builtin_and_recipe_themes_are_legible() {
    assert_legible("light", &Theme::light());
    assert_legible("dark", &Theme::dark());
    // The recipes behind the shipped Looks (editorial = duotone, terminal =
    // a green accent). fenestra-looks asserts the Looks themselves too.
    assert_legible(
        "editorial-dark",
        &Theme::duotone(152.0, 6.0, 72.0, Mode::Dark),
    );
    assert_legible(
        "editorial-light",
        &Theme::duotone(152.0, 6.0, 72.0, Mode::Light),
    );
    assert_legible("terminal-dark", &Theme::from_accent(145.0, Mode::Dark));
    assert_legible("terminal-light", &Theme::from_accent(145.0, Mode::Light));
}

#[test]
fn the_gate_actually_fires_on_illegible_text() {
    // Sanity: prove the check catches a real violation. Mid-gray text (N6) on
    // the near-white background is far below the body-text floor.
    let mut t = Theme::light();
    let muddy = t.neutrals.step(6);
    t.text = muddy;
    t.text_muted = muddy;
    let report = t.contrast_report();
    assert!(
        report.iter().any(|v| v.pair == "text on bg"),
        "low-contrast body text must be reported, got {report:?}"
    );
    assert!(t.validate_contrast().is_err());
}
