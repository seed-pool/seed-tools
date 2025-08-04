use crate::core::{Config, MediaType, UploadComponent};
use crate::metadata::tmdb::{fetch_external_ids, fetch_tmdb_id, fetch_youtube_trailer};
use crate::processing::components::mediainfo_utils::generate_mediainfo;
use crate::processing::description::DescriptionConfig;
use crate::utils::{check_all_duplicates, find_and_read_nfo};
use log::{error, info};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;

/// Field mapping for tracker-specific upload forms
#[derive(Debug, Clone)]
pub struct TrackerFieldMapping {
    /// Maps internal field names to tracker-specific field names
    pub field_map: HashMap<String, String>,
    /// Required fields for this tracker
    pub required_fields: Vec<String>,
    /// Optional fields for this tracker
    pub optional_fields: Vec<String>,
    /// Custom validation rules for fields
    pub validation_rules: HashMap<String, String>,
}

impl TrackerFieldMapping {
    pub fn new() -> Self {
        Self {
            field_map: HashMap::new(),
            required_fields: Vec::new(),
            optional_fields: Vec::new(),
            validation_rules: HashMap::new(),
        }
    }

    /// Add a field mapping
    pub fn add_mapping(&mut self, internal_name: &str, tracker_name: &str) -> &mut Self {
        self.field_map
            .insert(internal_name.to_string(), tracker_name.to_string());
        self
    }

    /// Add a required field
    pub fn add_required(&mut self, field: &str) -> &mut Self {
        self.required_fields.push(field.to_string());
        self
    }

    /// Add an optional field
    pub fn add_optional(&mut self, field: &str) -> &mut Self {
        self.optional_fields.push(field.to_string());
        self
    }

    /// Get the tracker-specific field name
    pub fn get_field_name(&self, internal_name: &str) -> Option<&String> {
        self.field_map.get(internal_name)
    }
}

/// Configuration for the upload builder
#[derive(Debug, Clone)]
pub struct UploadConfig {
    pub dry_run: bool,
    pub skip_duplicate_check: bool,
    pub skip_mediainfo: bool,
    pub skip_nfo: bool,
    pub skip_screenshots: bool,
    pub skip_sample: bool,
    pub skip_tmdb: bool,
    pub skip_torrent_creation: bool,
    pub skip_cover_art: bool,
    pub screenshot_count: usize,
    pub announce_url: Option<String>,
}

impl Default for UploadConfig {
    fn default() -> Self {
        Self {
            dry_run: false,
            skip_duplicate_check: false,
            skip_mediainfo: false,
            skip_nfo: false,
            skip_screenshots: false,
            skip_sample: false,
            skip_tmdb: false,
            skip_torrent_creation: false,
            skip_cover_art: false,
            screenshot_count: 4,
            announce_url: None,
        }
    }
}

/// Builder for creating uploads
pub struct UploadBuilder {
    input_path: String,
    media_type: MediaType,
    config: Arc<Config>,
    components: HashMap<String, UploadComponent>,
    upload_config: UploadConfig,

    // Media-specific metadata
    video_metadata: Option<crate::media::video::VideoMetadata>,
    title: Option<String>,
    year: Option<String>,

    // Tracker configuration (loaded automatically)
    active_tracker: Option<String>,

    // File filtering
    accepted_extensions: Option<Vec<String>>,

    // Description configuration
    description_config: Option<DescriptionConfig>,

    // Template support
    enriched_metadata: Option<HashMap<String, String>>,
    template_name: Option<String>,

    // Cached metadata from ProcessBuilder
    cached_metadata: Option<serde_json::Value>,
    // Media classification handled by ProcessBuilder
}

impl UploadBuilder {
    /// Create a new upload builder
    pub fn new(input_path: impl Into<String>, media_type: MediaType, config: Arc<Config>) -> Self {
        Self {
            input_path: input_path.into(),
            media_type,
            config,
            components: HashMap::new(),
            upload_config: UploadConfig::default(),
            video_metadata: None,
            title: None,
            year: None,
            active_tracker: None,
            accepted_extensions: None,
            description_config: None,
            enriched_metadata: None,
            template_name: None,
            cached_metadata: None,
        }
    }

    /// Set the upload configuration
    pub fn with_config(mut self, upload_config: UploadConfig) -> Self {
        self.upload_config = upload_config;
        self
    }

    /// Set dry run mode
    pub fn dry_run(mut self, dry_run: bool) -> Self {
        self.upload_config.dry_run = dry_run;
        self
    }

    /// Set accepted file extensions for filtering
    pub fn with_extensions(mut self, extensions: Vec<&str>) -> Self {
        self.accepted_extensions = Some(extensions.iter().map(|s| s.to_string()).collect());
        self
    }

    /// Set description configuration
    pub fn with_description_config(mut self, config: DescriptionConfig) -> Self {
        self.description_config = Some(config);
        self
    }

    /// Set video metadata (for video uploads)
    pub fn with_video_metadata(mut self, metadata: crate::media::video::VideoMetadata) -> Self {
        self.title = Some(metadata.title.clone());
        self.year = metadata.year.map(|y| y.to_string());
        self.video_metadata = Some(metadata);
        self
    }

    /// Set title and year (for non-video uploads)
    pub fn with_title_info(
        mut self,
        title: impl Into<String>,
        year: Option<impl Into<String>>,
    ) -> Self {
        self.title = Some(title.into());
        self.year = year.map(|y| y.into());
        self
    }

    /// Set enriched metadata for template processing
    pub fn with_enriched_metadata(mut self, metadata: HashMap<String, String>) -> Self {
        self.enriched_metadata = Some(metadata);
        self
    }

