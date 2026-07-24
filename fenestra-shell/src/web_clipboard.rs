//! The web clipboard: writes reach the system clipboard through the async
//! `navigator.clipboard` API (fire-and-forget — dropping the Promise still
//! queues the write); reads come from an in-app mirror, because the
//! browser only exposes cross-page paste through async permission flows a
//! synchronous [`Clipboard`] call cannot block on. In-app copy/paste is
//! therefore fully functional, and copy *out* to other apps works; paste
//! *in* from other apps is the remaining, documented gap.

use fenestra_core::Clipboard;

/// See the module docs.
#[derive(Default)]
pub struct WebClipboard(Option<String>);

impl Clipboard for WebClipboard {
    fn get(&mut self) -> Option<String> {
        self.0.clone()
    }

    fn set(&mut self, text: String) {
        self.0 = Some(text.clone());
        if let Some(window) = web_sys::window() {
            // Fire-and-forget: the returned Promise runs regardless.
            let _ = window.navigator().clipboard().write_text(&text);
        }
    }
}
