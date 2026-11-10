use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DomainError {
    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Entity not found: {entity} with id {id}")]
    NotFound { entity: &'static str, id: String },

    #[error("Invalid state transition from {from} to {to}")]
    InvalidStateTransition { from: String, to: String },

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Execution failed: {reason}")]
    ExecutionFailed { reason: String, details: Option<String> },

    #[error("VCS / Git error: {0}")]
    VcsError(String),
}