    /// Set cached metadata from ProcessBuilder
    pub fn with_cached_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.cached_metadata = Some(metadata);
        self
    }

    /// Set template name for description generation
    pub fn with_template(mut self, template_name: impl Into<String>) -> Self {
        self.template_name = Some(template_name.into());
        self
    }

    /// Enable NFO detection and reading
    pub fn with_nfo(mut self) -> Self {
        self.upload_config.skip_nfo = false;
        self
    }

    /// Enable mediainfo generation
    pub fn with_mediainfo(mut self) -> Self {
        self.upload_config.skip_mediainfo = false;
        self
    }

    /// Enable screenshot generation
    pub fn with_screenshots(mut self, count: usize) -> Self {
        self.upload_config.skip_screenshots = false;
        self.upload_config.screenshot_count = count;
        self
    }

    /// Enable sample creation
    pub fn with_sample(mut self) -> Self {
        self.upload_config.skip_sample = false;
        self
    }

    /// Enable duplicate checking
    pub fn with_duplicate_check(mut self) -> Self {
        self.upload_config.skip_duplicate_check = false;
        self
    }

    /// Enable TMDB lookup
    pub fn with_tmdb_lookup(mut self) -> Self {
        self.upload_config.skip_tmdb = false;
        self
    }

    /// Enable torrent creation with announce URL
    pub fn with_torrent_creation(mut self, announce_url: impl Into<String>) -> Self {
        self.upload_config.skip_torrent_creation = false;
        self.upload_config.announce_url = Some(announce_url.into());
        self
    }

    /// Enable cover art extraction for audio files
    pub fn with_cover_art(mut self) -> Self {
        self.upload_config.skip_cover_art = false;
        self
    }

    /// Add a custom component
    pub fn with_custom_component(
        mut self,
        name: impl Into<String>,
        component: UploadComponent,
    ) -> Self {
        self.components.insert(name.into(), component);
        self
    }

    /// Skip a specific component
    pub fn skip_component(mut self, component: &str) -> Self {
        match component {
            "nfo" => self.upload_config.skip_nfo = true,
            "mediainfo" => self.upload_config.skip_mediainfo = true,
            "screenshots" => self.upload_config.skip_screenshots = true,
            "sample" => self.upload_config.skip_sample = true,
            "duplicate_check" => self.upload_config.skip_duplicate_check = true,
            "tmdb" => self.upload_config.skip_tmdb = true,
            "torrent" => self.upload_config.skip_torrent_creation = true,
            _ => info!("Unknown component to skip: {}", component),
        }
        self
    }

    /// Detect and apply the active tracker configuration
    fn apply_tracker_config(&mut self) -> Result<(), String> {
        info!("UploadBuilder: apply_tracker_config - Loading tracker configurations");
        // Load tracker configs
        let seedpool_config = crate::utils::load_tracker_config::<crate::core::SeedpoolConfig>(
            "seedpool",
        )
        .map_err(|e| {
            error!(
                "UploadBuilder: apply_tracker_config - Failed to load seedpool config: {}",
                e
            );
            format!("Failed to load seedpool config: {}", e)
        })?;
        info!("UploadBuilder: apply_tracker_config - Seedpool config loaded successfully");

        let torrentleech_config =
            crate::utils::load_tracker_config::<crate::core::TorrentLeechConfig>("torrentleech")
                .map_err(|e| {
                    error!(
                "UploadBuilder: apply_tracker_config - Failed to load torrentleech config: {}",
                e
            );
                    format!("Failed to load torrentleech config: {}", e)
                })?;
        info!("UploadBuilder: apply_tracker_config - TorrentLeech config loaded successfully");

        // Determine which tracker is enabled and apply its settings
        info!("UploadBuilder: apply_tracker_config - Determining active tracker: seedpool.enabled={}, torrentleech.enabled={}", 
            seedpool_config.general.enabled, torrentleech_config.general.enabled);
        if seedpool_config.general.enabled {
            info!("UploadBuilder: apply_tracker_config - Applying Seedpool configuration");
            self.active_tracker = Some("seedpool".to_string());

            // Apply Seedpool settings
            self.upload_config.skip_duplicate_check =
                !seedpool_config.settings.enable_duplicate_check;
            self.upload_config.skip_mediainfo = !seedpool_config.settings.enable_mediainfo;
            self.upload_config.skip_nfo = !seedpool_config.settings.enable_nfo;
            self.upload_config.skip_screenshots = !seedpool_config.settings.enable_screenshots;
            self.upload_config.skip_sample = !seedpool_config.settings.enable_sample;
            self.upload_config.skip_tmdb = !seedpool_config.settings.enable_tmdb;
            self.upload_config.skip_torrent_creation =
                !seedpool_config.settings.enable_torrent_creation;
            self.upload_config.screenshot_count = seedpool_config.settings.screenshot_count;

            if seedpool_config.settings.enable_torrent_creation {
                self.upload_config.announce_url =
                    Some(seedpool_config.settings.announce_url.clone());
            }

            // Store tracker-specific metadata
            let mut metadata = HashMap::new();
            metadata.insert("tracker".to_string(), "seedpool".to_string());
            metadata.insert(
                "stripshit".to_string(),
                seedpool_config.settings.stripshit_from_videos.to_string(),
            );
            metadata.insert(
                "remote_path".to_string(),
                seedpool_config.screenshots.remote_path.clone(),
            );
            metadata.insert(
                "image_path".to_string(),
                seedpool_config.screenshots.image_path.clone(),
            );
            metadata.insert(
                "custom_description".to_string(),
                seedpool_config.settings.custom_description.clone(),
            );
            self.components.insert(
                "tracker_config".to_string(),
                UploadComponent::Metadata(metadata),
            );
        } else if torrentleech_config.general.enabled {
            info!("UploadBuilder: apply_tracker_config - Applying TorrentLeech configuration");
            self.active_tracker = Some("torrentleech".to_string());

            // Apply TorrentLeech settings
            self.upload_config.skip_duplicate_check =
                !torrentleech_config.settings.enable_duplicate_check;
            self.upload_config.skip_mediainfo = !torrentleech_config.settings.enable_mediainfo;
            self.upload_config.skip_nfo = !torrentleech_config.settings.enable_nfo;
            self.upload_config.skip_screenshots = !torrentleech_config.settings.enable_screenshots;
            self.upload_config.skip_sample = !torrentleech_config.settings.enable_sample;
            self.upload_config.skip_tmdb = !torrentleech_config.settings.enable_tmdb;
            self.upload_config.skip_torrent_creation =
                !torrentleech_config.settings.enable_torrent_creation;
            self.upload_config.screenshot_count = torrentleech_config.settings.screenshot_count;

            if torrentleech_config.settings.enable_torrent_creation
                && !torrentleech_config.general.announce_url_1.is_empty()
            {
                self.upload_config.announce_url =
                    Some(torrentleech_config.general.announce_url_1.clone());
            }

            // Store tracker-specific metadata
            let mut metadata = HashMap::new();
            metadata.insert("tracker".to_string(), "torrentleech".to_string());
            metadata.insert(
                "stripshit".to_string(),
                torrentleech_config
                    .settings
                    .stripshit_from_videos
                    .to_string(),
            );
            metadata.insert(
                "custom_description".to_string(),
                torrentleech_config.settings.custom_description.clone(),
            );
            self.components.insert(
                "tracker_config".to_string(),
                UploadComponent::Metadata(metadata),
            );
        } else {
            info!("UploadBuilder: apply_tracker_config - No tracker is enabled in configuration");
            self.active_tracker = None;
        }

        info!("UploadBuilder: apply_tracker_config - Configuration applied successfully, active_tracker: {:?}", self.active_tracker);
        Ok(())
    }

    /// Build the upload data
    pub fn build(mut self) -> Result<crate::media::video::UploadData, String> {
        info!("UploadBuilder: Starting build process");
        // Apply tracker configuration automatically
        info!("UploadBuilder: Applying tracker configuration");
        self.apply_tracker_config()?;
        info!("UploadBuilder: Tracker configuration applied successfully");

        info!(
            "Building upload data for: {} (tracker: {:?})",
            self.input_path, self.active_tracker
        );

        // Process NFO
        if !self.upload_config.skip_nfo {
            match find_and_read_nfo(&self.input_path) {
                Ok(nfo_data) => {
                    if let Some((path, content)) = nfo_data {
                        info!("Found NFO file: {}", path);
                        self.components.insert(
                            "nfo".to_string(),
                            UploadComponent::NfoData { path, content },
                        );
                    }
                }
                Err(e) => info!("Failed to find/read NFO: {}", e),
            }
        }

        // Process Mediainfo
        if !self.upload_config.skip_mediainfo {
            match generate_mediainfo(&self.input_path, &self.config) {
                Ok(mediainfo) => {
                    info!("Generated mediainfo");
                    self.components.insert(
                        "mediainfo".to_string(),
                        UploadComponent::Mediainfo(mediainfo),
                    );
                }
                Err(e) => info!("Failed to generate mediainfo: {}", e),
            }
        }

        // Process Duplicate Check
        info!(
            "UploadBuilder: Checking duplicate configuration: skip_duplicate_check={}",
            self.upload_config.skip_duplicate_check
        );
        if !self.upload_config.skip_duplicate_check {
            info!("UploadBuilder: Starting duplicate check");
            let check_title = self
                .title
                .as_ref()
                .or_else(|| self.video_metadata.as_ref().map(|m| &m.title))
                .ok_or("No title available for duplicate check")?;

            match check_all_duplicates(check_title) {
                Ok(duplicates) => {
                    if !duplicates.is_empty() {
                        info!("Found {} duplicate(s)", duplicates.len());
                        self.components.insert(
                            "duplicates".to_string(),
                            UploadComponent::DuplicateCheckResults(duplicates),
                        );
                    } else {
                        info!("No duplicates found");
                    }
                }
                Err(e) => info!("Failed to check duplicates: {}", e),
            }
        }

        // Process TMDB lookup (for video content)
        info!(
            "UploadBuilder: Checking TMDB configuration: skip_tmdb={}, media_type={:?}",
            self.upload_config.skip_tmdb, self.media_type
        );
        if !self.upload_config.skip_tmdb && matches!(self.media_type, MediaType::Video(_)) {
            info!("UploadBuilder: Starting TMDB lookup process");
            // Check if we already have TMDB data from components (e.g., from preflight)
            if let Some(UploadComponent::TmdbData {
                tmdb_id,
                imdb_id,
                tvdb_id,
                ..
            }) = self.components.get("tmdb")
            {
                info!("Using TMDB data from preflight: TMDB ID: {}", tmdb_id);
                info!("Enriched metadata should already be set from preflight");

                // Just keep the existing component, don't fetch again
                self.components.insert(
                    "tmdb".to_string(),
                    UploadComponent::TmdbData {
                        tmdb_id: *tmdb_id,
                        imdb_id: imdb_id.clone(),
                        tvdb_id: *tvdb_id,
                        title: self
                            .video_metadata
                            .as_ref()
                            .map(|m| m.title.clone())
                            .unwrap_or_default(),
                        year: self
                            .video_metadata
                            .as_ref()
                            .and_then(|m| m.year.map(|y| y.to_string())),
                    },
                );
            } else if let Some(metadata) = &self.video_metadata {
                info!("UploadBuilder: No cached TMDB component found, checking enriched metadata");

                // First check if we have TMDB data in enriched metadata
                let has_tmdb_in_enriched = self
                    .enriched_metadata
                    .as_ref()
                    .map(|em| {
                        let has_data = em.contains_key("tmdb_overview")
                            || em.contains_key("tmdb_title")
                            || em.contains_key("tmdb_directors");
                        info!(
                            "📊 Enriched metadata check: has TMDB data = {}, total fields = {}",
                            has_data,
                            em.len()
                        );
                        if has_data {
                            info!("✅ Found TMDB data in enriched metadata, skipping API call");
                            for (key, value) in em.iter().filter(|(k, _)| k.starts_with("tmdb_")) {
                                info!("  📌 {} = {}", key, value);
                            }
                        }
                        has_data
                    })
                    .unwrap_or(false);

                if has_tmdb_in_enriched {
                    // Extract IDs from enriched metadata and create TmdbData component
                    if let Some(enriched) = &self.enriched_metadata {
                        let tmdb_id = enriched.get("tmdb_id")
                            .and_then(|v| v.parse::<u32>().ok())
                            .unwrap_or(0);
                        let imdb_id = enriched.get("imdb_id")
                            .map(|s| {
                                // Strip "tt" prefix if present
                                if s.starts_with("tt") {
                                    s[2..].to_string()
                                } else {
                                    s.to_string()
                                }
                            });
                        let tvdb_id = enriched.get("tvdb_id")
                            .and_then(|v| v.parse::<u32>().ok());

                        if tmdb_id > 0 {
                            info!("✅ Creating TmdbData component from enriched metadata: TMDB ID: {}", tmdb_id);
                            let tmdb_component = UploadComponent::TmdbData {
                                tmdb_id,
                                imdb_id,
                                tvdb_id,
                                title: metadata.title.clone(),
                                year: metadata.year.map(|y| y.to_string()),
                            };
                            self.components.insert("tmdb".to_string(), tmdb_component);
                        }
                    }
                } else if !has_tmdb_in_enriched {
                    info!(
                        "⚠️ No TMDB data in enriched metadata, performing fresh lookup for: {}",
                        metadata.title
                    );
                    // Check if it's a movie or TV show based on metadata
                    let is_movie_or_tv = match &metadata.category {
                        cat if format!("{:?}", cat).contains("Movie") => true,
                        cat if format!("{:?}", cat).contains("TvShow") => true,
                        _ => false,
                    };

                    if is_movie_or_tv {
                        info!("UploadBuilder: Detected movie/TV show, fetching TMDB data");
                        let release_type = if format!("{:?}", metadata.category).contains("Movie") {
                            "Movie"
                        } else {
                            "TvShow"
                        };

                        match fetch_tmdb_id(
                            &metadata.title,
                            metadata.year.map(|y| y.to_string()),
                            &self.config.general.tmdb_api_key,
                            release_type,
                        ) {
                            Ok(tmdb_id) => {
                                info!("Found TMDB ID: {}", tmdb_id);

                                // Fetch IMDB/TVDB IDs
                                let (imdb_id, tvdb_id) = match fetch_external_ids(
                                    tmdb_id,
                                    release_type,
                                    &self.config.general.tmdb_api_key,
                                ) {
                                    Ok((imdb, tvdb)) => {
                                        if let Some(ref imdb_id) = imdb {
                                            info!("Found IMDB ID: {}", imdb_id);
                                        }
                                        if let Some(ref tvdb_id) = tvdb {
                                            info!("Found TVDB ID: {}", tvdb_id);
                                        }
                                        (imdb, tvdb)
                                    }
                                    Err(e) => {
                                        info!("Failed to fetch external IDs: {}", e);
                                        (None, None)
                                    }
                                };

                                // Fetch full TMDB details for enriched metadata
                                match crate::metadata::tmdb::fetch_tmdb_details(
                                    tmdb_id,
                                    release_type,
                                    &self.config.general.tmdb_api_key,
                                ) {
                                    Ok(tmdb_details) => {
                                        info!("Fetched TMDB details successfully");

                                        // Extract enriched metadata
                                        let tmdb_metadata =
                                            crate::metadata::tmdb::extract_tmdb_metadata(
                                                &tmdb_details,
                                                release_type,
                                            );

                                        // Merge TMDB metadata with existing enriched metadata
                                        if let Some(ref mut enriched) = self.enriched_metadata {
                                            enriched.extend(tmdb_metadata);
                                        } else {
                                            self.enriched_metadata = Some(tmdb_metadata);
                                        }
                                    }
                                    Err(e) => {
                                        info!("Failed to fetch TMDB details: {}", e);
                                    }
                                }

                                self.components.insert(
                                    "tmdb".to_string(),
                                    UploadComponent::TmdbData {
                                        tmdb_id,
                                        imdb_id,
                                        tvdb_id,
                                        title: metadata.title.clone(),
                                        year: metadata.year.map(|y| y.to_string()),
                                    },
                                );

                                // Fetch YouTube trailer if YouTube API key is configured
                                if let Some(ref youtube_api_key) =
                                    self.config.general.youtube_api_key
                                {
                                    if !youtube_api_key.is_empty() {
                                        match fetch_youtube_trailer(
                                            &metadata.title,
                                            metadata.year.map(|y| y.to_string()).as_deref(),
                                            youtube_api_key,
                                        ) {
                                            Ok(trailer_url) => {
                                                info!("Found YouTube trailer: {}", trailer_url);
                                                self.components.insert(
                                                    "trailer".to_string(),
                                                    UploadComponent::Trailer {
                                                        url: trailer_url,
                                                        platform: "YouTube".to_string(),
                                                    },
                                                );
                                            }
                                            Err(e) => {
                                                info!("No YouTube trailer found: {}", e);
                                            }
                                        }
                                    }
                                }
                            }
                            Err(e) => info!("Failed to fetch TMDB ID: {}", e),
                        }
                    }
                }
            }
        }

        // Process IGDB (for game content)
        if matches!(self.media_type, MediaType::Game(_)) {
            info!("UploadBuilder: Checking IGDB configuration: skip_igdb=false, media_type={:?}", self.media_type);
            
            // Check if we already have an IGDB component
            if !self.components.contains_key("igdb") {
                info!("UploadBuilder: No cached IGDB component found, checking enriched metadata");

                // Check if we have IGDB data in enriched metadata
                let has_igdb_in_enriched = self
                    .enriched_metadata
                    .as_ref()
                    .map(|em| {
                        let has_data = em.contains_key("igdb_summary")
                            || em.contains_key("igdb_title")
                            || em.contains_key("igdb_developer");
                        info!(
                            "📊 Enriched metadata check: has IGDB data = {}, total fields = {}",
                            has_data,
                            em.len()
                        );
                        if has_data {
                            info!("✅ Found IGDB data in enriched metadata, skipping API call");
                            for (key, value) in em.iter().filter(|(k, _)| k.starts_with("igdb_")) {
                                info!("  📌 {} = {}", key, value);
                            }
                        }
                        has_data
                    })
                    .unwrap_or(false);

                if has_igdb_in_enriched {
                    // Extract IGDB ID from enriched metadata and create IGDB component
                    if let Some(enriched) = &self.enriched_metadata {
                        let igdb_id = enriched.get("igdb_id")
                            .and_then(|v| v.parse::<u64>().ok())
                            .unwrap_or(0);

                        if igdb_id > 0 {
                            info!("✅ Creating IGDB component from enriched metadata: IGDB ID: {}", igdb_id);
                            
                            // Create IGDB component (placeholder for now)
                            // TODO: Create proper IGDB component type if needed
                        }
                    }
                } else {
                    info!("⚠️ No IGDB data in enriched metadata for game");
                }
            }
        }

        // Process IGDB Screenshots (for game content)
        if !self.upload_config.skip_screenshots && matches!(self.media_type, MediaType::Game(_)) {
            info!("UploadBuilder: Checking IGDB screenshot processing for games");
            
            // Get IGDB screenshots from enriched metadata
            if let Some(enriched) = &self.enriched_metadata {
                if let Some(screenshots_str) = enriched.get("igdb_screenshots") {
                    let screenshot_urls: Vec<&str> = screenshots_str
                        .split(',')
                        .map(|s| s.trim())
                        .filter(|s| !s.is_empty())
                        .collect();
                    
                    if !screenshot_urls.is_empty() {
                        info!("🎮 Found {} IGDB screenshots to download", screenshot_urls.len());
                        
                        // Get tracker config for CDN paths
                        let mut remote_path = None;
                        let mut image_path = None;
                        let mut imgbb_api_key = None;

                        if let Some(UploadComponent::Metadata(tracker_metadata)) =
                            self.components.get("tracker_config")
                        {
                            remote_path = tracker_metadata.get("remote_path").map(|s| s.as_str());
                            image_path = tracker_metadata.get("image_path").map(|s| s.as_str());
                            // Check for tracker-specific ImgBB key if available
                            if let Some(key) = tracker_metadata.get("imgbb_api_key") {
                                if !key.is_empty() {
                                    imgbb_api_key = Some(key.as_str());
                                }
                            }
                        }

                        // Fall back to global ImgBB config if no tracker-specific key
                        if imgbb_api_key.is_none() {
                            imgbb_api_key =
                                self.config.imgbb.as_ref().map(|c| c.imgbb_api_key.as_str());
                        }

                        // Check if we have CDN configuration
                        let has_cdn = remote_path.is_some() && image_path.is_some();
                        let has_imgbb = imgbb_api_key.map_or(false, |key| !key.is_empty());

                        if !has_cdn && !has_imgbb {
                            info!("⚠️ No CDN or ImgBB configuration found, skipping IGDB screenshot upload");
                        } else {
                            let mut uploaded_screenshots = Vec::new();
                            let mut uploaded_thumbnails = Vec::new();
                            
                            // Use configured screenshot count
                            let max_screenshots = self.upload_config.screenshot_count;
                            let screenshots_to_process = screenshot_urls.iter().take(max_screenshots);
                            
                            // Generate sanitized base name for files
                            let sanitized_input_name = if let Some(title) = enriched.get("igdb_title") {
                                crate::processing::naming::generate_release_name(title)
                            } else if let Some(filename) = enriched.get("filename") {
                                crate::processing::naming::generate_release_name(filename)
                            } else {
                                "game".to_string()
                            };

                            // Ensure the output directory exists for CDN
                            if has_cdn {
                                let output_dir = &self.config.paths.screenshots_dir;
                                if let Err(e) = std::fs::create_dir_all(output_dir) {
                                    info!("❌ Failed to create screenshots directory: {}", e);
                                } else {
                                    info!("🎮 Using CDN for IGDB screenshot upload");
                                    
                                    for (index, &url) in screenshots_to_process.enumerate() {
                                        info!("📸 Downloading IGDB screenshot {}/{}: {}", index + 1, max_screenshots, url);
                                        
                                        // Download the image
                                        match reqwest::blocking::get(url) {
                                            Ok(response) => {
                                                if response.status().is_success() {
                                                    match response.bytes() {
                                                        Ok(image_bytes) => {
                                                            // Save to screenshots directory
                                                            let screenshot_file = format!("{}/{}_{}.jpg", output_dir, sanitized_input_name, index + 1);
                                                            let thumbnail_file = format!("{}/{}_{}_thumb.jpg", output_dir, sanitized_input_name, index + 1);
                                                            
                                                            match std::fs::write(&screenshot_file, &image_bytes) {
                                                                Ok(_) => {
                                                                    // Create thumbnail by copying the same image (IGDB screenshots are already appropriately sized)
                                                                    if let Err(e) = std::fs::copy(&screenshot_file, &thumbnail_file) {
                                                                        info!("⚠️ Failed to create thumbnail, using original: {}", e);
                                                                    }
                                                                    
                                                                    // Set permissions
                                                                    #[cfg(unix)]
                                                                    {
                                                                        use std::os::unix::fs::PermissionsExt;
                                                                        let _ = std::fs::set_permissions(&screenshot_file, std::fs::Permissions::from_mode(0o777));
                                                                        let _ = std::fs::set_permissions(&thumbnail_file, std::fs::Permissions::from_mode(0o777));
                                                                    }
                                                                    
                                                                    // Upload to CDN
                                                                    if !self.upload_config.dry_run {
                                                                        let remote = remote_path.unwrap();
                                                                        match crate::utils::upload_to_cdn(&screenshot_file, &format!("{}/screenshots/", remote.trim_end_matches('/'))) {
                                                                            Ok(_) => {
                                                                                info!("✅ Uploaded IGDB screenshot to CDN");
                                                                                match crate::utils::upload_to_cdn(&thumbnail_file, &format!("{}/screenshots/", remote.trim_end_matches('/'))) {
                                                                                    Ok(_) => info!("✅ Uploaded IGDB thumbnail to CDN"),
                                                                                    Err(e) => info!("❌ Failed to upload IGDB thumbnail to CDN: {}", e),
                                                                                }
                                                                            }
                                                                            Err(e) => info!("❌ Failed to upload IGDB screenshot to CDN: {}", e),
                                                                        }
                                                                    } else {
                                                                        info!("[DRY RUN] Skipping IGDB screenshot/thumbnail upload to CDN");
                                                                    }
                                                                    
                                                                    // Add public-facing URLs
                                                                    let screenshot_filename = std::path::Path::new(&screenshot_file)
                                                                        .file_name()
                                                                        .unwrap()
                                                                        .to_string_lossy();
                                                                    let thumbnail_filename = std::path::Path::new(&thumbnail_file)
                                                                        .file_name()
                                                                        .unwrap()
                                                                        .to_string_lossy();

                                                                    let image = image_path.unwrap();
                                                                    uploaded_screenshots.push(format!("{}/{}", image, screenshot_filename));
                                                                    uploaded_thumbnails.push(format!("{}/{}", image, thumbnail_filename));
                                                                }
                                                                Err(e) => info!("❌ Failed to save IGDB screenshot: {}", e),
                                                            }
                                                        }
                                                        Err(e) => info!("❌ Failed to read image bytes: {}", e),
                                                    }
                                                } else {
                                                    info!("❌ Failed to download IGDB screenshot: HTTP {}", response.status());
                                                }
                                            }
                                            Err(e) => info!("❌ Failed to download IGDB screenshot: {}", e),
                                        }
                                    }
                                }
                            } else if has_imgbb {
                                info!("🎮 Using ImgBB for IGDB screenshot upload");
                                let api_key = imgbb_api_key.unwrap();
                                
                                for (index, &url) in screenshots_to_process.enumerate() {
                                    info!("📸 Downloading IGDB screenshot {}/{}: {}", index + 1, max_screenshots, url);
                                    
                                    // Download the image
                                    match reqwest::blocking::get(url) {
                                        Ok(response) => {
                                            if response.status().is_success() {
                                                match response.bytes() {
                                                    Ok(image_bytes) => {
                                                        // Save to temporary file
                                                        let temp_dir = std::env::temp_dir();
                                                        let temp_filename = format!("igdb_screenshot_{}_{}.jpg", index, chrono::Utc::now().timestamp());
                                                        let temp_path = temp_dir.join(temp_filename);
                                                        
                                                        match std::fs::write(&temp_path, &image_bytes) {
                                                            Ok(_) => {
                                                                // Upload to ImgBB
                                                                if let Some(temp_path_str) = temp_path.to_str() {
                                                                    match crate::utils::upload_to_imgbb(temp_path_str, api_key, self.upload_config.dry_run) {
                                                                        Ok((imgbb_url, _thumb_url)) => {
                                                                            info!("✅ Uploaded IGDB screenshot to ImgBB: {}", imgbb_url);
                                                                            uploaded_screenshots.push(imgbb_url.clone());
                                                                            uploaded_thumbnails.push(imgbb_url); // Use same URL for thumbnail
                                                                        }
                                                                        Err(e) => info!("❌ Failed to upload IGDB screenshot to ImgBB: {}", e),
                                                                    }
                                                                }
                                                                // Clean up temp file
                                                                let _ = std::fs::remove_file(&temp_path);
                                                            }
                                                            Err(e) => info!("❌ Failed to save temporary file: {}", e),
                                                        }
                                                    }
                                                    Err(e) => info!("❌ Failed to read image bytes: {}", e),
                                                }
                                            } else {
                                                info!("❌ Failed to download IGDB screenshot: HTTP {}", response.status());
                                            }
                                        }
                                        Err(e) => info!("❌ Failed to download IGDB screenshot: {}", e),
                                    }
                                }
                            }
                            
                            if !uploaded_screenshots.is_empty() {
                                info!("✅ Successfully processed {} IGDB screenshots", uploaded_screenshots.len());
                                self.components.insert(
                                    "screenshots".to_string(),
                                    UploadComponent::Screenshots(uploaded_screenshots.clone()),
                                );
                                self.components.insert(
                                    "thumbnails".to_string(),
                                    UploadComponent::Thumbnails(uploaded_thumbnails),
                                );
                            }
                        }
                    } else {
                        info!("⚠️ No IGDB screenshots found in metadata");
                    }
                } else {
                    info!("⚠️ No IGDB screenshot data found in enriched metadata");
                }
            } else {
                info!("⚠️ No enriched metadata available for IGDB screenshots");
            }
        }

        // Process Screenshots (for ebook content - includes all ebook types)
        if !self.upload_config.skip_screenshots {
            if let MediaType::Ebook(_ebook_type) = &self.media_type {
            info!("UploadBuilder: Checking ebook screenshot processing");
            
            // Find ebook files (PDF, CBR, CBZ, EPUB)
            let ebook_extensions = ["pdf", "cbr", "cbz", "epub"];
            match crate::utils::filter_files_by_extension(&self.input_path, &ebook_extensions) {
                Ok(files) if !files.is_empty() => {
                    let ebook_file = &files[0];
                    info!("📚 Found ebook file for screenshot processing: {}", ebook_file.display());
                    
                    // Get tracker config for CDN paths
                    let mut remote_path = None;
                    let mut image_path = None;
                    
                    if let Some(UploadComponent::Metadata(tracker_metadata)) = 
                        self.components.get("tracker_config") 
                    {
                        remote_path = tracker_metadata.get("remote_path").map(|s| s.as_str());
                        image_path = tracker_metadata.get("image_path").map(|s| s.as_str());
                    }
                    
                    if let (Some(remote), Some(image)) = (remote_path, image_path) {
                        // Determine input name for screenshots
                        let input_name = std::path::Path::new(&self.input_path)
                            .file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or("ebook");
                            
                        info!("📚 Generating ebook screenshots for: {}", input_name);
                        
                        // Generate ebook description (may include rich Open Library content)
                        match crate::media::ebook::generate_ebook_description(
                            ebook_file.to_str().unwrap_or(""),
                            input_name,
                            remote,
                            image,
                            self.upload_config.dry_run,
                            &self.config,
                        ) {
                            Ok(description) => {
                                info!("✅ Successfully generated ebook description");
                                // Store the description (rich Open Library or basic with screenshots)
                                self.components.insert(
                                    "description".to_string(),
                                    UploadComponent::Description(description),
                                );
                            }
                            Err(e) => {
                                info!("❌ Failed to generate ebook description: {}", e);
                            }
                        }
                    } else {
                        info!("⚠️ No CDN configuration found for ebook screenshots");
                    }
                }
                Ok(_) => {
                    info!("⚠️ No ebook files found for screenshot processing");
                }
                Err(e) => {
                    info!("❌ Failed to find ebook files: {}", e);
                }
            }
            }
        }

        // Process Screenshots (for video content)
        info!("UploadBuilder: Checking screenshot configuration: skip_screenshots={}, media_type={:?}", 
            self.upload_config.skip_screenshots, self.media_type);
        if !self.upload_config.skip_screenshots && matches!(self.media_type, MediaType::Video(_)) {
            info!("UploadBuilder: Starting screenshot generation");
            // Get the appropriate extensions for the media type
            let extensions = self
                .accepted_extensions
                .as_ref()
                .map(|exts| exts.iter().map(|s| s.as_str()).collect::<Vec<_>>())
                .unwrap_or_else(|| crate::core::types::VideoType::all_extensions());

            // Find video files using filter_files_by_extension
            match crate::utils::filter_files_by_extension(&self.input_path, &extensions) {
                Ok(files) if !files.is_empty() => {
                    let video_file = &files[0];
                    // Determine input name for screenshots - use release name with dots preserved
                    info!(
                        "Determining screenshot name - video_metadata exists: {}",
                        self.video_metadata.is_some()
                    );
                    if let Some(metadata) = &self.video_metadata {
                        info!("Video metadata release_name: {}", metadata.release_name);
                    }

                    let input_name = if let Some(metadata) = &self.video_metadata {
                        metadata.release_name.as_str()
                    } else {
                        // Fallback to directory/file name
                        let path = std::path::Path::new(&self.input_path);
                        info!("No video metadata, using path: {}", self.input_path);
                        let name = if path.is_dir() {
                            // For directories, use the directory name
                            path.file_name()
                                .and_then(|s| s.to_str())
                                .unwrap_or("unknown")
                        } else {
                            // For files, use the file stem
                            path.file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or("unknown")
                        };
                        info!("Using fallback name for screenshots: {}", name);
                        name
                    };

                    info!("Final input_name for screenshots: '{}'", input_name);

                    // Get tracker config to access CDN paths
                    let mut remote_path = None;
                    let mut image_path = None;
                    let mut imgbb_api_key = None;

                    if let Some(UploadComponent::Metadata(tracker_metadata)) =
                        self.components.get("tracker_config")
                    {
                        remote_path = tracker_metadata.get("remote_path").map(|s| s.as_str());
                        image_path = tracker_metadata.get("image_path").map(|s| s.as_str());
                        // Check for tracker-specific ImgBB key if available
                        if let Some(key) = tracker_metadata.get("imgbb_api_key") {
                            if !key.is_empty() {
                                imgbb_api_key = Some(key.as_str());
                            }
                        }
                    }

                    // Fall back to global ImgBB config if no tracker-specific key
                    if imgbb_api_key.is_none() {
                        imgbb_api_key =
                            self.config.imgbb.as_ref().map(|c| c.imgbb_api_key.as_str());
                    }

                    // Try to generate screenshots
                    match crate::processing::components::screenshot_utils::generate_screenshots(
                        video_file.to_str().unwrap_or(""),
                        &self.config,
                        imgbb_api_key,
                        remote_path,
                        image_path,
                        input_name,
                        self.upload_config.screenshot_count,
                        self.upload_config.dry_run,
                    ) {
                        Ok((screenshots, thumbnails)) => {
                            if !screenshots.is_empty() {
                                info!("Generated {} screenshots", screenshots.len());
                                self.components.insert(
                                    "screenshots".to_string(),
                                    UploadComponent::Screenshots(screenshots.clone()),
                                );
                                self.components.insert(
                                    "thumbnails".to_string(),
                                    UploadComponent::Thumbnails(thumbnails),
                                );
                            }
                        }
                        Err(e) => info!("Failed to generate screenshots: {}", e),
                    }
                }
                Ok(files) => {
                    if files.is_empty() {
                        info!("No video files found for screenshots");
                    }
                }
                Err(e) => info!("Failed to find video files: {}", e),
            }
        }

        // Process Sample (for video content)
        if !self.upload_config.skip_sample && matches!(self.media_type, MediaType::Video(_)) {
            // Get the appropriate extensions for the media type
            let extensions = self
                .accepted_extensions
                .as_ref()
                .map(|exts| exts.iter().map(|s| s.as_str()).collect::<Vec<_>>())
                .unwrap_or_else(|| crate::core::types::VideoType::all_extensions());

            // Find video files using filter_files_by_extension
            match crate::utils::filter_files_by_extension(&self.input_path, &extensions) {
                Ok(files) if !files.is_empty() => {
                    let video_file = &files[0];
                    // Determine input name for sample - use release name with dots preserved
                    let input_name = if let Some(metadata) = &self.video_metadata {
                        metadata.release_name.as_str()
                    } else {
                        // Fallback to directory/file name
                        let path = std::path::Path::new(&self.input_path);
                        let name = if path.is_dir() {
                            // For directories, use the directory name
                            path.file_name()
                                .and_then(|s| s.to_str())
                                .unwrap_or("unknown")
                        } else {
                            // For files, use the file stem
                            path.file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or("unknown")
                        };
                        info!("Using fallback name for sample: {}", name);
                        name
                    };

                    // Get binary paths
                    let (ffmpeg_path, _, _, _, _) =
                        crate::core::Config::get_binary_paths(&self.config);
                    let ffmpeg_path_str = ffmpeg_path
                        .to_str()
                        .ok_or("Invalid ffmpeg path")
                        .unwrap_or("ffmpeg");

                    // Generate sample
                    // Get CDN paths from tracker config
                    let mut remote_path = "";
                    let mut image_path = "";

                    if let Some(UploadComponent::Metadata(tracker_metadata)) =
                        self.components.get("tracker_config")
                    {
                        remote_path = tracker_metadata
                            .get("remote_path")
                            .map(|s| s.as_str())
                            .unwrap_or("");
                        image_path = tracker_metadata
                            .get("image_path")
                            .map(|s| s.as_str())
                            .unwrap_or("");
                    }

                    match crate::media::video::generate_sample(
                        video_file.to_str().unwrap_or(""),
                        &self.config.paths.screenshots_dir,
                        remote_path,
                        image_path,
                        ffmpeg_path_str,
                        input_name,
                        self.upload_config.dry_run,
                    ) {
                        Ok(sample_url) => {
                            info!("Generated sample: {}", sample_url);
                            let filename = std::path::Path::new(&sample_url)
                                .file_name()
                                .and_then(|f| f.to_str())
                                .unwrap_or("sample.mkv")
                                .to_string();

                            self.components.insert(
                                "sample".to_string(),
                                UploadComponent::Sample {
                                    url: sample_url,
                                    filename,
                                },
                            );
                        }
                        Err(e) => {
                            // Only warn if we're not in dry run mode or if we have upload paths
                            if !self.upload_config.dry_run {
                                info!("Failed to generate sample: {}", e);
                            }
                        }
                    }
                }
                Ok(files) => {
                    if files.is_empty() {
                        info!("No video files found for sample generation");
                    }
                }
                Err(e) => info!("Failed to find video files: {}", e),
            }
        }

        // Process Cover Art (for audio content)
        if !self.upload_config.skip_cover_art && matches!(self.media_type, MediaType::Audio(_)) {
            use crate::processing::components::cover_art_utils::extract_cover_art;

            // Get metadata from the audio_metadata component if available
            let metadata = if let Some(UploadComponent::Metadata(audio_meta)) =
                self.components.get("audio_metadata")
            {
                // Convert HashMap to serde_json::Value
                let mut map = serde_json::Map::new();
                for (k, v) in audio_meta {
                    map.insert(k.clone(), serde_json::Value::String(v.clone()));
                }
                serde_json::Value::Object(map)
            } else {
                serde_json::Value::Object(serde_json::Map::new())
            };

            let release_name = self
                .title
                .as_ref()
                .or_else(|| self.video_metadata.as_ref().map(|m| &m.title))
                .unwrap_or(&"unknown".to_string())
                .clone();

            match extract_cover_art(
                &self.input_path,
                &self.config,
                &release_name,
                &metadata,
                self.upload_config.dry_run,
            ) {
                Ok(Some(cover_url)) => {
                    info!("✅ Extracted cover art: {}", cover_url);
                    self.components.insert(
                        "cover_art".to_string(),
                        UploadComponent::CoverImage(cover_url),
                    );
                }
                Ok(None) => {
                    info!("No cover art found for audio files");
                }
                Err(e) => info!("Failed to extract cover art: {}", e),
            }
        }

        // Create torrent
        if !self.upload_config.skip_torrent_creation {
            if let Some(announce_url) = &self.upload_config.announce_url {
                // Determine stripshit setting from tracker config
                let stripshit = if let Some(UploadComponent::Metadata(metadata)) =
                    self.components.get("tracker_config")
                {
                    metadata
                        .get("stripshit")
                        .and_then(|s| s.parse::<bool>().ok())
                        .unwrap_or(true)
                } else {
                    true
                };

                // Create torrent with extension-aware filtering
                match self.create_torrent_with_extensions(announce_url, stripshit) {
                    Ok(torrent_path) => {
                        info!("Created torrent: {}", torrent_path);
                        self.components.insert(
                            "torrent".to_string(),
                            UploadComponent::TorrentPath(torrent_path),
                        );
                    }
                    Err(e) => error!("Failed to create torrent: {}", e),
                }
            } else {
                info!("No announce URL provided for torrent creation");
            }
        }

        // Build description using DescriptionComponent for template support
        self.add_description_component()?;

        // Build the final UploadData
        let mut upload_data = crate::media::video::UploadData::new();
        
        // Initialize IGDB ID from enriched metadata if available
        if let Some(enriched) = &self.enriched_metadata {
            if let Some(igdb_id_str) = enriched.get("igdb_id") {
                if let Ok(igdb_id) = igdb_id_str.parse::<u64>() {
                    if igdb_id > 0 {
                        upload_data.igdb_id = Some(igdb_id);
                        info!("✅ Set IGDB ID in upload_data: {}", igdb_id);
                    }
                }
            }
        }

        // Set release name and TV show metadata
        if let Some(metadata) = &self.video_metadata {
            // Use the original release_name with dots preserved for upload
            upload_data.release_name = Some(metadata.release_name.clone());
            upload_data.season = metadata.season;
            upload_data.episode = metadata.episode;
            upload_data.resolution = metadata.resolution.clone();
        } else if let Some(title) = &self.title {
            upload_data.release_name = Some(title.clone());
        } else {
            // Fallback: generate release name from input path
            let path = Path::new(&self.input_path);
            let base_name = if path.is_dir() {
                // For directories, use the directory name
                path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            } else {
                // For files, use the file name
                path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            };
            let release_name = crate::processing::naming::generate_release_name(&base_name);
            info!(
                "No title or video metadata found, generated release name from path: {}",
                release_name
            );
            upload_data.release_name = Some(release_name);
        }

        // Process components into UploadData
        for (name, component) in self.components {
            match component {
                UploadComponent::NfoData { path, content } => {
                    upload_data.nfo_data = Some((path, content));
                }
                UploadComponent::Mediainfo(mediainfo) => {
                    upload_data.mediainfo = Some(mediainfo);
                }
                UploadComponent::Screenshots(screenshots) => {
                    upload_data.screenshots = screenshots;
                }
                UploadComponent::Thumbnails(thumbnails) => {
                    upload_data.thumbnails = thumbnails;
                }
                UploadComponent::Sample { url, .. } => {
                    upload_data.sample_url = Some(url);
                }
                UploadComponent::TorrentPath(path) => {
                    upload_data.torrent_path = Some(path);
                }
                UploadComponent::Description(desc) => {
                    upload_data.description = Some(desc);
                }
                UploadComponent::TmdbData {
                    tmdb_id,
                    imdb_id,
                    tvdb_id,
                    ..
                } => {
                    upload_data.tmdb_id = Some(tmdb_id);
                    upload_data.imdb_id = imdb_id;
                    upload_data.tvdb_id = tvdb_id;
                }
                UploadComponent::CoverImage(cover_url) => {
                    upload_data.cover_url = Some(cover_url);
                }
                _ => {
                    // Other components might be used by tracker-specific code
                    info!("Component '{}' stored but not added to UploadData", name);
                }
            }
        }

        info!("UploadBuilder: Build process completed successfully");
        Ok(upload_data)
    }

    /// Get a component if it exists
    pub fn get_component(&self, name: &str) -> Option<&UploadComponent> {
        self.components.get(name)
    }

    /// Check if a component exists
    pub fn has_component(&self, name: &str) -> bool {
        self.components.contains_key(name)
    }

    /// Get the active tracker name (if any)
    pub fn get_active_tracker(&self) -> Option<&str> {
        self.active_tracker.as_deref()
    }

    /// Create torrent with extension-aware filtering
    fn create_torrent_with_extensions(
        &self,
        announce_url: &str,
        stripshit: bool,
    ) -> Result<String, String> {
        use crate::processing::naming::generate_release_name;
        use std::process::Command;

        let torrent_dir = &self.config.paths.torrent_dir;
        fs::create_dir_all(torrent_dir).map_err(|e| {
            format!(
                "Failed to create torrent directory '{}': {}",
                torrent_dir, e
            )
        })?;

        let base_name = Path::new(&self.input_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let release_name = generate_release_name(&base_name);
        let torrent_file = format!("{}/{}.torrent", torrent_dir, release_name);

        info!("Creating torrent for input path: {}", self.input_path);
        info!("Torrent File: {}", torrent_file);

        // Build the mkbrr command
        let mut command = Command::new(&self.config.paths.mkbrr);
        command.args(&[
            "create",
            "-t",
            announce_url,
            "-o",
            &torrent_file,
            "--source",
            "seedpool.org",
            &self.input_path,
        ]);

        // Build exclude pattern based on accepted extensions and stripshit setting
        let is_video = matches!(self.media_type, MediaType::Video(_));

        if (stripshit && is_video) || self.accepted_extensions.is_some() {
            let mut exclude_patterns = Vec::<String>::new();

            // Add standard exclusions only for video files if stripshit is enabled
            if stripshit && is_video {
                let standard_excludes = vec![
                    "[X]*",
                    "*sample*",
                    "*proof*",
                    "*screens*",
                    "*screenshots*",
                    "*.txt",
                    "*.jpg",
                    "*.jpeg",
                    "*.png",
                    "*.nfo",
                    "*.srr",
                    "*.doc",
                    "*.sfv",
                    "*.r??",
                ];
                exclude_patterns.extend(standard_excludes.into_iter().map(String::from));
            }

            // If we have accepted extensions, exclude everything else
            if let Some(accepted) = &self.accepted_extensions {
                // Get all possible file extensions from the file system
                let all_extensions = self.get_all_extensions_in_path(&self.input_path)?;

                // Exclude any extension not in our accepted list
                for ext in all_extensions {
                    if !accepted.contains(&ext) {
                        exclude_patterns.push(format!("*.{}", ext));
                    }
                }
            }

            if !exclude_patterns.is_empty() {
                let exclude_string = exclude_patterns.join(",");
                command.args(&["--exclude", &exclude_string]);
                info!("Torrent exclude patterns: {}", exclude_string);
            }
        }

        // Execute the mkbrr command with real-time output streaming
        use std::io::{BufRead, BufReader};
        use std::process::Stdio;
        use std::thread;

        info!("Starting mkbrr torrent creation process...");
        info!(
            "Command: {} {:?}",
            self.config.paths.mkbrr,
            command.get_args().collect::<Vec<_>>()
        );

        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());

        let mut child = command
            .spawn()
            .map_err(|e| format!("Failed to spawn mkbrr process: {}", e))?;

        // Handle stdout in a separate thread
        let stdout_handle = if let Some(stdout) = child.stdout.take() {
            Some(thread::spawn(move || {
                use std::io::Read;
                let mut reader = BufReader::new(stdout);
                let mut buffer = Vec::new();
                let mut byte = [0u8; 1];

                while reader.read_exact(&mut byte).is_ok() {
                    if byte[0] == b'\n' || byte[0] == b'\r' {
                        if !buffer.is_empty() {
                            if let Ok(line) = String::from_utf8(buffer.clone()) {
                                let trimmed = line.trim();
                                if !trimmed.is_empty() {
                                    info!("mkbrr: {}", trimmed);
                                }
                            }
                            buffer.clear();
                        }
                    } else {
                        buffer.push(byte[0]);
                    }
                }

                // Handle any remaining data in buffer
                if !buffer.is_empty() {
                    if let Ok(line) = String::from_utf8(buffer) {
                        let trimmed = line.trim();
                        if !trimmed.is_empty() {
                            info!("mkbrr: {}", trimmed);
                        }
                    }
                }
            }))
        } else {
            None
        };

        // Handle stderr in a separate thread
        let stderr_handle = if let Some(stderr) = child.stderr.take() {
            Some(thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines() {
                    match line {
                        Ok(line) => {
                            if !line.trim().is_empty() {
                                error!("mkbrr stderr: {}", line);
                            }
                        }
                        Err(e) => error!("Error reading mkbrr stderr: {}", e),
                    }
                }
            }))
        } else {
            None
        };

        // Wait for the process to complete
        let status = child
            .wait()
            .map_err(|e| format!("Failed to wait for mkbrr process: {}", e))?;

        // Wait for output threads to complete
        if let Some(handle) = stdout_handle {
            let _ = handle.join();
        }
        if let Some(handle) = stderr_handle {
            let _ = handle.join();
        }

        info!("mkbrr process completed");

        if !status.success() {
            return Err(format!(
                "mkbrr failed to create torrent for input path: {}. Exit code: {}",
                self.input_path,
                status.code().unwrap_or(-1)
            ));
        }

        info!("Created torrent: {}", torrent_file);
        Ok(torrent_file)
    }

    /// Get all file extensions present in the given path
    fn get_all_extensions_in_path(&self, path: &str) -> Result<Vec<String>, String> {
        use std::collections::HashSet;
        use walkdir::WalkDir;

        let mut extensions = HashSet::new();
        let path = Path::new(path);

        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                extensions.insert(ext.to_lowercase());
            }
        } else if path.is_dir() {
            for entry in WalkDir::new(path)
                .follow_links(true)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let entry_path = entry.path();
                if entry_path.is_file() {
                    if let Some(ext) = entry_path.extension().and_then(|e| e.to_str()) {
                        extensions.insert(ext.to_lowercase());
                    }
                }
            }
        }

        Ok(extensions.into_iter().collect())
    }

    /// Add description component that uses templates
    fn add_description_component(&mut self) -> Result<(), String> {
        use crate::processing::components::description::DescriptionComponent;
        use crate::processing::components::UploadComponent as ComponentTrait;

        // Prepare metadata for description generation
        let mut metadata = serde_json::json!({});

        // Add basic metadata passed from ProcessBuilder
        if let Some(cached_metadata) = &self.cached_metadata {
            // Start with the cached metadata as base
            metadata = cached_metadata.clone();
        }

        // Add/override with title
        if let Some(title) = &self.title {
            metadata["title"] = serde_json::json!(title);
        } else if let Some(video_metadata) = &self.video_metadata {
            metadata["title"] = serde_json::json!(video_metadata.title);
        }

        // Add audio metadata if available (includes artist, album, etc.)
        if let Some(UploadComponent::Metadata(audio_meta)) = self.components.get("audio_metadata") {
            for (key, value) in audio_meta {
                metadata[key] = serde_json::json!(value);
            }
        }

        // Add mediainfo if available
        if let Some(UploadComponent::Mediainfo(mediainfo)) = self.components.get("mediainfo") {
            metadata["mediainfo"] = serde_json::json!(mediainfo);
        }

        // Add sample URL if available
        if let Some(UploadComponent::Sample { url, filename }) = self.components.get("sample") {
            metadata["sample_url"] = serde_json::json!(url);
            metadata["sample_filename"] = serde_json::json!(filename);
        }

        // Add trailer if available
        if let Some(UploadComponent::Trailer { url, platform }) = self.components.get("trailer") {
            metadata["trailer_url"] = serde_json::json!(url);
            metadata["trailer_platform"] = serde_json::json!(platform);
        }

        // Add any TMDB data if available
        if let Some(UploadComponent::TmdbData {
            tmdb_id,
            imdb_id,
            tvdb_id,
            title,
            year,
            ..
        }) = self.components.get("tmdb")
        {
            metadata["tmdb_id"] = serde_json::json!(tmdb_id);
            if let Some(imdb) = imdb_id {
                metadata["tmdb_imdb_id"] = serde_json::json!(imdb);
            }
            if let Some(tvdb) = tvdb_id {
                metadata["tmdb_tvdb_id"] = serde_json::json!(tvdb);
            }
            metadata["tmdb_title"] = serde_json::json!(title);
            if let Some(y) = year {
                metadata["tmdb_year"] = serde_json::json!(y);
            }
        }

        // Add cover art URL if available
        if let Some(UploadComponent::CoverImage(cover_url)) = self.components.get("cover_art") {
            info!("Adding cover art to metadata: {}", cover_url);
            metadata["cover_url"] = serde_json::json!(cover_url);
            // Also add as cover_images array for template compatibility
            metadata["cover_images"] = serde_json::json!([cover_url]);
        } else {
            info!("No cover art component found");
        }

        // Check if a description component already exists (e.g., from ebook processing)
        if !self.components.contains_key("description") {
            // Add custom description from tracker config to metadata before creating component
            if let Some(UploadComponent::Metadata(tracker_meta)) = self.components.get("tracker_config") {
                if let Some(custom_desc) = tracker_meta.get("custom_description") {
                    if !custom_desc.is_empty() {
                        metadata["custom_description"] = serde_json::json!(custom_desc);
                        info!("📝 Adding custom description from tracker config: {} chars", custom_desc.len());
                    }
                }
            }

            // Create DescriptionComponent only if one doesn't already exist
            let mut desc_component =
                DescriptionComponent::new(self.input_path.clone(), self.media_type.clone(), metadata);

            // Add screenshots and thumbnails together
            let screenshots =
                if let Some(UploadComponent::Screenshots(s)) = self.components.get("screenshots") {
                    s.clone()
                } else {
                    Vec::new()
                };

            let thumbnails =
                if let Some(UploadComponent::Thumbnails(t)) = self.components.get("thumbnails") {
                    t.clone()
                } else {
                    Vec::new()
                };

            desc_component = desc_component.with_screenshots(screenshots, thumbnails);

            // Add mediainfo text
            if let Some(UploadComponent::Mediainfo(mediainfo)) = self.components.get("mediainfo") {
                desc_component = desc_component.with_mediainfo(mediainfo.clone());
            }

            // Add enriched metadata if available (for templates)
            if let Some(enriched) = &self.enriched_metadata {
                desc_component = desc_component.with_enriched_metadata(enriched.clone());
            }



            // Set template name if provided
            if let Some(template) = &self.template_name {
                desc_component = desc_component.with_template(template.clone());
            }

            // Process the component to generate description
            match desc_component.process() {
                Ok(result) => {
                    if let Some(description) = result.data {
                        self.components.insert(
                            "description".to_string(),
                            UploadComponent::Description(description),
                        );
                    } else {
                        return Err("Description component returned no data".to_string());
                    }
                }
                Err(e) => return Err(format!("Failed to generate description: {:?}", e)),
            }
        } else {
            info!("📝 Description component already exists, skipping template-based generation");
        }

        Ok(())
    }
}

