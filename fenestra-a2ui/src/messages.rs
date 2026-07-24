//! The A2UI v0.9 server→client message types, as specified by
//! <https://a2ui.org/specification/v0_9/server_to_client.json>.
//!
//! Parsing is deliberately tolerant where the standard's progressive
//! spirit calls for it (unknown component/message fields are kept or
//! skipped with a recorded note, never a hard failure), and strict where
//! identity matters (`surfaceId` routing, the `version` tag).

use serde::Deserialize;
use serde_json::Value;

use crate::catalog::Component;

/// One server→client message: a version tag plus exactly one payload.
///
/// Unknown payloads (message types from a newer protocol revision,
/// transport metadata) land in [`Self::extra`] instead of failing the
/// parse; the client skips them with a note on the surface they name.
#[derive(Debug, Clone, Deserialize)]
pub struct Envelope {
    /// The protocol version tag; v0.9 is what this crate implements.
    /// Missing tags are tolerated (some transports strip them).
    #[serde(default)]
    pub version: Option<String>,
    /// Initializes a new surface.
    #[serde(default, rename = "createSurface")]
    pub create_surface: Option<CreateSurface>,
    /// Adds or replaces component definitions on a surface.
    #[serde(default, rename = "updateComponents")]
    pub update_components: Option<UpdateComponents>,
    /// Updates the surface's data model at a JSON Pointer path.
    #[serde(default, rename = "updateDataModel")]
    pub update_data_model: Option<UpdateDataModel>,
    /// Removes a surface entirely.
    #[serde(default, rename = "deleteSurface")]
    pub delete_surface: Option<DeleteSurface>,
    /// Everything else in the envelope: unknown message types and
    /// transport metadata, kept for inspection rather than erroring the
    /// stream.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
}

/// `createSurface`: a new surface with a catalog and optional theme hints.
#[derive(Debug, Clone, Deserialize)]
pub struct CreateSurface {
    /// Unique surface identity; every later message routes by it.
    #[serde(rename = "surfaceId")]
    pub surface_id: String,
    /// The component catalog the surface's components come from. This
    /// crate implements the v0.9 *basic* catalog; other catalog ids render
    /// best-effort with notes.
    #[serde(default, rename = "catalogId")]
    pub catalog_id: Option<String>,
    /// Theme parameters (e.g. `{"primaryColor": "#FF0000"}`), advisory.
    #[serde(default)]
    pub theme: Option<Value>,
    /// Whether actions should carry the full data model back to the agent.
    #[serde(default, rename = "sendDataModel")]
    pub send_data_model: bool,
}

/// `updateComponents`: a flat adjacency list of component definitions.
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateComponents {
    /// The surface these components belong to.
    #[serde(rename = "surfaceId")]
    pub surface_id: String,
    /// Component definitions; ids already present are replaced. One
    /// component with id `root` anchors the tree.
    #[serde(default)]
    pub components: Vec<Component>,
}

/// `updateDataModel`: replace (or remove) the value at a JSON Pointer.
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateDataModel {
    /// The surface whose data model changes.
    #[serde(rename = "surfaceId")]
    pub surface_id: String,
    /// Where to write; omitted or `/` means the whole model.
    #[serde(default)]
    pub path: Option<String>,
    /// The new value; omitted removes the key at `path`.
    #[serde(default)]
    pub value: Option<Value>,
}

/// `deleteSurface`: remove a surface and its contents.
#[derive(Debug, Clone, Deserialize)]
pub struct DeleteSurface {
    /// The surface to remove.
    #[serde(rename = "surfaceId")]
    pub surface_id: String,
}

/// A named example/stream wrapper used by the official gallery files:
/// `{ "name": …, "description": …, "messages": [Envelope, …] }`. Also the
/// convenient authoring shape for files fed to the CLI/MCP.
#[derive(Debug, Clone, Deserialize)]
pub struct MessageStream {
    /// Display name (gallery metadata).
    #[serde(default)]
    pub name: Option<String>,
    /// What the stream demonstrates (gallery metadata).
    #[serde(default)]
    pub description: Option<String>,
    /// The messages, in order.
    pub messages: Vec<Envelope>,
}

/// Parses either a raw message array or the `{ "messages": [...] }`
/// wrapper the official examples use.
///
/// # Errors
/// A [`serde_json::Error`] when the input is neither shape.
pub fn parse_stream(json: &str) -> Result<Vec<Envelope>, serde_json::Error> {
    if let Ok(stream) = serde_json::from_str::<MessageStream>(json) {
        return Ok(stream.messages);
    }
    serde_json::from_str::<Vec<Envelope>>(json)
}
