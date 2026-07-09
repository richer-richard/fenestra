//! P0 regression: per-image clamps (`MAX_IMAGE_DIM`, the base64 length cap)
//! bound one image, but nothing originally bounded the *aggregate* decoded
//! bytes across every image in a document. A solid-color PNG compresses to a
//! few hundred KiB regardless of its declared canvas (well under the base64
//! cap), so a document nesting enough images — as container children,
//! `virtual_list` items, or inside `field`/`split_pane`/`tooltip`/`popover`/
//! `dropdown_menu` content — could force an unbounded aggregate RGBA8
//! allocation in a single [`to_element`]/[`to_element_lenient`] call.
//!
//! [`MAX_TOTAL_IMAGE_BYTES`] closes this: a shrinking per-call budget, spent
//! through the `image` crate's own `Limits::max_alloc` so an over-budget
//! image is refused *before* it allocates, not after.
//!
//! These tests decode real (small-file, large-canvas) PNGs — the same
//! "solid color compresses to nothing" shape as the actual attack — so they
//! exercise genuine decode/allocation behavior, not a header-only fixture
//! that would fail for an unrelated reason before ever reaching the budget
//! check. Every test here is sized to stay within a few hundred MiB of real,
//! transient memory (bounded by `MAX_TOTAL_IMAGE_BYTES` itself once the fix
//! is in place) — nowhere near the unbounded multi-gigabyte case the bug
//! allowed.

use fenestra_core::{Fonts, FrameState, Theme, build_frame};
use fenestra_describe::format::Description;
use fenestra_describe::parse::{MAX_TOTAL_IMAGE_BYTES, to_element, to_element_lenient, validate};
use image::ImageEncoder;

/// A syntactically valid, fully-decodable `side`×`side` RGBA8 PNG, solid
/// black — compresses to a tiny file regardless of `side` (real pixel data,
/// unlike the header-only fixture used elsewhere for the per-image dimension
/// clamp test, which fails for an unrelated reason — a missing IDAT chunk —
/// before it would ever reach an allocation-budget check).
fn solid_png(side: u32) -> Vec<u8> {
    solid_png_filled(side, 0)
}

/// Like [`solid_png`] but every byte is `fill`, so different `fill`s produce
/// distinct (still tiny, still fully-decodable) payloads — used to build
/// several *different* full-size images whose decodes can't be deduplicated by
/// a content cache, so the aggregate budget itself is what must bound them.
fn solid_png_filled(side: u32, fill: u8) -> Vec<u8> {
    let raw = vec![fill; (side as usize) * (side as usize) * 4];
    let mut buf = Vec::new();
    image::codecs::png::PngEncoder::new(&mut buf)
        .write_image(&raw, side, side, image::ExtendedColorType::Rgba8)
        .expect("encode a solid-color PNG");
    buf
}

fn base64_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);
        let n = (u32::from(b0) << 16) | (u32::from(b1) << 8) | u32::from(b2);
        out.push(ALPHABET[((n >> 18) & 0x3F) as usize] as char);
        out.push(ALPHABET[((n >> 12) & 0x3F) as usize] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[((n >> 6) & 0x3F) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[(n & 0x3F) as usize] as char
        } else {
            '='
        });
    }
    out
}

/// A base64-encoded, solid-color, full-size (8192×8192 — `MAX_IMAGE_DIM`)
/// PNG: ~256 MiB decoded, comfortably under the base64-length cap despite
/// the huge canvas, and reused across the tests below (generated once).
fn full_size_image_b64() -> String {
    base64_encode(&solid_png(8192))
}

#[test]
fn single_full_size_image_still_decodes_fine() {
    // (c) A single legitimate large image must not be over-clamped: one
    // full-size image is well within the aggregate budget on its own.
    let payload = full_size_image_b64();
    let json = format!(
        r#"{{"schema":"fenestra/1","root":{{"image":{{"png":"{payload}","label":"Full size"}}}}}}"#
    );
    assert!(
        validate(&json).is_ok(),
        "a single full-size image must validate"
    );
    let desc: Description = serde_json::from_str(&json).unwrap();
    assert!(
        to_element(&desc, &fenestra_core::Theme::light()).is_ok(),
        "a single full-size image must build without error"
    );
}

#[test]
fn aggregate_images_beyond_the_budget_are_rejected_not_allocated() {
    // (a) Two full-size (~256 MiB decoded each) images as children of one
    // `col` sum to ~512 MiB, over `MAX_TOTAL_IMAGE_BYTES` (384 MiB). The
    // first must still succeed (it alone fits); the second must degrade to
    // a spacer with a path-pointed budget error — proving the aggregate is
    // actually bounded, not just each image individually.
    let payload = full_size_image_b64();
    let json = format!(
        r#"{{"schema":"fenestra/1","root":{{"col":{{"children":[
            {{"image":{{"png":"{payload}","label":"First"}}}},
            {{"image":{{"png":"{payload}","label":"Second"}}}}
        ]}}}}}}"#
    );
    let desc: Description = serde_json::from_str(&json).unwrap();
    let (_, errs) = to_element_lenient(&desc, &fenestra_core::Theme::light());
    // The rejection can surface either from this crate's own explicit
    // budget check (message mentions "budget") or from the `image` crate's
    // own `Limits::max_alloc` reservation (message mentions "memory limit")
    // — which one fires first depends on the source's native color type
    // (see `image_node`'s doc comment); both are the correct outcome: the
    // image is refused, never decoded.
    assert!(
        errs.iter().any(|e| {
            e.path.contains("children/1")
                && (e.message.to_lowercase().contains("budget")
                    || e.message.to_lowercase().contains("memory limit"))
        }),
        "expected the second (over-budget) image to be refused; got {errs:?}"
    );
    // The first image, still within budget on its own, must not also be
    // flagged.
    assert!(
        !errs.iter().any(|e| e.path.contains("children/0")),
        "the first (within-budget) image should not error; got {errs:?}"
    );
}

