//! Meter goldens: a neutral measurement (accent) plus the three HTML `<meter>`
//! zones — good (success), suboptimal (warning), and poor (danger) — driven by
//! low/high/optimum thresholds. Light + dark.

use std::path::PathBuf;

use fenestra_core::{Element, SP4, SP6, Theme, col};
use fenestra_kit::meter;
use fenestra_shell::{render_element, testing::assert_png_snapshot};

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots")
}

const SIZE: (u32, u32) = (380, 236);

fn view(theme: &Theme) -> Element<()> {
    // No thresholds: a neutral, accent-filled measurement.
    let neutral: Element<()> = meter(62.0, 0.0, 100.0).label("Storage used").into();
    // Higher-is-better (default optimum = max): 85 clears the high mark → good.
    let good: Element<()> = meter(85.0, 0.0, 100.0)
        .low(30.0)
        .high(70.0)
        .label("Signal strength")
        .into();
    // Same band, 48 sits between low and high → suboptimal.
    let warn: Element<()> = meter(48.0, 0.0, 100.0)
        .low(30.0)
        .high(70.0)
        .label("Battery")
        .into();
    // Lower-is-better (optimum at the floor): 92 is past the high mark → poor.
    let poor: Element<()> = meter(92.0, 0.0, 100.0)
        .low(50.0)
        .high(80.0)
        .optimum(0.0)
        .label("CPU load")
        .into();
    col()
        .p(SP6)
        .gap(SP4)
        .bg(theme.bg)
        .children([neutral, good, warn, poor])
}

#[test]
fn meter_light() {
    let theme = Theme::light();
    let image = render_element(view(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "meter_light", &image);
}

#[test]
fn meter_dark() {
    let theme = Theme::dark();
    let image = render_element(view(&theme), &theme, SIZE);
    assert_png_snapshot(snapshot_dir(), "meter_dark", &image);
}
