//! The multi-pass paint plan: the data the shell needs to realize the filter
//! passes ([`Surface::Glass`](crate::Surface::Glass) backdrop blur and
//! foreground [`ElementFilter`]s) that a single vello scene cannot express.
//!
//! [`Frame::paint_backdrop`](crate::Frame::paint_backdrop) walks the tree in
//! [`PaintMode::Backdrop`], emitting one [`MultiPassSpec`] per filtered region
//! while painting a *backdrop scene* with every glass subtree skipped. The
//! shell renders that scene, reads the pixels back, blurs/filters each region on
//! the CPU, and feeds the resulting images to
//! [`Frame::paint_final`](crate::Frame::paint_final) (in [`PaintMode::Final`]),
//! which composites them into a second scene. A frame with no specs needs
//! neither extra pass — the backdrop scene *is* the final image, byte-for-byte
//! identical to the plain single-pass paint.

use std::collections::HashMap;

use kurbo::Rect;

use crate::id::WidgetId;
use crate::style::ElementFilter;

/// One region the shell must filter out-of-band, recorded during the backdrop
/// walk and keyed back to its element by [`id`](Self::id).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MultiPassSpec {
    /// The element whose paint this filter belongs to; the shell returns the
    /// processed image under this id for the final pass.
    pub id: WidgetId,
    /// The element's logical layout rect — the shell scales it to physical
    /// pixels to index the read-back backdrop.
    pub rect: Rect,
    /// What to do with the region's pixels.
    pub kind: PassKind,
}

/// The operation a [`MultiPassSpec`] applies to its region's pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PassKind {
    /// Frosted-glass backdrop blur: blur the content *behind* the pane. The
    /// standard deviation is in **physical** px (already multiplied by the frame
    /// scale), realized as a deterministic 3-pass integer box blur. The pane's
    /// rounded shape is applied when the blurred image is composited under the
    /// tint (it reuses the element's own corner radius), so it is not carried
    /// here.
    BackdropBlur {
        /// Gaussian standard deviation in physical px.
        std_dev: f32,
    },
    /// A foreground filter on the element's own content. Any blur radius it
    /// carries is in logical px (the shell scales it to physical).
    ElementFilter(ElementFilter),
}

/// How [`Frame::paint_with`](crate::Frame) threads the multi-pass plan through
/// one tree walk. The non-glass, non-filtered walk is identical in all three
/// modes, so a frame with no filtered node renders byte-for-byte as it always
/// has.
pub(crate) enum PaintMode<'a> {
    /// The single-pass look: glass paints as its translucent tint only and
    /// foreground filters are inert. This is exactly the pre-blur output, so
    /// every existing golden and the live window keep rendering through it.
    Full,
    /// The backdrop pass: glass subtrees paint *nothing* (so the pixels behind
    /// them can be read back) and every filtered region is recorded here.
    Backdrop(&'a mut Vec<MultiPassSpec>),
    /// The final pass: each filtered element draws its processed image (looked
    /// up by id) — a glass pane composites its blurred backdrop under the tint;
    /// a foreground-filtered element draws its filtered content in place.
    Final(&'a HashMap<WidgetId, peniko::ImageData>),
}

impl<'a> PaintMode<'a> {
    /// The processed image for `id` in the final pass, if one was produced
    /// (None in every other mode, or when the region was clamped away).
    pub(crate) fn injected(&self, id: WidgetId) -> Option<&'a peniko::ImageData> {
        match self {
            PaintMode::Final(images) => {
                // Copy the `&'a HashMap` out so the looked-up reference carries
                // the map's lifetime, not this short `&self` borrow.
                let images: &'a HashMap<WidgetId, peniko::ImageData> = images;
                images.get(&id)
            }
            _ => None,
        }
    }
}
