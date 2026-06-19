//! Serialized UI for fenestra: a JSON `Description` parses to the same
//! `Element` tree the builders produce, then runs through the identical
//! render and verification pipeline — the boundary an agent drives.
//!
//! The crate is windowless: it depends only on `fenestra-core` and
//! `fenestra-kit`, so parsing, the structural access tree, semantic
//! queries, aria snapshots, and accessibility checks all run without a
//! GPU. Pixel rendering lives one layer up, in `fenestra-render`.

pub mod color;
pub mod dto;
pub mod error;
pub mod format;
pub mod inspect;
pub mod parse;
pub mod state;
pub mod vocabulary;
