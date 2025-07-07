// True utility functions - generic helpers

pub mod binary_manager;
pub mod duplicate_check;
pub mod fs;
pub mod http;
pub mod logging;
pub mod validation;

// Re-export commonly used utilities
pub use duplicate_check::{check_all_duplicates, check_duplicates, load_tracker_config};
pub use fs::{count_files_in_directory, filter_files_by_extension, find_and_read_nfo};
pub use http::{download_file, extract_torrent_id, upload_to_cdn, upload_to_imgbb};
pub use logging::setup_logging;
pub use validation::validate_file_path;
pub use validation::{validate_api_key, validate_url};
