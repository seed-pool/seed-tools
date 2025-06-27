use crate::{
    types::{Config, MediaType, PreflightCheckResult},
    media::{detector::detect_media_type, video::UploadData},
    classification::{MediaClassification, ClassificationResult},
    upload::UploadBuilder,
    description::DescriptionConfig,
};
use std::sync::Arc;
use serde_json::Value as JsonValue;
use log::{debug, info, error};

/// Builder for creating custom media processing pipelines
pub struct ProcessBuilder {
    input_path: String,
    config: Arc<Config>,
    
    // Detection settings
    force_media_type: Option<MediaType>,
    auto_detect: bool,
    
    // Components to include
    include_classification: bool,
    include_upload_builder: bool,
    include_upload_processing: bool,
    include_duplicate_check: bool,
    include_metadata_extraction: bool,
    include_preflight_data: bool,
    
    // Component configurations
    description_config: Option<DescriptionConfig>,
    dry_run: bool,
    
    // Classification overrides
    force_category: Option<String>,
    force_type: Option<String>,
}

/// Result of the process pipeline
#[derive(Debug)]
pub struct ProcessResult {
    pub media_type: MediaType,
    pub title: String,
    pub metadata: JsonValue,
    pub classification: Option<ClassificationResult>,
    pub upload_data: Option<UploadData>,
    pub preflight_data: Option<PreflightCheckResult>,
    pub upload_result: Option<crate::upload::UploadResult>,
}

impl ProcessBuilder {
    /// Create a new process builder
    pub fn new(input_path: impl Into<String>, config: Arc<Config>) -> Self {
        Self {
            input_path: input_path.into(),
            config,
            force_media_type: None,
            auto_detect: true,
            include_classification: true,
            include_upload_builder: false,
            include_upload_processing: false,
            include_duplicate_check: false,
            include_metadata_extraction: true,
            include_preflight_data: false,
            description_config: None,
            dry_run: false,
            force_category: None,
            force_type: None,
        }
    }
    
    /// Force a specific media type instead of auto-detecting
    pub fn force_media_type(mut self, media_type: MediaType) -> Self {
        self.force_media_type = Some(media_type);
        self.auto_detect = false;
        self
    }
    
    /// Enable/disable auto-detection (enabled by default)
    pub fn auto_detect(mut self, enabled: bool) -> Self {
        self.auto_detect = enabled;
        if enabled {
            self.force_media_type = None;
        }
        self
    }
    
    /// Include classification in the pipeline
    pub fn with_classification(mut self, enabled: bool) -> Self {
        self.include_classification = enabled;
        self
    }
    
    /// Include upload builder in the pipeline
    pub fn with_upload_builder(mut self, enabled: bool) -> Self {
        self.include_upload_builder = enabled;
        self
    }
    
    /// Include upload processing in the pipeline
    pub fn with_upload_processing(mut self, enabled: bool) -> Self {
        self.include_upload_processing = enabled;
        self
    }
    
    /// Include duplicate checking
    pub fn with_duplicate_check(mut self, enabled: bool) -> Self {
        self.include_duplicate_check = enabled;
        self
    }
    
    /// Include metadata extraction
    pub fn with_metadata_extraction(mut self, enabled: bool) -> Self {
        self.include_metadata_extraction = enabled;
        self
    }
    
    /// Include preflight data generation
    pub fn with_preflight_data(mut self, enabled: bool) -> Self {
        self.include_preflight_data = enabled;
        self
    }
    
    /// Set description configuration
    pub fn with_description_config(mut self, config: DescriptionConfig) -> Self {
        self.description_config = Some(config);
        self
    }
    
    /// Set dry run mode
    pub fn dry_run(mut self, enabled: bool) -> Self {
        self.dry_run = enabled;
        self
    }
    
    /// Force a specific category (overrides classification)
    pub fn force_category(mut self, category: impl Into<String>) -> Self {
        self.force_category = Some(category.into());
        self
    }
    
    /// Force a specific type (overrides classification)
    pub fn force_type(mut self, type_code: impl Into<String>) -> Self {
        self.force_type = Some(type_code.into());
        self
    }
    
