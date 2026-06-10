//! The OS clipboard (arboard), injected into `FrameState` by the windowed
//! runner. Headless rendering keeps core's in-memory clipboard.

use fenestra_core::Clipboard;

/// Lazy arboard wrapper; failures (no display server) degrade to a no-op.
#[derive(Default)]
pub struct OsClipboard {
    inner: Option<arboard::Clipboard>,
}

impl OsClipboard {
    fn ensure(&mut self) -> Option<&mut arboard::Clipboard> {
        if self.inner.is_none() {
            self.inner = arboard::Clipboard::new().ok();
        }
        self.inner.as_mut()
    }
}

impl Clipboard for OsClipboard {
    fn get(&mut self) -> Option<String> {
        self.ensure().and_then(|c| c.get_text().ok())
    }

    fn set(&mut self, text: String) {
        if let Some(c) = self.ensure() {
            let _ = c.set_text(text);
        }
    }
}
