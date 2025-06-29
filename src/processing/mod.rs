// File processing and upload orchestration

pub mod upload;
pub mod torrent;
pub mod naming;
pub mod process_builder;
pub mod preflight;
pub mod components;
pub mod extraction;
pub mod description;

// Re-export main processing types
pub use upload::{UploadBuilder, UploadProcessor, UploadResult};
pub use torrent::create_torrent;
pub use naming::generate_release_name;
pub use process_builder::{ProcessBuilder, ProcessResult};
pub use preflight::preflight_check;
pub use crate::core::PreflightCheckResult;