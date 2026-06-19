//! Building MCP results: lead with a typed `structuredContent` value and a text
//! serialization, then attach a *downscaled* preview image (the full-resolution
//! PNG is written to a temp file and its path returned, so a large image never
//! bloats every response as base64).

use std::io::Cursor;
use std::sync::atomic::{AtomicU64, Ordering};

use base64::Engine as _;
use image::{ImageFormat, RgbaImage};
use rmcp::model::{CallToolResult, Content};
use serde_json::Value;

/// Longest-edge cap, in pixels, for the inline (token-cheap) preview image.
const PREVIEW_CAP: u32 = 768;

/// A successful result: a text serialization, the structured value as
/// `structuredContent`, and an optional inline downscaled preview image.
pub fn ok(text: String, structured: Value, image: Option<&RgbaImage>) -> CallToolResult {
    let mut content = vec![Content::text(text)];
    if let Some(png) = image {
        content.push(inline_image(png));
    }
    let mut result = CallToolResult::success(content);
    result.structured_content = Some(structured);
    result
}

/// An `isError` result carrying a text message and a structured payload. Used
/// for tool-level failures the agent should self-correct (e.g. an invalid
/// description from `validate`), as opposed to protocol errors (`ErrorData`).
pub fn error(text: String, structured: Value) -> CallToolResult {
    let mut result = CallToolResult::error(vec![Content::text(text)]);
    result.structured_content = Some(structured);
    result
}

/// A downscaled PNG as a base64 image content block.
fn inline_image(png: &RgbaImage) -> Content {
    let (w, h) = png.dimensions();
    let longest = w.max(h);
    let scaled = if longest > PREVIEW_CAP {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "preview dimensions are small and clamped to >= 1"
        )]
        let (nw, nh) = {
            let s = f64::from(PREVIEW_CAP) / f64::from(longest);
            (
                ((f64::from(w) * s) as u32).max(1),
                ((f64::from(h) * s) as u32).max(1),
            )
        };
        image::imageops::thumbnail(png, nw, nh)
    } else {
        png.clone()
    };
    let mut bytes = Vec::new();
    scaled
        .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
        .expect("encoding an in-memory PNG cannot fail");
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Content::image(b64, "image/png")
}

/// Writes the full-resolution PNG to a unique temp file; returns its path, or
/// `None` if the write fails (the preview still goes back inline).
pub fn save_full(png: &RgbaImage) -> Option<String> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("fenestra-mcp-{}-{n}.png", std::process::id()));
    png.save(&path).ok()?;
    Some(path.to_string_lossy().into_owned())
}
