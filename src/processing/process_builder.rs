use crate::{
    classification::{ClassificationResult, MediaClassification},
    core::{Config, MediaType, PreflightCheckResult, VideoType, AudioType, EbookType, GameType, HobbyType, SeedpoolConfig, TorrentLeechConfig},
    media::{detector::detect_media_type, video::UploadData},
    processing::{
        component_config::ComponentConfig, description::DescriptionConfig, upload::{UploadBuilder, TrackerUploadExt},
    },
};
use log::{error, info};
use serde_json::Value as JsonValue;
use std::{collections::HashMap, sync::Arc};

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
    component_config: Option<ComponentConfig>,
    dry_run: bool,

    // Classification overrides
    force_category: Option<String>,
    force_type: Option<String>,
    
    // Original torrent info with preserved category/type codes
    original_torrent_info: Option<crate::trackers::seedpool::SeedpoolTorrentInfo>,

    // Preflight data reuse
    cached_preflight_data: Option<PreflightCheckResult>,
    cached_classification: Option<ClassificationResult>,
    cached_metadata: Option<JsonValue>,

    // Tracker configurations
    seedpool_config: Option<Arc<SeedpoolConfig>>,
    torrentleech_config: Option<Arc<TorrentLeechConfig>>,
}

/// Result of the process pipeline
#[derive(Debug, Clone)]
pub struct ProcessResult {
    pub media_type: MediaType,
    pub title: String,
    pub metadata: JsonValue,
    pub classification: Option<ClassificationResult>,
    pub upload_data: Option<UploadData>,
    pub preflight_data: Option<PreflightCheckResult>,
    pub upload_result: Option<crate::processing::upload::UploadResult>,
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
            component_config: None,
            dry_run: false,
            force_category: None,
            force_type: None,
            original_torrent_info: None,
            cached_preflight_data: None,
            cached_classification: None,
            cached_metadata: None,
            seedpool_config: None,
            torrentleech_config: None,
        }
    }

    /// Force a specific media type instead of auto-detecting
    pub fn force_media_type(mut self, media_type: MediaType) -> Self {
        self.force_media_type = Some(media_type);
        self.auto_detect = false;
        self
    }

    /// Manual ebook type detection by scanning files directly
    fn manual_ebook_detection(path: &str) -> Option<crate::core::EbookType> {
        use std::fs;
        use std::path::Path;
        
        let path_obj = Path::new(path);
        
        // If it's a single file, detect from extension
        if path_obj.is_file() {
            return path_obj
                .extension()
                .and_then(|ext| ext.to_str())
                .and_then(|ext| crate::core::EbookType::from_extension(ext));
        }
        
        // If it's a directory, scan for ebook files
        if let Ok(entries) = fs::read_dir(path_obj) {
            for entry in entries.flatten() {
                let entry_path = entry.path();
                if entry_path.is_file() {
                    if let Some(ebook_type) = entry_path
                        .extension()
                        .and_then(|ext| ext.to_str())
                        .and_then(|ext| crate::core::EbookType::from_extension(ext))
                    {
                        return Some(ebook_type);
                    }
                }
            }
        }
        
        None
    }

    /// Run ebook cleanup with the correct detected type
    fn run_ebook_cleanup(
        input_path: &str, 
        ebook_type: &crate::core::EbookType, 
        ebook_results: &[(crate::core::EbookFile, crate::media::ebook::EbookMetadata)]
    ) {
        match ebook_type {
            crate::core::EbookType::Pdf => {
                info!("📚 Running PDF ebook cleanup (keeping only PDF and NFO files)");
            }
            crate::core::EbookType::Epub => {
                info!("📚 Running EPUB ebook cleanup (keeping EPUB and NFO files, removing archives)");
            }
            _ => {
                info!("📚 Running {:?} ebook cleanup", ebook_type);
            }
        }
        
        if let Err(cleanup_err) = crate::media::ebook::cleanup_ebook_files(input_path, ebook_results) {
            info!("⚠️ Ebook cleanup failed: {}", cleanup_err);
        } else {
            info!("✅ Ebook cleanup completed successfully");
        }
    }

    /// Run basic ebook cleanup when processing failed
    fn run_ebook_cleanup_basic(input_path: &str, ebook_type: &crate::core::EbookType) {
        info!("📚 Running fallback cleanup for type: {:?}", ebook_type);
        // Create a dummy results vector for cleanup
        let dummy_results = vec![(
            crate::core::EbookFile {
                path: std::path::PathBuf::from(input_path),
                ebook_type: ebook_type.clone(),
            },
            crate::media::ebook::EbookMetadata::default(),
        )];
        
        if let Err(cleanup_err) = crate::media::ebook::cleanup_ebook_files(input_path, &dummy_results) {
            info!("⚠️ Fallback ebook cleanup failed: {}", cleanup_err);
        } else {
            info!("✅ Fallback ebook cleanup completed successfully");
        }
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
    
    /// Set original torrent info with preserved category/type codes
    pub fn with_original_torrent_info(mut self, torrent_info: crate::trackers::seedpool::SeedpoolTorrentInfo) -> Self {
        self.original_torrent_info = Some(torrent_info);
        self
    }

    /// Set cached preflight data to avoid recomputation
    pub fn with_cached_preflight_data(mut self, preflight_data: PreflightCheckResult) -> Self {
        self.cached_preflight_data = Some(preflight_data);
        self
    }

    /// Set cached classification result to avoid recomputation
    pub fn with_cached_classification(mut self, classification: ClassificationResult) -> Self {
        self.cached_classification = Some(classification);
        self
    }

    /// Set cached metadata to avoid recomputation
    pub fn with_cached_metadata(mut self, metadata: JsonValue) -> Self {
        self.cached_metadata = Some(metadata);
        self
    }

    /// Set component configuration
    pub fn with_component_config(mut self, config: ComponentConfig) -> Self {
        self.component_config = Some(config);
        self
    }

    /// Set seedpool configuration
    pub fn with_seedpool_config(mut self, config: Arc<SeedpoolConfig>) -> Self {
        self.seedpool_config = Some(config);
        self
    }

    /// Set torrentleech configuration
    pub fn with_torrentleech_config(mut self, config: Arc<TorrentLeechConfig>) -> Self {
        self.torrentleech_config = Some(config);
        self
    }

    /// Build and execute the processing pipeline
    pub fn build(self) -> Result<ProcessResult, String> {
        info!("ProcessBuilder: Starting build for path: {}", self.input_path);

        // Step 1: Determine media type
        let (media_type, raw_metadata) = if let Some(forced_type) = self.force_media_type.clone() {
            // Use forced media type
            (forced_type, JsonValue::Object(serde_json::Map::new()))
        } else if let Some(ref forced_category) = self.force_category {
            // Infer media type from forced category
            let media_type = if forced_category.starts_with("VideoCategory::") {
                MediaType::Video(VideoType::Mkv) // Default to MKV for video directories
            } else if forced_category.starts_with("AudioCategory::") {
                // Only use FLAC if "flac" is explicitly in the path, otherwise default to MP3
                let audio_type = if self.input_path.to_lowercase().contains("flac") {
                    AudioType::Flac
                } else {
                    AudioType::Mp3
                };
                MediaType::Audio(audio_type)  
            } else if forced_category.starts_with("EbookCategory::") {
                // For ebook categories, detect actual file type instead of guessing
                let media_files = detect_media_type(&self.input_path)?;
                if let Some(first_file) = media_files.first() {
                    if let MediaType::Ebook(ebook_type) = &first_file.media_type {
                        MediaType::Ebook(ebook_type.clone())
                    } else {
                        // If detect_media_type doesn't return an ebook, try manual detection from files
                        let detected_type = Self::manual_ebook_detection(&self.input_path);
                        
                        // Fall back to category-based guessing if manual detection fails
                        if let Some(ebook_type) = detected_type {
                            MediaType::Ebook(ebook_type)
                        } else if forced_category.contains("Comic") {
                            MediaType::Ebook(EbookType::Cbr) // Comic uses CBR
                        } else if forced_category.contains("Magazine") {
                            MediaType::Ebook(EbookType::Pdf) // Magazine uses PDF  
                        } else if forced_category.contains("E-Pub") {
                            MediaType::Ebook(EbookType::Epub) // E-Pub uses EPUB
                        } else {
                            MediaType::Ebook(EbookType::Pdf) // Default to PDF for other ebook types
                        }
                    }
                } else {
                    return Err("No ebook files detected in the input path".to_string());
                }
            } else if forced_category.starts_with("GameCategory::") {
                MediaType::Game(GameType::Directory)
            } else if forced_category.starts_with("HobbyCategory::") {
                MediaType::Hobby(HobbyType::Directory)
            } else {
                // Default to auto-detection if category format is unrecognized
                let media_files = detect_media_type(&self.input_path)?;
                if let Some(first_file) = media_files.first() {
                    first_file.media_type.clone()
                } else {
                    return Err("No media files detected in the input path".to_string());
                }
            };
            info!("ProcessBuilder: Inferred media type from forced category '{}': {:?}", forced_category, media_type);
            (media_type, JsonValue::Object(serde_json::Map::new()))
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

        info!("ProcessBuilder: Detected media type: {:?}", media_type);

        // Step 2: Extract media-specific metadata if enabled
        let mut metadata = raw_metadata;
        let mut media_type = media_type; // Make media_type mutable for correction
        if let Some(cached_metadata) = self.cached_metadata.clone() {
            info!("ProcessBuilder: Using cached metadata");
            metadata = cached_metadata;
        } else if self.include_metadata_extraction {
            info!("ProcessBuilder: Extracting metadata");
            let (extracted_metadata, corrected_media_type) = self.extract_metadata_with_type_correction(&media_type, metadata)?;
            metadata = extracted_metadata;
            // Update media type if corrected during extraction  
            media_type = corrected_media_type;
            info!(
                "ProcessBuilder: Extracted metadata: {}",
                serde_json::to_string_pretty(&metadata)
                    .unwrap_or_else(|_| "Failed to serialize".to_string())
            );
        }

        // Step 3: Run classification if enabled
        let classification = if let Some(cached_classification) = self.cached_classification.clone()
        {
            info!("ProcessBuilder: Using cached classification");
            Some(cached_classification)
        } else if self.include_classification {
            info!("ProcessBuilder: Running classification");
            Some(self.classify_media(&media_type, &metadata)?)
        } else {
            None
        };

        // Step 4: Build upload data if enabled
        let upload_data = if self.include_upload_builder {
            info!("ProcessBuilder: Building upload data - Starting UploadBuilder");
            let result = self.build_upload_data(&media_type, &metadata, &classification)?;
            info!("ProcessBuilder: Building upload data - UploadBuilder completed successfully");
            Some(result)
        } else {
            info!("ProcessBuilder: Skipping upload data build (upload_builder disabled)");
            None
        };

        // Step 5: Generate preflight data if enabled
        let preflight_data = if let Some(cached_preflight_data) = self.cached_preflight_data.clone()
        {
            info!("ProcessBuilder: Using cached preflight data");
            Some(cached_preflight_data)
        } else if self.include_preflight_data {
            info!("ProcessBuilder: Generating preflight data");
            Some(self.generate_preflight_data(&media_type, &metadata, &classification)?)
        } else {
            None
        };

        // Step 6: Process upload if enabled
        let upload_result = if self.include_upload_processing {
            if let Some(upload_data) = &upload_data {
                info!("ProcessBuilder: Processing upload");
                info!("🚨 DEBUG: About to call process_upload - this should NOT trigger video processing again");
                let result = self.process_upload(upload_data, &classification, &media_type)?;
                info!("🚨 DEBUG: process_upload completed, checking for any side effects...");
                Some(result)
            } else {
                return Err("Upload processing enabled but no upload data available".to_string());
            }
        } else {
            None
        };

        // Extract title from metadata
        let title = metadata
            .get("title")
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

    /// Extract metadata based on media type (returns corrected media type if needed)
    fn extract_metadata_with_type_correction(
        &self,
        media_type: &MediaType,
        metadata: JsonValue,
    ) -> Result<(JsonValue, MediaType), String> {
        let (extracted_metadata, corrected_type) = self.extract_metadata_internal(media_type, metadata)?;
        Ok((extracted_metadata, corrected_type))
    }

    /// Extract metadata based on media type
    fn extract_metadata_internal(
        &self,
        media_type: &MediaType,
        metadata: JsonValue,
    ) -> Result<(JsonValue, MediaType), String> {
        info!("🔍 extract_metadata_internal called for: {:?}", media_type);
        // Extract basic metadata from path for all media types
        let path = std::path::Path::new(&self.input_path);
        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        let mut enriched = metadata;
        if let Some(obj) = enriched.as_object_mut() {
            // Add the filename for classification to use
            obj.insert(
                "filename".to_string(),
                serde_json::Value::String(filename.to_string()),
            );
            obj.insert(
                "input_path".to_string(),
                serde_json::Value::String(self.input_path.clone()),
            );

            // Extract extension
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                obj.insert(
                    "extension".to_string(),
                    serde_json::Value::String(ext.to_string()),
                );
            }

            // For video files, run video-specific classification to extract metadata
            match media_type {
                MediaType::Video(_) => {
                    // Process video (including archive extraction if needed)
                    info!("🎬 Processing video files and extracting archives if needed");
                    let video_results = crate::media::video::process_video(&self.input_path, &self.config, false)?;
                    
                    info!("🎬 Found {} video file(s) after processing", video_results.len());
                    for (i, (video_file, metadata)) in video_results.iter().enumerate() {
                        info!("  Video {}: {} -> {}", i + 1, 
                              video_file.path.file_name().and_then(|n| n.to_str()).unwrap_or("unknown"),
                              metadata.release_name);
                    }
                    
                    // Use the first video file's metadata if available
                    let video_metadata = if let Some((_, metadata)) = video_results.first() {
                        info!("🎬 Using metadata from first video file: {}", metadata.release_name);
                        metadata.clone()
                    } else {
                        // Fallback to direct classification if no results
                        info!("🎬 No video results found, falling back to direct classification");
                        crate::media::video::classify_video_content(&self.input_path)
                    };
                    
                    info!("Video metadata extracted: title='{}', year={:?}, season={:?}, category={:?}",
                        video_metadata.title, video_metadata.year, video_metadata.season, video_metadata.category);

                    // Add video-specific metadata
                    obj.insert(
                        "title".to_string(),
                        serde_json::Value::String(video_metadata.title.clone()),
                    );
                    obj.insert(
                        "release_name".to_string(),
                        serde_json::Value::String(video_metadata.release_name.clone()),
                    );
                    if let Some(year) = video_metadata.year {
                        obj.insert(
                            "year".to_string(),
                            serde_json::Value::Number(serde_json::Number::from(year)),
                        );
                    }
                    if let Some(season) = video_metadata.season {
                        obj.insert(
                            "season".to_string(),
                            serde_json::Value::Number(serde_json::Number::from(season)),
                        );
                    }
                    if let Some(episode) = video_metadata.episode {
                        obj.insert(
                            "episode".to_string(),
                            serde_json::Value::Number(serde_json::Number::from(episode)),
                        );
                    }
                    obj.insert(
                        "category".to_string(),
                        serde_json::Value::String(format!("{:?}", video_metadata.category)),
                    );
                    obj.insert(
                        "source_type".to_string(),
                        serde_json::Value::String(format!("{:?}", video_metadata.source_type)),
                    );
                    obj.insert(
                        "is_boxset".to_string(),
                        serde_json::Value::Bool(video_metadata.is_boxset),
                    );
                    obj.insert(
                        "is_dated_tv".to_string(),
                        serde_json::Value::Bool(video_metadata.is_dated_tv),
                    );
                    if let Some(resolution) = video_metadata.resolution {
                        obj.insert(
                            "resolution".to_string(),
                            serde_json::Value::String(resolution),
                        );
                    }
                    if let Some(codec) = video_metadata.codec {
                        obj.insert("codec".to_string(), serde_json::Value::String(codec));
                    }

                    // Fetch TMDB data if available
                    if !self.config.general.tmdb_api_key.is_empty() {
                        let category = format!("{:?}", video_metadata.category);
                        let is_movie_or_tv =
                            category.contains("Movie") || category.contains("TvShow");

                        if is_movie_or_tv {
                            let release_type = if category.contains("Movie") {
                                "movie"
                            } else {
                                "tv"
                            };

                            info!(
                                "🎬 Fetching TMDB data during metadata extraction for: {}",
                                video_metadata.title
                            );

                            match crate::metadata::tmdb::fetch_tmdb_id(
                                &video_metadata.title,
                                video_metadata.year.map(|y| y.to_string()),
                                &self.config.general.tmdb_api_key,
                                release_type,
                            ) {
                                Ok(tmdb_id) if tmdb_id > 0 => {
                                    info!("✅ Found TMDB ID: {}", tmdb_id);
                                    obj.insert(
                                        "tmdb_id".to_string(),
                                        serde_json::Value::Number(serde_json::Number::from(
                                            tmdb_id,
                                        )),
                                    );

                                    // Fetch full TMDB details
                                    match crate::metadata::tmdb::fetch_tmdb_details(
                                        tmdb_id,
                                        release_type,
                                        &self.config.general.tmdb_api_key,
                                    ) {
                                        Ok(tmdb_details) => {
                                            info!("✅ Successfully fetched TMDB details");

                                            // Extract and add all TMDB metadata
                                            let tmdb_metadata =
                                                crate::metadata::tmdb::extract_tmdb_metadata(
                                                    &tmdb_details,
                                                    release_type,
                                                );

                                            info!(
                                                "📊 Adding {} TMDB fields to metadata",
                                                tmdb_metadata.len()
                                            );
                                            for (key, value) in tmdb_metadata {
                                                obj.insert(key, serde_json::Value::String(value));
                                            }

                                            // Also fetch external IDs
                                            if let Ok((imdb_id, tvdb_id)) =
                                                crate::metadata::tmdb::fetch_external_ids(
                                                    tmdb_id,
                                                    release_type,
                                                    &self.config.general.tmdb_api_key,
                                                )
                                            {
                                                if let Some(imdb) = imdb_id {
                                                    obj.insert(
                                                        "imdb_id".to_string(),
                                                        serde_json::Value::String(imdb),
                                                    );
                                                }
                                                if let Some(tvdb) = tvdb_id {
                                                    obj.insert(
                                                        "tvdb_id".to_string(),
                                                        serde_json::Value::Number(
                                                            serde_json::Number::from(tvdb),
                                                        ),
                                                    );
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            info!("❌ Failed to fetch TMDB details: {}", e);
                                        }
                                    }
                                }
                                Ok(_) => {
                                    info!("⚠️ TMDB ID is 0, skipping TMDB enrichment");
                                }
                                Err(e) => {
                                    info!("❌ Failed to fetch TMDB ID: {}", e);
                                }
                            }
                        }
                    }
                }
                MediaType::Game(_) => {
                    // For games, extract title and do IGDB lookup if credentials are available
                    let game_title = obj.get("title")
                        .and_then(|t| t.as_str())
                        .unwrap_or(&filename)
                        .to_string();
                    
                    // Clean the game title for IGDB search
                    let cleaned_title = crate::metadata::igdb::clean_game_title_for_search(&game_title, &self.config);
                    
                    if !self.config.general.igdb_client_id.is_empty()
                        && !self.config.general.igdb_bearer_token.is_empty()
                    {
                        info!(
                            "🎮 Fetching IGDB data during metadata extraction for: {}",
                            game_title
                        );

                        match crate::metadata::igdb::search_igdb_game(
                            &cleaned_title,
                            &self.config.general.igdb_client_id,
                            &self.config.general.igdb_bearer_token,
                            &self.config,
                        ) {
                            Ok(games) if !games.is_empty() => {
                                info!("✅ Found {} games on IGDB", games.len());
                                
                                // Take the first result (best match)
                                if let Some(game) = games.first() {
                                    if let Some(igdb_id) = game["id"].as_u64() {
                                        info!("✅ Found IGDB ID: {}", igdb_id);
                                        obj.insert(
                                            "igdb_id".to_string(),
                                            serde_json::Value::Number(serde_json::Number::from(igdb_id)),
                                        );

                                        // Fetch detailed game information
                                        match crate::metadata::igdb::get_igdb_game_details(
                                            igdb_id,
                                            &self.config.general.igdb_client_id,
                                            &self.config.general.igdb_bearer_token,
                                        ) {
                                            Ok(igdb_details) => {
                                                info!("✅ Successfully fetched IGDB details");

                                                // Extract and add all IGDB metadata
                                                let igdb_metadata = crate::metadata::igdb::extract_igdb_metadata(&igdb_details);

                                                info!(
                                                    "📊 Adding {} IGDB fields to metadata",
                                                    igdb_metadata.len()
                                                );
                                                for (key, value) in igdb_metadata {
                                                    obj.insert(key, serde_json::Value::String(value));
                                                }
                                            }
                                            Err(e) => {
                                                info!("❌ Failed to fetch IGDB details: {}", e);
                                            }
                                        }
                                    }
                                }
                            }
                            Ok(_) => {
                                info!("⚠️ No games found on IGDB for: {}", cleaned_title);
                            }
                            Err(e) => {
                                info!("❌ Failed to search IGDB: {}", e);
                            }
                        }
                    } else {
                        info!("⚠️ IGDB credentials not configured, skipping game lookup");
                    }
                }
                MediaType::Audio(_) => {
                    // For audio files, extract metadata from tags and MusicBrainz
                    info!("🎵 Processing audio metadata extraction");

                    // Variables to hold extracted metadata
                    let mut artist = None;
                    let mut album = None;
                    let mut year = None;
                    let mut genre = None;

                    // First, let's try to get basic metadata from the first audio file
                    if let Ok(files) = crate::utils::filter_files_by_extension(
                        &self.input_path,
                        &crate::core::AudioType::all_extensions(),
                    ) {
                        if let Some(first_file) = files.first() {
                            if let Some(file_path) = first_file.to_str() {
                                // Use mediainfo to extract metadata
                                match crate::processing::components::mediainfo_utils::generate_mediainfo(file_path, &self.config) {
                                    Ok(mediainfo_output) => {
                                        // Parse mediainfo output for audio metadata
                                        for line in mediainfo_output.lines() {
                                            let line = line.trim();
                                            // Use more specific parsing to handle spaces better
                                            if line.starts_with("Album") && line.contains(":") && !line.starts_with("Album/") {
                                                let parts: Vec<&str> = line.splitn(2, ':').collect();
                                                if parts.len() == 2 {
                                                    album = Some(parts[1].trim().to_string());
                                                }
                                            } else if line.starts_with("Performer") && line.contains(":") {
                                                let parts: Vec<&str> = line.splitn(2, ':').collect();
                                                if parts.len() == 2 {
                                                    artist = Some(parts[1].trim().to_string());
                                                }
                                            } else if line.starts_with("Recorded date") && line.contains(":") {
                                                let parts: Vec<&str> = line.splitn(2, ':').collect();
                                                if parts.len() == 2 {
                                                    if let Some(date) = parts[1].trim().split('-').next() {
                                                        year = Some(date.to_string());
                                                    }
                                                }
                                            } else if line.starts_with("Genre") && line.contains(":") {
                                                let parts: Vec<&str> = line.splitn(2, ':').collect();
                                                if parts.len() == 2 {
                                                    genre = Some(parts[1].trim().to_string());
                                                }
                                            }
                                        }

                                        info!("🎵 Extracted from mediainfo - Artist: {:?}, Album: {:?}, Year: {:?}, Genre: {:?}", 
                                              artist, album, year, genre);

                                        // Store extracted metadata
                                        if let Some(ref artist_name) = artist {
                                            obj.insert("artist".to_string(), serde_json::Value::String(artist_name.clone()));
                                            // Don't set title to artist name, let it use the filename fallback
                                        }
                                        if let Some(ref album_name) = album {
                                            obj.insert("album".to_string(), serde_json::Value::String(album_name.clone()));
                                        }
                                        if let Some(ref year_str) = year {
                                            obj.insert("year".to_string(), serde_json::Value::String(year_str.clone()));
                                        }
                                        if let Some(ref genre_name) = genre {
                                            obj.insert("genre".to_string(), serde_json::Value::String(genre_name.clone()));
                                        }

                                        // Now try MusicBrainz lookup if we have artist and album
                                        if let (Some(artist_name), Some(album_name)) = (artist.clone(), album.clone()) {
                                            info!("🎵 Searching MusicBrainz for: {} - {}", artist_name, album_name);

                                            match crate::metadata::musicbrainz::search_musicbrainz_release_with_year(
                                                &artist_name,
                                                &album_name,
                                                year.as_deref()
                                            ) {
                                                Ok(releases) if !releases.is_empty() => {
                                                    // Use the first release
                                                    if let Some(release_id) = releases[0]["id"].as_str() {
                                                        info!("✅ Found MusicBrainz release ID: {}", release_id);

                                                        // Fetch full details
                                                        match crate::metadata::musicbrainz::get_musicbrainz_release_details(release_id) {
                                                            Ok(mb_details) => {
                                                                info!("✅ Successfully fetched MusicBrainz details");

                                                                // Extract metadata
                                                                let mb_metadata = crate::metadata::musicbrainz::extract_musicbrainz_metadata(&mb_details);

                                                                info!("📊 Adding {} MusicBrainz fields to metadata", mb_metadata.len());
                                                                
                                                                // Store MusicBrainz IDs in environment variables for keyword generation
                                                                if let Some(release_id) = mb_metadata.get("musicbrainz_release_id") {
                                                                    std::env::set_var("SEEDBRR_MB_RELEASE_ID", release_id);
                                                                }
                                                                if let Some(release_group_id) = mb_metadata.get("musicbrainz_release_group_id") {
                                                                    std::env::set_var("SEEDBRR_MB_RELEASE_GROUP_ID", release_group_id);
                                                                }
                                                                if let Some(artist_ids) = mb_metadata.get("musicbrainz_artist_ids") {
                                                                    std::env::set_var("SEEDBRR_MB_ARTIST_IDS", artist_ids);
                                                                }
                                                                if let Some(barcode) = mb_metadata.get("musicbrainz_barcode") {
                                                                    std::env::set_var("SEEDBRR_MB_BARCODE", barcode);
                                                                }
                                                                if let Some(catalog_numbers) = mb_metadata.get("musicbrainz_catalog_numbers") {
                                                                    std::env::set_var("SEEDBRR_MB_CATALOG_NUMBERS", catalog_numbers);
                                                                }
                                                                
                                                                for (key, value) in mb_metadata {
                                                                    obj.insert(key, serde_json::Value::String(value));
                                                                }
                                                            }
                                                            Err(e) => {
                                                                info!("❌ Failed to fetch MusicBrainz details: {}", e);
                                                            }
                                                        }
                                                    }
                                                }
                                                Ok(_) => {
                                                    info!("⚠️ No MusicBrainz releases found for: {} - {}", artist_name, album_name);
                                                }
                                                Err(e) => {
                                                    info!("❌ Failed to search MusicBrainz: {}", e);
                                                }
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        info!("Failed to generate mediainfo for audio metadata: {}", e);
                                    }
                                }
                            }
                        }
                    }

                    // Generate detailed tracklist by scanning all audio files
                    let audio_files = crate::utils::filter_files_by_extension(
                        &self.input_path,
                        &crate::core::AudioType::all_extensions(),
                    )
                    .unwrap_or_default();

                    if !audio_files.is_empty() {
                        let mut tracklist_rows = Vec::new();
                        let mut sorted_files = audio_files.clone();
                        sorted_files.sort();

                        for (idx, file) in sorted_files.iter().enumerate() {
                            if let Some(file_path) = file.to_str() {
                                if let Some(file_name) = file.file_name().and_then(|n| n.to_str()) {
                                    // Extract track info from filename (common pattern: 01-artist-title.mp3)
                                    let mut track_title = file_name.to_string();
                                    let mut track_artist =
                                        artist.as_ref().unwrap_or(&"Unknown".to_string()).clone();

                                    // Try to parse track number and title from filename
                                    if let Some(name_without_ext) = file_name.split('.').next() {
                                        let parts: Vec<&str> =
                                            name_without_ext.split('-').collect();
                                        if parts.len() >= 3 {
                                            // Format: 01-artist-title
                                            if parts[1].to_lowercase()
                                                == track_artist.to_lowercase()
                                            {
                                                track_title =
                                                    parts[2..].join("-").replace('_', " ");
                                                // Capitalize words
                                                track_title = track_title
                                                    .split_whitespace()
                                                    .map(|word| {
                                                        let mut chars = word.chars();
                                                        match chars.next() {
                                                            None => String::new(),
                                                            Some(first) => {
                                                                first
                                                                    .to_uppercase()
                                                                    .collect::<String>()
                                                                    + chars.as_str()
                                                            }
                                                        }
                                                    })
                                                    .collect::<Vec<String>>()
                                                    .join(" ");
                                            }
                                        }
                                    }

                                    // Get file size
                                    let file_size = file
                                        .metadata()
                                        .ok()
                                        .map(|m| format!("{:.2} MB", m.len() as f64 / 1_048_576.0))
                                        .unwrap_or_else(|| "Unknown".to_string());

                                    // Get audio format info from mediainfo for this specific track
                                    let (format, bitrate, duration) = match crate::processing::components::mediainfo_utils::generate_mediainfo(file_path, &self.config) {
                                        Ok(track_mediainfo) => {
                                            let mut track_format = file_name.split('.').last().unwrap_or("mp3").to_uppercase();
                                            let mut track_bitrate = "Unknown".to_string();
                                            let mut track_duration = "Unknown".to_string();

                                            // Parse mediainfo output for this track
                                            let mut in_audio_section = false;
                                            let mut format_found = false;

                                            for line in track_mediainfo.lines() {
                                                let line = line.trim();

                                                // Check for section headers
                                                if line == "Audio" {
                                                    in_audio_section = true;
                                                    continue;
                                                } else if line == "Image" || line == "Video" || line == "General" {
                                                    in_audio_section = false;
                                                    continue;
                                                }

                                                if line.contains(':') {
                                                    let parts: Vec<&str> = line.splitn(2, ':').collect();
                                                    if parts.len() == 2 {
                                                        let key = parts[0].trim();
                                                        let value = parts[1].trim();

                                                        match key {
                                                            "Format" => {
                                                                if in_audio_section && !line.starts_with("Format profile") && !line.starts_with("Format settings") && !format_found {
                                                                    track_format = value.to_string();
                                                                    format_found = true;
                                                                }
                                                            }
                                                            "Bit rate" => {
                                                                if in_audio_section {
                                                                    track_bitrate = value.to_string();
                                                                }
                                                            }
                                                            "Overall bit rate" => {
                                                                if !in_audio_section && track_bitrate == "Unknown" {
                                                                    track_bitrate = value.to_string();
                                                                }
                                                            }
                                                            "Duration" => {
                                                                // Convert duration format (e.g., "3 min 45 s" or "00:03:45")
                                                                if value.contains("min") || value.contains("mn") {
                                                                    track_duration = value.to_string();
                                                                } else if value.contains(':') {
                                                                    // Convert HH:MM:SS to min:sec format
                                                                    let time_parts: Vec<&str> = value.split(':').collect();
                                                                    if time_parts.len() >= 2 {
                                                                        if let (Ok(hours), Ok(mins), Ok(secs)) = (
                                                                            time_parts.get(0).unwrap_or(&"0").parse::<u32>(),
                                                                            time_parts.get(1).unwrap_or(&"0").parse::<u32>(),
                                                                            time_parts.get(2).unwrap_or(&"0").split('.').next().unwrap_or("0").parse::<u32>()
                                                                        ) {
                                                                            let total_mins = hours * 60 + mins;
                                                                            track_duration = format!("{}:{:02} min", total_mins, secs);
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                            _ => {}
                                                        }
                                                    }
                                                }
                                            }

                                            (track_format, track_bitrate, track_duration)
                                        }
                                        Err(e) => {
                                            info!("Failed to get mediainfo for track {}: {}", file_name, e);
                                            (
                                                file_name.split('.').last().unwrap_or("mp3").to_uppercase(),
                                                "Unknown".to_string(),
                                                "Unknown".to_string()
                                            )
                                        }
                                    };

                                    let row = format!(
                                        "[tr][td]{}[/td][td]{}[/td][td]{}[/td][td]{}[/td][td]{}[/td][td]{}[/td][td]{}[/td][/tr]",
                                        idx + 1,
                                        track_artist,
                                        track_title,
                                        duration,
                                        file_size,
                                        format,
                                        bitrate
                                    );
                                    tracklist_rows.push(row);
                                }
                            }
                        }

                        if !tracklist_rows.is_empty() {
                            obj.insert(
                                "tracklist_rows".to_string(),
                                serde_json::Value::String(tracklist_rows.join("\n")),
                            );
                        }
                    }

                    // Add audio format information from the first track's mediainfo if not already set
                    if !obj.contains_key("audio_format") && !audio_files.is_empty() {
                        let mut sorted_for_format = audio_files.clone();
                        sorted_for_format.sort();
                        if let Some(first_file) = sorted_for_format.first() {
                            if let Some(file_path) = first_file.to_str() {
                                match crate::processing::components::mediainfo_utils::generate_mediainfo(file_path, &self.config) {
                                    Ok(mediainfo_output) => {
                                        // Extract overall audio format info
                                        // We need to track which section we're in to avoid getting format from Image section
                                        let mut in_audio_section = false;
                                        let mut audio_format_found = false;

                                        for line in mediainfo_output.lines() {
                                            let line = line.trim();

                                            // Check for section headers
                                            if line == "Audio" {
                                                in_audio_section = true;
                                                continue;
                                            } else if line == "Image" || line == "Video" || line == "General" {
                                                in_audio_section = false;
                                                continue;
                                            }

                                            // Only process lines in the Audio section for format
                                            if in_audio_section && line.contains(':') {
                                                let parts: Vec<&str> = line.splitn(2, ':').collect();
                                                if parts.len() == 2 {
                                                    let key = parts[0].trim();
                                                    let value = parts[1].trim();

                                                    match key {
                                                        "Format" => {
                                                            if !line.starts_with("Format profile") && 
                                                               !line.starts_with("Format settings") && 
                                                               !audio_format_found {
                                                                obj.insert("audio_format".to_string(), 
                                                                          serde_json::Value::String(value.to_string()));
                                                                audio_format_found = true;
                                                            }
                                                        }
                                                        "Bit rate" => {
                                                            if !obj.contains_key("audio_bitrate") {
                                                                obj.insert("audio_bitrate".to_string(), 
                                                                          serde_json::Value::String(value.to_string()));
                                                            }
                                                        }
                                                        "Sampling rate" => {
                                                            obj.insert("audio_sample_rate".to_string(), 
                                                                      serde_json::Value::String(value.to_string()));
                                                        }
                                                        _ => {}
                                                    }
                                                }
                                            } else if !in_audio_section && line.contains(':') {
                                                // Handle Overall bit rate which appears in General section
                                                let parts: Vec<&str> = line.splitn(2, ':').collect();
                                                if parts.len() == 2 {
                                                    let key = parts[0].trim();
                                                    let value = parts[1].trim();

                                                    if key == "Overall bit rate" && !obj.contains_key("audio_bitrate") {
                                                        obj.insert("audio_bitrate".to_string(), 
                                                                  serde_json::Value::String(value.to_string()));
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    Err(_) => {
                                        // Fallback values
                                        obj.insert("audio_format".to_string(), 
                                                  serde_json::Value::String("Audio".to_string()));
                                    }
                                }
                            }
                        }
                    }

                    // Fallback to filename if no title extracted
                    if !obj.contains_key("title") {
                        obj.insert(
                            "title".to_string(),
                            serde_json::Value::String(filename.to_string()),
                        );
                    }
                }
                MediaType::Ebook(_) => {
                    // For ebook files, process archives and extract metadata
                    info!("📚 Processing ebook metadata extraction and archive processing");
                    
                    // First, process and extract any archives
                    match crate::media::ebook::process_ebook(&self.input_path, &self.config, false) {
                        Ok(ebook_results) => {
                            info!("✅ Successfully processed {} ebook file(s)", ebook_results.len());
                            
                            // If we have ebook results, use the first one for metadata
                            if let Some((ebook_file, mut ebook_metadata)) = ebook_results.first().cloned() {
                                // Override category if forced category is set
                                if let Some(ref forced_cat) = self.force_category {
                                    if forced_cat.contains("EbookCategory::") {
                                        let category_name = forced_cat.replace("EbookCategory::", "");
                                        match category_name.as_str() {
                                            "Comic" => ebook_metadata.category = crate::core::EbookCategory::Comic,
                                            "Novel" => ebook_metadata.category = crate::core::EbookCategory::Novel,
                                            "Magazine" => ebook_metadata.category = crate::core::EbookCategory::Magazine,
                                            "Newspaper" => ebook_metadata.category = crate::core::EbookCategory::Newspaper,
                                            "Technical" => ebook_metadata.category = crate::core::EbookCategory::Technical,
                                            "Educational" => ebook_metadata.category = crate::core::EbookCategory::Educational,
                                            _ => {}
                                        }
                                        info!("📚 Override ebook category to: {:?}", ebook_metadata.category);
                                    }
                                }
                                
                                // Add ebook-specific metadata
                                obj.insert(
                                    "ebook_type".to_string(),
                                    serde_json::Value::String(format!("{:?}", ebook_file.ebook_type)),
                                );
                                obj.insert(
                                    "ebook_category".to_string(),
                                    serde_json::Value::String(format!("{:?}", ebook_metadata.category)),
                                );
                                if let Some(format_type) = &ebook_metadata.format_type {
                                    obj.insert(
                                        "ebook_format_type".to_string(),
                                        serde_json::Value::String(format!("{:?}", format_type)),
                                    );
                                }
                                
                                // Add other ebook metadata if available
                                if let Some(author) = &ebook_metadata.author {
                                    obj.insert(
                                        "ebook_author".to_string(),
                                        serde_json::Value::String(author.clone()),
                                    );
                                }
                                if let Some(year) = ebook_metadata.year {
                                    obj.insert(
                                        "ebook_year".to_string(),
                                        serde_json::Value::Number(serde_json::Number::from(year)),
                                    );
                                }
                                if let Some(publisher) = &ebook_metadata.publisher {
                                    obj.insert(
                                        "ebook_publisher".to_string(),
                                        serde_json::Value::String(publisher.clone()),
                                    );
                                }
                                
                                // Use the ebook file path as the title (without extension)
                                if let Some(file_stem) = ebook_file.path.file_stem().and_then(|s| s.to_str()) {
                                    obj.insert(
                                        "title".to_string(),
                                        serde_json::Value::String(file_stem.to_string()),
                                    );
                                }
                                
                                info!("📚 Added ebook metadata: type={:?}, category={:?}", 
                                      ebook_file.ebook_type, ebook_metadata.category);
                                
                                // Note: Cleanup is already handled by process_ebook function
                                info!("📚 Detected ebook type: {:?} (cleanup already completed by process_ebook)", ebook_file.ebook_type);
                                
                                // Store the corrected media type for return
                                obj.insert("__corrected_media_type".to_string(), 
                                          serde_json::Value::String(format!("{:?}", ebook_file.ebook_type)));
                                info!("📚 Correcting media type from {:?} to Ebook({:?})", media_type, ebook_file.ebook_type);
                                
                                // Rename EPUB files to standard naming convention if needed
                                if ebook_file.ebook_type == crate::core::EbookType::Epub {
                                    match self.rename_epub_file(&ebook_file.path) {
                                        Ok(new_path) => {
                                            // For single file uploads, store the new path in metadata for UploadBuilder
                                            // For folder uploads, we keep the original input_path pointing to the folder
                                            // Check if the input path has a file extension (indicating it's a file, not a folder)
                                            let input_path = std::path::Path::new(&self.input_path);
                                            let is_single_file = input_path.extension().is_some();
                                            
                                            if is_single_file {
                                                info!("📚 Single file upload detected - storing renamed path in metadata: {}", new_path.display());
                                                obj.insert("__renamed_input_path".to_string(), 
                                                          serde_json::Value::String(new_path.to_string_lossy().to_string()));
                                            } else {
                                                info!("📚 Folder upload detected - keeping original input_path: {}", self.input_path);
                                            }
                                            

                                        },
                                        Err(e) => {
                                            info!("⚠️ Failed to rename EPUB file: {}", e);
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            info!("⚠️ Failed to process ebook, trying to extract archives anyway: {}", e);
                            
                            // Still try to extract archives even if classification failed
                            let _ = crate::processing::extraction::process_and_extract_archives(&self.input_path);
                            
                            // Fallback to filename as title
                            obj.insert(
                                "title".to_string(),
                                serde_json::Value::String(filename.to_string()),
                            );
                            
                            // Note: Cleanup for failed processing is handled elsewhere if needed
                        }
                    }

                }
                _ => {
                    // For other media types, just use filename as title for now
                    obj.insert(
                        "title".to_string(),
                        serde_json::Value::String(filename.to_string()),
                    );
                }
            }
        }
        // Check if we have a corrected media type from ebook processing
        let corrected_media_type = if let Some(corrected_type) = enriched.get("__corrected_media_type") {
            if let Some(type_str) = corrected_type.as_str() {
                match type_str {
                    "Epub" => MediaType::Ebook(crate::core::EbookType::Epub),
                    "Pdf" => MediaType::Ebook(crate::core::EbookType::Pdf), 
                    "Cbr" => MediaType::Ebook(crate::core::EbookType::Cbr),
                    "Cbz" => MediaType::Ebook(crate::core::EbookType::Cbz),
                    _ => media_type.clone(),
                }
            } else {
                media_type.clone()
            }
        } else {
            media_type.clone()
        };
        
        // Remove the internal field
        let mut final_enriched = enriched;
        if final_enriched.is_object() {
            final_enriched.as_object_mut().unwrap().remove("__corrected_media_type");
        }
        
        Ok((final_enriched, corrected_media_type))
    }

    /// Classify media using the classification system
    fn classify_media(
        &self,
        media_type: &MediaType,
        metadata: &JsonValue,
    ) -> Result<ClassificationResult, String> {
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
        metadata: &JsonValue,
        _classification: &Option<ClassificationResult>,
    ) -> Result<UploadData, String> {
        info!("ProcessBuilder: build_upload_data - Creating UploadBuilder");
        
        // Check if we have a renamed input path from EPUB processing
        let actual_input_path = if let Some(renamed_path) = metadata.get("__renamed_input_path").and_then(|p| p.as_str()) {
            // For EPUB files, always preserve the folder structure by using the original folder path
            // This ensures that if an EPUB is inside a folder, the folder structure is maintained
            if matches!(media_type, MediaType::Ebook(_)) {
                info!("ProcessBuilder: build_upload_data - EPUB detected, preserving folder structure using original path: {}", self.input_path);
                self.input_path.clone()
            } else {
                info!("ProcessBuilder: build_upload_data - Using renamed input path: {}", renamed_path);
                renamed_path.to_string()
            }
        } else {
            self.input_path.clone()
        };
        
        let mut builder =
            UploadBuilder::new(&actual_input_path, media_type.clone(), self.config.clone());
        info!("ProcessBuilder: build_upload_data - UploadBuilder created successfully");
        
        // 🚨 FIX: Pass the metadata to UploadBuilder so it can access rich TMDB data
        builder = builder.with_cached_metadata(metadata.clone());
        info!("ProcessBuilder: build_upload_data - Set cached metadata with {} fields", 
              metadata.as_object().map(|obj| obj.len()).unwrap_or(0));

        // Apply description config if provided
        if let Some(desc_config) = &self.description_config {
            info!("ProcessBuilder: build_upload_data - Applying description config");
            builder = builder.with_description_config(desc_config.clone());
        }

        info!(
            "ProcessBuilder: build_upload_data - Processing media type: {:?}",
            media_type
        );
        // Add metadata based on media type
        match media_type {
            MediaType::Video(_) => {
                info!("ProcessBuilder: build_upload_data - Processing video metadata");
                // Extract video metadata and pass to builder
                if let (Some(title), Some(category), Some(source_type)) = (
                    metadata.get("title").and_then(|t| t.as_str()),
                    metadata.get("category").and_then(|c| c.as_str()),
                    metadata.get("source_type").and_then(|s| s.as_str()),
                ) {
                    info!(
                        "ProcessBuilder: build_upload_data - Creating video metadata for: {}",
                        title
                    );
                    let mut video_metadata = crate::media::video::VideoMetadata::default();
                    video_metadata.title = title.to_string();

                    // Set release_name - try to get from metadata first
                    if let Some(release_name) =
                        metadata.get("release_name").and_then(|r| r.as_str())
                    {
                        video_metadata.release_name = release_name.to_string();
                        info!(
                            "ProcessBuilder: Using cached release_name: {}",
                            video_metadata.release_name
                        );
                    } else if let Some(filename) = metadata.get("filename").and_then(|f| f.as_str())
                    {
                        video_metadata.release_name =
                            crate::processing::naming::generate_release_name(filename);
                        info!(
                            "ProcessBuilder: Set release_name from filename: {}",
                            video_metadata.release_name
                        );
                    } else if let Some(input_path) =
                        metadata.get("input_path").and_then(|p| p.as_str())
                    {
                        let path = std::path::Path::new(input_path);
                        let name = if path.is_dir() {
                            path.file_name()
                                .and_then(|s| s.to_str())
                                .unwrap_or("unknown")
                        } else {
                            path.file_name()
                                .and_then(|s| s.to_str())
                                .unwrap_or("unknown")
                        };
                        video_metadata.release_name =
                            crate::processing::naming::generate_release_name(name);
                        info!(
                            "ProcessBuilder: Set release_name from input_path: {}",
                            video_metadata.release_name
                        );
                    } else {
                        // Last resort - use the title
                        video_metadata.release_name = title.to_string();
                        info!(
                            "ProcessBuilder: Using title as release_name: {}",
                            video_metadata.release_name
                        );
                    }

                    // Parse category
                    video_metadata.category = match category {
                        "Movie" => crate::core::VideoCategory::Movie,
                        "TvShow" => crate::core::VideoCategory::TvShow,
                        "Anime" => crate::core::VideoCategory::Anime,
                        "Sports" => crate::core::VideoCategory::Sports,
                        "Documentary" => crate::core::VideoCategory::Documentary,
                        "Concert" => crate::core::VideoCategory::Concert,
                        _ => crate::core::VideoCategory::Unknown,
                    };

                    // Parse source type
                    video_metadata.source_type = match source_type {
                        "BluRay" => crate::core::VideoSourceType::BluRay,
                        "UHDBluRay" => crate::core::VideoSourceType::UHDBluRay,
                        "DVD" => crate::core::VideoSourceType::DVD,
                        "WebDL" => crate::core::VideoSourceType::WebDL,
                        "WebRip" => crate::core::VideoSourceType::WebRip,
                        "HDTV" => crate::core::VideoSourceType::HDTV,
                        "PDTV" => crate::core::VideoSourceType::PDTV,
                        "SDTV" => crate::core::VideoSourceType::SDTV,
                        "Remux" => crate::core::VideoSourceType::Remux,
                        "Encode" => crate::core::VideoSourceType::Encode,
                        "FullDisc" => crate::core::VideoSourceType::FullDisc,
                        "SeasonPack" => crate::core::VideoSourceType::SeasonPack,
                        "Upscale" => crate::core::VideoSourceType::Upscale,
                        _ => crate::core::VideoSourceType::Unknown,
                    };

                    // Extract other metadata
                    if let Some(year) = metadata.get("year").and_then(|y| y.as_u64()) {
                        video_metadata.year = Some(year as u32);
                    }
                    if let Some(season) = metadata.get("season").and_then(|s| s.as_u64()) {
                        video_metadata.season = Some(season as u32);
                    }
                    if let Some(episode) = metadata.get("episode").and_then(|e| e.as_u64()) {
                        video_metadata.episode = Some(episode as u32);
                    }
                    video_metadata.is_boxset = metadata
                        .get("is_boxset")
                        .and_then(|b| b.as_bool())
                        .unwrap_or(false);
                    video_metadata.is_dated_tv = metadata
                        .get("is_dated_tv")
                        .and_then(|b| b.as_bool())
                        .unwrap_or(false);

                    if let Some(resolution) = metadata.get("resolution").and_then(|r| r.as_str()) {
                        video_metadata.resolution = Some(resolution.to_string());
                    }
                    if let Some(codec) = metadata.get("codec").and_then(|c| c.as_str()) {
                        video_metadata.codec = Some(codec.to_string());
                    }

                    info!("ProcessBuilder: build_upload_data - Adding video metadata to builder");
                    builder = builder.with_video_metadata(video_metadata);
                    info!("ProcessBuilder: build_upload_data - Video metadata added successfully");
                } else {
                    info!("ProcessBuilder: build_upload_data - Video metadata extraction failed - missing required fields");
                }
            }
            MediaType::Game(_) => {
                info!("ProcessBuilder: build_upload_data - Processing game metadata");
                // For games, use IGDB title if available, otherwise use filename
                let title = metadata.get("igdb_title")
                    .and_then(|t| t.as_str())
                    .or_else(|| metadata.get("filename").and_then(|f| f.as_str()))
                    .map(|s| s.to_string());
                
                if let Some(title) = title {
                    info!("ProcessBuilder: build_upload_data - Using game title: {}", title);
                    let year = metadata
                        .get("igdb_release_year")
                        .and_then(|y| y.as_str())
                        .map(|y| y.to_string())
                        .or_else(|| metadata.get("year").and_then(|y| y.as_u64()).map(|y| y.to_string()));
                    
                    builder = builder.with_title_info(title, year);
                    
                    // Add IGDB data from metadata if available
                    if let Some(metadata_obj) = metadata.as_object() {
                        if let Some(igdb_id) = metadata_obj.get("igdb_id").and_then(|v| v.as_u64()) {
                            info!("📊 ProcessBuilder: Found IGDB ID in metadata: {}", igdb_id);
                        }
                    }
                } else {
                    info!("⚠️ ProcessBuilder: No title found for game - using fallback");
                    // Fallback: use directory/file name
                    let fallback_title = if let Some(input_path) = metadata.get("input_path").and_then(|p| p.as_str()) {
                        let path = std::path::Path::new(input_path);
                        path.file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or("Unknown Game")
                            .to_string()
                    } else {
                        "Unknown Game".to_string()
                    };
                    builder = builder.with_title_info(fallback_title, None::<String>);
                }
            }
            MediaType::Ebook(_) => {
                // For ebooks, prioritize filename (release name) over extracted title
                let title = metadata.get("filename").and_then(|t| t.as_str())
                    .or_else(|| metadata.get("title").and_then(|t| t.as_str()));
                
                if let Some(title) = title {
                    let year = metadata
                        .get("ebook_year")
                        .and_then(|y| y.as_u64())
                        .map(|y| y.to_string())
                        .or_else(|| metadata.get("year").and_then(|y| y.as_u64()).map(|y| y.to_string()));
                    builder = builder.with_title_info(title, year);
                }
            }
            MediaType::Audio(_)
            | MediaType::Hobby(_) => {
                // For non-video media, use title info if available
                if let Some(title) = metadata.get("title").and_then(|t| t.as_str()) {
                    let year = metadata
                        .get("year")
                        .and_then(|y| y.as_u64())
                        .map(|y| y.to_string());
                    builder = builder.with_title_info(title, year);
                }
            }
        }

        // Pass enriched metadata from preflight (TMDB and MusicBrainz fields are in the main metadata object)
        let mut enriched_map = HashMap::new();
        if let Some(metadata_obj) = metadata.as_object() {
            info!(
                "📊 ProcessBuilder: Checking metadata for enriched fields. Total fields: {}",
                metadata_obj.len()
            );

            // Extract all TMDB, IGDB, and MusicBrainz related fields
            for (key, value) in metadata_obj {
                if key.starts_with("tmdb_") || key.starts_with("igdb_") || key.starts_with("musicbrainz_") {
                    if let Some(str_value) = value.as_str() {
                        info!("  📌 Found enriched field: {} = {}", key, str_value);
                        enriched_map.insert(key.clone(), str_value.to_string());
                    } else if let Some(num_value) = value.as_f64() {
                        info!("  📌 Found enriched field: {} = {}", key, num_value);
                        enriched_map.insert(key.clone(), num_value.to_string());
                    } else if let Some(bool_value) = value.as_bool() {
                        info!("  📌 Found enriched field: {} = {}", key, bool_value);
                        enriched_map.insert(key.clone(), bool_value.to_string());
                    }
                }
            }

            // Also pass through basic audio metadata fields for templates
            for field in [
                "artist",
                "album",
                "year",
                "genre",
                "format",
                "category",
                "source_type",
                "tracklist_rows",
                "audio_format",
                "audio_bitrate",
                "audio_sample_rate",
                "filename",
                "cover_url",
                "cover_images",
                "imdb_id",
                "tvdb_id",
                "igdb_id",
            ] {
                if let Some(value) = metadata_obj.get(field) {
                    if let Some(str_value) = value.as_str() {
                        info!("  📌 Found audio field: {} = {}", field, str_value);
                        enriched_map.insert(field.to_string(), str_value.to_string());
                    } else if let Some(num_value) = value.as_f64() {
                        info!("  📌 Found audio field: {} = {}", field, num_value);
                        enriched_map.insert(field.to_string(), num_value.to_string());
                    } else if field == "cover_images" && value.is_array() {
                        // Handle cover_images array specially - the template processor will handle it
                        info!(
                            "  📌 Found cover_images array with {} items",
                            value.as_array().map(|a| a.len()).unwrap_or(0)
                        );
                    }
                }
            }

            if !enriched_map.is_empty() {
                info!("✅ ProcessBuilder: build_upload_data - Passing {} enriched metadata fields to UploadBuilder", enriched_map.len());
                builder = builder.with_enriched_metadata(enriched_map);
            } else {
                info!("⚠️ ProcessBuilder: build_upload_data - No enriched metadata fields found in metadata");
            }
        }

        // Add common components
        if self.include_duplicate_check {
            builder = builder.with_duplicate_check();
        }

        builder = builder.dry_run(self.dry_run);

        // Inject cached preflight data if available
        if let Some(ref preflight_data) = self.cached_preflight_data {
            use crate::core::types::UploadComponent;

            // Add TMDB data if available
            if preflight_data.tmdb_id > 0 {
                let tmdb_component = UploadComponent::TmdbData {
                    tmdb_id: preflight_data.tmdb_id,
                    imdb_id: preflight_data.imdb_id.clone(),
                    tvdb_id: preflight_data.tvdb_id,
                    title: preflight_data.release_name.clone(),
                    year: None,
                };
                builder = builder.with_custom_component("tmdb", tmdb_component);
            }

            // Add IGDB data for games if available
            if let Some(igdb_id) = preflight_data.igdb_id {
                let mut igdb_metadata = HashMap::new();
                igdb_metadata.insert("igdb_id".to_string(), igdb_id.to_string());
                igdb_metadata.insert("title".to_string(), preflight_data.release_name.clone());
                if let Some(ref dev) = preflight_data.igdb_developer {
                    igdb_metadata.insert("developer".to_string(), dev.clone());
                }
                if let Some(ref pub_name) = preflight_data.igdb_publisher {
                    igdb_metadata.insert("publisher".to_string(), pub_name.clone());
                }
                if let Some(ref genres) = preflight_data.igdb_genres {
                    igdb_metadata.insert("genres".to_string(), genres.clone());
                }
                if let Some(rating) = preflight_data.igdb_rating {
                    igdb_metadata.insert("rating".to_string(), rating.to_string());
                }
                if let Some(ref summary) = preflight_data.igdb_summary {
                    igdb_metadata.insert("summary".to_string(), summary.clone());
                }
                if let Some(ref platforms) = preflight_data.igdb_platforms {
                    igdb_metadata.insert("platforms".to_string(), platforms.join(","));
                }
                builder =
                    builder.with_custom_component("igdb", UploadComponent::Metadata(igdb_metadata));
            }

            // Add audio language data if available
            if !preflight_data.audio_languages.is_empty() {
                let mut audio_metadata = HashMap::new();
                audio_metadata.insert(
                    "audio_languages".to_string(),
                    preflight_data.audio_languages.join(","),
                );
                builder = builder
                    .with_custom_component("audio_data", UploadComponent::Metadata(audio_metadata));
            }

            // Add release name data
            let release_name_component =
                UploadComponent::ReleaseName(preflight_data.generated_release_name.clone());
            builder = builder.with_custom_component("release_name", release_name_component);

            // Skip duplicate check since it was already done in preflight
            if !preflight_data.dupe_check.is_empty() {
                info!(
                    "Skipping duplicate check - already completed in preflight: {}",
                    preflight_data.dupe_check
                );
            }
        }

        // Apply component configuration
        info!("ProcessBuilder: build_upload_data - Applying component configuration");
        if let Some(component_config) = &self.component_config {
            // Screenshots
            if component_config.screenshot.enabled {
                info!(
                    "ProcessBuilder: build_upload_data - Enabling screenshots: {}",
                    component_config.screenshot.count
                );
                builder = builder.with_screenshots(component_config.screenshot.count);
                // TODO: Pass screenshot layout to UploadBuilder
            }

            // MediaInfo
            if component_config.mediainfo.enabled {
                builder = builder.with_mediainfo();
            }

            // NFO
            if component_config.nfo.enabled {
                builder = builder.with_nfo();
            }

            // Sample
            if component_config.sample.enabled {
                builder = builder.with_sample();
            }

            // Cover Art
            if component_config.cover_art.enabled {
                builder = builder.with_cover_art();
            }
        } else {
            info!("ProcessBuilder: build_upload_data - Using default media-specific components");
            // Use default media-specific components
            match media_type {
                MediaType::Video(_) => {
                    info!("ProcessBuilder: build_upload_data - Video: enabling mediainfo, nfo (screenshots already configured by tracker)");
                    builder = builder.with_mediainfo().with_nfo();
                }
                MediaType::Audio(_) => {
                    info!(
                        "ProcessBuilder: build_upload_data - Audio: enabling mediainfo, cover_art"
                    );
                    builder = builder.with_mediainfo().with_cover_art();
                }
                MediaType::Ebook(_) => {
                    info!("ProcessBuilder: build_upload_data - Ebook: enabling screenshots(8), cover_art, nfo");
                    builder = builder.with_screenshots(8).with_cover_art().with_nfo();
                }
                MediaType::Game(_) => {
                    info!("ProcessBuilder: build_upload_data - Game: enabling screenshots(4), nfo");
                    builder = builder.with_screenshots(4).with_nfo();
                }
                MediaType::Hobby(_) => {
                    info!("ProcessBuilder: build_upload_data - Hobby: enabling nfo");
                    builder = builder.with_nfo();
                }
            }
        }

        // Apply tracker-specific configuration
        if let Some(ref seedpool_config) = self.seedpool_config {
            info!("ProcessBuilder: build_upload_data - Applying Seedpool tracker configuration");
            builder = builder.for_seedpool(seedpool_config);
        } else if let Some(ref torrentleech_config) = self.torrentleech_config {
            info!("ProcessBuilder: build_upload_data - Applying TorrentLeech tracker configuration");
            builder = builder.for_torrentleech(torrentleech_config);
        }

        info!("ProcessBuilder: build_upload_data - Calling UploadBuilder.build()");
        let result = builder.build();
        match &result {
            Ok(_) => info!(
                "ProcessBuilder: build_upload_data - UploadBuilder.build() completed: SUCCESS"
            ),
            Err(e) => error!(
                "ProcessBuilder: build_upload_data - UploadBuilder.build() FAILED with error: {}",
                e
            ),
        }
        result
    }

    /// Process the upload using UploadProcessor
    fn process_upload(
        &self,
        upload_data: &UploadData,
        classification: &Option<ClassificationResult>,
        media_type: &MediaType,
    ) -> Result<crate::processing::upload::UploadResult, String> {
        use crate::processing::upload::UploadProcessor;

        info!("🚨 DEBUG: Creating UploadProcessor with media_type: {:?}", media_type);
        // Use actual input path if available (for renamed files), otherwise use original
        let input_path_for_processor = upload_data.actual_input_path
            .as_ref()
            .unwrap_or(&self.input_path)
            .clone();
        
        let mut processor =
            UploadProcessor::new(upload_data.clone(), self.config.clone())
                .with_media_info(media_type.clone(), input_path_for_processor)
                .dry_run(self.dry_run);
        
        // Pass original torrent info if available
        if let Some(ref torrent_info) = self.original_torrent_info {
            processor = processor.with_original_torrent_info(torrent_info.clone());
        }

        // Add classification if available
        if let Some(classification) = classification {
            if let Some(category) = &classification.category {
                // Format category and source_type properly for tracker mapping
                let formatted_category = if category.starts_with("VideoCategory::") || 
                                            category.starts_with("AudioCategory::") || 
                                            category.starts_with("GameCategory::") ||
                                            category.starts_with("EbookCategory::") ||
                                            category.starts_with("HobbyCategory::") {
                    // Already formatted
                    Some(category.clone())
                } else {
                    // Infer media type from category and format appropriately
                    if category == "TvShow" || category == "Movie" || category == "MusicVideo" || category == "StandupComedy" {
                        Some(format!("VideoCategory::{}", category))
                    } else if category == "Music" || category == "Audiobook" || category == "Podcast" {
                        Some(format!("AudioCategory::{}", category))
                    } else if category == "PC" || category == "PlayStation" || category == "Xbox" || category == "Nintendo" {
                        Some(format!("GameCategory::{}", category))
                    } else if category == "Fiction" || category == "NonFiction" || category == "Textbook" {
                        Some(format!("EbookCategory::{}", category))
                    } else {
                        // Default to VideoCategory for unknown categories since most content is video
                        Some(format!("VideoCategory::{}", category))
                    }
                };

                let formatted_source_type = if let Some(source_type) = &classification.source_type {
                    if source_type.starts_with("VideoSourceType::") || 
                       source_type.starts_with("AudioSourceType::") ||
                       source_type.starts_with("GameSourceType::") {
                        // Already formatted
                        Some(source_type.clone())
                    } else {
                        // Infer media type from source type and format appropriately
                        if source_type == "Encode" || source_type == "Remux" || source_type == "WebRip" || 
                           source_type == "WebDL" || source_type == "HDTV" || source_type == "Bluray" ||
                           source_type == "DVD" || source_type == "HDDVD" || source_type == "TV" {
                            Some(format!("VideoSourceType::{}", source_type))
                        } else if source_type == "CD" || source_type == "WEB" || source_type == "Vinyl" {
                            Some(format!("AudioSourceType::{}", source_type))
                        } else {
                            // Default to VideoSourceType for unknown source types since most content is video
                            Some(format!("VideoSourceType::{}", source_type))
                        }
                    }
                } else {
                    None
                };

                processor = processor.with_media_classification(
                    formatted_category,
                    formatted_source_type,
                );
            }
        }

        info!("🚨 DEBUG: About to call UploadProcessor.process() - this should NOT trigger video processing");
        let result = processor.process();
        info!("🚨 DEBUG: UploadProcessor.process() completed with result: {:?}", result.is_ok());
        result
    }

    /// Generate preflight check data
    fn generate_preflight_data(
        &self,
        media_type: &MediaType,
        metadata: &JsonValue,
        classification: &Option<ClassificationResult>,
    ) -> Result<PreflightCheckResult, String> {
        use crate::metadata::tmdb::{fetch_external_ids, fetch_tmdb_id};
        use crate::processing::components::mediainfo_utils::generate_mediainfo;
        use crate::processing::naming::generate_release_name;
        use crate::utils::filter_files_by_extension;
        use std::path::Path;

        // Use metadata from classification if available, otherwise fall back to passed metadata
        let effective_metadata = if let Some(classification) = classification {
            info!(
                "Using classification metadata: {}",
                serde_json::to_string_pretty(&classification.media_metadata).unwrap_or_default()
            );
            &classification.media_metadata
        } else {
            info!(
                "No classification metadata, using default: {}",
                serde_json::to_string_pretty(metadata).unwrap_or_default()
            );
            metadata
        };

        let title = match media_type {
            crate::core::MediaType::Ebook(_) => {
                // For ebooks, prioritize filename (release name) over extracted PDF title
                effective_metadata
                    .get("filename")
                    .and_then(|t| t.as_str())
                    .or_else(|| effective_metadata.get("title").and_then(|t| t.as_str()))
                    .unwrap_or("Unknown")
                    .to_string()
            }
            _ => {
                effective_metadata
                    .get("title")
                    .and_then(|t| t.as_str())
                    .unwrap_or("Unknown")
                    .to_string()
            }
        };
        info!("Extracted title for preflight: '{}'", title);

        // Generate release name
        let base_name = Path::new(&self.input_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let generated_release_name = generate_release_name(&base_name);

        // Format release type with emojis
        let release_type = match media_type {
            crate::core::MediaType::Video(_) => {
                let category = effective_metadata["category"].as_str().unwrap_or("");
                if category.contains("TvShow") {
                    "📺 TV Show".to_string()
                } else {
                    "🎥 Movie".to_string()
                }
            }
            crate::core::MediaType::Audio(atype) => {
                format!("🎧 {}", format!("{:?}", atype).to_uppercase())
            }
            crate::core::MediaType::Ebook(_) => "📚 Ebook".to_string(),
            crate::core::MediaType::Game(_) => "🎮 Game".to_string(),
            crate::core::MediaType::Hobby(_) => "🎨 Hobby".to_string(),
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
            season_number: effective_metadata
                .get("season")
                .and_then(|s| s.as_u64())
                .map(|n| n as u32),
            episode_number: effective_metadata
                .get("episode")
                .and_then(|e| e.as_u64())
                .map(|n| n as u32),
            is_boxset: effective_metadata
                .get("is_boxset")
                .and_then(|b| b.as_bool())
                .unwrap_or(false),
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
                    let dupe_list = duplicates
                        .iter()
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
            crate::core::MediaType::Video(_) => {
                // Fetch TMDB info for movies and TV shows
                let category = effective_metadata["category"].as_str().unwrap_or("");
                let is_movie_or_tv = category.contains("Movie") || category.contains("TvShow");

                if is_movie_or_tv {
                    // Check if TMDB data is already in metadata (from extract_metadata)
                    if let Some(tmdb_id) = effective_metadata["tmdb_id"].as_u64() {
                        info!("📊 Using TMDB ID from metadata: {}", tmdb_id);
                        result.tmdb_id = tmdb_id as u32;

                        // Get other IDs from metadata if available
                        if let Some(imdb_id) = effective_metadata["imdb_id"].as_str() {
                            result.imdb_id = Some(imdb_id.to_string());
                        }
                        if let Some(tvdb_id) = effective_metadata["tvdb_id"].as_u64() {
                            result.tvdb_id = Some(tvdb_id as u32);
                        }
                    } else if !self.config.general.tmdb_api_key.is_empty() {
                        // Fallback: fetch TMDB data if not in metadata
                        info!("⚠️ TMDB data not found in metadata, fetching during preflight");
                        let year = effective_metadata["year"].as_u64().map(|y| y.to_string());
                        let release_type = if category.contains("Movie") {
                            "movie"
                        } else {
                            "tv"
                        };

                        match fetch_tmdb_id(
                            &title,
                            year,
                            &self.config.general.tmdb_api_key,
                            release_type,
                        ) {
                            Ok(tmdb_id) => {
                                result.tmdb_id = tmdb_id;

                                // Fetch external IDs (IMDb, TVDB) from TMDB
                                if tmdb_id > 0 {
                                    match fetch_external_ids(
                                        tmdb_id,
                                        release_type,
                                        &self.config.general.tmdb_api_key,
                                    ) {
                                        Ok((imdb_id, tvdb_id)) => {
                                            result.imdb_id = imdb_id;
                                            result.tvdb_id = tvdb_id;
                                        }
                                        Err(e) => {
                                            info!("Failed to fetch external IDs: {}", e);
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                info!("Failed to fetch TMDB ID: {}", e);
                            }
                        }
                    }
                }

                // Extract audio languages from mediainfo
                let video_extensions = crate::core::VideoType::all_extensions();
                if let Ok(files) = filter_files_by_extension(&self.input_path, &video_extensions) {
                    if let Some(first_file) = files.first() {
                        if let Some(file_path) = first_file.to_str() {
                            match generate_mediainfo(file_path, &self.config) {
                                Ok(mediainfo_output) => {
                                    result.audio_languages =
                                        Self::extract_audio_languages(&mediainfo_output);
                                    info!(
                                        "Extracted {} audio language(s) from mediainfo",
                                        result.audio_languages.len()
                                    );
                                }
                                Err(e) => {
                                    info!("Failed to generate mediainfo: {}", e);
                                }
                            }
                        }
                    }
                }
            }
            crate::core::MediaType::Audio(_) => {
                // Check for album cover
                result.album_cover = Self::check_for_cover_image(&self.input_path);

                // Use the format from metadata
                if let Some(format) = effective_metadata["format"].as_str() {
                    result.audio_languages = vec![format.to_string()];
                }
            }
            crate::core::MediaType::Ebook(_) => {
                // Check for cover image
                result.album_cover = Self::check_for_cover_image(&self.input_path);
            }
            crate::core::MediaType::Game(_) => {
                // IGDB lookup for games if credentials are available
                info!("Checking IGDB for game: {}", title);
                if !self.config.general.igdb_client_id.is_empty()
                    && !self.config.general.igdb_bearer_token.is_empty()
                {
                    info!("IGDB credentials found, searching for game...");
                    // Search for the game on IGDB
                    match crate::metadata::igdb::search_igdb_game(
                        &title,
                        &self.config.general.igdb_client_id,
                        &self.config.general.igdb_bearer_token,
                        &self.config,
                    ) {
                        Ok(games) if !games.is_empty() => {
                            info!("Found {} games on IGDB", games.len());
                            // Take the first result
                            if let Some(game) = games.first() {
                                // Store IGDB ID in metadata (similar to TMDB ID)
                                if let Some(igdb_id) = game["id"].as_u64() {
                                    info!("Found IGDB ID: {} for game: {}", igdb_id, title);

                                    // Get detailed game information
                                    match crate::metadata::igdb::get_igdb_game_details(
                                        igdb_id,
                                        &self.config.general.igdb_client_id,
                                        &self.config.general.igdb_bearer_token,
                                    ) {
                                        Ok(details) => {
                                            // Store IGDB ID
                                            result.igdb_id = Some(igdb_id);

                                            // Extract cover image URL
                                            if let Some(cover) = details.get("cover") {
                                                if crate::metadata::igdb::extract_igdb_cover_url(
                                                    cover,
                                                )
                                                .is_some()
                                                {
                                                    result.album_cover =
                                                        "Available (IGDB)".to_string();
                                                }
                                            }

                                            // Extract platforms
                                            if let Some(platforms) = details["platforms"].as_array()
                                            {
                                                let platform_names: Vec<String> = platforms
                                                    .iter()
                                                    .filter_map(|p| p["name"].as_str())
                                                    .map(|s| s.to_string())
                                                    .collect();
                                                if !platform_names.is_empty() {
                                                    result.igdb_platforms =
                                                        Some(platform_names.clone());
                                                    // Also keep in audio_languages for backward compatibility
                                                    result.audio_languages = platform_names;
                                                }
                                            }

                                            // Extract genres
                                            if let Some(genres) = details["genres"].as_array() {
                                                let genre_names: Vec<String> = genres
                                                    .iter()
                                                    .filter_map(|g| g["name"].as_str())
                                                    .map(|s| s.to_string())
                                                    .collect();
                                                if !genre_names.is_empty() {
                                                    result.igdb_genres =
                                                        Some(genre_names.join(", "));
                                                }
                                            }

                                            // Extract developer/publisher from involved_companies
                                            if let Some(companies) =
                                                details.get("involved_companies")
                                            {
                                                let (developers, publishers) =
                                                    crate::metadata::igdb::extract_igdb_companies(
                                                        companies,
                                                    );
                                                if !developers.is_empty() {
                                                    result.igdb_developer =
                                                        Some(developers.join(", "));
                                                }
                                                if !publishers.is_empty() {
                                                    result.igdb_publisher =
                                                        Some(publishers.join(", "));
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

                                            info!(
                                                "Successfully fetched IGDB game details for ID: {}",
                                                igdb_id
                                            );
                                            info!("IGDB data populated - Genres: {:?}, Developer: {:?}, Publisher: {:?}",
                                                  result.igdb_genres, result.igdb_developer, result.igdb_publisher);
                                        }
                                        Err(e) => {
                                            info!("Failed to fetch IGDB game details: {}", e);
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
                    info!("IGDB credentials not configured, skipping game lookup");
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
            } else if trimmed.is_empty()
                || trimmed.starts_with("Text")
                || trimmed.starts_with("Menu")
            {
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
                        let lang = lang_part
                            .trim()
                            .split('/')
                            .next()
                            .unwrap_or(lang_part.trim());
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
    
    /// Rename EPUB file to standard naming convention "lastname, firstname - title.epub"
    /// Returns the new path if successful
    fn rename_epub_file(&self, epub_path: &std::path::Path) -> Result<std::path::PathBuf, String> {
        // Extract metadata from EPUB to get proper title and author
        let (title_opt, author_opt) = crate::media::ebook::extract_metadata_from_epub(
            epub_path.to_str().unwrap_or("")
        ).map_err(|e| format!("Failed to extract EPUB metadata: {}", e))?;
        
        let title = title_opt.unwrap_or_else(|| {
            // Fallback to input path name if title extraction fails
            std::path::Path::new(&self.input_path)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("Unknown Title")
                .to_string()
        });
        let author = author_opt.unwrap_or_else(|| "Unknown Author".to_string());
        
        info!("📚 Renaming EPUB file with metadata - Title: '{}', Author: '{}'", title, author);
        
        // Format author as "lastname, firstname"
        let sanitized_author = {
            let parts: Vec<&str> = author.split_whitespace().collect();
            if parts.len() > 1 {
                format!("{}, {}", parts.last().unwrap(), parts[..parts.len() - 1].join(" "))
            } else {
                author.to_string()
            }
        };
        
        // Sanitize title for filename
        let sanitized_title = title
            .replace(".", " ")
            .replace(":", " ")
            .replace("'", "")
            .replace("/", " ")
            .replace("\\", " ")
            .replace("&", "and")
            .replace("?", "")
            .replace("*", "");
        
        let new_filename = format!("{} - {}.epub", sanitized_author, sanitized_title);
        let new_path = epub_path.with_file_name(&new_filename);
        
        info!("📚 Renaming EPUB: '{}' -> '{}'", 
              epub_path.file_name().and_then(|n| n.to_str()).unwrap_or("unknown"),
              new_filename);
        
        std::fs::rename(epub_path, &new_path)
            .map_err(|e| format!("Failed to rename EPUB file: {}", e))?;
        
        info!("✅ Successfully renamed EPUB file");
        Ok(new_path)
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
        .with_classification(true) // Enable classification for proper category/type mapping
        .with_upload_builder(true)
        .with_upload_processing(true)
        .with_duplicate_check(true)
        .with_metadata_extraction(true)
        .with_preflight_data(false)
        .dry_run(false)
}

/// Create a process builder configured for upload with preflight data reuse
pub fn upload_builder_with_preflight(
    input_path: &str,
    config: Arc<Config>,
    preflight_result: &ProcessResult,
) -> ProcessBuilder {
    let mut builder = ProcessBuilder::new(input_path, config)
        .with_classification(false) // Skip classification - use cached data
        .with_upload_builder(true)
        .with_upload_processing(true)
        .with_duplicate_check(false) // Skip duplicate check - already done in preflight
        .with_metadata_extraction(false) // Skip metadata extraction - use cached data
        .with_preflight_data(false) // Skip preflight generation - use cached data
        .dry_run(false);

    // Set cached data from preflight result
    if let Some(ref metadata) = preflight_result.metadata.as_object() {
        builder = builder.with_cached_metadata(preflight_result.metadata.clone());
    }

    if let Some(ref classification) = preflight_result.classification {
        builder = builder.with_cached_classification(classification.clone());
    }

    if let Some(ref preflight_data) = preflight_result.preflight_data {
        builder = builder.with_cached_preflight_data(preflight_data.clone());
    }

    builder
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
        .with_preflight_data(true) // Enable preflight data to get dupe check results
        .dry_run(true)
}
