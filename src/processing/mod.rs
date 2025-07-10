// File processing and upload orchestration

pub mod component_config;
pub mod components;
pub mod description;
pub mod extraction;
pub mod naming;
pub mod preflight;
pub mod process_builder;
pub mod torrent;
pub mod upload;

// Re-export main processing types
pub use crate::core::PreflightCheckResult;
pub use naming::generate_release_name;
pub use preflight::preflight_check;
pub use process_builder::{ProcessBuilder, ProcessResult};
pub use upload::{UploadBuilder, UploadProcessor, UploadResult};
