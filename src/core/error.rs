// Common error types for seedbrr

use std::fmt;
use std::error::Error;

/// Main error type for seedbrr
#[derive(Debug)]
pub enum SeedError {
    // Configuration errors
    Config(String),
    
    // IO errors
    Io(std::io::Error),
    
    // Media detection errors
    MediaDetection(String),
    
    // Classification errors
    Classification(String),
    
    // API errors
    ApiError(String),
    
    // Upload errors
    Upload(String),
    
    // Torrent client errors
    ClientError(String),
    
    // Validation errors
    Validation(String),
    
    // Parse errors
    Parse(String),
    
    // Generic errors
    Other(String),
}

impl fmt::Display for SeedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SeedError::Config(msg) => write!(f, "Configuration error: {}", msg),
            SeedError::Io(err) => write!(f, "IO error: {}", err),
            SeedError::MediaDetection(msg) => write!(f, "Media detection error: {}", msg),
            SeedError::Classification(msg) => write!(f, "Classification error: {}", msg),
            SeedError::ApiError(msg) => write!(f, "API error: {}", msg),
            SeedError::Upload(msg) => write!(f, "Upload error: {}", msg),
            SeedError::ClientError(msg) => write!(f, "Client error: {}", msg),
            SeedError::Validation(msg) => write!(f, "Validation error: {}", msg),
            SeedError::Parse(msg) => write!(f, "Parse error: {}", msg),
            SeedError::Other(msg) => write!(f, "Error: {}", msg),
        }
    }
}

impl Error for SeedError {}

impl From<std::io::Error> for SeedError {
    fn from(err: std::io::Error) -> Self {
        SeedError::Io(err)
    }
}

impl From<String> for SeedError {
    fn from(err: String) -> Self {
        SeedError::Other(err)
    }
}

impl From<&str> for SeedError {
    fn from(err: &str) -> Self {
        SeedError::Other(err.to_string())
    }
}

/// Result type alias for seedbrr
pub type Result<T> = std::result::Result<T, SeedError>;