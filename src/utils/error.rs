use std::fmt;

#[derive(Debug)]
pub enum LmError {
    NotFound(String),
    InvalidData(String),
    ConnectionFailed(String),
    Other(String),
}

impl fmt::Display for LmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LmError::NotFound(msg) => write!(f, "Not Found: {}", msg),
            LmError::InvalidData(msg) => write!(f, "Invalid Data: {}", msg),
            LmError::ConnectionFailed(msg) => write!(f, "Connection Failed: {}", msg),
            LmError::Other(msg) => write!(f, "Error: {}", msg),
        }
    }
}

impl std::error::Error for LmError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None // Custom error type doesn't wrap other errors
    }
}

// Implementation of additional error handling traits can go here.
