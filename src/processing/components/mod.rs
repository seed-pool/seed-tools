// Upload components - modular pieces for building uploads

pub mod description;
pub mod mediainfo;
pub mod nfo;
pub mod sample;
pub mod screenshots;
pub mod title;

// Utility modules for components
pub mod cover_art_utils;
pub mod mediainfo_utils;
pub mod screenshot_utils;

use crate::core::error::Result;

/// Common trait for upload components
pub trait UploadComponent {
    /// Component name
    fn name(&self) -> &'static str;

    /// Process the component
    fn process(&self) -> Result<ComponentResult>;

    /// Is this component required?
    fn required(&self) -> bool {
        false
    }
}

/// Result from processing a component
pub struct ComponentResult {
    pub success: bool,
    pub data: Option<String>,
    pub files: Vec<String>,
    pub error: Option<String>,
}
