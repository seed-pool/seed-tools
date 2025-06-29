// Title generation component

use crate::core::error::Result;
use crate::processing::naming::generate_release_name;
use super::{UploadComponent, ComponentResult};

pub struct TitleComponent {
    input_path: String,
    override_title: Option<String>,
}

impl TitleComponent {
    pub fn new(input_path: String) -> Self {
        Self {
            input_path,
            override_title: None,
        }
    }
    
    pub fn with_override(mut self, title: String) -> Self {
        self.override_title = Some(title);
        self
    }
}

impl UploadComponent for TitleComponent {
    fn name(&self) -> &'static str {
        "Title"
    }
    
    fn required(&self) -> bool {
        true
    }
    
    fn process(&self) -> Result<ComponentResult> {
        let title = if let Some(ref override_title) = self.override_title {
            override_title.clone()
        } else {
            // Extract filename from path
            let path = std::path::Path::new(&self.input_path);
            let filename = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Unknown");
            
            generate_release_name(filename)
        };
        
        Ok(ComponentResult {
            success: true,
            data: Some(title),
            files: Vec::new(),
            error: None,
        })
    }
}