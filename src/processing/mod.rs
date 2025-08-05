// File processing and upload orchestration

pub mod alt_description;
pub mod component_config;
pub mod components;
pub mod description;
pub mod description_handler;
pub mod extraction;
pub mod naming;
pub mod preflight;
pub mod process_builder;
pub mod torrent;
pub mod upload;

// Re-export main processing types
pub use crate::core::PreflightCheckResult;
pub use alt_description::{create_image_description, create_image_description_with_metadata, create_image_description_with_config, generate_from_metadata, ImageDescriptionConfig};
pub use description_handler::{generate_description, generate_description_with_config, DescriptionResult, create_final_description};
pub use naming::generate_release_name;
pub use preflight::preflight_check;
pub use process_builder::{ProcessBuilder, ProcessResult};
pub use upload::{UploadBuilder, UploadProcessor, UploadResult};
