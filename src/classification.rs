use crate::types::MediaType;
use serde_json::Value as JsonValue;

/// Media classification system that coordinates classification from media modules
pub struct MediaClassification {
    media_type: Option<MediaType>,
    metadata: Option<JsonValue>,
    input_path: Option<String>,
    force_category: Option<String>,
    force_type: Option<String>,
}

/// Result of media classification including tracker mappings
#[derive(Debug, Clone)]
pub struct ClassificationResult {
    pub category: Option<String>,
    pub source_type: Option<String>,
    pub media_metadata: JsonValue,
    pub tracker_mappings: Vec<(String, String)>, // [(tracker_name, category_string)]
}

/// Trait that each media module implements for classification
pub trait MediaClassifier {
    /// Classify the media and return category and optional source type
    fn classify(&self, input_path: &str, metadata: &JsonValue) -> Result<(Option<String>, Option<String>, JsonValue), String>;
    
    /// Get the media type this classifier handles
    fn media_type(&self) -> &'static str;
}

impl MediaClassification {
    pub fn new() -> Self {
        Self {
            media_type: None,
            metadata: None,
            input_path: None,
            force_category: None,
            force_type: None,
        }
    }
    
    pub fn with_media_type(mut self, media_type: MediaType) -> Self {
        self.media_type = Some(media_type);
        self
    }
    
    pub fn with_metadata(mut self, metadata: JsonValue) -> Self {
        self.metadata = Some(metadata);
        self
    }
    
    pub fn with_input_path(mut self, path: impl Into<String>) -> Self {
        self.input_path = Some(path.into());
        self
    }
    
    pub fn with_category(mut self, category: impl Into<String>) -> Self {
        self.force_category = Some(category.into());
        self
    }
    
    pub fn with_type(mut self, type_code: impl Into<String>) -> Self {
        self.force_type = Some(type_code.into());
        self
    }
    
    /// Perform the classification
    pub fn classify(self) -> Result<ClassificationResult, String> {
        use log::{info, debug};
        
        let media_type = self.media_type.ok_or("Media type not set")?;
        let input_path = self.input_path.ok_or("Input path not set")?;
        let metadata = self.metadata.unwrap_or(JsonValue::Object(serde_json::Map::new()));
        let force_category = self.force_category.clone();
        let force_type = self.force_type.clone();
        
        info!("🔍 Starting classification for media type: {:?}", media_type);
        info!("📁 Input path: {}", input_path);
        debug!("📊 Input metadata: {}", serde_json::to_string_pretty(&metadata).unwrap_or_else(|_| "Invalid JSON".to_string()));
        
        if let Some(ref cat) = force_category {
            info!("🎯 Forced category: {}", cat);
        }
        if let Some(ref typ) = force_type {
            info!("🎯 Forced type: {}", typ);
        }
        
        // Determine category and type
        let (category, source_type, media_metadata) = if let (Some(cat), Some(typ)) = (force_category.clone(), force_type.clone()) {
            // Use forced values
            info!("✅ Using forced category and type");
            (Some(cat), Some(typ), metadata)
        } else if let Some(cat) = force_category {
            // Only category forced, determine type from media module
            info!("⚡ Category forced, auto-detecting type from media module");
            let (_, source_type, media_metadata) = Self::classify_with_media_module_static(&media_type, &input_path, &metadata)?;
            (Some(cat), source_type, media_metadata)
        } else {
            // Auto-classify using media module
            info!("🤖 Auto-classifying using media module");
            Self::classify_with_media_module_static(&media_type, &input_path, &metadata)?
        };
        
        if let Some(ref cat) = category {
            info!("📂 Determined category: {}", cat);
        }
        if let Some(ref src) = source_type {
            info!("🏷️  Determined source type: {}", src);
        }
        
        // Generate tracker mappings
        info!("🎪 Generating tracker mappings...");
        let tracker_mappings = Self::generate_tracker_mappings(&category, &source_type, &media_metadata)?;
        
        info!("✅ Classification complete! Found {} tracker mappings:", tracker_mappings.len());
        for (tracker, mapping) in &tracker_mappings {
            info!("  📍 {}: {}", tracker, mapping);
        }
        
        Ok(ClassificationResult {
            category,
            source_type,
            media_metadata,
            tracker_mappings,
        })
    }
    
    /// Classify using the appropriate media module
    fn classify_with_media_module_static(
        media_type: &MediaType,
        input_path: &str,
        metadata: &JsonValue,
    ) -> Result<(Option<String>, Option<String>, JsonValue), String> {
        match media_type {
            MediaType::Video(_) => {
                // Call video module's classification
                crate::media::video::classify_for_upload(input_path, metadata)
            }
            MediaType::Audio(_) => {
                // Call audio module's classification
                crate::media::audio::classify_for_upload(input_path, metadata)
            }
            MediaType::Ebook(_) => {
                // Call ebook module's classification
                crate::media::ebook::classify_for_upload(input_path, metadata)
            }
            MediaType::Game(_) => {
                // Call game module's classification
                crate::media::game::classify_for_upload(input_path, metadata)
            }
            MediaType::Hobby(_) => {
                // Call hobby module's classification
                crate::media::hobby::classify_for_upload(input_path, metadata)
            }
        }
    }
    
    /// Generate tracker-specific mappings
    fn generate_tracker_mappings(
        category: &Option<String>,
        source_type: &Option<String>,
        metadata: &JsonValue,
    ) -> Result<Vec<(String, String)>, String> {
        let mut mappings = Vec::new();
        
        // Generate Seedpool mapping
        if let Some(cat) = category {
            use crate::definitions::seedpool::create_torrent_info_from_media_strings;
            
            if let Ok(torrent_info) = create_torrent_info_from_media_strings(
                Some(cat),
                source_type.as_deref()
            ) {
                mappings.push((
                    "Seedpool".to_string(),
                    format!("{} ({}) → {} ({})", 
                        torrent_info.category.name(), 
                        torrent_info.category_code(),
                        torrent_info.torrent_type.name(),
                        torrent_info.type_code()
                    )
                ));
            }
        }
        
        // Generate TorrentLeech mapping for video
        if let Some(cat) = category {
            if cat.contains("VideoCategory") {
                if cat.contains("TvShow") {
                    mappings.push((
                        "TorrentLeech".to_string(),
                        "TV Shows".to_string()
                    ));
                } else if cat.contains("Movie") {
                    // Check resolution from metadata
                    let resolution = metadata.get("resolution")
                        .and_then(|r| r.as_str())
                        .unwrap_or("");
                    let is_4k = resolution.contains("2160") || resolution.contains("4K");
                    
                    mappings.push((
                        "TorrentLeech".to_string(),
                        if is_4k { "Movies/4K" } else { "Movies/HD" }.to_string()
                    ));
                }
            }
        }
        
        // TODO: Add other tracker mappings as needed
        
        Ok(mappings)
    }
}