/// Media processing modules
pub mod audio;
pub mod detector;
pub mod ebook;
pub mod game;
pub mod hobby;
pub mod process;
pub mod video;

// Re-export commonly used types from process module
pub use process::{auto_detect_content_type, DetectionResult};