// True utility functions - generic helpers

pub mod fs;
pub mod http;
pub mod logging;
pub mod duplicate_check;
pub mod validation;
pub mod binary_manager;

// Re-export commonly used utilities
pub use fs::{filter_files_by_extension, count_files_in_directory, find_and_read_nfo};
pub use validation::validate_file_path;
pub use http::{download_file, upload_to_imgbb, upload_to_cdn, extract_torrent_id};
pub use logging::setup_logging;
pub use duplicate_check::{check_duplicates, check_all_duplicates, load_tracker_config};
pub use validation::{validate_api_key, validate_url};