//! Hostile theme files: parsing either errors cleanly or resolves to a
//! usable theme — never panics.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &str| {
    if let Ok(spec) = fenestra_core::ThemeSpec::from_json(data) {
        let _theme = spec.theme();
    }
});
