//! Path-pointed errors: every parse or validation failure names *where* in
//! the description it happened, so an agent can fix the JSON without a render.

/// One problem with a description, located by a slash-path into the tree
/// (e.g. `root/children/2/bg`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescribeError {
    /// Slash-path to the offending node or field.
    pub path: String,
    /// Human-actionable description of what is wrong.
    pub message: String,
}

impl DescribeError {
    /// A new error at `path` with `message`.
    pub fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for DescribeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "at {}: {}", self.path, self.message)
    }
}

impl std::error::Error for DescribeError {}
