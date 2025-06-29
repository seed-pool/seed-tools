// Sample generation component

use std::path::Path;
use crate::core::{Config, error::Result};
use super::{UploadComponent, ComponentResult};

pub struct SampleComponent {
    input_path: String,
    config: Config,
    sample_duration: u32, // Duration in seconds
}

impl SampleComponent {
    pub fn new(input_path: String, config: Config) -> Self {
        Self {
            input_path,
            config,
            sample_duration: 60, // Default 60 seconds
        }
    }
    
    pub fn with_duration(mut self, duration: u32) -> Self {
        self.sample_duration = duration;
        self
    }
}

impl UploadComponent for SampleComponent {
    fn name(&self) -> &'static str {
        "Sample"
    }
    
    fn process(&self) -> Result<ComponentResult> {
        // Check if input is a video file
        let path = Path::new(&self.input_path);
        let is_video_file = if let Some(ext) = path.extension() {
            let ext_str = ext.to_str().unwrap_or("").to_lowercase();
            matches!(ext_str.as_str(), "mp4" | "mkv" | "avi" | "mov" | "wmv" | "flv" | "webm" | "m4v" | "ts" | "mpg" | "mpeg")
        } else {
            false
        };
        
        if !is_video_file {
            return Ok(ComponentResult {
                success: true,
                data: None,
                files: Vec::new(),
                error: Some("Not a video file, skipping sample generation".to_string()),
            });
        }
        
        // Generate sample using video module
        match crate::media::video::generate_sample(
            &self.input_path, // video_file
            &self.config.paths.screenshots_dir,
            self.config.paths.cdnpaths.as_ref().and_then(|p| p.remote_path.as_ref()).unwrap_or(&"".to_string()),
            self.config.paths.cdnpaths.as_ref().and_then(|p| p.image_path.as_ref()).unwrap_or(&"".to_string()),
            &self.config.paths.ffmpeg,
            &self.input_path, // input_name/release_name
            false, // dry_run
        ) {
            Ok(sample_path) => Ok(ComponentResult {
                success: true,
                data: Some(sample_path.clone()),
                files: vec![sample_path],
                error: None,
            }),
            Err(e) => Ok(ComponentResult {
                success: false,
                data: None,
                files: Vec::new(),
                error: Some(e),
            }),
        }
    }
}