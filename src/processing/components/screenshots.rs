// Screenshot generation component

use std::path::Path;
use crate::core::{Config, error::Result};
use super::{UploadComponent, ComponentResult, screenshot_utils};

pub struct ScreenshotComponent {
    input_path: String,
    count: usize,
    layout: ScreenshotLayout,
    config: Config,
    release_name: String,
    dry_run: bool,
}

#[derive(Debug, Clone)]
pub enum ScreenshotLayout {
    Grid2x2,
    TwoColumn,
    SingleColumn,
}

impl ScreenshotComponent {
    pub fn new(input_path: String, release_name: String, config: Config) -> Self {
        Self {
            input_path,
            count: 4, // Default to 4 screenshots
            layout: ScreenshotLayout::Grid2x2,
            config,
            release_name,
            dry_run: false,
        }
    }
    
    pub fn with_layout(mut self, layout: ScreenshotLayout) -> Self {
        self.layout = layout;
        self
    }
    
    pub fn with_count(mut self, count: usize) -> Self {
        self.count = count;
        self
    }
    
    pub fn dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }
}

impl UploadComponent for ScreenshotComponent {
    fn name(&self) -> &'static str {
        "Screenshots"
    }
    
    fn process(&self) -> Result<ComponentResult> {
        // Check if input is a video file
        let path = Path::new(&self.input_path);
        let is_video = if let Some(ext) = path.extension() {
            let ext_str = ext.to_str().unwrap_or("").to_lowercase();
            matches!(ext_str.as_str(), "mp4" | "mkv" | "avi" | "mov" | "wmv" | "flv" | "webm" | "m4v" | "ts" | "mpg" | "mpeg")
        } else {
            false
        };
        
        if !is_video {
            return Ok(ComponentResult {
                success: true,
                data: None,
                files: Vec::new(),
                error: Some("Not a video file, skipping screenshot generation".to_string()),
            });
        }
        
        // Get video file path
        let video_file = if path.is_file() {
            self.input_path.clone()
        } else {
            // Find first video file in directory
            use crate::core::types::VideoType;
            use crate::utils::filter_files_by_extension;
            let video_extensions = VideoType::all_extensions();
            let video_files = filter_files_by_extension(&self.input_path, &video_extensions)?;
            if video_files.is_empty() {
                return Ok(ComponentResult {
                    success: false,
                    data: None,
                    files: Vec::new(),
                    error: Some("No video files found in directory".to_string()),
                });
            }
            video_files[0].to_string_lossy().to_string()
        };
        
        // Generate screenshots
        let imgbb_api_key = self.config.imgbb.as_ref().map(|c| c.imgbb_api_key.as_str());
        let remote_path = self.config.paths.cdnpaths.as_ref().and_then(|p| p.remote_path.as_ref()).map(|s| s.as_str());
        let image_path = self.config.paths.cdnpaths.as_ref().and_then(|p| p.image_path.as_ref()).map(|s| s.as_str());
        
        match screenshot_utils::generate_screenshots(
            &video_file,
            &self.config,
            imgbb_api_key,
            remote_path,
            image_path,
            &self.release_name,
            self.dry_run,
        ) {
            Ok((screenshots, thumbnails)) => {
                let screenshot_data = format!(
                    "Screenshots: {}\nThumbnails: {}",
                    screenshots.join(", "),
                    thumbnails.join(", ")
                );
                Ok(ComponentResult {
                    success: true,
                    data: Some(screenshot_data),
                    files: Vec::new(), // Files are already uploaded
                    error: None,
                })
            }
            Err(e) => Ok(ComponentResult {
                success: false,
                data: None,
                files: Vec::new(),
                error: Some(e.to_string()),
            }),
        }
    }
}