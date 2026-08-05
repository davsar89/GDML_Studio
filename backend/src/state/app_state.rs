use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::eval::engine::EvalEngine;
use crate::gdml::model::GdmlDocument;
use crate::mesh::types::TriangleMesh;

pub struct LoadedDocument {
    /// Exactly what was parsed from the file. This is what gets exported, so a
    /// save writes the user's `<loop>` elements back verbatim.
    pub document: GdmlDocument,
    /// The same document with every `<loop>` expanded, when it had any.
    ///
    /// Geometry is built from this so the preview shows all N placements; the
    /// source above is untouched. Only `structure` and `solids` are read from
    /// it -- materials always come from `document`, so a material edit does not
    /// need this rebuilt and the two cannot drift.
    pub render: Option<GdmlDocument>,
    pub engine: EvalEngine,
    pub meshes: HashMap<String, TriangleMesh>,
    pub warnings: Vec<String>,
    pub file_path: String,
}

impl LoadedDocument {
    /// The document geometry is built from: the expanded one when loops were
    /// present, otherwise the source.
    pub fn geometry(&self) -> &GdmlDocument {
        self.render.as_ref().unwrap_or(&self.document)
    }
}

pub type SharedState = Arc<RwLock<AppState>>;

pub struct AppState {
    pub loaded: Option<LoadedDocument>,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        Self { loaded: None }
    }
}

pub fn create_shared_state() -> SharedState {
    Arc::new(RwLock::new(AppState::new()))
}
