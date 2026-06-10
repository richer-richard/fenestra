//! Clipboard abstraction: the core stays windowless, so the OS clipboard
//! (arboard) is injected by the shell; headless rendering uses the
//! deterministic in-memory default.

/// Read/write access to a clipboard.
pub trait Clipboard {
    /// Current clipboard text, if any.
    fn get(&mut self) -> Option<String>;
    /// Replaces the clipboard text.
    fn set(&mut self, text: String);
}

/// The default in-memory clipboard (headless tests use this).
#[derive(Default)]
pub struct MemoryClipboard(Option<String>);

impl Clipboard for MemoryClipboard {
    fn get(&mut self) -> Option<String> {
        self.0.clone()
    }

    fn set(&mut self, text: String) {
        self.0 = Some(text);
    }
}
