//! A clonable, paint-only snapshot of a realized subtree — the "ghost" an
//! exiting element leaves behind while it animates out.
//!
//! [`FrameNode`](crate::frame) is not `Clone` (it carries interactive,
//! per-frame state: scroll geometry, accessibility projection, live editors).
//! An exit animation outlives the element, so it cannot borrow the live node;
//! it snapshots just what painting needs into this owned, clonable tree. There
//! is no live widget behind a ghost — a text input collapses to a plain box
//! ([`GhostPaint::InputBox`]), since the editor is gone with the element.

use kurbo::Rect;

use crate::element::{ImageData, PathData, Span};
use crate::style::Style;
use crate::text::ResolvedText;

/// One node of a snapshotted subtree: an absolute rect, the resolved style to
/// paint, the inherited clip it lived within, its paint payload, and children
/// in paint order. Cheap to clone (the heavy path/image payloads are `Arc`-shared).
#[derive(Clone)]
pub(crate) struct GhostNode {
    /// Absolute logical rect, frozen at snapshot time.
    pub(crate) rect: Rect,
    /// Resolved style (fill, border, radius, shadows, opacity, transform).
    pub(crate) style: Style,
    /// Effective clip rect inherited from ancestors at snapshot time (None =
    /// unclipped); honored so a ghost cannot paint outside the viewport its
    /// live counterpart was clipped to.
    pub(crate) visible: Option<Rect>,
    /// What this node draws on top of its box.
    pub(crate) paint: GhostPaint,
    /// Children in paint order.
    pub(crate) children: Vec<GhostNode>,
}

/// The paint payload of a [`GhostNode`], mirroring the live frame's paint
/// kinds. Inputs collapse to [`GhostPaint::InputBox`] (the box only — no live
/// editor, caret, or selection in a snapshot).
#[derive(Clone)]
pub(crate) enum GhostPaint {
    /// A plain container box (the box layers come from the style).
    Box,
    /// A static text run.
    Text {
        /// The text payload.
        text: String,
        /// Resolved text style.
        style: ResolvedText,
    },
    /// A styled rich-text paragraph.
    Rich {
        /// The styled runs.
        spans: Vec<Span>,
        /// Resolved paragraph style.
        style: ResolvedText,
    },
    /// A vector path (icons, check marks), painted frozen (no spin rotation).
    Path(PathData),
    /// An RGBA8 image.
    Image(ImageData),
    /// A text input, collapsed to its box (no editor behind the ghost).
    InputBox,
}
