// seedbrr - Automated tool for processing and uploading releases to trackers
// 
// Domain-driven architecture with clear separation of concerns

// Core functionality
pub mod core;

// Media detection and classification
pub mod media;
pub mod classification;

// External metadata services
pub mod metadata;

// Tracker integrations
pub mod trackers;

// File processing and uploads
pub mod processing;

// Template system for customizable descriptions
pub mod templates;

// External service clients
pub mod clients;

// User interfaces
pub mod ui;

// Utilities
pub mod utils;

// Public API 
pub use core::{Config, MediaType, SeedError, Result};
pub use media::detector::detect_media_type;
pub use classification::{MediaClassification, ClassificationResult};
pub use processing::{ProcessBuilder, UploadBuilder, preflight_check};
pub use ui::launch_ui;

// Version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const NAME: &str = env!("CARGO_PKG_NAME");