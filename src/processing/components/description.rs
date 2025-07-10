// Description generation component

use super::{ComponentResult, UploadComponent};
use crate::core::{error::Result, MediaType};
use serde_json::Value as JsonValue;
use std::collections::HashMap;

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
        use log::info;

        info!("📝 DescriptionComponent: Starting description generation");
        info!("  Media type: {:?}", self.media_type);
        info!("  Template: {:?}", self.template_name);

        if let Some(ref enriched) = self.enriched_metadata {
            info!(
                "  📊 Enriched metadata available: {} fields",
                enriched.len()
            );
            for (key, value) in enriched.iter() {
                if key.starts_with("tmdb_")
                    || key.starts_with("musicbrainz_")
                    || key == "tracklist_rows"
                {
                    let preview = if value.len() > 100 {
                        format!("{}...", &value[..100])
                    } else {
                        value.clone()
                    };
                    info!("    📌 {} = {}", key, preview);
                }
            }
        } else {
            info!("  ⚠️ No enriched metadata available");
        }

        // Prepare metadata for template processing
        let mut metadata = self.metadata.clone();

        // Extract additional metadata from mediainfo if available for audio
        if matches!(self.media_type, MediaType::Audio(_)) && self.mediainfo.is_some() {
            if let Some(mediainfo_text) = &self.mediainfo {
                // Try to extract metadata from mediainfo that might not be in enriched data
                let mut extracted_from_mediainfo = std::collections::HashMap::new();

                for line in mediainfo_text.lines() {
                    let line = line.trim();
                    if line.contains(':') {
                        let parts: Vec<&str> = line.splitn(2, ':').collect();
                        if parts.len() == 2 {
                            let key = parts[0].trim();
                            let value = parts[1].trim();

                            // Map mediainfo fields to template fields
                            match key {
                                "Album" if !line.starts_with("Album/") => {
                                    extracted_from_mediainfo
                                        .insert("mediainfo_album".to_string(), value.to_string());
                                }
                                "Performer" => {
                                    extracted_from_mediainfo
                                        .insert("mediainfo_artist".to_string(), value.to_string());
                                }
                                "Genre" => {
                                    extracted_from_mediainfo
                                        .insert("mediainfo_genre".to_string(), value.to_string());
                                }
                                "Recorded date" => {
                                    if let Some(year) = value.split('-').next() {
                                        extracted_from_mediainfo
                                            .insert("mediainfo_year".to_string(), year.to_string());
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }

                // Add extracted mediainfo fields to metadata (they can be used as fallbacks in templates)
                for (key, value) in extracted_from_mediainfo {
                    metadata[key] = serde_json::json!(value);
                }
            }
        }

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
            MediaType::Video(_) => crate::media::video::generate_description_with_template(
                &metadata,
                self.enriched_metadata.as_ref(),
                self.template_name.as_deref(),
            )
            .unwrap_or_else(|_| "Video description generation failed".to_string()),
            MediaType::Audio(_) => crate::media::audio::generate_description_with_template(
                &metadata,
                self.enriched_metadata.as_ref(),
                self.template_name.as_deref(),
            )
            .unwrap_or_else(|_| "Audio description generation failed".to_string()),
            MediaType::Ebook(_) => crate::media::ebook::generate_description_with_template(
                &metadata,
                self.enriched_metadata.as_ref(),
                self.template_name.as_deref(),
            )
            .unwrap_or_else(|_| "Ebook description generation failed".to_string()),
            MediaType::Game(_) => crate::media::game::generate_description_with_template(
                &metadata,
                self.enriched_metadata.as_ref(),
                self.template_name.as_deref(),
            )
            .unwrap_or_else(|_| "Game description generation failed".to_string()),
            MediaType::Hobby(_) => crate::media::hobby::generate_description_with_template(
                &metadata,
                self.enriched_metadata.as_ref(),
                self.template_name.as_deref(),
            )
            .unwrap_or_else(|_| "Hobby description generation failed".to_string()),
        };

        Ok(ComponentResult {
            success: true,
            data: Some(description),
            files: vec![],
            error: None,
        })
    }
}
