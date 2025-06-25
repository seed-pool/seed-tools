use std::collections::HashMap;
use std::sync::Arc;
use std::path::Path;
use std::fs;
use crate::types::{MediaType, Config, UploadComponent};
use crate::utils::{
    find_and_read_nfo, generate_mediainfo, check_all_duplicates,
    fetch_tmdb_id, upload_to_cdn
};
use log::{info, warn, error};

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
        self.field_map.insert(internal_name.to_string(), tracker_name.to_string());
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
    description_config: Option<crate::description::DescriptionConfig>,
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
    pub fn with_description_config(mut self, config: crate::description::DescriptionConfig) -> Self {
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
    pub fn with_title_info(mut self, title: impl Into<String>, year: Option<impl Into<String>>) -> Self {
        self.title = Some(title.into());
        self.year = year.map(|y| y.into());
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
    
    /// Add a custom component
    pub fn with_custom_component(mut self, name: impl Into<String>, component: UploadComponent) -> Self {
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
            _ => warn!("Unknown component to skip: {}", component),
        }
        self
    }
    
    /// Detect and apply the active tracker configuration
    fn apply_tracker_config(&mut self) -> Result<(), String> {
        // Load tracker configs
        let seedpool_config = crate::utils::load_tracker_config::<crate::types::SeedpoolConfig>("seedpool")
            .map_err(|e| format!("Failed to load seedpool config: {}", e))?;
        let torrentleech_config = crate::utils::load_tracker_config::<crate::types::TorrentLeechConfig>("torrentleech")
            .map_err(|e| format!("Failed to load torrentleech config: {}", e))?;
        
        // Determine which tracker is enabled and apply its settings
        if seedpool_config.general.enabled {
            info!("Applying Seedpool configuration");
            self.active_tracker = Some("seedpool".to_string());
            
            // Apply Seedpool settings
            self.upload_config.skip_duplicate_check = !seedpool_config.settings.enable_duplicate_check;
            self.upload_config.skip_mediainfo = !seedpool_config.settings.enable_mediainfo;
            self.upload_config.skip_nfo = !seedpool_config.settings.enable_nfo;
            self.upload_config.skip_screenshots = !seedpool_config.settings.enable_screenshots;
            self.upload_config.skip_sample = !seedpool_config.settings.enable_sample;
            self.upload_config.skip_tmdb = !seedpool_config.settings.enable_tmdb;
            self.upload_config.skip_torrent_creation = !seedpool_config.settings.enable_torrent_creation;
            self.upload_config.screenshot_count = seedpool_config.settings.screenshot_count;
            
            if seedpool_config.settings.enable_torrent_creation {
                self.upload_config.announce_url = Some(seedpool_config.settings.announce_url.clone());
            }
            
            // Store tracker-specific metadata
            let mut metadata = HashMap::new();
            metadata.insert("tracker".to_string(), "seedpool".to_string());
            metadata.insert("stripshit".to_string(), seedpool_config.settings.stripshit_from_videos.to_string());
            metadata.insert("remote_path".to_string(), seedpool_config.screenshots.remote_path.clone());
            metadata.insert("image_path".to_string(), seedpool_config.screenshots.image_path.clone());
            metadata.insert("custom_description".to_string(), seedpool_config.settings.custom_description.clone());
            self.components.insert("tracker_config".to_string(), UploadComponent::Metadata(metadata));
            
        } else if torrentleech_config.general.enabled {
            info!("Applying TorrentLeech configuration");
            self.active_tracker = Some("torrentleech".to_string());
            
            // Apply TorrentLeech settings
            self.upload_config.skip_duplicate_check = !torrentleech_config.settings.enable_duplicate_check;
            self.upload_config.skip_mediainfo = !torrentleech_config.settings.enable_mediainfo;
            self.upload_config.skip_nfo = !torrentleech_config.settings.enable_nfo;
            self.upload_config.skip_screenshots = !torrentleech_config.settings.enable_screenshots;
            self.upload_config.skip_sample = !torrentleech_config.settings.enable_sample;
            self.upload_config.skip_tmdb = !torrentleech_config.settings.enable_tmdb;
            self.upload_config.skip_torrent_creation = !torrentleech_config.settings.enable_torrent_creation;
            self.upload_config.screenshot_count = torrentleech_config.settings.screenshot_count;
            
            if torrentleech_config.settings.enable_torrent_creation && !torrentleech_config.general.announce_url_1.is_empty() {
                self.upload_config.announce_url = Some(torrentleech_config.general.announce_url_1.clone());
            }
            
            // Store tracker-specific metadata
            let mut metadata = HashMap::new();
            metadata.insert("tracker".to_string(), "torrentleech".to_string());
            metadata.insert("stripshit".to_string(), torrentleech_config.settings.stripshit_from_videos.to_string());
            metadata.insert("custom_description".to_string(), torrentleech_config.settings.custom_description.clone());
            self.components.insert("tracker_config".to_string(), UploadComponent::Metadata(metadata));
            
        } else {
            warn!("No tracker is enabled in configuration");
            self.active_tracker = None;
        }
        
        Ok(())
    }
    
    /// Build the upload data
    pub fn build(mut self) -> Result<crate::media::video::UploadData, String> {
        // Apply tracker configuration automatically
        self.apply_tracker_config()?;
        
        info!("Building upload data for: {} (tracker: {:?})", self.input_path, self.active_tracker);
        
        // Process NFO
        if !self.upload_config.skip_nfo {
            match find_and_read_nfo(&self.input_path) {
                Ok(nfo_data) => {
                    if let Some((path, content)) = nfo_data {
                        info!("Found NFO file: {}", path);
                        self.components.insert(
                            "nfo".to_string(),
                            UploadComponent::NfoData { path, content }
                        );
                    }
                }
                Err(e) => warn!("Failed to find/read NFO: {}", e),
            }
        }
        
        // Process Mediainfo
        if !self.upload_config.skip_mediainfo {
            match generate_mediainfo(&self.input_path, &self.config) {
                Ok(mediainfo) => {
                    info!("Generated mediainfo");
                    self.components.insert(
                        "mediainfo".to_string(),
                        UploadComponent::Mediainfo(mediainfo)
                    );
                }
                Err(e) => warn!("Failed to generate mediainfo: {}", e),
            }
        }
        
        // Process Duplicate Check
        if !self.upload_config.skip_duplicate_check {
            let check_title = self.title.as_ref()
                .or_else(|| self.video_metadata.as_ref().map(|m| &m.title))
                .ok_or("No title available for duplicate check")?;
                
            match check_all_duplicates(check_title) {
                Ok(duplicates) => {
                    if !duplicates.is_empty() {
                        warn!("Found {} duplicate(s)", duplicates.len());
                        self.components.insert(
                            "duplicates".to_string(),
                            UploadComponent::DuplicateCheckResults(duplicates)
                        );
                    } else {
                        info!("No duplicates found");
                    }
                }
                Err(e) => warn!("Failed to check duplicates: {}", e),
            }
        }
        
        // Process TMDB lookup (for video content)
        if !self.upload_config.skip_tmdb && matches!(self.media_type, MediaType::Video(_)) {
            if let Some(metadata) = &self.video_metadata {
                // Check if it's a movie or TV show based on metadata
                let is_movie_or_tv = match &metadata.category {
                    cat if format!("{:?}", cat).contains("Movie") => true,
                    cat if format!("{:?}", cat).contains("TvShow") => true,
                    _ => false,
                };
                
                if is_movie_or_tv {
                    let release_type = if format!("{:?}", metadata.category).contains("Movie") {
                        "Movie"
                    } else {
                        "TvShow"
                    };
                    
                    match fetch_tmdb_id(
                        &metadata.title,
                        metadata.year.map(|y| y.to_string()),
                        &self.config.general.tmdb_api_key,
                        release_type
                    ) {
                        Ok(tmdb_id) => {
                            info!("Found TMDB ID: {}", tmdb_id);
                            // TODO: Also fetch IMDB/TVDB IDs
                            self.components.insert(
                                "tmdb".to_string(),
                                UploadComponent::TmdbData {
                                    tmdb_id,
                                    imdb_id: None,
                                    tvdb_id: None,
                                    title: metadata.title.clone(),
                                    year: metadata.year.map(|y| y.to_string()),
                                }
                            );
                        }
                        Err(e) => warn!("Failed to fetch TMDB ID: {}", e),
                    }
                }
            }
        }
        
        // Process Screenshots (for video content)
        if !self.upload_config.skip_screenshots && matches!(self.media_type, MediaType::Video(_)) {
            // Get the appropriate extensions for the media type
            let extensions = self.accepted_extensions.as_ref()
                .map(|exts| exts.iter().map(|s| s.as_str()).collect::<Vec<_>>())
                .unwrap_or_else(|| crate::types::VideoType::all_extensions());
            
            // Find video files using filter_files_by_extension
            match crate::utils::filter_files_by_extension(&self.input_path, &extensions) {
                Ok(files) if !files.is_empty() => {
                    let video_file = &files[0];
                    // Determine input name for screenshots
                    let input_name = self.title.as_ref()
                        .or_else(|| self.video_metadata.as_ref().map(|m| &m.title))
                        .map(|s| s.as_str())
                        .unwrap_or_else(|| std::path::Path::new(&self.input_path)
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("unknown"));
                    
                    // Try to generate screenshots
                    // Note: This will use either ImgBB (if configured) or CDN paths
                    match crate::utils::generate_screenshots(
                        video_file.to_str().unwrap_or(""),
                        &self.config,
                        self.config.imgbb.as_ref().map(|c| c.imgbb_api_key.as_str()),
                        None, // remote_path - would need tracker-specific config
                        None, // image_path - would need tracker-specific config
                        input_name,
                        self.upload_config.dry_run,
                    ) {
                        Ok((screenshots, thumbnails)) => {
                            if !screenshots.is_empty() {
                                info!("Generated {} screenshots", screenshots.len());
                                self.components.insert(
                                    "screenshots".to_string(),
                                    UploadComponent::Screenshots(screenshots.clone())
                                );
                                self.components.insert(
                                    "thumbnails".to_string(),
                                    UploadComponent::Thumbnails(thumbnails)
                                );
                            }
                        }
                        Err(e) => warn!("Failed to generate screenshots: {}", e),
                    }
                }
                Ok(files) => {
                    if files.is_empty() {
                        warn!("No video files found for screenshots");
                    }
                }
                Err(e) => warn!("Failed to find video files: {}", e),
            }
        }
        
        // Process Sample (for video content)
        if !self.upload_config.skip_sample && matches!(self.media_type, MediaType::Video(_)) {
            // Get the appropriate extensions for the media type
            let extensions = self.accepted_extensions.as_ref()
                .map(|exts| exts.iter().map(|s| s.as_str()).collect::<Vec<_>>())
                .unwrap_or_else(|| crate::types::VideoType::all_extensions());
            
            // Find video files using filter_files_by_extension
            match crate::utils::filter_files_by_extension(&self.input_path, &extensions) {
                Ok(files) if !files.is_empty() => {
                    let video_file = &files[0];
                    // Determine input name for sample
                    let input_name = self.title.as_ref()
                        .or_else(|| self.video_metadata.as_ref().map(|m| &m.title))
                        .map(|s| s.as_str())
                        .unwrap_or_else(|| std::path::Path::new(&self.input_path)
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("unknown"));
                    
                    // Get binary paths
                    let (ffmpeg_path, _, _, _) = crate::types::Config::get_binary_paths(&self.config);
                    let ffmpeg_path_str = ffmpeg_path.to_str().ok_or("Invalid ffmpeg path").unwrap_or("ffmpeg");
                    
                    // Generate sample
                    // Note: Would need tracker-specific config for remote_path and image_path
                    match crate::media::video::generate_sample(
                        video_file.to_str().unwrap_or(""),
                        &self.config.paths.screenshots_dir,
                        "", // remote_path - would need tracker-specific config
                        "", // image_path - would need tracker-specific config
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
                                UploadComponent::Sample { url: sample_url, filename }
                            );
                        }
                        Err(e) => {
                            // Only warn if we're not in dry run mode or if we have upload paths
                            if !self.upload_config.dry_run {
                                warn!("Failed to generate sample: {}", e);
                            }
                        }
                    }
                }
                Ok(files) => {
                    if files.is_empty() {
                        warn!("No video files found for sample generation");
                    }
                }
                Err(e) => warn!("Failed to find video files: {}", e),
            }
        }
        
        // Create torrent
        if !self.upload_config.skip_torrent_creation {
            if let Some(announce_url) = &self.upload_config.announce_url {
                // Determine stripshit setting from tracker config
                let stripshit = if let Some(UploadComponent::Metadata(metadata)) = self.components.get("tracker_config") {
                    metadata.get("stripshit")
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
                            UploadComponent::TorrentPath(torrent_path)
                        );
                    }
                    Err(e) => error!("Failed to create torrent: {}", e),
                }
            } else {
                warn!("No announce URL provided for torrent creation");
            }
        }
        
        // Build description if we have the necessary components
        self.build_description()?;
        
        // Build the final UploadData
        let mut upload_data = crate::media::video::UploadData::new();
        
        // Set release name
        if let Some(metadata) = &self.video_metadata {
            upload_data.release_name = Some(metadata.title.clone());
        } else if let Some(title) = &self.title {
            upload_data.release_name = Some(title.clone());
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
                _ => {
                    // Other components might be used by tracker-specific code
                    info!("Component '{}' stored but not added to UploadData", name);
                }
            }
        }
        
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
    fn create_torrent_with_extensions(&self, announce_url: &str, stripshit: bool) -> Result<String, String> {
        use std::process::Command;
        use crate::naming::generate_release_name;
        
        let torrent_dir = &self.config.paths.torrent_dir;
        fs::create_dir_all(torrent_dir)
            .map_err(|e| format!("Failed to create torrent directory '{}': {}", torrent_dir, e))?;
        
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
            "-t", announce_url,
            "-o", &torrent_file,
            "--source", "seedpool.org",
            &self.input_path,
        ]);
        
        // Build exclude pattern based on accepted extensions and stripshit setting
        let is_video = matches!(self.media_type, MediaType::Video(_));
        
        if (stripshit && is_video) || self.accepted_extensions.is_some() {
            let mut exclude_patterns = Vec::<String>::new();
            
            // Add standard exclusions only for video files if stripshit is enabled
            if stripshit && is_video {
                let standard_excludes = vec![
                    "[X]*", "*sample*", "*proof*", "*screens*", "*screenshots*",
                    "*.txt", "*.jpg", "*.jpeg", "*.png", "*.nfo", "*.srr", "*.doc", "*.sfv", "*.r??"
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
        
        // Execute the mkbrr command
        let output = command.output()
            .map_err(|e| format!("Failed to run mkbrr: {}", e))?;
        
        if !output.stdout.is_empty() {
            info!("mkbrr stdout:\n{}", String::from_utf8_lossy(&output.stdout));
        }
        if !output.stderr.is_empty() {
            error!("mkbrr stderr:\n{}", String::from_utf8_lossy(&output.stderr));
        }
        
        if !output.status.success() {
            return Err(format!(
                "mkbrr failed to create torrent for input path: {}. Exit code: {}",
                self.input_path,
                output.status.code().unwrap_or(-1)
            ));
        }
        
        info!("Created torrent: {}", torrent_file);
        Ok(torrent_file)
    }
    
    /// Get all file extensions present in the given path
    fn get_all_extensions_in_path(&self, path: &str) -> Result<Vec<String>, String> {
        use walkdir::WalkDir;
        use std::collections::HashSet;
        
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
    
    /// Build the description using DescriptionBuilder
    fn build_description(&mut self) -> Result<(), String> {
        use crate::description::{DescriptionBuilder, DescriptionConfig};
        
        // Use provided config or create default for media type
        let config = self.description_config.clone()
            .unwrap_or_else(|| DescriptionConfig::default());
        
        let mut builder = DescriptionBuilder::with_config(
            self.media_type.clone(),
            config
        );
        
        // Add title if available
        if let Some(title) = &self.title {
            builder = builder.title(title);
        } else if let Some(metadata) = &self.video_metadata {
            builder = builder.title(&metadata.title);
        }
        
        // Add screenshots if available
        if let Some(UploadComponent::Screenshots(screenshots)) = self.components.get("screenshots") {
            if !screenshots.is_empty() {
                builder = builder.images(screenshots.clone());
            }
        }
        
        // Add sample if available
        if let Some(UploadComponent::Sample { url, filename }) = self.components.get("sample") {
            builder = builder.sample(url, filename);
        }
        
        // Add any custom description from tracker config
        if let Some(UploadComponent::Metadata(metadata)) = self.components.get("tracker_config") {
            if let Some(custom_desc) = metadata.get("custom_description") {
                if !custom_desc.is_empty() {
                    builder = builder.raw(custom_desc);
                }
            }
        }
        
        // Build the description and store it as a component
        let description = builder.build();
        self.components.insert(
            "description".to_string(),
            UploadComponent::Description(description)
        );
        
        Ok(())
    }
}

/// Helper function to create a video upload builder with sensible defaults
/// Video uploads typically want: nfo, mediainfo, screenshots, sample, duplicate check, tmdb
/// The actual inclusion of these components depends on tracker config settings
pub fn create_video_upload(
    input_path: &str, 
    config: Arc<Config>,
    metadata: crate::media::video::VideoMetadata,
) -> UploadBuilder {
    UploadBuilder::new(input_path, MediaType::Video(crate::types::VideoType::Mkv), config)
        .with_extensions(crate::types::VideoType::all_extensions())
        .with_video_metadata(metadata)
        .with_nfo()
        .with_mediainfo()
        .with_screenshots(4)
        .with_sample()
        .with_duplicate_check()
        .with_tmdb_lookup()
}

/// Helper function to create an ebook upload builder with sensible defaults
/// Ebook uploads typically want: nfo, mediainfo, duplicate check
/// Comics/magazines might also want screenshots (handled separately)
/// The actual inclusion of these components depends on tracker config settings
pub fn create_ebook_upload(
    input_path: &str,
    config: Arc<Config>,
    title: &str,
    author: Option<&str>,
) -> UploadBuilder {
    // Get all ebook extensions
    let extensions: Vec<&str> = vec!["epub", "pdf", "cbz", "cbr", "mobi", "azw", "azw3", "lit", "pdb"];
    
    let mut builder = UploadBuilder::new(
        input_path, 
        MediaType::Ebook(crate::types::EbookType::Epub), 
        config
    )
    .with_extensions(extensions)
    .with_title_info(title, None::<String>)
    .with_nfo()
    .with_mediainfo()
    .with_duplicate_check();
    
    // Add author to metadata if provided
    if let Some(author) = author {
        let mut metadata = HashMap::new();
        metadata.insert("author".to_string(), author.to_string());
        builder = builder.with_custom_component(
            "metadata",
            UploadComponent::Metadata(metadata)
        );
    }
    
    builder
}

/// Helper function to create a comic/magazine upload builder
/// Comics and magazines typically want: nfo, mediainfo, duplicate check, screenshots
/// The actual inclusion of these components depends on tracker config settings
pub fn create_comic_upload(
    input_path: &str,
    config: Arc<Config>,
    title: &str,
    issue_number: Option<&str>,
) -> UploadBuilder {
    use crate::description::DescriptionConfig;
    use crate::types::ImageLayout;
    
    // Configure description for comics/magazines
    let mut desc_config = DescriptionConfig::default();
    desc_config.image_layout = ImageLayout::TwoColumn; // Comics use 2 column layout
    desc_config.max_images = 10; // Show more preview pages
    desc_config.image_width = 350; // Smaller width for comic pages
    
    let mut builder = UploadBuilder::new(
        input_path, 
        MediaType::Ebook(crate::types::EbookType::Cbz), 
        config
    )
    .with_extensions(vec!["cbz", "cbr"])
    .with_title_info(title, None::<String>)
    .with_description_config(desc_config)
    .with_nfo()
    .with_mediainfo()
    .with_duplicate_check()
    .with_screenshots(4); // Comics benefit from preview pages
    
    // Add issue metadata if provided
    if let Some(issue) = issue_number {
        let mut metadata = HashMap::new();
        metadata.insert("issue".to_string(), issue.to_string());
        builder = builder.with_custom_component(
            "metadata",
            UploadComponent::Metadata(metadata)
        );
    }
    
    builder
}

/// Helper function to create an audio upload builder
/// Audio uploads typically want: nfo, mediainfo, duplicate check
/// The actual inclusion of these components depends on tracker config settings
pub fn create_audio_upload(
    input_path: &str,
    config: Arc<Config>,
    album: &str,
    artist: &str,
    year: Option<&str>,
) -> UploadBuilder {
    let mut metadata = HashMap::new();
    metadata.insert("album".to_string(), album.to_string());
    metadata.insert("artist".to_string(), artist.to_string());
    
    UploadBuilder::new(
        input_path,
        MediaType::Audio(crate::types::AudioType::Flac),
        config
    )
    .with_extensions(crate::types::AudioType::all_extensions())
    .with_title_info(album, year)
    .with_nfo()
    .with_mediainfo()
    .with_duplicate_check()
    .with_custom_component("metadata", UploadComponent::Metadata(metadata))
}

/// Helper function to create a game upload builder
/// Game uploads typically want: nfo, duplicate check
/// Screenshots might be relevant for some games
/// The actual inclusion of these components depends on tracker config settings
pub fn create_game_upload(
    input_path: &str,
    config: Arc<Config>,
    title: &str,
    platform: &str,
) -> UploadBuilder {
    let mut metadata = HashMap::new();
    metadata.insert("platform".to_string(), platform.to_string());
    
    UploadBuilder::new(
        input_path,
        MediaType::Game(crate::types::GameType::Directory),
        config
    )
    .with_title_info(title, None::<String>)
    .with_nfo()
    .with_duplicate_check()
    .with_custom_component("metadata", UploadComponent::Metadata(metadata))
}

/// Extension trait for UploadBuilder to add tracker-specific functionality
/// 
/// NOTE: This trait is optional as UploadBuilder automatically detects
/// and applies the enabled tracker's configuration. You only need to use
/// these methods if you want to override the automatic detection.
pub trait TrackerUploadExt {
    /// Configure for Seedpool upload (overrides automatic detection)
    fn for_seedpool(self, seedpool_config: &crate::types::SeedpoolConfig) -> Self;
    
    /// Configure for TorrentLeech upload (overrides automatic detection)
    fn for_torrentleech(self, tl_config: &crate::types::TorrentLeechConfig) -> Self;
}

impl TrackerUploadExt for UploadBuilder {
    fn for_seedpool(mut self, seedpool_config: &crate::types::SeedpoolConfig) -> Self {
        // Apply tracker-specific upload configuration
        self.upload_config.skip_duplicate_check = !seedpool_config.settings.enable_duplicate_check;
        self.upload_config.skip_mediainfo = !seedpool_config.settings.enable_mediainfo;
        self.upload_config.skip_nfo = !seedpool_config.settings.enable_nfo;
        self.upload_config.skip_screenshots = !seedpool_config.settings.enable_screenshots;
        self.upload_config.skip_sample = !seedpool_config.settings.enable_sample;
        self.upload_config.skip_tmdb = !seedpool_config.settings.enable_tmdb;
        self.upload_config.skip_torrent_creation = !seedpool_config.settings.enable_torrent_creation;
        self.upload_config.screenshot_count = seedpool_config.settings.screenshot_count;
        
        // Add announce URL for torrent creation if enabled
        if seedpool_config.settings.enable_torrent_creation {
            self.upload_config.announce_url = Some(seedpool_config.settings.announce_url.clone());
        }
        
        // Store tracker config as a component for later use
        let mut metadata = HashMap::new();
        metadata.insert("tracker".to_string(), "seedpool".to_string());
        metadata.insert("stripshit".to_string(), 
            seedpool_config.settings.stripshit_from_videos.to_string());
        metadata.insert("remote_path".to_string(), seedpool_config.screenshots.remote_path.clone());
        metadata.insert("image_path".to_string(), seedpool_config.screenshots.image_path.clone());
        
        self.with_custom_component("tracker_config", UploadComponent::Metadata(metadata))
    }
    
    fn for_torrentleech(mut self, tl_config: &crate::types::TorrentLeechConfig) -> Self {
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
        if tl_config.settings.enable_torrent_creation && !tl_config.general.announce_url_1.is_empty() {
            self.upload_config.announce_url = Some(tl_config.general.announce_url_1.clone());
        }
        
        // Store tracker config
        let mut metadata = HashMap::new();
        metadata.insert("tracker".to_string(), "torrentleech".to_string());
        metadata.insert("stripshit".to_string(), 
            tl_config.settings.stripshit_from_videos.to_string());
        
        self.with_custom_component("tracker_config", UploadComponent::Metadata(metadata))
    }
}

/// Process a cover image for ebooks/media
pub fn process_cover_image(
    builder: &mut UploadBuilder,
    cover_path: &str,
    upload_path: Option<&str>,
    dry_run: bool,
) -> Result<(), String> {
    if let Some(upload_path) = upload_path {
        if !dry_run {
            // Upload the cover
            upload_to_cdn(cover_path, upload_path)?;
        }
        
        builder.components.insert(
            "cover".to_string(),
            UploadComponent::CoverImage(cover_path.to_string())
        );
    }
    
    Ok(())
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
pub fn example_usage() {}

/// Result of an upload operation
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
    config: Arc<Config>,
    /// Dry run mode
    dry_run: bool,
    /// Media classification for mapping
    media_category: Option<String>,
    media_source_type: Option<String>,
    /// Field mapping override
    field_mapping: Option<TrackerFieldMapping>,
    /// Mapping engine override
    mapping_engine: Option<crate::tracker_mappings::TrackerMappingEngine>,
}

impl UploadProcessor {
    /// Create a new upload processor that auto-detects the active tracker
    pub fn new(
        upload_data: crate::media::video::UploadData,
        config: Arc<Config>,
    ) -> Self {
        Self {
            upload_data,
            config,
            dry_run: false,
            media_category: None,
            media_source_type: None,
            field_mapping: None,
            mapping_engine: None,
        }
    }
    
    /// Set dry run mode
    pub fn dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
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
    
    /// Override mapping engine
    pub fn with_mapping_engine(mut self, engine: crate::tracker_mappings::TrackerMappingEngine) -> Self {
        self.mapping_engine = Some(engine);
        self
    }
    
    /// Process the upload to the active tracker
    pub fn process(self) -> Result<UploadResult, String> {
        // Determine active tracker and get its configuration
        let (tracker_name, field_mapping, mapping_engine) = self.determine_active_tracker()?;
        
        info!("Processing upload for tracker: {}", tracker_name);
        
        // Use media classification if provided
        let media_category = self.media_category.as_deref();
        let media_type = self.media_source_type.as_deref();
        
        // Map internal categories/types to tracker-specific ones
        let (tracker_category, tracker_type) = self.map_to_tracker_categories(
            &tracker_name,
            &mapping_engine,
            media_category,
            media_type,
        )?;
        
        // Build the form data based on tracker field mappings
        let form_data = self.build_form_data(tracker_category, tracker_type, &field_mapping)?;
        
        // Validate required fields
        self.validate_required_fields(&form_data, &field_mapping)?;
        
        if self.dry_run {
            info!("DRY RUN: Would upload to {} with data:", tracker_name);
            for (key, value) in &form_data {
                info!("  {}: {}", key, value);
            }
            
            Ok(UploadResult {
                success: true,
                tracker: tracker_name,
                torrent_id: None,
                message: "Dry run completed successfully".to_string(),
            })
        } else {
            // Perform the actual upload
            self.perform_upload(&tracker_name, form_data)
        }
    }
    
    /// Determine which tracker is active and load its configuration
    fn determine_active_tracker(&self) -> Result<(String, TrackerFieldMapping, crate::tracker_mappings::TrackerMappingEngine), String> {
        // Use override mappings if provided
        if let (Some(ref fm), Some(ref me)) = (&self.field_mapping, &self.mapping_engine) {
            // Try to determine tracker from field mapping or default to seedpool
            let tracker_name = if fm.field_map.contains_key("descr") {
                "torrentleech"
            } else {
                "seedpool"
            };
            return Ok((tracker_name.to_string(), fm.clone(), me.clone()));
        }
        
        // Load tracker configs and determine which is active
        let seedpool_config = crate::utils::load_tracker_config::<crate::types::SeedpoolConfig>("seedpool")
            .map_err(|e| format!("Failed to load seedpool config: {}", e))?;
        let torrentleech_config = crate::utils::load_tracker_config::<crate::types::TorrentLeechConfig>("torrentleech")
            .map_err(|e| format!("Failed to load torrentleech config: {}", e))?;
        
        if seedpool_config.general.enabled {
            return Ok((
                "seedpool".to_string(),
                crate::definitions::seedpool::create_seedpool_field_mapping(),
                crate::definitions::seedpool::create_seedpool_mappings(),
            ));
        }
        
        if torrentleech_config.general.enabled {
            return Ok((
                "torrentleech".to_string(),
                crate::definitions::torrentleech::create_torrentleech_field_mapping(),
                crate::definitions::torrentleech::create_torrentleech_mappings(),
            ));
        }
        
        Err("No tracker is enabled in configuration".to_string())
    }
    
    
    /// Map internal categories/types to tracker-specific ones
    fn map_to_tracker_categories(
        &self,
        tracker_name: &str,
        engine: &crate::tracker_mappings::TrackerMappingEngine,
        category: Option<&str>,
        media_type: Option<&str>,
    ) -> Result<(u32, Option<u32>), String> {
        if let (Some(cat), Some(typ)) = (category, media_type) {
            // Try to find mapping with both category and type
            if let Some((cat_id, type_id)) = engine.find_mapping(cat, Some(typ)) {
                info!("Mapped {}:{} to category {} type {:?}", cat, typ, cat_id, type_id);
                return Ok((cat_id, type_id));
            }
        }
        
        if let Some(cat) = category {
            // Try category-only mapping
            if let Some((cat_id, type_id)) = engine.find_mapping(cat, None) {
                info!("Mapped {} to category {} type {:?}", cat, cat_id, type_id);
                return Ok((cat_id, type_id));
            }
        }
        
        // Use defaults if no mapping found
        let defaults = match tracker_name {
            "seedpool" => crate::definitions::seedpool::get_seedpool_defaults(),
            "torrentleech" => crate::definitions::torrentleech::get_torrentleech_defaults(),
            _ => (11, Some(17)), // Generic other/other
        };
        
        warn!("No mapping found for {:?}/{:?}, using defaults: {:?}", 
              category, media_type, defaults);
        Ok(defaults)
    }
    
    /// Build form data using field mappings
    fn build_form_data(
        &self,
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
                let field_name = field_mapping.get_field_name(internal_name)
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| internal_name.to_string());
                form_data.insert(field_name, value.to_string());
            }
        }
        
        // Category and type
        let cat_field = field_mapping.get_field_name("category")
            .map(|s| s.to_string())
            .unwrap_or_else(|| "category".to_string());
        form_data.insert(cat_field, category.to_string());
        
        if let Some(type_id) = type_id {
            let type_field = field_mapping.get_field_name("type")
                .map(|s| s.to_string())
                .unwrap_or_else(|| "type".to_string());
            form_data.insert(type_field, type_id.to_string());
        }
        
        // Screenshots (if any)
        if !self.upload_data.screenshots.is_empty() {
            let screenshot_field = field_mapping.get_field_name("screenshots")
                .map(|s| s.to_string())
                .unwrap_or_else(|| "screenshots".to_string());
            form_data.insert(
                screenshot_field,
                self.upload_data.screenshots.join("\n")
            );
        }
        
        // Torrent file path
        if let Some(torrent_path) = &self.upload_data.torrent_path {
            let torrent_field = field_mapping.get_field_name("torrent")
                .map(|s| s.to_string())
                .unwrap_or_else(|| "torrent".to_string());
            form_data.insert(torrent_field, torrent_path.clone());
        }
        
        // NFO file (if any)
        if let Some((nfo_path, _)) = &self.upload_data.nfo_data {
            let nfo_field = field_mapping.get_field_name("nfo")
                .map(|s| s.to_string())
                .unwrap_or_else(|| "nfo".to_string());
            form_data.insert(nfo_field, nfo_path.clone());
        }
        
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
    fn perform_upload(&self, tracker_name: &str, form_data: HashMap<String, String>) -> Result<UploadResult, String> {
        match tracker_name {
            "seedpool" => self.upload_to_seedpool(form_data),
            "torrentleech" => self.upload_to_torrentleech(form_data),
            _ => Err(format!("Unknown tracker: {}", tracker_name)),
        }
    }
    
    /// Upload to Seedpool
    fn upload_to_seedpool(&self, _form_data: HashMap<String, String>) -> Result<UploadResult, String> {
        // This will be implemented to call the actual Seedpool upload API
        // For now, return a placeholder
        warn!("Seedpool upload not yet implemented in UploadProcessor");
        Ok(UploadResult {
            success: false,
            tracker: "seedpool".to_string(),
            torrent_id: None,
            message: "Upload processor for Seedpool not yet implemented".to_string(),
        })
    }
    
    /// Upload to TorrentLeech
    fn upload_to_torrentleech(&self, _form_data: HashMap<String, String>) -> Result<UploadResult, String> {
        // This will be implemented to call the actual TorrentLeech upload API
        // For now, return a placeholder
        warn!("TorrentLeech upload not yet implemented in UploadProcessor");
        Ok(UploadResult {
            success: false,
            tracker: "torrentleech".to_string(),
            torrent_id: None,
            message: "Upload processor for TorrentLeech not yet implemented".to_string(),
        })
    }
}