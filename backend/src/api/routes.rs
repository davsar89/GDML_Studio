use axum::extract::Request;
use axum::http::{header, uri::Authority, StatusCode};
use axum::middleware::{from_fn, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::Router;

use super::{errors::ApiError, handlers};
use crate::state::app_state::SharedState;

/// Refuse a request addressed to any host but this machine. After DNS
/// rebinding, a page on another site reaches 127.0.0.1 as its own origin, so
/// CORS never applies; its requests still name that site in Host.
async fn require_local_host(req: Request, next: Next) -> Response {
    let host = req
        .headers()
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.parse::<Authority>().ok());
    if host.is_some_and(|a| {
        ["localhost", "127.0.0.1", "[::1]"]
            .iter()
            .any(|local| a.host().eq_ignore_ascii_case(local))
    }) {
        return next.run(req).await;
    }
    ApiError {
        status: StatusCode::FORBIDDEN,
        message: "Only requests addressed to localhost are served".to_string(),
    }
    .into_response()
}

pub fn create_router(state: SharedState) -> Router {
    Router::new()
        .route("/api/files/upload-multi", post(handlers::upload_files))
        .route("/api/document/summary", get(handlers::get_summary))
        .route("/api/document/meshes", get(handlers::get_meshes))
        .route("/api/document/scene", get(handlers::get_scene))
        .route("/api/document/defines", get(handlers::get_defines))
        .route("/api/document/materials", get(handlers::get_materials))
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
        .layer(from_fn(require_local_host))
        .with_state(state)
}