    /// Build and execute the processing pipeline
    pub fn build(self) -> Result<ProcessResult, String> {
        use log::debug;
        
        debug!("ProcessBuilder: Starting build for path: {}", self.input_path);
        
        // Step 1: Determine media type
        let (media_type, raw_metadata) = if let Some(forced_type) = self.force_media_type.clone() {
            // Use forced media type
            (forced_type, JsonValue::Object(serde_json::Map::new()))
        } else if self.auto_detect {
            // Auto-detect media type
            let media_files = detect_media_type(&self.input_path)?;
            if let Some(first_file) = media_files.first() {
                // Take the first detected media file
                let media_type = first_file.media_type.clone();
                let metadata = JsonValue::Object(serde_json::Map::new());
                (media_type, metadata)
            } else {
                return Err("No media files detected in the input path".to_string());
            }
        } else {
            return Err("No media type specified and auto-detect is disabled".to_string());
        };
        
        debug!("ProcessBuilder: Detected media type: {:?}", media_type);
        
        // Step 2: Extract media-specific metadata if enabled
        let mut metadata = raw_metadata;
        if self.include_metadata_extraction {
            debug!("ProcessBuilder: Extracting metadata");
            metadata = self.extract_metadata(&media_type, metadata)?;
        }
        
        // Step 3: Run classification if enabled
        let classification = if self.include_classification {
            debug!("ProcessBuilder: Running classification");
            Some(self.classify_media(&media_type, &metadata)?)
        } else {
            None
        };

        // Step 4: Build upload data if enabled
        let upload_data = if self.include_upload_builder {
            Some(self.build_upload_data(&media_type, &metadata, &classification)?)
        } else {
            None
        };
        
        // Step 5: Generate preflight data if enabled
        let preflight_data = if self.include_preflight_data {
            debug!("ProcessBuilder: Generating preflight data");
            Some(self.generate_preflight_data(&media_type, &metadata, &classification)?)
        } else {
            None
        };
        
        // Step 6: Process upload if enabled
        let upload_result = if self.include_upload_processing {
            if let Some(upload_data) = &upload_data {
                debug!("ProcessBuilder: Processing upload");
                Some(self.process_upload(upload_data, &classification)?)
            } else {
                return Err("Upload processing enabled but no upload data available".to_string());
            }
        } else {
            None
        };
        
        // Extract title from metadata
        let title = metadata.get("title")
            .and_then(|t| t.as_str())
            .unwrap_or("Unknown")
            .to_string();
        
        Ok(ProcessResult {
            media_type,
            title,
            metadata,
            classification,
            upload_data,
            preflight_data,
            upload_result,
        })
    }
    
    /// Extract metadata based on media type
    fn extract_metadata(&self, _media_type: &MediaType, metadata: JsonValue) -> Result<JsonValue, String> {
        // For now, just return the metadata as-is
        // The actual metadata extraction happens during detection
        // This could be expanded later to do additional metadata enrichment
        Ok(metadata)
    }
    
    /// Classify media using the classification system
    fn classify_media(&self, media_type: &MediaType, metadata: &JsonValue) -> Result<ClassificationResult, String> {
        let mut classification = MediaClassification::new();
        
        // Apply forced category/type if specified
        if let Some(category) = &self.force_category {
            classification = classification.with_category(category);
        }
        if let Some(type_code) = &self.force_type {
            classification = classification.with_type(type_code);
        }
        
        // Run classification
        classification
            .with_media_type(media_type.clone())
            .with_metadata(metadata.clone())
            .with_input_path(&self.input_path)
            .classify()
    }
    