#[test]
fn virtual_list_of_large_images_is_bounded() {
    // (b) `virtual_list`'s eager per-row validation pass (run once, at parse
    // time, to surface errors — see its doc comment) must respect the same
    // shared budget: three full-size images (~768 MiB) sum well past the
    // 384 MiB cap (which fits exactly one, with ~128 MiB left over — not
    // enough for a second ~256 MiB image), so the eager pass must stop
    // *decoding* once the budget is spent, rather than unconditionally
    // decoding all of them regardless of size.
    let payload = full_size_image_b64();
    let json = format!(
        r#"{{"schema":"fenestra/1","root":{{"virtual_list":{{"row_height":32,"items":[
            {{"image":{{"png":"{payload}","label":"Row 0"}}}},
            {{"image":{{"png":"{payload}","label":"Row 1"}}}},
            {{"image":{{"png":"{payload}","label":"Row 2"}}}}
        ]}}}}}}"#
    );
    let desc: Description = serde_json::from_str(&json).unwrap();
    let (_, errs) = to_element_lenient(&desc, &fenestra_core::Theme::light());
    let is_budget_error = |path_fragment: &str| {
        errs.iter().any(|e| {
            e.path.contains(path_fragment)
                && (e.message.to_lowercase().contains("budget")
                    || e.message.to_lowercase().contains("memory limit"))
        })
    };
    // Only the first (~256 MiB) row fits in the 384 MiB budget; both
    // subsequent rows must be refused, not silently decoded anyway.
    assert!(!errs.iter().any(|e| e.path.contains("items/0")), "{errs:?}");
    assert!(is_budget_error("items/1"), "{errs:?}");
    assert!(is_budget_error("items/2"), "{errs:?}");
}

#[test]
fn virtual_list_render_path_bounds_aggregate_decode() {
    // The tests above exercise the eager parse-time validation pass. The rows a
    // user actually sees are built lazily by `virtual_list`'s render closure,
    // inside `build_frame` — the path none of the above touch. A tiny
    // `row_height` collapses the virtual window onto *every* row at once, so
    // without one image-decode budget shared across a frame's rows (each row
    // otherwise reset to the full document cap), the aggregate DoS reopens on
    // the paint path. Render it and assert only budget-worth of full-size
    // images materialize; the rest degrade to spacers (their labels disappear).
    // Three *distinct* full-size images (~256 MiB decoded each): distinctness
    // matters, so a content cache can't collapse them into one shared decode —
    // the per-frame budget itself is what has to stop the aggregate.
    let p0 = base64_encode(&solid_png_filled(8192, 0));
    let p1 = base64_encode(&solid_png_filled(8192, 1));
    let p2 = base64_encode(&solid_png_filled(8192, 2));
    let json = format!(
        r#"{{"schema":"fenestra/1","root":{{"virtual_list":{{"row_height":1,"items":[
            {{"image":{{"png":"{p0}","label":"IMGZERO"}}}},
            {{"image":{{"png":"{p1}","label":"IMGONE"}}}},
            {{"image":{{"png":"{p2}","label":"IMGTWO"}}}}
        ]}}}}}}"#
    );
    let desc: Description = serde_json::from_str(&json).unwrap();
    let (el, _) = to_element_lenient(&desc, &Theme::light());
    let mut fonts = Fonts::embedded();
    let mut state = FrameState::new();
    // A 480px-tall viewport over 1px rows virtualizes all three rows into view.
    let frame = build_frame(
        &el,
        &Theme::light(),
        &mut fonts,
        &mut state,
        (640.0, 480.0),
        1.0,
    );
    let yaml = frame.access_yaml();
    let materialized = ["IMGZERO", "IMGONE", "IMGTWO"]
        .iter()
        .filter(|label| yaml.contains(**label))
        .count();
    // 384 MiB budget / ~256 MiB per full-size image → exactly one fits per
    // frame; if two or three materialize, the per-row budget reset bug is back.
    assert!(
        materialized <= 1,
        "virtual_list render path must bound aggregate image decode to the \
         per-frame budget; {materialized} of 3 full-size images materialized"
    );
}

#[test]
fn budget_is_fresh_per_call_not_shared_across_calls() {
    // (d) A single full-size image is within budget. Calling `to_element`
    // repeatedly on the same description must succeed every time — a
    // process-global or otherwise cross-call budget would starve on a later
    // call even though each call, alone, is well within bounds.
    let payload = full_size_image_b64();
    let json = format!(
        r#"{{"schema":"fenestra/1","root":{{"image":{{"png":"{payload}","label":"Reused"}}}}}}"#
    );
    let desc: Description = serde_json::from_str(&json).unwrap();
    for call in 0..3 {
        assert!(
            to_element(&desc, &fenestra_core::Theme::light()).is_ok(),
            "call {call} should get a fresh per-call budget and succeed"
        );
    }
}

#[test]
fn max_total_image_bytes_fits_one_full_size_image_with_headroom() {
    // Sanity-check the documented rationale for the constant itself: it
    // must comfortably exceed one full-`MAX_IMAGE_DIM` image (8192² × 4
    // bytes/px) so ordinary single-image usage is never clamped.
    let one_full_image = 8192usize * 8192 * 4;
    assert!(one_full_image < MAX_TOTAL_IMAGE_BYTES);
    assert!(MAX_TOTAL_IMAGE_BYTES - one_full_image > 0);
}