/// Extension trait for UploadBuilder to add tracker-specific functionality
///
/// NOTE: This trait is optional as UploadBuilder automatically detects
/// and applies the enabled tracker's configuration. You only need to use
/// these methods if you want to override the automatic detection.
pub trait TrackerUploadExt {
    /// Configure for Seedpool upload (overrides automatic detection)
    fn for_seedpool(self, seedpool_config: &crate::core::SeedpoolConfig) -> Self;

    /// Configure for TorrentLeech upload (overrides automatic detection)
    fn for_torrentleech(self, tl_config: &crate::core::TorrentLeechConfig) -> Self;
}

impl TrackerUploadExt for UploadBuilder {
    fn for_seedpool(mut self, seedpool_config: &crate::core::SeedpoolConfig) -> Self {
        // Apply tracker-specific upload configuration
        self.upload_config.skip_duplicate_check = !seedpool_config.settings.enable_duplicate_check;
        self.upload_config.skip_mediainfo = !seedpool_config.settings.enable_mediainfo;
        self.upload_config.skip_nfo = !seedpool_config.settings.enable_nfo;
        self.upload_config.skip_screenshots = !seedpool_config.settings.enable_screenshots;
        self.upload_config.skip_sample = !seedpool_config.settings.enable_sample;
        self.upload_config.skip_tmdb = !seedpool_config.settings.enable_tmdb;
        self.upload_config.skip_torrent_creation =
            !seedpool_config.settings.enable_torrent_creation;
        self.upload_config.screenshot_count = seedpool_config.settings.screenshot_count;

        // Add announce URL for torrent creation if enabled
        if seedpool_config.settings.enable_torrent_creation {
            self.upload_config.announce_url = Some(seedpool_config.settings.announce_url.clone());
        }

        // Store tracker config as a component for later use
        let mut metadata = HashMap::new();
        metadata.insert("tracker".to_string(), "seedpool".to_string());
        metadata.insert(
            "stripshit".to_string(),
            seedpool_config.settings.stripshit_from_videos.to_string(),
        );
        metadata.insert(
            "remote_path".to_string(),
            seedpool_config.screenshots.remote_path.clone(),
        );
        metadata.insert(
            "image_path".to_string(),
            seedpool_config.screenshots.image_path.clone(),
        );

        self.with_custom_component("tracker_config", UploadComponent::Metadata(metadata))
    }

