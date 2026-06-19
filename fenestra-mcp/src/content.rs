//! Building MCP results: lead with a typed `structuredContent` value and a text
//! serialization, attach a *downscaled* inline preview image, and add a
//! `resource_link` to the full-resolution PNG (a `file://` temp path) so a large
//! image never bloats every response as base64 yet stays one fetch away.

use std::io::Cursor;
use std::sync::atomic::{AtomicU64, Ordering};

use base64::Engine as _;
use image::{ImageFormat, RgbaImage};
use rmcp::model::{CallToolResult, Content, RawResource};
use serde_json::Value;

/// Longest-edge cap, in pixels, for the inline (token-cheap) preview image.
const PREVIEW_CAP: u32 = 768;

/// A successful result: a text serialization, the structured value as
/// `structuredContent`, and (when an image is given) a downscaled inline preview
/// plus a `resource_link` to the full-resolution PNG.
pub fn ok(text: String, structured: Value, image: Option<&RgbaImage>) -> CallToolResult {
    let mut content = vec![Content::text(text)];
    if let Some(png) = image {
        content.push(inline_image(png));
        if let Some(link) = full_res_link(png) {
            content.push(link);
        }
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

/// How many full-resolution temp PNGs to retain per process. Each render writes
/// one; we delete the file from `KEEP_FULL_RES` renders ago, so a long-lived
/// server session keeps at most this many on disk instead of leaking unbounded.
/// A client would have to lag this many renders behind to miss a file it still
/// wants — implausible for a synchronous agent.
const KEEP_FULL_RES: u64 = 64;

/// Writes the full-resolution PNG to a unique temp file and returns a
/// `resource_link` content block pointing at it (a `file://` URI), or `None` if
/// the write fails (the inline preview still goes back).
fn full_res_link(png: &RgbaImage) -> Option<Content> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir();
    let pid = std::process::id();
    let path = dir.join(format!("fenestra-mcp-{pid}-{n}.png"));
    png.save(&path).ok()?;
    // Bound the temp footprint: drop the render from KEEP_FULL_RES calls ago.
    if let Some(old) = n.checked_sub(KEEP_FULL_RES) {
        let _ = std::fs::remove_file(dir.join(format!("fenestra-mcp-{pid}-{old}.png")));
    }
    let mut resource = RawResource::new(
        format!("file://{}", path.display()),
        "full-resolution-render.png",
    );
    resource.mime_type = Some("image/png".to_string());
    Some(Content::resource_link(resource))
}
