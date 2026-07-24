//! Surface state: the component adjacency list, the data model, and the
//! message fold that builds both from a server→client stream.

use std::collections::{BTreeMap, HashMap};

use serde_json::Value;

use crate::catalog::Component;
use crate::messages::Envelope;

/// Errors from applying a message stream. Everything renderable degrades
/// with a note instead; these are the structural failures.
#[derive(Debug)]
pub enum A2uiError {
    /// The envelope carried no payload.
    EmptyMessage,
    /// A message referenced a surface that was never created.
    UnknownSurface(String),
    /// `createSurface` for an id that already exists.
    DuplicateSurface(String),
}

impl std::fmt::Display for A2uiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyMessage => write!(f, "message carries no payload"),
            Self::UnknownSurface(id) => write!(
                f,
                "message for surface {id:?}, which was never created (missing createSurface?)"
            ),
            Self::DuplicateSurface(id) => write!(
                f,
                "createSurface for {id:?}, which already exists (delete it first)"
            ),
        }
    }
}

impl std::error::Error for A2uiError {}

/// Transient per-surface UI state the protocol leaves to the client.
#[derive(Debug, Default)]
pub(crate) struct UiState {
    /// Modals currently open, by Modal component id.
    pub open_modals: std::collections::HashSet<String>,
    /// Active tab index per Tabs component id.
    pub active_tabs: HashMap<String, usize>,
    /// Local edits to literal-valued inputs, by component id (bound inputs
    /// write to the data model instead).
    pub local_edits: HashMap<String, Value>,
}

/// One A2UI surface: components, data model, and client-side UI state.
#[derive(Debug)]
pub struct Surface {
    /// The surface id every message routed by.
    pub(crate) id: String,
    pub(crate) components: HashMap<String, Component>,
    pub(crate) data: Value,
    pub(crate) send_data_model: bool,
    /// Theme parameters from `createSurface` (advisory; exposed to hosts).
    pub(crate) theme: Option<Value>,
    pub(crate) catalog_id: Option<String>,
    /// Path-pointed notes: unknown components, unresolved functions,
    /// truncations. Silence means full fidelity.
    pub(crate) notes: Vec<String>,
    pub(crate) ui: UiState,
}

impl Surface {
    fn new(create: &crate::messages::CreateSurface) -> Self {
        Self {
            id: create.surface_id.clone(),
            components: HashMap::new(),
            data: Value::Object(serde_json::Map::new()),
            send_data_model: create.send_data_model,
            theme: create.theme.clone(),
            catalog_id: create.catalog_id.clone(),
            notes: Vec::new(),
            ui: UiState::default(),
        }
    }

    /// The surface id.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The current data model.
    #[must_use]
    pub fn data(&self) -> &Value {
        &self.data
    }

    /// Theme parameters from `createSurface`, if any (advisory).
    #[must_use]
    pub fn theme_params(&self) -> Option<&Value> {
        self.theme.as_ref()
    }

    /// The catalog the surface declared.
    #[must_use]
    pub fn catalog_id(&self) -> Option<&str> {
        self.catalog_id.as_deref()
    }

    /// Path-pointed fidelity notes accumulated by parsing and rendering.
    /// Empty means everything resolved and mapped cleanly.
    #[must_use]
    pub fn notes(&self) -> &[String] {
        &self.notes
    }

    /// Number of component definitions currently on the surface.
    #[must_use]
    pub fn component_count(&self) -> usize {
        self.components.len()
    }

    /// Writes `value` at the JSON Pointer `path` (creating intermediate
    /// objects), or removes the key when `value` is `None`.
    pub(crate) fn write(&mut self, path: &str, value: Option<Value>) {
        if path.is_empty() || path == "/" {
            self.data = value.unwrap_or(Value::Object(serde_json::Map::new()));
            return;
        }
        pointer_write(&mut self.data, path, value);
    }
}

