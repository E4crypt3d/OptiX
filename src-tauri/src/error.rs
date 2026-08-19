use serde::Serialize;

/// Central error type for the backend. Serialized to the frontend so the UI
/// can render a human-readable message for every failure mode.
// All variants are part of the stable backend error surface; some are only
// constructed by later phases (elevation, Windows APIs, permissions).
#[allow(dead_code)]
#[derive(Debug, thiserror::Error, Serialize)]
#[serde(tag = "kind", content = "message")]
pub enum OptixError {
    #[error("database error: {0}")]
    Database(String),

    #[error("unsupported platform: {0}")]
    UnsupportedPlatform(String),

    #[error("administrator privileges are required: {0}")]
    NotElevated(String),

    #[error("Windows API error: {0}")]
    Windows(String),

    #[error("I/O error: {0}")]
    Io(String),

    #[error("invalid state: {0}")]
    InvalidState(String),

    #[error("operation not permitted: {0}")]
    NotPermitted(String),

    #[error("{0}")]
    Other(String),
}

impl From<rusqlite::Error> for OptixError {
    fn from(e: rusqlite::Error) -> Self {
        OptixError::Database(e.to_string())
    }
}

impl From<std::io::Error> for OptixError {
    fn from(e: std::io::Error) -> Self {
        OptixError::Io(e.to_string())
    }
}

impl From<serde_json::Error> for OptixError {
    fn from(e: serde_json::Error) -> Self {
        OptixError::Other(e.to_string())
    }
}

impl From<zip::result::ZipError> for OptixError {
    fn from(e: zip::result::ZipError) -> Self {
        OptixError::Other(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, OptixError>;
