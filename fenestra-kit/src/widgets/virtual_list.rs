//! A virtualized vertical list: a scroll container whose rows materialize
//! only when scrolled into view, so 100k-row tables stay a screenful of
//! work per frame.
//!
//! ```
//! use fenestra_core::{Element, row, text};
//! use fenestra_kit::virtual_list;
//!
//! #[derive(Clone)]
//! enum Msg {
//!     Open(usize),
//! }
//!
//! let el: Element<Msg> = virtual_list(100_000, 36.0, |i| {
//!     row()
//!         .items_center()
//!         .px(12.0)
//!         .on_click(Msg::Open(i))
//!         .children([text(format!("Item {i}"))])
//! })
//! .id("items");
//! ```

use fenestra_core::{Element, col};

/// A scrollable column showing `count` fixed-height rows, materializing
/// only the visible window (`builder(i)` per row). Rows are forced to
/// `row_height` and keyed by index; give the list a stable `.id(..)` so
/// its scroll position persists. Overlays inside rows are unsupported.
pub fn virtual_list<Msg: 'static>(
    count: usize,
    row_height: f32,
    builder: impl Fn(usize) -> Element<Msg> + 'static,
) -> Element<Msg> {
    col()
        .w_full()
        .h_full()
        .scroll_y()
        .virtual_rows(count, row_height, builder)
}
