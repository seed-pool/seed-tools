// MediaInfo generation component

use super::{mediainfo_utils, ComponentResult, UploadComponent};
use crate::core::{error::Result, Config};

pub struct MediaInfoComponent {
    input_path: String,
    config: Config,
}

impl MediaInfoComponent {
    pub fn new(input_path: String, config: Config) -> Self {
        Self { input_path, config }
    }
}

impl UploadComponent for MediaInfoComponent {
    fn name(&self) -> &'static str {
        "MediaInfo"
    }

    fn process(&self) -> Result<ComponentResult> {
        match mediainfo_utils::generate_mediainfo(&self.input_path, &self.config) {
            Ok(mediainfo_output) => Ok(ComponentResult {
                success: true,
                data: Some(mediainfo_output),
                files: Vec::new(),
                error: None,
            }),
            Err(e) => Ok(ComponentResult {
                success: false,
                data: None,
                files: Vec::new(),
                error: Some(e.to_string()),
            }),
        }
    }
}