    fn for_torrentleech(mut self, tl_config: &crate::core::TorrentLeechConfig) -> Self {
        // Apply tracker-specific upload configuration
        self.upload_config.skip_duplicate_check = !tl_config.settings.enable_duplicate_check;
        self.upload_config.skip_mediainfo = !tl_config.settings.enable_mediainfo;
        self.upload_config.skip_nfo = !tl_config.settings.enable_nfo;
        self.upload_config.skip_screenshots = !tl_config.settings.enable_screenshots;
        self.upload_config.skip_sample = !tl_config.settings.enable_sample;
        self.upload_config.skip_tmdb = !tl_config.settings.enable_tmdb;
        self.upload_config.skip_torrent_creation = !tl_config.settings.enable_torrent_creation;
        self.upload_config.screenshot_count = tl_config.settings.screenshot_count;

        // Add announce URL for torrent creation if enabled
        if tl_config.settings.enable_torrent_creation
            && !tl_config.general.announce_url_1.is_empty()
        {
            self.upload_config.announce_url = Some(tl_config.general.announce_url_1.clone());
        }

        // Store tracker config
        let mut metadata = HashMap::new();
        metadata.insert("tracker".to_string(), "torrentleech".to_string());
        metadata.insert(
            "stripshit".to_string(),
            tl_config.settings.stripshit_from_videos.to_string(),
        );

        self.with_custom_component("tracker_config", UploadComponent::Metadata(metadata))
    }
}

