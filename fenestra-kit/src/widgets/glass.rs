//! Frosted-glass containers: ergonomic wrappers over the
//! [`Surface::Glass`](fenestra_core::Surface) material. That material now
//! carries a real backdrop blur — the shell reads back the content behind the
//! pane and blurs it on the CPU (deterministically), then composites the
//! frosted backdrop under the vibrancy tint. These helpers set the glass
//! material and clip children to the rounded silhouette so nothing spills past
//! the frosted edge; [`glass_panel`] also adds concentric padding for nested
//! rows (the command-palette recipe).
//!
//! The backdrop blur is realized in headless rendering (the golden source of
//! truth); the single-pass live window falls back to the translucent tint.
//!
//! Glass is for a floating, untransformed pane over content. The backdrop is
//! sampled at the pane's layout rect, so a paint transform on the pane (a slide
//! or press-scale) can make the frost lag it, and the blur does not nest
//! (glass-in-glass, or glass inside a foreground `element_filter`). See
//! ARCHITECTURE.md ("Real frosted-glass backdrop blur") for the full envelope.

use fenestra_core::{Element, IntoChildren, SP1, Surface, col};

/// A bare frosted-glass surface: the [`Surface::Glass`] material with its
/// `children` clipped to the rounded pane. Add your own padding and gap, or use
/// [`glass_panel`] for sensible defaults.
pub fn glass_surface<Msg: 'static, M>(children: impl IntoChildren<Msg, M>) -> Element<Msg> {
    col()
        .surface(Surface::Glass)
        .overflow_hidden()
        .children(children)
}

/// A frosted-glass panel ready for content: the [`Surface::Glass`] material,
/// children clipped to the rounded pane, with `SP1` padding and gap so nested
/// rows sit concentrically inside the frost.
pub fn glass_panel<Msg: 'static, M>(children: impl IntoChildren<Msg, M>) -> Element<Msg> {
    col()
        .surface(Surface::Glass)
        .overflow_hidden()
        .p(SP1)
        .gap(SP1)
        .children(children)
}
