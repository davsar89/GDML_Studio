use axum::routing::{get, post, put};
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
        // NIST database
        .route("/api/nist/materials", get(handlers::get_nist_materials))
        .route("/api/nist/material", get(handlers::get_nist_material))
        // Material CRUD
        .route(
            "/api/document/materials/update",
            put(handlers::update_material),
        )
        .route("/api/document/materials/add", post(handlers::add_material))
        .route(
            "/api/document/materials/delete",
            post(handlers::delete_material),
        )
        // Element CRUD
        .route(
            "/api/document/elements/update",
            put(handlers::update_element),
        )
        .route("/api/document/elements/add", post(handlers::add_element))
        .route(
            "/api/document/elements/delete",
            post(handlers::delete_element),
        )
        // Volume material ref
        .route(
            "/api/document/structure/material-ref",
            put(handlers::update_volume_material_ref),
        )
        // Export
        .route("/api/document/export", post(handlers::export_gdml))
        .with_state(state)
}
