use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

pub struct ApiError {
    pub status: StatusCode,
    pub message: String,
}

impl ApiError {
    pub fn not_found(msg: &str) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: msg.to_string(),
        }
    }

    pub fn bad_request(msg: &str) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: msg.to_string(),
        }
    }

    pub fn internal(msg: &str) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: msg.to_string(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = json!({ "error": self.message });
        (self.status, axum::Json(body)).into_response()
    }
}
