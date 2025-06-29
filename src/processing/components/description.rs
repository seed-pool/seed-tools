// Description generation component

use serde_json::Value as JsonValue;
use std::collections::HashMap;
use crate::core::{MediaType, error::Result};
use super::{UploadComponent, ComponentResult};

pub struct DescriptionComponent {
    input_path: String,
    media_type: MediaType,
    metadata: JsonValue,
    tmdb_id: Option<u32>,
    imdb_id: Option<String>,
    mediainfo: Option<String>,
    screenshots: Vec<String>,
    thumbnails: Vec<String>,
    enriched_metadata: Option<HashMap<String, String>>,
    template_name: Option<String>,
}

impl DescriptionComponent {
    pub fn new(input_path: String, media_type: MediaType, metadata: JsonValue) -> Self {
        Self {
            input_path,
            media_type,
            metadata,
            tmdb_id: None,
            imdb_id: None,
            mediainfo: None,
            screenshots: Vec::new(),
            thumbnails: Vec::new(),
            enriched_metadata: None,
            template_name: None,
        }
    }
    
    pub fn with_tmdb_id(mut self, tmdb_id: u32) -> Self {
        self.tmdb_id = Some(tmdb_id);
        self
    }
    
    pub fn with_imdb_id(mut self, imdb_id: String) -> Self {
        self.imdb_id = Some(imdb_id);
        self
    }
    
    pub fn with_mediainfo(mut self, mediainfo: String) -> Self {
        self.mediainfo = Some(mediainfo);
        self
    }
    
    pub fn with_screenshots(mut self, screenshots: Vec<String>, thumbnails: Vec<String>) -> Self {
        self.screenshots = screenshots;
        self.thumbnails = thumbnails;
        self
    }
    
    pub fn with_enriched_metadata(mut self, enriched_metadata: HashMap<String, String>) -> Self {
        self.enriched_metadata = Some(enriched_metadata);
        self
    }
    
    pub fn with_template(mut self, template_name: String) -> Self {
        self.template_name = Some(template_name);
        self
    }
}

impl UploadComponent for DescriptionComponent {
    fn name(&self) -> &'static str {
        "Description"
    }
    
    fn process(&self) -> Result<ComponentResult> {
        // Prepare metadata for template processing
        let mut metadata = self.metadata.clone();
        
        // Add screenshots and mediainfo to metadata if available
        if !self.screenshots.is_empty() {
            metadata["screenshots"] = serde_json::json!(self.screenshots);
        }
        if !self.thumbnails.is_empty() {
            metadata["thumbnails"] = serde_json::json!(self.thumbnails);
        }
        if let Some(mediainfo) = &self.mediainfo {
            metadata["mediainfo"] = serde_json::json!(mediainfo);
        }
        
        let description = match &self.media_type {
            MediaType::Video(_) => {
                crate::media::video::generate_description_with_template(
                    &metadata,
                    self.enriched_metadata.as_ref(),
                    self.template_name.as_deref(),
                ).unwrap_or_else(|_| "Video description generation failed".to_string())
            }
            MediaType::Audio(_) => {
                crate::media::audio::generate_description_with_template(
                    &metadata,
                    self.enriched_metadata.as_ref(),
                    self.template_name.as_deref(),
                ).unwrap_or_else(|_| "Audio description generation failed".to_string())
            }
            MediaType::Ebook(_) => {
                crate::media::ebook::generate_description_with_template(
                    &metadata,
                    self.enriched_metadata.as_ref(),
                    self.template_name.as_deref(),
                ).unwrap_or_else(|_| "Ebook description generation failed".to_string())
            }
            MediaType::Game(_) => {
                crate::media::game::generate_description_with_template(
                    &metadata,
                    self.enriched_metadata.as_ref(),
                    self.template_name.as_deref(),
                ).unwrap_or_else(|_| "Game description generation failed".to_string())
            }
            MediaType::Hobby(_) => {
                crate::media::hobby::generate_description_with_template(
                    &metadata,
                    self.enriched_metadata.as_ref(),
                    self.template_name.as_deref(),
                ).unwrap_or_else(|_| "Hobby description generation failed".to_string())
            }
        };
        
        Ok(ComponentResult {
            success: true,
            data: Some(description),
            files: vec![],
            error: None,
        })
    }
}