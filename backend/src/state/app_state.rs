use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::eval::engine::EvalEngine;
use crate::gdml::model::GdmlDocument;
use crate::mesh::types::TriangleMesh;

pub struct LoadedDocument {
    pub document: GdmlDocument,
    pub engine: EvalEngine,
    pub meshes: HashMap<String, TriangleMesh>,
    pub warnings: Vec<String>,
    pub file_path: String,
}

pub type SharedState = Arc<RwLock<AppState>>;

pub struct AppState {
    pub loaded: Option<LoadedDocument>,
}

impl AppState {
    pub fn new() -> Self {
        Self { loaded: None }
    }
}

pub fn create_shared_state() -> SharedState {
    Arc::new(RwLock::new(AppState::new()))
}