/// Example of how to use UploadBuilder with automatic tracker detection
///
/// ```rust
/// // Create upload for a video with specific components
/// let builder = create_video_upload(&input_path, config.clone(), metadata);
///
/// // The builder automatically:
/// // 1. Detects which tracker is enabled (seedpool or torrentleech)
/// // 2. Loads that tracker's configuration
/// // 3. Applies the enable_* settings from the tracker config
/// // 4. Only processes components that are both:
/// //    - Requested by the media type (e.g., video wants screenshots)
/// //    - Enabled in the tracker config (e.g., enable_screenshots: true)
///
/// let upload_data = builder.build()?;
/// ```

/// Result of an upload operation
#[derive(Debug, Clone)]
pub struct UploadResult {
    pub success: bool,
    pub tracker: String,
    pub torrent_id: Option<String>,
    pub message: String,
}

/// Processor for handling uploads to specific trackers
pub struct UploadProcessor {
    /// The upload data to process
    upload_data: crate::media::video::UploadData,
    /// Full configuration
    #[allow(dead_code)] // Used in some upload scenarios, keep for now
    config: Arc<Config>,
    /// Dry run mode
    dry_run: bool,
    /// Media classification for mapping
    media_category: Option<String>,
    media_source_type: Option<String>,
    /// Field mapping override
    field_mapping: Option<TrackerFieldMapping>,
    /// Original torrent info with preserved category/type codes
    original_torrent_info: Option<crate::trackers::seedpool::SeedpoolTorrentInfo>,
    /// Media type for cover generation
    media_type: Option<MediaType>,
    /// Input path for PDF cover generation
    input_path: Option<String>,
}

impl UploadProcessor {
    /// Create a new upload processor that auto-detects the active tracker
    pub fn new(upload_data: crate::media::video::UploadData, config: Arc<Config>) -> Self {
        Self {
            upload_data,
            config,
            dry_run: false,
            media_category: None,
            media_source_type: None,
            field_mapping: None,
            original_torrent_info: None,
            media_type: None,
            input_path: None,
        }
    }

    /// Set media type and input path for cover generation
    pub fn with_media_info(mut self, media_type: MediaType, input_path: String) -> Self {
        self.media_type = Some(media_type);
        self.input_path = Some(input_path);
        self
    }

