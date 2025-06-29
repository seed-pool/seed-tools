// Upload components - modular pieces for building uploads

pub mod title;
pub mod mediainfo;
pub mod screenshots;
pub mod description;
pub mod sample;
pub mod nfo;

// Utility modules for components
pub mod screenshot_utils;
pub mod mediainfo_utils;

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