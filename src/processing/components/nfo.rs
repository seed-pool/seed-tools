// NFO file handling component

use crate::core::error::Result;
use crate::utils::fs::find_and_read_nfo;
use super::{UploadComponent, ComponentResult};

pub struct NfoComponent {
    working_path: String,
}

impl NfoComponent {
    pub fn new(working_path: String) -> Self {
        Self { working_path }
    }
}

impl UploadComponent for NfoComponent {
    fn name(&self) -> &'static str {
        "NFO"
    }
    
    fn process(&self) -> Result<ComponentResult> {
        match find_and_read_nfo(&self.working_path)? {
            Some((nfo_path, _content)) => {
                Ok(ComponentResult {
                    success: true,
                    data: Some(nfo_path.clone()),
                    files: vec![nfo_path],
                    error: None,
                })
            }
            None => {
                Ok(ComponentResult {
                    success: true,
                    data: None,
                    files: Vec::new(),
                    error: Some("No NFO file found".to_string()),
                })
            }
        }
    }
}