    /// Set dry run mode
    pub fn dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }
    
    /// Set original torrent info with preserved category/type codes  
    pub fn with_original_torrent_info(mut self, torrent_info: crate::trackers::seedpool::SeedpoolTorrentInfo) -> Self {
        self.original_torrent_info = Some(torrent_info);
        self
    }

    /// Set media classification for mapping
    pub fn with_media_classification(
        mut self,
        category: Option<String>,
        source_type: Option<String>,
    ) -> Self {
        self.media_category = category;
        self.media_source_type = source_type;
        self
    }

    /// Override field mapping
    pub fn with_field_mapping(mut self, mapping: TrackerFieldMapping) -> Self {
        self.field_mapping = Some(mapping);
        self
    }

    /// Process the upload to the active tracker
    pub fn process(self) -> Result<UploadResult, String> {
        // Determine active tracker and get its configuration
        let (tracker_name, field_mapping) = self.determine_active_tracker()?;

        info!("Processing upload for tracker: {}", tracker_name);

        // Map media classification to tracker-specific codes
        let (tracker_category, tracker_type) = self.map_to_tracker_codes(
            &tracker_name,
            self.media_category.as_deref(),
            self.media_source_type.as_deref(),
        )?;

        // Build the form data based on tracker field mappings
        let form_data = self.build_form_data(
            &tracker_name,
            tracker_category,
            tracker_type,
            &field_mapping,
        )?;

        // Validate required fields
        self.validate_required_fields(&form_data, &field_mapping)?;

        if self.dry_run {
            info!("DRY RUN: Would upload to {} with data:", tracker_name);
            for (key, value) in &form_data {
                info!("  {}: {}", key, value);
            }

            // Save description to test file
            let description_file = if let Some(description) = form_data
                .get("description")
                .or_else(|| form_data.get("descr"))
            {
                Some(self.save_description_to_test_file(description, &tracker_name)?)
            } else {
                None
            };

            let message = if let Some(file_path) = description_file {
                format!(
                    "Dry run completed successfully. Description saved to: {}",
                    file_path
                )
            } else {
                "Dry run completed successfully".to_string()
            };

            Ok(UploadResult {
                success: true,
                tracker: tracker_name,
                torrent_id: None,
                message,
            })
        } else {
            // Perform the actual upload
            self.perform_upload(&tracker_name, form_data)
        }
    }

    /// Determine which tracker is active and load its configuration
    fn determine_active_tracker(&self) -> Result<(String, TrackerFieldMapping), String> {
        // Use override mappings if provided
        if let Some(ref fm) = &self.field_mapping {
            // Try to determine tracker from field mapping or default to seedpool
            let tracker_name = if fm.field_map.contains_key("descr") {
                "torrentleech"
            } else {
                "seedpool"
            };
            return Ok((tracker_name.to_string(), fm.clone()));
        }

        // Load tracker configs and determine which is active
        let seedpool_config =
            crate::utils::load_tracker_config::<crate::core::SeedpoolConfig>("seedpool")
                .map_err(|e| format!("Failed to load seedpool config: {}", e))?;
        let torrentleech_config =
            crate::utils::load_tracker_config::<crate::core::TorrentLeechConfig>("torrentleech")
                .map_err(|e| format!("Failed to load torrentleech config: {}", e))?;

        if seedpool_config.general.enabled {
            return Ok((
                "seedpool".to_string(),
                crate::trackers::seedpool::create_seedpool_field_mapping(),
            ));
        }

        if torrentleech_config.general.enabled {
            return Ok((
                "torrentleech".to_string(),
                crate::trackers::torrentleech::create_torrentleech_field_mapping(),
            ));
        }

        Err("No tracker is enabled in configuration".to_string())
    }

    /// Map media classification strings to tracker-specific category/type codes
    fn map_to_tracker_codes(
        &self,
        tracker_name: &str,
        media_category: Option<&str>,
        media_source_type: Option<&str>,
    ) -> Result<(u32, Option<u32>), String> {
        match tracker_name {
            "seedpool" => {
                // Use original torrent info with preserved codes if available
                if let Some(ref original_info) = self.original_torrent_info {
                    info!("🎯 Using original category/type codes: {}/{}", 
                          original_info.category_code(), original_info.type_code());
                    return Ok((
                        original_info.category_code() as u32,
                        Some(original_info.type_code() as u32),
                    ));
                }
                
                // Convert media strings to Seedpool TorrentInfo
                let torrent_info =
                    crate::trackers::seedpool::create_torrent_info_from_media_strings(
                        media_category,
                        media_source_type,
                    )?;
                Ok((
                    torrent_info.category_code() as u32,
                    Some(torrent_info.type_code() as u32),
                ))
            }
            "torrentleech" => {
                // Convert media strings to TorrentLeech category
                let category_code = crate::trackers::torrentleech::get_category_from_media_strings(
                    media_category,
                    media_source_type,
                )?;
                Ok((category_code as u32, None))
            }
            _ => {
                // Default fallback
                Ok((0, None))
            }
        }
    }

    /// Build form data using field mappings
    fn build_form_data(
        &self,
        tracker_name: &str,
        category: u32,
        type_id: Option<u32>,
        field_mapping: &TrackerFieldMapping,
    ) -> Result<HashMap<String, String>, String> {
        let mut form_data = HashMap::new();

        // Standard fields with mappings
        let standard_fields = vec![
            ("name", self.upload_data.release_name.as_deref()),
            ("description", self.upload_data.description.as_deref()),
            ("mediainfo", self.upload_data.mediainfo.as_deref()),
        ];

        for (internal_name, value) in standard_fields {
            if let Some(value) = value {
                let field_name = field_mapping
                    .get_field_name(internal_name)
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| internal_name.to_string());
                form_data.insert(field_name, value.to_string());
            }
        }

        // Category and type
        let cat_field = field_mapping
            .get_field_name("category")
            .map(|s| s.to_string())
            .unwrap_or_else(|| "category".to_string());
        form_data.insert(cat_field, category.to_string());

        if let Some(type_id) = type_id {
            let type_field = field_mapping
                .get_field_name("type")
                .map(|s| s.to_string())
                .unwrap_or_else(|| "type".to_string());
            form_data.insert(type_field, type_id.to_string());
        }

        // Screenshots/Images handling
        // Note: For Seedpool, cover images are uploaded separately after getting the torrent ID
        // They must follow the naming scheme: torrent-cover_{torrent_id}.jpg
        // Screenshots are already uploaded to CDN, we just need to handle them differently per tracker
        if !self.upload_data.screenshots.is_empty() && tracker_name != "seedpool" {
            // Only add screenshots for non-Seedpool trackers
            let screenshot_field = field_mapping
                .get_field_name("screenshots")
                .map(|s| s.to_string())
                .unwrap_or_else(|| "screenshots".to_string());
            form_data.insert(screenshot_field, self.upload_data.screenshots.join("\n"));
        }

        // Torrent file path
        if let Some(torrent_path) = &self.upload_data.torrent_path {
            let torrent_field = field_mapping
                .get_field_name("torrent")
                .map(|s| s.to_string())
                .unwrap_or_else(|| "torrent".to_string());
            form_data.insert(torrent_field, torrent_path.clone());
        }

        // NFO file (if any)
        if let Some((nfo_path, _)) = &self.upload_data.nfo_data {
            let nfo_field = field_mapping
                .get_field_name("nfo")
                .map(|s| s.to_string())
                .unwrap_or_else(|| "nfo".to_string());
            form_data.insert(nfo_field, nfo_path.clone());
        }

        // Add TMDB/IMDB/TVDB IDs from upload data
        form_data.insert(
            "tmdb".to_string(),
            self.upload_data
                .tmdb_id
                .map(|id| id.to_string())
                .unwrap_or("0".to_string()),
        );
        form_data.insert(
            "imdb".to_string(),
            self.upload_data.imdb_id.clone().unwrap_or("0".to_string()),
        );
        form_data.insert(
            "tvdb".to_string(),
            self.upload_data
                .tvdb_id
                .map(|id| id.to_string())
                .unwrap_or("0".to_string()),
        );

        // Default values for other optional fields
        form_data.insert("mal".to_string(), "0".to_string());
        
        // Add IGDB ID if available
        form_data.insert(
            "igdb".to_string(),
            self.upload_data
                .igdb_id
                .map(|id| id.to_string())
                .unwrap_or("0".to_string()),
        );

        // Generate keywords from release name
        if let Some(release_name) = &self.upload_data.release_name {
            let keywords = release_name
                .split(|c: char| !c.is_alphanumeric())
                .filter(|s| !s.is_empty() && s.len() > 2)
                .map(|s| s.to_lowercase())
                .collect::<Vec<_>>()
                .join(" ");
            form_data.insert("keywords".to_string(), keywords);
        }

        // Add resolution_id for Seedpool video uploads (movies, TV shows, anime, etc.)
        if tracker_name == "seedpool" {
            // Map resolution to resolution_id for all video content
            if let Some(resolution) = &self.upload_data.resolution {
                if let Some(resolution_id) =
                    crate::trackers::seedpool::map_resolution_to_id(resolution)
                {
                    form_data.insert("resolution_id".to_string(), resolution_id);
                }
            }
        }

        // Add TV show specific fields for Seedpool  
        if tracker_name == "seedpool" && category == 2 {
            // TV Show category

            // Add season number
            if let Some(season) = self.upload_data.season {
                form_data.insert("season_number".to_string(), season.to_string());
            }

            // Add episode number
            if let Some(episode) = self.upload_data.episode {
                form_data.insert("episode_number".to_string(), episode.to_string());
            }
        }

        // Note: Cover images are NOT included in the main torrent upload form
        // They are uploaded separately to CDN after getting the torrent ID

        Ok(form_data)
    }

    /// Validate that all required fields are present
    fn validate_required_fields(
        &self,
        form_data: &HashMap<String, String>,
        field_mapping: &TrackerFieldMapping,
    ) -> Result<(), String> {
        let mut missing_fields = Vec::new();

        for required_field in &field_mapping.required_fields {
            if !form_data.contains_key(required_field) {
                missing_fields.push(required_field.clone());
            }
        }

        if !missing_fields.is_empty() {
            Err(format!("Missing required fields: {:?}", missing_fields))
        } else {
            Ok(())
        }
    }

    /// Perform the actual upload to the tracker
    fn perform_upload(
        &self,
        tracker_name: &str,
        form_data: HashMap<String, String>,
    ) -> Result<UploadResult, String> {
        match tracker_name {
            "seedpool" => self.upload_to_seedpool(form_data),
            "torrentleech" => self.upload_to_torrentleech(form_data),
            _ => Err(format!("Unknown tracker: {}", tracker_name)),
        }
    }

    /// Upload to Seedpool
    fn upload_to_seedpool(
        &self,
        form_data: HashMap<String, String>,
    ) -> Result<UploadResult, String> {
        use crate::utils::http::extract_torrent_id;
        use reqwest::blocking::{multipart::Form, Client};

        // Load Seedpool configuration
        let seedpool_config =
            crate::utils::load_tracker_config::<crate::core::SeedpoolConfig>("seedpool")
                .map_err(|e| format!("Failed to load seedpool config: {}", e))?;

        // Log all form_data entries
        info!("=== FORM DATA CONTENTS ===");
        for (key, value) in &form_data {
            info!("  {}: {}", key, value);
        }
        info!("=== END FORM DATA ===");

        // Get required fields from form_data
        let torrent_path = form_data
            .get("torrent")
            .ok_or("Missing torrent file path")?;
        
        // For Seedpool, use original filename (minus extension) instead of sanitized release name
        let name = if let Some(input_path) = &self.input_path {
            let path = std::path::Path::new(input_path);
            if path.is_file() {
                // For files, use file stem (filename without extension)
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| {
                        // Fallback to sanitized name if we can't get the original
                        form_data.get("name").map(|s| s.clone()).unwrap_or_else(|| "Unknown".to_string())
                    })
            } else {
                // For directories, use directory name
                path.file_name()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| {
                        // Fallback to sanitized name if we can't get the original
                        form_data.get("name").map(|s| s.clone()).unwrap_or_else(|| "Unknown".to_string())
                    })
            }
        } else {
            // No input path available, use sanitized name
            form_data.get("name").map(|s| s.clone()).unwrap_or_else(|| "Unknown".to_string())
        };
        let category_id = form_data.get("category_id").ok_or("Missing category ID")?;
        let type_id = form_data.get("type_id").ok_or("Missing type ID")?;
        let default_description = String::new();
        let description = form_data.get("description").unwrap_or(&default_description);

        // Get optional fields with defaults
        let default_zero = "0".to_string();
        let tmdb = form_data.get("tmdb").unwrap_or(&default_zero);
        let imdb = form_data.get("imdb").unwrap_or(&default_zero);
        let tvdb = form_data.get("tvdb").unwrap_or(&default_zero);
        let mal = form_data.get("mal").unwrap_or(&default_zero);
        let igdb = form_data.get("igdb").unwrap_or(&default_zero);

        // Log all the data we're sending
        info!("=== SEEDPOOL UPLOAD REQUEST DATA ===");
        info!("  torrent: {}", torrent_path);
        info!("  name: {}", name);
        info!("  category_id: {}", category_id);
        info!("  type_id: {}", type_id);
        info!("  tmdb: {}", tmdb);
        info!("  imdb: {}", imdb);
        info!("  tvdb: {}", tvdb);
        info!("  description length: {} chars", description.len());
        info!("  mal: {}", mal);
        info!("  igdb: {}", igdb);

        // Build multipart form
        let mut form = Form::new()
            .file("torrent", torrent_path)
            .map_err(|e| format!("Failed to attach torrent file: {}", e))?
            .text("name", name.clone())
            .text("category_id", category_id.clone())
            .text("type_id", type_id.clone())
            .text("tmdb", tmdb.clone())
            .text("imdb", imdb.clone())
            .text("tvdb", tvdb.clone())
            .text("anonymous", "0")
            .text("description", description.clone())
            .text("mal", mal.clone())
            .text("igdb", igdb.clone())
            .text("stream", "0")
            .text("sd", "0");

        // Add keywords if present
        if let Some(keywords) = form_data.get("keywords") {
            info!("  keywords: {}", keywords);
            form = form.text("keywords", keywords.clone());
        }

        // Add mediainfo if present
        if let Some(mediainfo) = form_data.get("mediainfo") {
            info!("  mediainfo: included ({} chars)", mediainfo.len());
            form = form.text("mediainfo", mediainfo.clone());
        }

        // Add TV show specific fields
        if let Some(resolution_id) = form_data.get("resolution_id") {
            info!("  resolution_id: {}", resolution_id);
            form = form.text("resolution_id", resolution_id.clone());
        }
        if let Some(season_number) = form_data.get("season_number") {
            info!("  season_number: {}", season_number);
            form = form.text("season_number", season_number.clone());
        }
        if let Some(episode_number) = form_data.get("episode_number") {
            info!("  episode_number: {}", episode_number);
            form = form.text("episode_number", episode_number.clone());
        }

        info!("=== END SEEDPOOL UPLOAD REQUEST DATA ===");

        // Add NFO file if present
        if let Some(nfo_path) = form_data.get("nfo") {
            form = form
                .file("nfo", nfo_path)
                .map_err(|e| format!("Failed to attach NFO file: {}", e))?;
        }

        // Create HTTP client
        let client = Client::new();

        info!(
            "Uploading to Seedpool: {}",
            seedpool_config.settings.upload_url
        );

        // Send the upload request
        let response = client
            .post(&seedpool_config.settings.upload_url)
            .header(
                "Authorization",
                format!("Bearer {}", seedpool_config.general.api_key),
            )
            .multipart(form)
            .send()
            .map_err(|e| format!("Failed to send request to Seedpool: {}", e))?;

        let status = response.status();
        let response_text = response
            .text()
            .unwrap_or_else(|_| "Failed to read response body".to_string());

        info!("Seedpool API Response Status: {}", status);
        info!("Seedpool API Response: {}", response_text);

        if !status.is_success() {
            return Ok(UploadResult {
                success: false,
                tracker: "seedpool".to_string(),
                torrent_id: None,
                message: format!("Upload failed with status {}: {}", status, response_text),
            });
        }

        // Extract torrent ID from response
        match extract_torrent_id(&response_text) {
            Ok(torrent_id) => {
                        info!(
            "Successfully uploaded to Seedpool. Torrent ID: {}",
            torrent_id
        );

        // Process ebooks with Open Library lookup and cover upload after successful upload
        if let Some(media_type) = &self.media_type {
            match media_type {
                crate::core::MediaType::Ebook(ebook_type) => {
                    match ebook_type {
                        crate::core::EbookType::Epub => {
                            // Check if Open Library processing was already attempted during screenshot generation
                            if std::env::var("SEEDBRR_OPEN_LIBRARY_ATTEMPTED").is_ok() {
                                info!("📚 Open Library processing already completed during upload, skipping duplicate processing");
                                std::env::remove_var("SEEDBRR_OPEN_LIBRARY_ATTEMPTED"); // Clean up
                            } else {
                                info!("📚 Starting EPUB Open Library processing...");
                                if let Err(e) = self.process_ebook_open_library(&torrent_id, &seedpool_config, ebook_type) {
                                    error!("❌ Failed to process EPUB with Open Library: {}", e);
                                } else {
                                    info!("✅ Successfully processed EPUB with Open Library");
                                }
                            }

                            // Generate and upload EPUB cover for torrent
                            info!("📚 EPUB detected, finding and uploading cover image...");
                            match self.generate_epub_cover_for_upload(&torrent_id, &seedpool_config) {
                                Ok(cover_url) => {
                                    info!("✅ EPUB cover processed successfully: {}", cover_url);
                                    // Upload the cover to Seedpool
                                    match self.upload_cover_to_seedpool(&torrent_id, &cover_url, &seedpool_config) {
                                        Ok(_) => info!("✅ EPUB cover uploaded to Seedpool successfully"),
                                        Err(e) => error!("❌ Failed to upload EPUB cover to Seedpool: {}", e),
                                    }
                                }
                                Err(e) => {
                                    error!("❌ Failed to process EPUB cover: {}", e);
                                }
                            }
                        }
                        crate::core::EbookType::Pdf => {
                            info!("📚 Starting PDF Open Library processing...");
                            if let Err(e) = self.process_ebook_open_library(&torrent_id, &seedpool_config, ebook_type) {
                                error!("❌ Failed to process PDF with Open Library: {}", e);
                            } else {
                                info!("✅ Successfully processed PDF with Open Library");
                            }
                        }
                        _ => {
                            info!("📚 Ebook type {:?} - skipping Open Library processing", ebook_type);
                        }
                    }
                }
                _ => {}
            }
        }

                // Generate PDF cover if this is a PDF ebook
                let mut generated_cover_url: Option<String> = None;
                if let Some(media_type) = &self.media_type {
                    if let crate::core::MediaType::Ebook(crate::core::EbookType::Pdf) = media_type {
                        info!("📚 PDF ebook detected, generating cover image...");
                        match self.generate_pdf_cover_for_upload(&torrent_id, &seedpool_config) {
                            Ok(cover_url) => {
                                info!("✅ PDF cover generated successfully: {}", cover_url);
                                generated_cover_url = Some(cover_url);
                            }
                            Err(e) => {
                                error!("❌ Failed to generate PDF cover: {}", e);
                            }
                        }
                    }
                }

                // Upload cover image if available (existing or generated)
                let cover_to_upload = generated_cover_url
                    .as_ref()
                    .or(self.upload_data.cover_url.as_ref());
                    
                if let Some(cover_url) = cover_to_upload {
                    info!("Found cover URL: {}, uploading to CDN...", cover_url);
                    match self.upload_cover_to_seedpool(&torrent_id, cover_url, &seedpool_config) {
                        Ok(_) => info!("Cover image uploaded successfully"),
                        Err(e) => error!("Failed to upload cover image: {}", e),
                    }
                } else {
                    info!("No cover URL available for upload");
                }

                // Add torrent to qBittorrent for seeding
                if let Some(torrent_path) = &self.upload_data.torrent_path {
                    match self.add_torrent_to_qbittorrent(torrent_path) {
                        Ok(_) => info!("Torrent added to qBittorrent for seeding"),
                        Err(e) => info!("Failed to add torrent to qBittorrent: {}", e),
                    }
                }

                Ok(UploadResult {
                    success: true,
                    tracker: "seedpool".to_string(),
                    torrent_id: Some(torrent_id.clone()),
                    message: format!("Upload successful. Torrent ID: {}", torrent_id),
                })
            }
            Err(e) => {
                info!("Upload succeeded but failed to extract torrent ID: {}", e);
                Ok(UploadResult {
                    success: true,
                    tracker: "seedpool".to_string(),
                    torrent_id: None,
                    message: "Upload successful but couldn't extract torrent ID".to_string(),
                })
            }
        }
    }

    /// Upload to TorrentLeech
    fn upload_to_torrentleech(
        &self,
        _form_data: HashMap<String, String>,
    ) -> Result<UploadResult, String> {
        // This will be implemented to call the actual TorrentLeech upload API
        // For now, return a placeholder
        info!("TorrentLeech upload not yet implemented in UploadProcessor");
        Ok(UploadResult {
            success: false,
            tracker: "torrentleech".to_string(),
            torrent_id: None,
            message: "Upload processor for TorrentLeech not yet implemented".to_string(),
        })
    }

    /// Save description to a test file during dry-run
    fn save_description_to_test_file(
        &self,
        description: &str,
        tracker_name: &str,
    ) -> Result<String, String> {
        use chrono::Local;
        use std::io::Write;

        // Create a test output directory if it doesn't exist
        let test_dir = "test_descriptions";
        fs::create_dir_all(test_dir)
            .map_err(|e| format!("Failed to create test directory: {}", e))?;

        // Generate filename with timestamp and tracker name
        let timestamp = Local::now().format("%Y%m%d_%H%M%S");
        let release_name = self
            .upload_data
            .release_name
            .as_ref()
            .map(|s| s.replace("/", "_").replace("\\", "_").replace(":", "_"))
            .unwrap_or_else(|| "unknown".to_string());

        let filename = format!(
            "{}/{}_{}_{}_{}.txt",
            test_dir, timestamp, tracker_name, release_name, "description"
        );

        // Write the description to file
        let mut file = fs::File::create(&filename)
            .map_err(|e| format!("Failed to create description file: {}", e))?;

        // Write header with metadata
        writeln!(file, "// Dry-run description output").map_err(|e| e.to_string())?;
        writeln!(
            file,
            "// Generated: {}",
            Local::now().format("%Y-%m-%d %H:%M:%S")
        )
        .map_err(|e| e.to_string())?;
        writeln!(file, "// Tracker: {}", tracker_name).map_err(|e| e.to_string())?;
        if let Some(name) = &self.upload_data.release_name {
            writeln!(file, "// Release: {}", name).map_err(|e| e.to_string())?;
        }
        writeln!(file, "// ========================================\n")
            .map_err(|e| e.to_string())?;

        // Write the actual description
        file.write_all(description.as_bytes())
            .map_err(|e| format!("Failed to write description content: {}", e))?;

        // Also create a plain text version for easier reading
        let plain_filename = filename.replace(".txt", "_plain.txt");
        let plain_text = self.bbcode_to_plain(description);

        let mut plain_file = fs::File::create(&plain_filename)
            .map_err(|e| format!("Failed to create plain text description file: {}", e))?;

        writeln!(plain_file, "// Plain text version (BBCode removed)")
            .map_err(|e| e.to_string())?;
        writeln!(
            plain_file,
            "// Generated: {}",
            Local::now().format("%Y-%m-%d %H:%M:%S")
        )
        .map_err(|e| e.to_string())?;
        writeln!(plain_file, "// Tracker: {}", tracker_name).map_err(|e| e.to_string())?;
        if let Some(name) = &self.upload_data.release_name {
            writeln!(plain_file, "// Release: {}", name).map_err(|e| e.to_string())?;
        }
        writeln!(plain_file, "// ========================================\n")
            .map_err(|e| e.to_string())?;

        plain_file
            .write_all(plain_text.as_bytes())
            .map_err(|e| format!("Failed to write plain text description content: {}", e))?;

        info!(
            "Description saved to: {} (BBCode) and {} (plain text)",
            filename, plain_filename
        );

        Ok(filename)
    }

    /// Convert BBCode to plain text for easier reading
    fn bbcode_to_plain(&self, bbcode: &str) -> String {
        let mut result = bbcode.to_string();

        // Remove common BBCode tags
        let patterns = vec![
            // Basic formatting
            (r"\[b\](.*?)\[/b\]", "$1"),
            (r"\[i\](.*?)\[/i\]", "$1"),
            (r"\[u\](.*?)\[/u\]", "$1"),
            // Size and color
            (r"\[size=\d+\](.*?)\[/size\]", "$1"),
            (r"\[color=#?\w+\](.*?)\[/color\]", "$1"),
            // Structure
            (r"\[center\](.*?)\[/center\]", "$1"),
            (r"\[quote\](.*?)\[/quote\]", "\n---\n$1\n---\n"),
            (
                r"\[spoiler(?:=[^\]]+)?\](.*?)\[/spoiler\]",
                "\n[SPOILER]\n$1\n[/SPOILER]\n",
            ),
            // Links and images
            (r"\[url=(.*?)\](.*?)\[/url\]", "$2 ($1)"),
            (r"\[img(?:\s+width=\d+)?\](.*?)\[/img\]", "[Image: $1]"),
            // Tables
            (r"\[table\]", "\n"),
            (r"\[/table\]", "\n"),
            (r"\[tr\]", ""),
            (r"\[/tr\]", "\n"),
            (r"\[td\]", " | "),
            (r"\[/td\]", ""),
        ];

        // Apply regex replacements
        for (pattern, replacement) in patterns {
            if let Ok(re) = regex::Regex::new(pattern) {
                result = re.replace_all(&result, replacement).to_string();
            }
        }

        // Clean up extra whitespace
        result = result
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty() || line == &"")
            .collect::<Vec<_>>()
            .join("\n");

        // Replace multiple consecutive newlines with double newlines
        while result.contains("\n\n\n") {
            result = result.replace("\n\n\n", "\n\n");
        }

        result.trim().to_string()
    }

    /// Generate EPUB cover for upload
    fn generate_epub_cover_for_upload(
        &self,
        torrent_id: &str,
        _seedpool_config: &crate::core::SeedpoolConfig,
    ) -> Result<String, String> {
        use std::fs;
        use std::fs::File;
        use std::io::copy;
        use zip::ZipArchive;

        info!("📚 Extracting EPUB cover for torrent ID: {}", torrent_id);

        let input_path = self.input_path.as_ref()
            .ok_or("No input path available for EPUB cover processing")?;

        // Find EPUB file
        let epub_files = crate::utils::filter_files_by_extension(input_path, &["epub"])
            .map_err(|e| format!("Failed to find EPUB files: {}", e))?;

        let epub_file = epub_files.first()
            .ok_or("No EPUB file found for processing")?;

        let epub_path = epub_file.to_str()
            .ok_or("Invalid EPUB path")?;

        info!("📚 Extracting cover from EPUB: {}", epub_path);

        // Extract cover directly from EPUB file
        let file = File::open(epub_path)
            .map_err(|e| format!("Failed to open EPUB: {}", e))?;
        let mut archive = ZipArchive::new(file)
            .map_err(|e| format!("Failed to read EPUB as zip: {}", e))?;

        // Look for cover image in EPUB
        let mut cover_found = None;
        for i in 0..archive.len() {
            let file = archive.by_index(i)
                .map_err(|e| format!("Failed to access EPUB entry: {}", e))?;
            let name = file.name().to_lowercase();
            
            // Look for cover images (prioritize cover.* files)
            if name.contains("cover") && (name.ends_with(".jpg") || name.ends_with(".jpeg") || name.ends_with(".png")) {
                cover_found = Some(i);
                break;
            }
        }

        // If no cover found, try first image
        if cover_found.is_none() {
            for i in 0..archive.len() {
                let file = archive.by_index(i)
                    .map_err(|e| format!("Failed to access EPUB entry: {}", e))?;
                let name = file.name().to_lowercase();
                
                if name.ends_with(".jpg") || name.ends_with(".jpeg") || name.ends_with(".png") {
                    cover_found = Some(i);
                    break;
                }
            }
        }

        let cover_index = cover_found
            .ok_or("No cover image found in EPUB file")?;

        // Extract the cover to temp directory
        let temp_dir = format!("{}/temp_covers", self.config.paths.screenshots_dir);
        fs::create_dir_all(&temp_dir)
            .map_err(|e| format!("Failed to create temp directory: {}", e))?;

        let cover_filename = format!("epub-cover-{}.jpg", torrent_id);
        let cover_path = format!("{}/{}", temp_dir, cover_filename);

        {
            let mut cover_file = archive.by_index(cover_index)
                .map_err(|e| format!("Failed to access cover in EPUB: {}", e))?;
            let mut out_file = File::create(&cover_path)
                .map_err(|e| format!("Failed to create cover file: {}", e))?;
            copy(&mut cover_file, &mut out_file)
                .map_err(|e| format!("Failed to extract cover: {}", e))?;
        }

        // Create the correctly named JPEG file: torrent-cover_{torrent_id}.jpg
        let final_cover_filename = format!("torrent-cover_{}.jpg", torrent_id);
        let final_cover_path = format!("{}/{}", temp_dir, final_cover_filename);

        if self.dry_run {
            info!("🔄 Dry run: Would process EPUB cover to {}", final_cover_path);
            return Ok(format!("file://{}", final_cover_path));
        }

        // Convert to JPEG if needed using ffmpeg
        let (ffmpeg_path, _, _, _, _) = crate::core::Config::get_binary_paths(&self.config);
        let ffmpeg_path_str = ffmpeg_path.to_str().ok_or("Invalid ffmpeg path")?;

        let convert_output = std::process::Command::new(ffmpeg_path_str)
            .args(&[
                "-i",
                &cover_path,
                "-q:v",
                "2", // Quality (1-31, 1 is best)
                "-y", // Overwrite output
                &final_cover_path,
            ])
            .output()
            .map_err(|e| format!("Failed to run ffmpeg for EPUB cover conversion: {}", e))?;

        if !convert_output.status.success() {
            let stderr = String::from_utf8_lossy(&convert_output.stderr);
            return Err(format!("Failed to convert EPUB cover to JPEG: {}", stderr));
        }

        // Clean up temporary extracted cover
        let _ = fs::remove_file(&cover_path);

        // Set permissions to 777 for web server readability
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            info!("Setting permissions to 777 for EPUB cover: {}", final_cover_path);
            fs::set_permissions(&final_cover_path, fs::Permissions::from_mode(0o777))
                .map_err(|e| {
                    format!(
                        "Failed to set permissions for EPUB cover '{}': {}",
                        final_cover_path, e
                    )
                })?;
            info!("Successfully set permissions to 777 for EPUB cover: {}", final_cover_path);
        }

        info!("✅ Successfully processed EPUB cover: {}", final_cover_path);
        Ok(format!("file://{}", final_cover_path))
    }

    /// Generate PDF cover for upload
    fn generate_pdf_cover_for_upload(
        &self,
        torrent_id: &str,
        _seedpool_config: &crate::core::SeedpoolConfig,
    ) -> Result<String, String> {
        use crate::media::ebook::detect_ebook_files;

        // Find PDF files in the input path
        let input_path = self.input_path.as_ref()
            .ok_or("No input path available for PDF cover generation")?;

        let ebook_files = detect_ebook_files(input_path)
            .map_err(|e| format!("Failed to detect ebook files: {}", e))?;

        // Find the first PDF file
        let pdf_file = ebook_files
            .iter()
            .find(|file| matches!(file.ebook_type, crate::core::EbookType::Pdf))
            .ok_or("No PDF file found for cover generation")?;

        let pdf_path = pdf_file.path.to_str()
            .ok_or("Invalid PDF path")?;

        info!("📚 Generating cover from PDF: {}", pdf_path);

        // Generate the cover using our new function
        let cover_path = crate::media::ebook::generate_pdf_cover(
            pdf_path,
            torrent_id,
            &self.config,
            self.dry_run,
        )?;

        // Convert local path to a file:// URL for upload_cover_to_seedpool
        let cover_url = format!("file://{}", cover_path);
        
        info!("📚 Generated PDF cover at: {}", cover_url);
        Ok(cover_url)
    }

    /// Upload cover image to CDN with correct naming scheme for Seedpool
    fn upload_cover_to_seedpool(
        &self,
        torrent_id: &str,
        cover_url: &str,
        seedpool_config: &crate::core::SeedpoolConfig,
    ) -> Result<(), String> {
        use crate::utils::http::{download_file, upload_to_cdn};
        use std::fs;

        info!(
            "Uploading cover image to CDN for torrent ID: {}",
            torrent_id
        );

        // Use the seedpool-specific CDN paths
        let remote_path = &seedpool_config.screenshots.remote_path;

        // Create temporary directory for processing
        let temp_dir = format!("{}/temp_covers", self.config.paths.screenshots_dir);
        fs::create_dir_all(&temp_dir)
            .map_err(|e| format!("Failed to create temp directory: {}", e))?;

        let temp_cover_path = if cover_url.starts_with("file://") {
            // Handle local file URLs (e.g., from PDF cover generation)
            let local_path = cover_url.strip_prefix("file://").unwrap();
            info!("📚 Using local cover file: {}", local_path);
            
            // Verify the local file exists
            if !std::path::Path::new(local_path).exists() {
                return Err(format!("Local cover file not found: {}", local_path));
            }
            
            local_path.to_string()
        } else {
            // Handle remote URLs (existing behavior)
            let cover_data = download_file(cover_url, 30)
                .map_err(|e| format!("Failed to download cover image: {}", e))?;

            // Determine the image format from the URL or data
            let temp_filename = if cover_url.to_lowercase().ends_with(".png") {
                format!("temp_cover_{}.png", torrent_id)
            } else if cover_url.to_lowercase().ends_with(".webp") {
                format!("temp_cover_{}.webp", torrent_id)
            } else {
                format!("temp_cover_{}.jpg", torrent_id)
            };

            let temp_path = format!("{}/{}", temp_dir, temp_filename);

            // Write the cover data to a temporary file
            fs::write(&temp_path, cover_data)
                .map_err(|e| format!("Failed to write temporary cover file: {}", e))?;
                
            temp_path
        };

        // Create the correctly named JPEG file: torrent-cover_{torrent_id}.jpg
        let cover_filename = format!("torrent-cover_{}.jpg", torrent_id);
        let local_cover_path = format!("{}/{}", temp_dir, cover_filename);

        // Convert to JPEG if needed using ffmpeg
        if !temp_cover_path.to_lowercase().ends_with(".jpg") {
            info!("Converting cover image to JPEG format");
            let (ffmpeg_path, _, _, _, _) = Config::get_binary_paths(&self.config);
            let ffmpeg_path_str = ffmpeg_path.to_str().ok_or("Invalid ffmpeg path")?;

            let convert_output = std::process::Command::new(ffmpeg_path_str)
                .args(&[
                    "-i",
                    &temp_cover_path,
                    "-vf",
                    "scale='min(1000,iw)':'min(1000,ih)'", // Limit to max 1000x1000
                    "-q:v",
                    "2",  // High quality JPEG
                    "-y", // Overwrite output
                    &local_cover_path,
                ])
                .output()
                .map_err(|e| format!("Failed to run ffmpeg for cover conversion: {}", e))?;

            if !convert_output.status.success() {
                let stderr = String::from_utf8_lossy(&convert_output.stderr);
                return Err(format!("Failed to convert cover to JPEG: {}", stderr));
            }

            // Clean up temporary file
            if let Err(e) = fs::remove_file(&temp_cover_path) {
                info!("Failed to clean up temporary cover file: {}", e);
            }
        } else {
            // Already JPEG, just rename
            fs::rename(&temp_cover_path, &local_cover_path)
                .map_err(|e| format!("Failed to rename cover file: {}", e))?;
        }

        // Set permissions to 777 for web server readability
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            info!("Setting permissions to 777 for cover image: {}", local_cover_path);
            fs::set_permissions(&local_cover_path, fs::Permissions::from_mode(0o777))
                .map_err(|e| {
                    format!(
                        "Failed to set permissions for cover image '{}': {}",
                        local_cover_path, e
                    )
                })?;
            info!("Successfully set permissions to 777 for cover image: {}", local_cover_path);
        }

        // Upload to CDN with the correct path structure
        let remote_cover_path = format!("{}/covers/", remote_path.trim_end_matches('/'));
        info!("Uploading cover to CDN path: {}", remote_cover_path);

        if self.dry_run {
            info!(
                "[DRY RUN] Would upload cover to CDN: {} -> {}",
                local_cover_path, remote_cover_path
            );
            return Ok(());
        }

        upload_to_cdn(&local_cover_path, &remote_cover_path)
            .map_err(|e| format!("Failed to upload cover to CDN: {}", e))?;

        // Clean up temporary file
        if let Err(e) = fs::remove_file(&local_cover_path) {
            info!("Failed to clean up temporary cover file: {}", e);
        }

        info!("Successfully uploaded cover: {}", cover_filename);
        Ok(())
    }

    /// Add torrent to qBittorrent for seeding after successful upload
    fn add_torrent_to_qbittorrent(&self, torrent_path: &str) -> Result<(), String> {
        use crate::clients::qbittorrent::QBittorrentClient;
        use std::fs;

        info!("Adding torrent to qBittorrent: {}", torrent_path);

        // Load main configuration to get qBittorrent configs
        let config_content = fs::read_to_string("config/config.yaml")
            .map_err(|e| format!("Failed to read config file: {}", e))?;
        let main_config: crate::core::Config = serde_yaml::from_str(&config_content)
            .map_err(|e| format!("Failed to parse config: {}", e))?;

        if main_config.qbittorrent.is_empty() {
            return Err("No qBittorrent configurations found in config".to_string());
        }

        // Find the first qBittorrent instance with valid configuration
        let qb_config = main_config
            .qbittorrent
            .iter()
            .find(|config| !config.webui_url.is_empty() && !config.username.is_empty())
            .ok_or("No valid qBittorrent configuration found")?;

        if self.dry_run {
            info!(
                "[DRY RUN] Would add torrent to qBittorrent: {}",
                qb_config.webui_url
            );
            return Ok(());
        }

        // Create qBittorrent client and add torrent
        let qb_client = QBittorrentClient::new(qb_config.clone())
            .map_err(|e| format!("Failed to create qBittorrent client: {}", e))?;

        qb_client
            .add_torrent_file(torrent_path)
            .map_err(|e| format!("Failed to add torrent to qBittorrent: {}", e))?;

        info!("Successfully added torrent to qBittorrent for seeding");
        Ok(())
    }

    /// Process ebook with Open Library lookup and cover extraction
    fn process_ebook_open_library(
        &self,
        torrent_id: &str,
        seedpool_config: &crate::core::SeedpoolConfig,
        ebook_type: &crate::core::EbookType,
    ) -> Result<(), String> {
        use crate::media::ebook::{extract_metadata_from_epub, generate_ebook_bbcode_description};
        use reqwest::blocking::Client;

        // Find ebook file in input path based on type
        let input_path = self.input_path.as_ref()
            .ok_or("No input path available for ebook processing")?;

        let (ebook_files, file_type_name) = match ebook_type {
            crate::core::EbookType::Epub => {
                let files = crate::utils::filter_files_by_extension(input_path, &["epub"])
                    .map_err(|e| format!("Failed to find EPUB files: {}", e))?;
                (files, "EPUB")
            }
            crate::core::EbookType::Pdf => {
                let files = crate::utils::filter_files_by_extension(input_path, &["pdf"])
                    .map_err(|e| format!("Failed to find PDF files: {}", e))?;
                (files, "PDF")
            }
            _ => return Err(format!("Ebook type {:?} not supported for Open Library processing", ebook_type)),
        };

        let ebook_file = ebook_files.first()
            .ok_or(format!("No {} file found for processing", file_type_name))?;

        let ebook_path = ebook_file.to_str()
            .ok_or("Invalid ebook path")?;

        info!("📚 Extracting metadata from {}: {}", file_type_name, ebook_path);

        // Extract title and author based on ebook type
        let (title, author) = match ebook_type {
            crate::core::EbookType::Epub => {
                extract_metadata_from_epub(ebook_path)
                    .map_err(|e| format!("Failed to extract EPUB metadata: {}", e))?
            }
            crate::core::EbookType::Pdf => {
                crate::media::ebook::extract_metadata_from_pdf(ebook_path)
                    .map_err(|e| format!("Failed to extract PDF metadata: {}", e))?
            }
            _ => return Err(format!("Unsupported ebook type: {:?}", ebook_type)),
        };

        let title = title.unwrap_or_else(|| "Unknown Title".to_string());
        let author = author.unwrap_or_else(|| "Unknown Author".to_string());

        info!("📚 Extracted - Title: '{}', Author: '{}'", title, author);

        // Only try Open Library if we have at least a title or author
        if title != "Unknown Title" || author != "Unknown Author" {
            let query = format!(
                "https://openlibrary.org/search.json?title={}&author={}",
                urlencoding::encode(&title),
                urlencoding::encode(&author)
            );

            info!("📚 Querying Open Library API: {}", query);

            let client = Client::new();
            let response = client
                .get(&query)
                .send()
                .map_err(|e| format!("Failed to query Open Library API: {}", e))?;

            if response.status().is_success() {
                let json: serde_json::Value = response
                    .json()
                    .map_err(|e| format!("Failed to parse Open Library API response: {}", e))?;

                if let Some(first_result) = json["docs"].as_array().and_then(|docs| docs.get(0)) {
                    // Use Open Library's title and author if available
                    let ol_title = first_result["title"].as_str().unwrap_or(&title).to_string();
                    let ol_author = first_result["author_name"]
                        .as_array()
                        .and_then(|authors| authors.get(0))
                        .and_then(|author| author.as_str())
                        .unwrap_or(&author)
                        .to_string();

                    info!("📚 Open Library found - Title: '{}', Author: '{}'", ol_title, ol_author);

                    // Extract Open Library work and author keys
                    let open_library_work_key = first_result["key"]
                        .as_str()
                        .unwrap_or("")
                        .trim_start_matches("/works/")
                        .to_string();
                    let open_library_author_key = first_result["author_key"]
                        .as_array()
                        .and_then(|keys| keys.get(0))
                        .and_then(|key| key.as_str())
                        .unwrap_or("")
                        .to_string();

                    // Extract cover ID for potential cover download
                    let cover_id = first_result["cover_i"].as_u64();

                    info!("📚 Open Library keys - Work: '{}', Author: '{}', Cover ID: {:?}", 
                          open_library_work_key, open_library_author_key, cover_id);

                    // Generate rich BBCode description using Open Library data
                    if !open_library_work_key.is_empty() && !open_library_author_key.is_empty() {
                        match generate_ebook_bbcode_description(
                            &ol_title,
                            &ol_author,
                            &open_library_work_key,
                            &open_library_author_key,
                            &client,
                        ) {
                            Ok((description, _subjects)) => {
                                info!("✅ Generated rich description from Open Library");
                                info!("📚 Description length: {} characters", description.len());
                                // TODO: Update the torrent description on Seedpool with the rich description
                                // This would require a separate API call to update the torrent description
                            }
                            Err(e) => {
                                info!("⚠️ Failed to generate rich description: {}", e);
                            }
                        }
                    }

                    // Download and upload cover if available
                    if let Some(cover_id) = cover_id {
                        let cover_url = format!("https://covers.openlibrary.org/b/id/{}-L.jpg", cover_id);
                        info!("📚 Downloading cover from Open Library: {}", cover_url);

                        match self.upload_cover_to_seedpool(torrent_id, &cover_url, seedpool_config) {
                            Ok(_) => info!("✅ Successfully uploaded Open Library cover"),
                            Err(e) => info!("⚠️ Failed to upload Open Library cover: {}", e),
                        }
                    } else {
                        info!("📚 No cover available in Open Library");
                    }
                                    } else {
                        info!("📚 No results found in Open Library for title: '{}', author: '{}'", title, author);
                        
                        // Generate fallback description with extracted metadata
                        info!("📚 Generating fallback description with extracted metadata");
                        let fallback_description = format!(
                            "[center][b][size=32][color=#2E86C1]{}[/color][/size][/b][/center]\n\n[center][b]Title:[/b] {}\n[b]Author:[/b] {}\n[b]Format:[/b] EPUB[/center]\n\n[center][b][size=12][color=#757575]Created with mkbrr, ffmpeg, and mediainfo. Posted to this fine tracker with seedbrr.[/color][/size][/b]\n\n[url=https://github.com/seed-pool/seed-tools][img]https://cdn.seedpool.org/sp.png[/img][/url]  [url=https://github.com/autobrr/mkbrr][img]https://cdn.seedpool.org/mkbrr.png[/img][/url]  [url=https://www.rust-lang.org][img]https://cdn.seedpool.org/rust.png[/img][/url][/center]",
                            self.input_path.as_ref().and_then(|p| std::path::Path::new(p).file_name()).and_then(|n| n.to_str()).unwrap_or("EPUB"),
                            title,
                            author
                        );
                        
                        info!("📚 Generated fallback description with extracted EPUB metadata");
                        info!("📚 Fallback description length: {} characters", fallback_description.len());
                        // TODO: Update torrent description with fallback description
                    }
                } else {
                    info!("⚠️ Open Library API returned status: {}", response.status());
                    
                    // Generate fallback description even for API errors
                    if title != "Unknown Title" || author != "Unknown Author" {
                        info!("📚 Generating fallback description due to API error");
                        let fallback_description = format!(
                            "[center][b][size=32][color=#2E86C1]{}[/color][/size][/b][/center]\n\n[center][b]Title:[/b] {}\n[b]Author:[/b] {}\n[b]Format:[/b] EPUB[/center]\n\n[center][b][size=12][color=#757575]Created with mkbrr, ffmpeg, and mediainfo. Posted to this fine tracker with seedbrr.[/color][/size][/b]\n\n[url=https://github.com/seed-pool/seed-tools][img]https://cdn.seedpool.org/sp.png[/img][/url]  [url=https://github.com/autobrr/mkbrr][img]https://cdn.seedpool.org/mkbrr.png[/img][/url]  [url=https://www.rust-lang.org][img]https://cdn.seedpool.org/rust.png[/img][/url][/center]",
                            self.input_path.as_ref().and_then(|p| std::path::Path::new(p).file_name()).and_then(|n| n.to_str()).unwrap_or("EPUB"),
                            title,
                            author
                        );
                        
                        info!("📚 Generated fallback description due to API error");
                        info!("📚 Fallback description length: {} characters", fallback_description.len());
                        // TODO: Update torrent description with fallback description
                    }
                }
            } else {
                info!("📚 Skipping Open Library lookup - no valid title or author");
            }

        Ok(())
    }
}
