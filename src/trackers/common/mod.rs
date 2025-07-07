// Common functionality for all trackers

pub mod api;

pub use api::{TrackerApi, TrackerConfig, UploadData, UploadResponse};

/// Common functions for tracker operations
pub fn format_description_bbcode(description: &str) -> String {
    // Common BBCode formatting
    description.to_string()
}

/// Validate upload data before sending to tracker
pub fn validate_upload_data(data: &api::UploadData) -> Result<(), String> {
    if data.title.is_empty() {
        return Err("Title cannot be empty".to_string());
    }
    if data.category.is_empty() {
        return Err("Category cannot be empty".to_string());
    }
    if data.torrent_file.is_empty() {
        return Err("Torrent file cannot be empty".to_string());
    }
    Ok(())
}
