//! Hostile descriptions: validating, parsing, and converting an arbitrary JSON
//! string either errors cleanly or yields a renderable element — never panics.
//! serde_json's recursion limit bounds nesting depth, so deep input is rejected
//! rather than overflowing the stack.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &str| {
    // Validation (serde parse + semantic check) must never panic.
    let _ = fenestra_describe::parse::validate(data);

    // A description that deserializes must convert to an element without
    // panicking, in both the strict and lenient forms.
    if let Ok(desc) = serde_json::from_str::<fenestra_describe::format::Description>(data) {
        let theme = fenestra_core::Theme::light();
        let _ = fenestra_describe::parse::to_element(&desc, &theme);
        let _ = fenestra_describe::parse::to_element_lenient(&desc, &theme);
    }
});
