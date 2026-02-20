use axum::routing::{get, post};
use axum::Router;

use super::handlers;
use crate::state::app_state::SharedState;

pub fn create_router(state: SharedState) -> Router {
    Router::new()
        .route("/api/files/upload", post(handlers::upload_file))
        .route("/api/files/upload-multi", post(handlers::upload_files))
        .route("/api/document/summary", get(handlers::get_summary))
        .route("/api/document/meshes", get(handlers::get_meshes))
        .route("/api/document/defines", get(handlers::get_defines))
        .route("/api/document/materials", get(handlers::get_materials))
        .route("/api/document/solids", get(handlers::get_solids))
        .route("/api/document/structure", get(handlers::get_structure))
        .with_state(state)
}
