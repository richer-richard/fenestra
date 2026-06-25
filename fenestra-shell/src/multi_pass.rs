//! Turns a [`MultiPassSpec`] plan plus a read-back backdrop image into the
//! per-element filtered images the final paint pass composites. This is the CPU
//! middle stage of the two-pass renderer: render with glass skipped → read back
//! → **`process_specs`** → paint the final scene with each filtered image.

use std::collections::HashMap;

use fenestra_core::{ElementFilter, MultiPassSpec, PassKind, WidgetId};
use image::RgbaImage;
use kurbo::Rect;
use vello::peniko;

use crate::blur::{apply_element_filter, box_blur_rgba8, box_radius_for_std_dev, refract_edges};

/// Filters each spec's region of the read-back `backdrop`, returning the image
/// the final pass draws for that element (keyed by [`WidgetId`]). `scale` maps a
/// spec's logical rect — and a foreground blur's logical radius — onto the
/// physical backdrop. Regions that clamp to nothing (off-screen or zero-size)
/// are skipped, so a missing entry simply means "paint normally".
#[must_use]
pub fn process_specs(
    backdrop: &RgbaImage,
    specs: &[MultiPassSpec],
    scale: f64,
) -> HashMap<WidgetId, peniko::ImageData> {
    let mut out = HashMap::with_capacity(specs.len());
    let (iw, ih) = (backdrop.width(), backdrop.height());
    for spec in specs {
        let Some((x, y, w, h)) = physical_rect(spec.rect, scale, iw, ih) else {
            continue;
        };
        let sub = image::imageops::crop_imm(backdrop, x, y, w, h).to_image();
        let filtered = match spec.kind {
            PassKind::BackdropBlur { std_dev, radius } => {
                let blurred = box_blur_rgba8(&sub, box_radius_for_std_dev(std_dev));
                // Bend the blurred backdrop at the rounded rim (the lensing pass).
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "physical corner radius fits in f32"
                )]
                let radius_px = (f64::from(radius) * scale) as f32;
                refract_edges(&blurred, radius_px)
            }
            PassKind::ElementFilter(filter) => {
                apply_element_filter(&sub, scale_filter(filter, scale))
            }
        };
        out.insert(spec.id, to_image_data(&filtered));
    }
    out
}

/// A logical rect scaled to an integer pixel rect clamped to the image, or
/// `None` when it has no area inside the image.
fn physical_rect(rect: Rect, scale: f64, iw: u32, ih: u32) -> Option<(u32, u32, u32, u32)> {
    let left = clamp_coord((rect.x0 * scale).floor(), iw);
    let top = clamp_coord((rect.y0 * scale).floor(), ih);
    let right = clamp_coord((rect.x1 * scale).ceil(), iw);
    let bottom = clamp_coord((rect.y1 * scale).ceil(), ih);
    if right <= left || bottom <= top {
        return None;
    }
    Some((left, top, right - left, bottom - top))
}

/// Clamps a (possibly out-of-range) coordinate to `[0, max]` and converts it to
/// a pixel index.
fn clamp_coord(v: f64, max: u32) -> u32 {
    let v = v.clamp(0.0, f64::from(max));
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "clamped to [0, max], finite"
    )]
    let out = v as u32;
    out
}

/// Scales a foreground filter's logical blur radius to physical px; other
/// filters are scale-independent.
fn scale_filter(filter: ElementFilter, scale: f64) -> ElementFilter {
    match filter {
        ElementFilter::Blur(r) => {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "scaled blur radius fits in f32"
            )]
            let scaled = (f64::from(r) * scale) as f32;
            ElementFilter::Blur(scaled)
        }
        other => other,
    }
}

/// Wraps RGBA8 pixels as a straight-alpha peniko image (mirrors
/// `fenestra_core::image_rgba8`).
fn to_image_data(img: &RgbaImage) -> peniko::ImageData {
    peniko::ImageData {
        data: img.as_raw().clone().into(),
        format: peniko::ImageFormat::Rgba8,
        alpha_type: peniko::ImageAlphaType::Alpha,
        width: img.width(),
        height: img.height(),
    }
}
