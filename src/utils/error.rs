// ======================================================================
// CUSTOM ERROR TYPES
// File: src/utils/error.rs
// Description: Custom error enum for the self-evolving LM project
// ======================================================================

use std::fmt;

#[derive(Debug)]
pub enum LmError {
    NotFound(String),
    InvalidData(String),
    ConnectionFailed(String),
    PermissionDenied(String),
    Timeout(String),
    Other(String),
}

impl fmt::Display for LmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LmError::NotFound(msg) => write!(f, "Not Found: {}", msg),
            LmError::InvalidData(msg) => write!(f, "Invalid Data: {}", msg),
            LmError::ConnectionFailed(msg) => write!(f, "Connection Failed: {}", msg),
            LmError::PermissionDenied(msg) => write!(f, "Permission Denied: {}", msg),
            LmError::Timeout(msg) => write!(f, "Timeout: {}", msg),
            LmError::Other(msg) => write!(f, "Error: {}", msg),
        }
    }
}

impl std::error::Error for LmError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

// ======================================================================
// CONVERSIONS FROM OTHER ERROR TYPES
// ======================================================================

impl From<std::io::Error> for LmError {
    fn from(err: std::io::Error) -> Self {
        LmError::Other(format!("IO Error: {}", err))
    }
}

impl From<serde_json::Error> for LmError {
    fn from(err: serde_json::Error) -> Self {
        LmError::InvalidData(format!("JSON Error: {}", err))
    }
}

impl From<reqwest::Error> for LmError {
    fn from(err: reqwest::Error) -> Self {
        LmError::ConnectionFailed(format!("HTTP Error: {}", err))
    }
}

impl From<tokio::time::error::Elapsed> for LmError {
    fn from(_err: tokio::time::error::Elapsed) -> Self {
        LmError::Timeout("Operation timed out".to_string())
    }
}

impl From<heed::Error> for LmError {
    fn from(err: heed::Error) -> Self {
        LmError::Other(format!("Database Error: {}", err))
    }
}

// ======================================================================
// TYPE ALIAS FOR RESULT
// ======================================================================

pub type LmResult<T> = Result<T, LmError>;