    /// Build upload data if needed
    fn build_upload_data(
        &self,
        media_type: &MediaType,
        _metadata: &JsonValue,
        _classification: &Option<ClassificationResult>,
    ) -> Result<UploadData, String> {
        let mut builder = UploadBuilder::new(&self.input_path, media_type.clone(), self.config.clone());
        
        // Apply description config if provided
        if let Some(desc_config) = &self.description_config {
            builder = builder.with_description_config(desc_config.clone());
        }
        
        // Classification is handled separately and doesn't need to be passed to UploadBuilder
        
        // Add common components
        if self.include_duplicate_check {
            builder = builder.with_duplicate_check();
        }
        
        builder = builder.dry_run(self.dry_run);
        
        // Build based on media type
        match media_type {
            MediaType::Video(_) => {
                builder = builder
                    .with_screenshots(4)
                    .with_mediainfo()
                    .with_nfo();
            }
            MediaType::Audio(_) => {
                builder = builder
                    .with_mediainfo()
                    .with_cover_art();
            }
            MediaType::Ebook(_) => {
                builder = builder
                    .with_cover_art()
                    .with_nfo();
            }
            MediaType::Game(_) => {
                builder = builder
                    .with_screenshots(4)
                    .with_nfo();
            }
            MediaType::Hobby(_) => {
                builder = builder.with_nfo();
            }
        }
        
        builder.build()
    }
    
    /// Process the upload using UploadProcessor
    fn process_upload(
        &self,
        upload_data: &UploadData,
        classification: &Option<ClassificationResult>,
    ) -> Result<crate::upload::UploadResult, String> {
        use crate::upload::UploadProcessor;
        
        let mut processor = UploadProcessor::new(
            upload_data.clone(),
            self.config.clone(),
        )
        .dry_run(self.dry_run);
        
        // Add classification if available
        if let Some(classification) = classification {
            if let Some(category) = &classification.category {
                processor = processor.with_media_classification(
                    Some(category.clone()),
                    classification.source_type.clone(),
                );
            }
        }
        
        processor.process()
    }
    
