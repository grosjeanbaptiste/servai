use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

use crate::domain::errors::DomainError;

pub struct ApiError(pub DomainError);

impl From<DomainError> for ApiError {
    fn from(err: DomainError) -> Self {
        Self(err)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match &self.0 {
            DomainError::ValidationError(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            DomainError::NotFound { entity, id } => (
                StatusCode::NOT_FOUND,
                format!("{} with id '{}' not found", entity, id),
            ),
            DomainError::InvalidStateTransition { from, to } => (
                StatusCode::CONFLICT,
                format!("Invalid state transition from {} to {}", from, to),
            ),
            DomainError::Conflict(msg) => (StatusCode::CONFLICT, msg.clone()),
            DomainError::ExecutionFailed { reason, details } => {
                let detail_str = details
                    .as_ref()
                    .map(|d| format!(": {}", d))
                    .unwrap_or_default();
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("{}{}", reason, detail_str),
                )
            }
            DomainError::VcsError(msg) => (
                StatusCode::BAD_GATEWAY,
                format!("VCS Error: {}", msg),
            ),
        };

        let body = Json(json!({
            "error": message
        }));

        (status, body).into_response()
    }
}