/// Sets or removes a value at a JSON Pointer, creating intermediate
/// objects along the way (arrays index numerically; appending uses the
/// next index).
fn pointer_write(root: &mut Value, pointer: &str, value: Option<Value>) {
    let mut parts: Vec<String> = pointer
        .split('/')
        .skip(1)
        .map(|p| p.replace("~1", "/").replace("~0", "~"))
        .collect();
    let Some(last) = parts.pop() else { return };
    let mut cur = root;
    for part in &parts {
        // Descend, converting non-containers into objects as needed.
        let next = match cur {
            Value::Array(items) => match part.parse::<usize>() {
                Ok(i) if i < items.len() => &mut items[i],
                _ => return,
            },
            Value::Object(map) => map
                .entry(part.clone())
                .or_insert_with(|| Value::Object(serde_json::Map::new())),
            other => {
                *other = Value::Object(serde_json::Map::new());
                match other {
                    Value::Object(map) => map
                        .entry(part.clone())
                        .or_insert_with(|| Value::Object(serde_json::Map::new())),
                    _ => unreachable!("just assigned an object"),
                }
            }
        };
        cur = next;
    }
    match cur {
        Value::Array(items) => {
            if let Ok(i) = last.parse::<usize>() {
                match value {
                    Some(v) if i < items.len() => items[i] = v,
                    Some(v) if i == items.len() => items.push(v),
                    Some(_) => {}
                    None if i < items.len() => {
                        items.remove(i);
                    }
                    None => {}
                }
            }
        }
        Value::Object(map) => match value {
            Some(v) => {
                map.insert(last, v);
            }
            None => {
                map.remove(&last);
            }
        },
        other => {
            let mut map = serde_json::Map::new();
            if let Some(v) = value {
                map.insert(last, v);
            }
            *other = Value::Object(map);
        }
    }
}

/// An A2UI client: every live surface, fed by a message stream.
#[derive(Debug, Default)]
pub struct Client {
    surfaces: BTreeMap<String, Surface>,
}

impl Client {
    /// An empty client.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Applies one message.
    ///
    /// # Errors
    /// [`A2uiError`] for structural failures (unknown/duplicate surface,
    /// empty message); content-level issues degrade with surface notes.
    pub fn apply(&mut self, msg: &Envelope) -> Result<(), A2uiError> {
        if let Some(create) = &msg.create_surface {
            if self.surfaces.contains_key(&create.surface_id) {
                return Err(A2uiError::DuplicateSurface(create.surface_id.clone()));
            }
            self.surfaces
                .insert(create.surface_id.clone(), Surface::new(create));
            return Ok(());
        }
        if let Some(update) = &msg.update_components {
            let surface = self
                .surfaces
                .get_mut(&update.surface_id)
                .ok_or_else(|| A2uiError::UnknownSurface(update.surface_id.clone()))?;
            for component in &update.components {
                surface
                    .components
                    .insert(component.id.clone(), component.clone());
            }
            return Ok(());
        }
        if let Some(update) = &msg.update_data_model {
            let surface = self
                .surfaces
                .get_mut(&update.surface_id)
                .ok_or_else(|| A2uiError::UnknownSurface(update.surface_id.clone()))?;
            surface.write(update.path.as_deref().unwrap_or("/"), update.value.clone());
            return Ok(());
        }
        if let Some(delete) = &msg.delete_surface {
            self.surfaces
                .remove(&delete.surface_id)
                .ok_or_else(|| A2uiError::UnknownSurface(delete.surface_id.clone()))?;
            return Ok(());
        }
        Err(A2uiError::EmptyMessage)
    }

    /// Applies a whole stream in order.
    ///
    /// # Errors
    /// The first structural failure, with everything before it applied.
    pub fn apply_all(&mut self, msgs: &[Envelope]) -> Result<(), A2uiError> {
        for msg in msgs {
            self.apply(msg)?;
        }
        Ok(())
    }

    /// The surface with this id.
    #[must_use]
    pub fn surface(&self, id: &str) -> Option<&Surface> {
        self.surfaces.get(id)
    }

    /// Mutable access to a surface (input handling).
    #[must_use]
    pub fn surface_mut(&mut self, id: &str) -> Option<&mut Surface> {
        self.surfaces.get_mut(id)
    }

    /// Every live surface, in stable (sorted) order.
    pub fn surfaces(&self) -> impl Iterator<Item = &Surface> {
        self.surfaces.values()
    }

    /// The only surface, when exactly one exists — the common CLI/MCP case.
    #[must_use]
    pub fn single_surface(&self) -> Option<&Surface> {
        if self.surfaces.len() == 1 {
            self.surfaces.values().next()
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pointer_write_creates_intermediates() {
        let mut root = Value::Object(serde_json::Map::new());
        pointer_write(&mut root, "/user/name", Some(Value::String("Ada".into())));
        assert_eq!(root.pointer("/user/name").unwrap(), "Ada");
        pointer_write(&mut root, "/user/name", None);
        assert!(root.pointer("/user/name").is_none());
    }

    #[test]
    fn pointer_write_indexes_arrays() {
        let mut root = serde_json::json!({"items": [1, 2, 3]});
        pointer_write(&mut root, "/items/1", Some(Value::from(9)));
        assert_eq!(root.pointer("/items/1").unwrap(), 9);
        pointer_write(&mut root, "/items/3", Some(Value::from(4)));
        assert_eq!(root.pointer("/items/3").unwrap(), 4);
    }
}
