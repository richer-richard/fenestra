//! Stable widget identity: `WidgetId = hash(parent_id, child_index | key)`.
//!
//! Uses FNV-1a so ids are deterministic across runs and platforms (std's
//! hasher makes no such promise). Stateful widgets (scroll views, inputs,
//! anything animated) need stable identity; `.id("...")` pins it wherever
//! children can reorder.

/// A stable identity for one element in the tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WidgetId(pub u64);

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

fn fnv1a(mut hash: u64, bytes: &[u8]) -> u64 {
    for &b in bytes {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

impl WidgetId {
    /// The root of the element tree.
    pub const ROOT: Self = Self(FNV_OFFSET);

    /// Derives a child id from this id plus the child's index, or its user
    /// key when one was set via `.id("...")`.
    pub fn child(self, index: usize, key: Option<&str>) -> Self {
        let hash = fnv1a(FNV_OFFSET, &self.0.to_le_bytes());
        // Tag bytes keep keyed and indexed children in disjoint domains.
        let hash = match key {
            Some(key) => fnv1a(fnv1a(hash, &[0x4b]), key.as_bytes()),
            None => fnv1a(fnv1a(hash, &[0x49]), &(index as u64).to_le_bytes()),
        };
        Self(hash)
    }
}