    /// Generate preflight check data
    fn generate_preflight_data(
        &self,
        media_type: &MediaType,
        metadata: &JsonValue,
        classification: &Option<ClassificationResult>,
    ) -> Result<PreflightCheckResult, String> {
        use crate::utils::{fetch_tmdb_id, fetch_external_ids, generate_mediainfo, filter_files_by_extension};
        use crate::naming::generate_release_name;
        use std::path::Path;
        use log::debug;
        
        // Use metadata from classification if available, otherwise fall back to passed metadata
        let effective_metadata = if let Some(classification) = classification {
            &classification.media_metadata
        } else {
            metadata
        };
        
        let title = effective_metadata.get("title").and_then(|t| t.as_str()).unwrap_or("Unknown").to_string();
        
        // Generate release name
        let base_name = Path::new(&self.input_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let generated_release_name = generate_release_name(&base_name);
        
        // Format release type with emojis
        let release_type = match media_type {
            crate::types::MediaType::Video(_) => {
                let category = effective_metadata["category"].as_str().unwrap_or("");
                if category.contains("TvShow") {
                    "📺 TV Show".to_string()
                } else {
                    "🎥 Movie".to_string()
                }
            }
            crate::types::MediaType::Audio(atype) => {
                format!("🎧 {}", format!("{:?}", atype).to_uppercase())
            }
            crate::types::MediaType::Ebook(_) => "📚 Ebook".to_string(),
            crate::types::MediaType::Game(_) => "🎮 Game".to_string(),
            crate::types::MediaType::Hobby(_) => "🎨 Hobby".to_string(),
        };
        
        let mut result = PreflightCheckResult {
            release_name: title.clone(),
            generated_release_name,
            dupe_check: "Not checked".to_string(),
            tmdb_id: 0,
            imdb_id: None,
            tvdb_id: None,
            excluded_files: "N/A".to_string(),
            album_cover: "N/A".to_string(),
            audio_languages: Vec::new(),
            release_type,
            season_number: effective_metadata.get("season").and_then(|s| s.as_u64()).map(|n| n as u32),
            episode_number: effective_metadata.get("episode").and_then(|e| e.as_u64()).map(|n| n as u32),
            is_boxset: effective_metadata.get("is_boxset").and_then(|b| b.as_bool()).unwrap_or(false),
            tracker_categories: Vec::new(),
            // Initialize IGDB fields
            igdb_id: None,
            igdb_genres: None,
            igdb_developer: None,
            igdb_publisher: None,
            igdb_rating: None,
            igdb_summary: None,
            igdb_platforms: None,
        };
        
        // Add tracker categories from classification
        if let Some(classification) = classification {
            result.tracker_categories = classification.tracker_mappings.clone();
        }
        
        // Run duplicate check if enabled
        if self.include_duplicate_check {
            match crate::utils::check_all_duplicates(&result.release_name) {
                Ok(duplicates) if !duplicates.is_empty() => {
                    let dupe_list = duplicates.iter()
                        .map(|(tracker, _)| tracker.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    result.dupe_check = format!("FAIL (found on: {})", dupe_list);
                }
                _ => result.dupe_check = "✔️ PASS".to_string(),
            }
        }
        
        // Media-specific enrichment
        match media_type {
            crate::types::MediaType::Video(_) => {
                // Fetch TMDB info for movies and TV shows
                let category = effective_metadata["category"].as_str().unwrap_or("");
                let is_movie_or_tv = category.contains("Movie") || category.contains("TvShow");
                
                if is_movie_or_tv && !self.config.general.tmdb_api_key.is_empty() {
                    let year = effective_metadata["year"].as_u64().map(|y| y.to_string());
                    let release_type = if category.contains("Movie") { "movie" } else { "tv" };
                    
                    match fetch_tmdb_id(&title, year, &self.config.general.tmdb_api_key, release_type) {
                        Ok(tmdb_id) => {
                            result.tmdb_id = tmdb_id;
                            
                            // Fetch external IDs (IMDb, TVDB) from TMDB
                            if tmdb_id > 0 {
                                match fetch_external_ids(tmdb_id, release_type, &self.config.general.tmdb_api_key) {
                                    Ok((imdb_id, tvdb_id)) => {
                                        result.imdb_id = imdb_id;
                                        result.tvdb_id = tvdb_id;
                                    }
                                    Err(e) => {
                                        debug!("Failed to fetch external IDs: {}", e);
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            debug!("Failed to fetch TMDB ID: {}", e);
                        }
                    }
                }
                
                // Extract audio languages from mediainfo
                let video_extensions = crate::types::VideoType::all_extensions();
                if let Ok(files) = filter_files_by_extension(&self.input_path, &video_extensions) {
                    if let Some(first_file) = files.first() {
                        if let Some(file_path) = first_file.to_str() {
                            match generate_mediainfo(file_path, &self.config) {
                                Ok(mediainfo_output) => {
                                    result.audio_languages = Self::extract_audio_languages(&mediainfo_output);
                                    debug!("Extracted {} audio language(s) from mediainfo", result.audio_languages.len());
                                }
                                Err(e) => {
                                    debug!("Failed to generate mediainfo: {}", e);
                                }
                            }
                        }
                    }
                }
            }
            crate::types::MediaType::Audio(_) => {
                // Check for album cover
                result.album_cover = Self::check_for_cover_image(&self.input_path);
                
                // Use the format from metadata
                if let Some(format) = effective_metadata["format"].as_str() {
                    result.audio_languages = vec![format.to_string()];
                }
            }
            crate::types::MediaType::Ebook(_) => {
                // Check for cover image
                result.album_cover = Self::check_for_cover_image(&self.input_path);
            }
            crate::types::MediaType::Game(_) => {
                // IGDB lookup for games if credentials are available
                info!("Checking IGDB for game: {}", title);
                if !self.config.general.igdb_client_id.is_empty() && !self.config.general.igdb_bearer_token.is_empty() {
                    info!("IGDB credentials found, searching for game...");
                    // Search for the game on IGDB
                    match crate::utils::search_igdb_game(
                        &title,
                        &self.config.general.igdb_client_id,
                        &self.config.general.igdb_bearer_token,
                        &self.config
                    ) {
                        Ok(games) if !games.is_empty() => {
                            info!("Found {} games on IGDB", games.len());
                            // Take the first result
                            if let Some(game) = games.first() {
                                // Store IGDB ID in metadata (similar to TMDB ID)
                                if let Some(igdb_id) = game["id"].as_u64() {
                                    info!("Found IGDB ID: {} for game: {}", igdb_id, title);
                                    
                                    // Get detailed game information
                                    match crate::utils::get_igdb_game_details(
                                        igdb_id,
                                        &self.config.general.igdb_client_id,
                                        &self.config.general.igdb_bearer_token
                                    ) {
                                        Ok(details) => {
                                            // Store IGDB ID
                                            result.igdb_id = Some(igdb_id);
                                            
                                            // Extract cover image URL
                                            if let Some(cover) = details.get("cover") {
                                                if crate::utils::extract_igdb_cover_url(cover).is_some() {
                                                    result.album_cover = "Available (IGDB)".to_string();
                                                }
                                            }
                                            
                                            // Extract platforms
                                            if let Some(platforms) = details["platforms"].as_array() {
                                                let platform_names: Vec<String> = platforms.iter()
                                                    .filter_map(|p| p["name"].as_str())
                                                    .map(|s| s.to_string())
                                                    .collect();
                                                if !platform_names.is_empty() {
                                                    result.igdb_platforms = Some(platform_names.clone());
                                                    // Also keep in audio_languages for backward compatibility
                                                    result.audio_languages = platform_names;
                                                }
                                            }
                                            
                                            // Extract genres
                                            if let Some(genres) = details["genres"].as_array() {
                                                let genre_names: Vec<String> = genres.iter()
                                                    .filter_map(|g| g["name"].as_str())
                                                    .map(|s| s.to_string())
                                                    .collect();
                                                if !genre_names.is_empty() {
                                                    result.igdb_genres = Some(genre_names.join(", "));
                                                }
                                            }
                                            
                                            // Extract developer/publisher from involved_companies
                                            if let Some(companies) = details.get("involved_companies") {
                                                let (developers, publishers) = crate::utils::extract_igdb_companies(companies);
                                                if !developers.is_empty() {
                                                    result.igdb_developer = Some(developers.join(", "));
                                                }
                                                if !publishers.is_empty() {
                                                    result.igdb_publisher = Some(publishers.join(", "));
                                                }
                                            }
                                            
                                            // Extract rating
                                            if let Some(rating) = details["total_rating"].as_f64() {
                                                result.igdb_rating = Some(rating);
                                                info!("IGDB Rating: {}/100", rating);
                                            }
                                            
                                            // Extract summary (truncate if too long for display)
                                            if let Some(summary) = details["summary"].as_str() {
                                                let truncated = if summary.len() > 200 {
                                                    format!("{}...", &summary[..197])
                                                } else {
                                                    summary.to_string()
                                                };
                                                info!("IGDB Summary: {}", &truncated);
                                                result.igdb_summary = Some(truncated);
                                            }
                                            
                                            info!("Successfully fetched IGDB game details for ID: {}", igdb_id);
                                            info!("IGDB data populated - Genres: {:?}, Developer: {:?}, Publisher: {:?}", 
                                                  result.igdb_genres, result.igdb_developer, result.igdb_publisher);
                                        }
                                        Err(e) => {
                                            debug!("Failed to fetch IGDB game details: {}", e);
                                        }
                                    }
                                }
                            }
                        }
                        Ok(_) => {
                            info!("No games found on IGDB for: {}", title);
                        }
                        Err(e) => {
                            error!("Failed to search IGDB: {}", e);
                        }
                    }
                } else {
                    debug!("IGDB credentials not configured, skipping game lookup");
                }
                
                // Fallback: Check for local cover image
                if result.album_cover == "N/A" {
                    result.album_cover = Self::check_for_cover_image(&self.input_path);
                }
            }
            _ => {}
        }
        
        Ok(result)
    }
    
    /// Extract audio languages from mediainfo output
    fn extract_audio_languages(mediainfo_output: &str) -> Vec<String> {
        use std::collections::HashMap;
        
        let mut audio_languages: Vec<String> = Vec::new();
        let mut in_audio_section = false;
        let mut current_audio_track: HashMap<String, String> = HashMap::new();

        for line in mediainfo_output.lines() {
            let trimmed = line.trim();
            
            // Check if we're entering an Audio section
            if trimmed.starts_with("Audio") {
                // Save previous audio track if it had a language
                if let Some(lang) = current_audio_track.get("Language") {
                    if !audio_languages.contains(lang) {
                        audio_languages.push(lang.clone());
                    }
                }
                current_audio_track.clear();
                in_audio_section = true;
            } else if trimmed.is_empty() || trimmed.starts_with("Text") || trimmed.starts_with("Menu") {
                // We've left the audio section
                if in_audio_section {
                    // Save the last audio track
                    if let Some(lang) = current_audio_track.get("Language") {
                        if !audio_languages.contains(lang) {
                            audio_languages.push(lang.clone());
                        }
                    }
                    current_audio_track.clear();
                    in_audio_section = false;
                }
            }

            if in_audio_section && trimmed.contains(':') {
                let parts: Vec<&str> = trimmed.splitn(2, ':').collect();
                if parts.len() == 2 {
                    let key = parts[0].trim();
                    let value = parts[1].trim();
                    
                    if key == "Language" {
                        // Handle various language formats
                        let lang = if value.contains('/') {
                            // Handle "English / English" format
                            value.split('/').next().unwrap_or(value).trim()
                        } else {
                            value
                        };
                        current_audio_track.insert("Language".to_string(), lang.to_string());
                    }
                }
            }
        }

        // Don't forget the last audio track
        if in_audio_section {
            if let Some(lang) = current_audio_track.get("Language") {
                if !audio_languages.contains(lang) {
                    audio_languages.push(lang.clone());
                }
            }
        }

        // If no languages found, check for a simpler format
        if audio_languages.is_empty() {
            for line in mediainfo_output.lines() {
                if line.contains("Language") && line.contains(':') {
                    if let Some(lang_part) = line.split(':').nth(1) {
                        let lang = lang_part.trim().split('/').next().unwrap_or(lang_part.trim());
                        if !lang.is_empty() && !audio_languages.contains(&lang.to_string()) {
                            audio_languages.push(lang.to_string());
                        }
                    }
                }
            }
        }

        // Return "Unknown" if no languages were found
        if audio_languages.is_empty() {
            vec!["Unknown".to_string()]
        } else {
            audio_languages
        }
    }
    
    /// Check if cover image exists in the path
    fn check_for_cover_image(input_path: &str) -> String {
        use walkdir::WalkDir;
        
        let has_cover = WalkDir::new(input_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .any(|entry| {
                if let Some(ext) = entry.path().extension().and_then(|e| e.to_str()) {
                    matches!(ext.to_lowercase().as_str(), "jpg" | "jpeg" | "png")
                } else {
                    false
                }
            });
        
        if has_cover {
            "Available".to_string()
        } else {
            "Not Available".to_string()
        }
    }
}

/// Create a process builder configured for preflight checks
pub fn preflight_builder(input_path: &str, config: Arc<Config>) -> ProcessBuilder {
    ProcessBuilder::new(input_path, config)
        .with_classification(true)
        .with_upload_builder(false)
        .with_duplicate_check(true)
        .with_metadata_extraction(true)
        .with_preflight_data(true)
        .dry_run(true)
}

/// Create a process builder configured for full upload
pub fn upload_builder(input_path: &str, config: Arc<Config>) -> ProcessBuilder {
    ProcessBuilder::new(input_path, config)
        .with_classification(true)
        .with_upload_builder(true)
        .with_upload_processing(true)
        .with_duplicate_check(true)
        .with_metadata_extraction(true)
        .with_preflight_data(false)
        .dry_run(false)
}

/// Create a process builder configured for sync operations
pub fn sync_builder(input_path: &str, config: Arc<Config>) -> ProcessBuilder {
    ProcessBuilder::new(input_path, config)
        .with_classification(true)
        .with_upload_builder(false)
        .with_duplicate_check(true)
        .with_metadata_extraction(false)
        .with_preflight_data(false)
        .dry_run(false)
}

/// Create a process builder configured for duplicate checking only
pub fn duplicate_check_builder(input_path: &str, config: Arc<Config>) -> ProcessBuilder {
    ProcessBuilder::new(input_path, config)
        .with_classification(true)
        .with_upload_builder(false)
        .with_duplicate_check(true)
        .with_metadata_extraction(true)
        .with_preflight_data(true)  // Enable preflight data to get dupe check results
        .dry_run(true)
}