//! Every shipped Look is APCA-legible in both modes — the design-language
//! counterpart to fenestra-core's per-theme contrast gate.

use fenestra_core::Mode;
use fenestra_looks::all;

#[test]
fn every_look_passes_apca_floors() {
    for mode in [Mode::Light, Mode::Dark] {
        for look in all(mode) {
            if let Err(violations) = look.theme.validate_contrast() {
                let lines: Vec<String> = violations.iter().map(ToString::to_string).collect();
                panic!(
                    "look {} ({mode:?}) fails its APCA floors:\n  {}",
                    look.name,
                    lines.join("\n  ")
                );
            }
        }
    }
}
