use std::fs;
use std::path::Path;
use std::process::Command;
use log::{info, error, debug, warn};
use regex::Regex;

use crate::types::{PathsConfig, VideoSettings, VideoFile, VideoType, MediaFile, MediaType, VideoCategory, VideoSourceType};
use crate::naming::generate_release_name;
use crate::extraction::process_and_extract_archives;

/// Metadata extracted from video filename
#[derive(Debug, Clone)]
pub struct VideoMetadata {
    pub title: String,
    pub year: Option<u32>,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    pub category: VideoCategory,
    pub source_type: VideoSourceType,
    pub is_boxset: bool,
    pub is_dated_tv: bool,
    pub resolution: Option<String>,
    pub codec: Option<String>,
}

impl Default for VideoMetadata {
    fn default() -> Self {
        Self {
            title: String::new(),
            year: None,
            season: None,
            episode: None,
            category: VideoCategory::Unknown,
            source_type: VideoSourceType::Unknown,
            is_boxset: false,
            is_dated_tv: false,
            resolution: None,
            codec: None,
        }
    }
}

/// Processed data ready for upload (media-agnostic)
/// This struct accumulates all the data needed for the upload process
#[derive(Debug, Clone)]
pub struct UploadData {
    pub nfo_data: Option<(String, Vec<u8>)>,  // (path, content)
    pub mediainfo: Option<String>,
    pub screenshots: Vec<String>,
    pub thumbnails: Vec<String>,
    pub sample_url: Option<String>,
    pub torrent_path: Option<String>,
    pub release_name: Option<String>,
    pub description: Option<String>,
}

impl UploadData {
    pub fn new() -> Self {
        Self {
            nfo_data: None,
            mediainfo: None,
            screenshots: Vec::new(),
            thumbnails: Vec::new(),
            sample_url: None,
            torrent_path: None,
            release_name: None,
            description: None,
        }
    }
}

pub fn find_video_files<T>(
    input_path: &str,
    _paths: &PathsConfig,
    settings: &T,
) -> Result<(Vec<String>, Option<String>), String>
where
    T: VideoSettings,
{
    let supported_extensions = ["mkv", "mp4", "avi", "mov", "wmv", "flv", "webm", "m4v", "ts", "mpg", "mpeg"];
    let path = Path::new(input_path);

    let mut video_files = Vec::new();
    let mut nfo_file: Option<String> = None;

    let exclusions_enabled = settings.stripshit_from_videos();
    info!("Exclusions enabled: {}", exclusions_enabled);

    fn process_path(
        file_path: &Path,
        video_files: &mut Vec<String>,
        nfo_file: &mut Option<String>,
        supported_extensions: &[&str],
        exclusions_enabled: bool,
    ) -> Result<(), String> {
        if file_path.is_dir() {
            for entry in fs::read_dir(file_path).map_err(|e| format!("Failed to read directory: {}", e))? {
                let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
                let entry_path = entry.path();
                process_path(&entry_path, video_files, nfo_file, supported_extensions, exclusions_enabled)?;
            }
        } else {
            debug!("Processing file: {}", file_path.display());
            process_file(file_path, video_files, nfo_file, supported_extensions, exclusions_enabled)?;
        }
        Ok(())
    }

    process_path(path, &mut video_files, &mut nfo_file, &supported_extensions, exclusions_enabled)?;

    if video_files.is_empty() {
        error!("No valid video files detected after exclusions.");
        return Err("No valid video files detected.".to_string());
    }

    info!("Final NFO file: {:?}", nfo_file);

    Ok((video_files, nfo_file))
}


pub fn process_file(
    file_path: &Path,
    video_files: &mut Vec<String>,
    nfo_file: &mut Option<String>,
    _supported_extensions: &[&str], // Legacy parameter, now unused
    exclusions_enabled: bool,
) -> Result<(), String> {
    let file_name = file_path.file_name().unwrap_or_default().to_string_lossy().to_string();

    if let Some(ext) = file_path.extension() {
        let ext = ext.to_string_lossy().to_lowercase();
        if VideoType::from_extension(&ext).is_some() {
            video_files.push(file_path.to_string_lossy().to_string());
        } else if ext == "nfo" && nfo_file.is_none() {
            *nfo_file = Some(file_path.to_string_lossy().to_string());
        }
    } else if exclusions_enabled && contains_excluded_keywords(&file_name) {
        info!("Excluding file due to keywords: {}", file_name);
    }

    Ok(())
}

pub fn contains_excluded_keywords(name: &str) -> bool {
    let keywords = ["sample", "screens", "screenshots", "proof"];
    let lowercase_name = name.to_lowercase();
    let result = keywords.iter().any(|keyword| lowercase_name.contains(keyword));
    info!("Checking if '{}' contains excluded keywords: {}", name, result);
    result
}

pub fn generate_sample(
    video_file: &str,
    screenshots_dir: &str,
    remote_path: &str,
    image_path: &str,
    ffmpeg_path: &str,
    input_name: &str,
    dry_run: bool,
) -> Result<String, String> {
    let sanitized_input_name = generate_release_name(input_name);
    let sample_file = format!("{}/{}.sample.mkv", screenshots_dir, sanitized_input_name);

    // Generate the sample file
    let ffmpeg_command = format!(
        "{} -i \"{}\" -ss 00:05:00 -t 00:00:20 -map 0 -c copy \"{}\"",
        ffmpeg_path, video_file, sample_file
    );
    let output = Command::new("sh")
        .arg("-c")
        .arg(ffmpeg_command)
        .output()
        .map_err(|e| format!("Failed to execute ffmpeg: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "Failed to generate sample file. ffmpeg output: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    // Set permissions to 777
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&sample_file, fs::Permissions::from_mode(0o777))
            .map_err(|e| format!("Failed to set permissions for sample file '{}': {}", sample_file, e))?;
    }

    // Upload the sample file
    if !dry_run {
        crate::utils::upload_to_cdn(
            &sample_file,
            &format!("{}/previews/", remote_path.trim_end_matches('/'))
        )?;
        info!("Sample file uploaded to CDN.");
    } else {
        info!("[DRY RUN] Would upload sample to CDN: {} {}", &format!("{}/previews/", remote_path.trim_end_matches('/')), sanitized_input_name);
    }

    // Return the public-facing URL for the sample
    Ok(format!("{}/{}.sample.mkv", image_path, sanitized_input_name))
}

pub fn get_video_duration(video_file: &str, ffprobe_path: &str) -> Result<f64, String> {
    let ffprobe_output = Command::new(ffprobe_path)
        .args(&[
            "-v", "error",
            "-show_entries", "format=duration",
            "-of", "default=noprint_wrappers=1:nokey=1",
            video_file,
        ])
        .output()
        .map_err(|e| format!("Failed to run ffprobe: {}", e))?;

    if !ffprobe_output.status.success() {
        return Err(format!(
            "ffprobe failed with status: {}. Stderr: {}",
            ffprobe_output.status,
            String::from_utf8_lossy(&ffprobe_output.stderr)
        ));
    }

    let duration_str = String::from_utf8_lossy(&ffprobe_output.stdout).trim().to_string();
    duration_str.parse::<f64>().map_err(|_| "Failed to parse video duration.".to_string())
}

pub fn default_non_video_description() -> String {
    format!(
        "[b][size=12][color=#757575]Created with mkbrr, ffmpeg, and mediainfo. Posted to this fine tracker with seedbrr.[/color][/size][/b]
        
        [url=https://github.com/seed-pool/seed-tools][img]https://cdn.seedpool.org/sp.png[/img][/url]  \
        [url=https://github.com/autobrr/mkbrr][img]https://cdn.seedpool.org/mkbrr.png[/img][/url]  \
        [url=https://www.rust-lang.org][img]https://cdn.seedpool.org/rust.png[/img][/url]"
    )
}

pub fn generate_description(
    screenshots: &[String],
    _thumbnails: &[String],
    sample_url: &str,
    _datestamp: &str,
    custom_description: Option<&str>,
    youtube_trailer_url: Option<&str>,
    _base_url: &str,
    _release_name: &str,
) -> String {
    use crate::description::{DescriptionBuilder, DescriptionConfig};
    use crate::types::ImageLayout;
    
    // Create config for video screenshots
    let mut config = DescriptionConfig::default();
    config.image_layout = ImageLayout::Grid2x2;
    
    let mut builder = DescriptionBuilder::with_config(
        crate::types::MediaType::Video(crate::types::VideoType::Mkv),
        config
    );
    
    // Add screenshots
    if !screenshots.is_empty() {
        builder = builder.images(screenshots.to_vec());
    }
    
    // Add sample
    if !sample_url.is_empty() {
        if let Some(filename) = Path::new(sample_url).file_name().and_then(|f| f.to_str()) {
            builder = builder.sample(sample_url, filename);
        }
    }
    
    // Add trailer
    if let Some(trailer_url) = youtube_trailer_url {
        builder = builder.trailer(trailer_url, "YouTube");
    }
    
    // Add custom description
    if let Some(custom_desc) = custom_description {
        builder = builder.raw(custom_desc);
    }
    
    builder.build()
}



/// Recursively process a directory for video files
fn process_directory_recursive(
    dir: &Path,
    results: &mut Vec<(VideoFile, VideoMetadata)>,
    rejected_files: &mut Vec<String>,
) -> Result<(), String> {
    for entry in fs::read_dir(dir)
        .map_err(|e| format!("Failed to read directory {:?}: {}", dir, e))? 
    {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
        let entry_path = entry.path();
        
        if entry_path.is_dir() {
            // Recursively process subdirectories
            process_directory_recursive(&entry_path, results, rejected_files)?;
        } else if entry_path.is_file() {
            if let Some(extension) = entry_path.extension().and_then(|ext| ext.to_str()) {
                if let Some(video_type) = VideoType::from_extension(extension) {
                    let video_file = VideoFile {
                        path: entry_path.clone(),
                        video_type,
                    };
                    
                    let filename = entry_path.file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("");
                    
                    // Pass the full path for classification
                    let metadata = classify_video_content(entry_path.to_str().unwrap_or(filename));
                    
                    if metadata.category == VideoCategory::Unknown {
                        rejected_files.push(filename.to_string());
                        warn!("Rejected video file with unknown category: {}", filename);
                        continue;
                    }
                    
                    info!("Processed video: {} -> Category: {:?}, Source: {:?}", 
                          filename, metadata.category, metadata.source_type);
                    
                    results.push((video_file, metadata));
                }
            }
        }
    }
    
    Ok(())
}

/// Process video file(s) from a path (file or directory) and classify content
pub fn process_video(
    input_path: &str,
    _config: &crate::types::Config,
    _dry_run: bool,
) -> Result<Vec<(VideoFile, VideoMetadata)>, String> {
        
    let path = Path::new(input_path);
    
    if !path.exists() {
        return Err(format!("Path not found: {}", input_path));
    }
    
    // Extract any archives first and get the path to process
    let processing_path = process_and_extract_archives(input_path)?;

    // Now process the path (which may contain extracted files)
    let mut results = Vec::new();
    let mut rejected_files = Vec::new();
    
    // Update path to use the processing path
    let path = Path::new(&processing_path);
    
    if path.is_file() {
        // Single file case (non-archive video file)
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .ok_or_else(|| "Could not determine file extension".to_string())?;

        let video_type = VideoType::from_extension(extension)
            .ok_or_else(|| format!("Unsupported video file type: {}", extension))?;
        
        let video_file = VideoFile {
            path: path.to_path_buf(),
            video_type,
        };
        
        let filename = path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        
        // Pass the full path for classification
        let metadata = classify_video_content(path.to_str().unwrap_or(filename));
        
        if metadata.category == VideoCategory::Unknown {
            return Err(format!(
                "Unable to determine video category for '{}'. File must have recognizable TV show (S##E##), movie (year), anime, sports, documentary, or concert patterns in the filename.", 
                filename
            ));
        }
        
        results.push((video_file, metadata));
        
    } else if path.is_dir() {
        // Handle directory - recursively process all video files
        process_directory_recursive(path, &mut results, &mut rejected_files)?;
        
        if results.is_empty() {
            if !rejected_files.is_empty() {
                return Err(format!(
                    "No valid video files found. {} file(s) rejected due to unrecognizable naming patterns: {}",
                    rejected_files.len(),
                    rejected_files.join(", ")
                ));
            } else {
                return Err("No video files found in directory".to_string());
            }
        }
        
        if !rejected_files.is_empty() {
            warn!("Processed {} valid video files, rejected {} files with unknown categories", 
                  results.len(), rejected_files.len());
        }
    } else {
        return Err("Path is neither a file nor a directory".to_string());
    }
    
    // After we have the results, build the upload data if we have videos
    if !results.is_empty() {
        use crate::upload::UploadBuilder;
        use std::sync::Arc;
        
        let (video_file, metadata) = &results[0];
        
        // Build upload data directly using UploadBuilder
        use crate::description::DescriptionConfig;
        use crate::types::ImageLayout;
        
        // Configure description for video
        let mut desc_config = DescriptionConfig::default();
        desc_config.image_layout = ImageLayout::Grid2x2; // Videos use 2x2 grid for screenshots
        desc_config.max_images = 8;
        
        let _upload_data = UploadBuilder::new(
            &processing_path,
            MediaType::Video(video_file.video_type.clone()),
            Arc::new((*_config).clone())
        )
        .with_extensions(VideoType::all_extensions())
        .with_video_metadata(metadata.clone())
        .with_description_config(desc_config)
        .with_nfo()
        .with_mediainfo()
        .with_screenshots(4)
        .with_sample()
        .with_duplicate_check()
        .with_tmdb_lookup()
        .dry_run(_dry_run)
        .build()?;
        
        info!("Built upload data for video processing");
        
        // Create the upload processor - it will auto-detect the active tracker
        let mut processor = crate::upload::UploadProcessor::new(
            _upload_data,
            std::sync::Arc::new(_config.clone()),
        )
        .dry_run(_dry_run);
        
        // Get media classification for mapping
        if !results.is_empty() {
            let (_, metadata) = &results[0];
            let category_str = format!("VideoCategory::{:?}", metadata.category);
            let source_str = Some(format!("VideoSourceType::{:?}", metadata.source_type));
            
            processor = processor.with_media_classification(
                Some(category_str),
                source_str,
            );
        }
        
        // Process the upload - it handles tracker detection and mapping internally
        let upload_result = processor.process()?;
        
        if upload_result.success {
            info!("Upload completed successfully to {}", upload_result.tracker);
            if let Some(torrent_id) = upload_result.torrent_id {
                info!("Torrent ID: {}", torrent_id);
            }
        } else {
            warn!("Upload failed: {}", upload_result.message);
        }
    }
    
    Ok(results)
}

/// Detect video files in a path (without classification)
pub fn detect_video_files(path: &str) -> Result<Vec<VideoFile>, String> {
    let mut video_files = Vec::new();
    detect_video_files_recursive(Path::new(path), &mut video_files)?;
    Ok(video_files)
}

/// Recursively search for video files in a directory tree
fn detect_video_files_recursive(path: &Path, video_files: &mut Vec<VideoFile>) -> Result<(), String> {
    if path.is_file() {
        if let Some(extension) = path.extension().and_then(|ext| ext.to_str()) {
            if let Some(video_type) = VideoType::from_extension(extension) {
                video_files.push(VideoFile {
                    path: path.to_path_buf(),
                    video_type,
                });
            }
        }
    } else if path.is_dir() {
        for entry in fs::read_dir(path)
            .map_err(|e| format!("Failed to read directory {:?}: {}", path, e))? 
        {
            let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
            let entry_path = entry.path();
            
            // Recursively process subdirectories and files
            detect_video_files_recursive(&entry_path, video_files)?;
        }
    }
    
    Ok(())
}

/// Convert VideoFile to MediaFile
pub fn to_media_file(video_file: &VideoFile) -> MediaFile {
    MediaFile {
        path: video_file.path.clone(),
        media_type: MediaType::Video(video_file.video_type.clone()),
    }
}

/// Classify video content based on filename patterns
/// Enhanced version of determine_release_type_and_title from seedpool.rs
pub fn classify_video_content(path: &str) -> VideoMetadata {
    let mut metadata = VideoMetadata::default();
    
    // Determine what to classify - prioritize directory name for directories
    let path_obj = Path::new(path);
    let filename_str = if path_obj.is_dir() {
        // Use directory name for classification
        path_obj.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    } else {
        // Use filename for single files
        path_obj.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    };
    let filename = filename_str.as_str();
    
    // Initialize regex patterns (enhanced from seedpool.rs)
    let season_episode_regex = Regex::new(r"(?i)S(\d{1,2})E(\d{1,3})").unwrap();
    let season_only_regex = Regex::new(r"(?i)S(\d{1,2})").unwrap();
    let episode_only_regex = Regex::new(r"(?i)\bE(\d{1,4})\b").unwrap();  // Support E1-E9999 for anime
    let boxset_regex = Regex::new(r"(?i)\b(boxset|complete|collection|season\s*\d+.*complete)\b").unwrap();
    let year_regex = Regex::new(r"\b(19|20)\d{2}\b").unwrap();
    let full_date_regex = Regex::new(r"\b((19|20)\d{2})[.\-](0[1-9]|1[0-2])[.\-](0[1-9]|[12][0-9]|3[01])\b").unwrap();
    
    // Enhanced pattern matching for anime, sports, documentaries
    // Common anime titles and keywords
    let anime_regex = Regex::new(r"(?i)\b(anime|dubbed|subbed|jpn|japanese|[Ss]ub|[Dd]ub|naruto|one\.piece|attack\.on\.titan|bleach|dragon\.ball|demon\.slayer|jujutsu\.kaisen|my\.hero\.academia|boku\.no\.hero|death\.note|hunter\.x\.hunter|fullmetal\.alchemist|sword\.art\.online|tokyo\.ghoul|steins\.gate|evangelion|cowboy\.bebop|one\.punch\.man|mob\.psycho|chainsaw\.man|spy\.x\.family|vinland\.saga|haikyuu|fairy\.tail|black\.clover|boruto|shippuden|kimetsu\.no\.yaiba)\b").unwrap();
    
    // Sports patterns - more specific to avoid false positives
    let sports_regex = Regex::new(r"(?i)\b(nba|nfl|nhl|mlb|uefa|fifa|premier\.league|bundesliga|la\.liga|serie\.a|ligue\.1|championship|tournament|vs\.|boxing|mma|ufc|wwe|aew|f1|formula\.1|formula\.one|olympics?|world\.cup|super\.bowl|wrestlemania|summerslam|grand\.prix|tennis|wimbledon|golf|pga|cricket|rugby)\b").unwrap();
    
    let documentary_regex = Regex::new(r"(?i)\b(documentary|docu|national\.geographic|discovery|history|nature|wildlife|science|biography|bio)\b").unwrap();
    let concert_regex = Regex::new(r"(?i)\b(concert|live\.at|tour|festival|acoustic|unplugged|live\.from)\b").unwrap();
    
    // Source type patterns - order matters for proper detection
    let uhd_bluray_regex = Regex::new(r"(?i)\b(uhd\.?blu.?ray|4k\.?blu.?ray)\b").unwrap();
    let bluray_regex = Regex::new(r"(?i)\b(blu.?ray|bd|m2ts)\b").unwrap();
    let dvd_regex = Regex::new(r"(?i)\b(dvd|dvdrip)\b").unwrap();
    let remux_regex = Regex::new(r"(?i)\b(remux)\b").unwrap();
    let full_disc_regex = Regex::new(r"(?i)\b(full\.?disc|complete\.?disc|bdmv|disc\.?image)\b").unwrap();
    let iso_regex = Regex::new(r"(?i)\.iso$").unwrap();
    let web_dl_regex = Regex::new(r"(?i)\b(web[\.\-]?dl|webdl|amzn|nf|hmax|dsnp|atvp|hulu|pcok|pmtp)\b").unwrap();
    let web_rip_regex = Regex::new(r"(?i)\b(web[\.\-]?rip|webrip)\b").unwrap();
    let hdtv_regex = Regex::new(r"(?i)\b(hdtv)\b").unwrap();
    let pdtv_regex = Regex::new(r"(?i)\b(pdtv)\b").unwrap();
    let sdtv_regex = Regex::new(r"(?i)\b(sdtv)\b").unwrap();
    let encode_regex = Regex::new(r"(?i)\b(encode|x264|x265|h264|h265|hevc|xvid|divx)\b").unwrap();
    let upscale_regex = Regex::new(r"(?i)\b(upscale|upscaled|ai.?upscale)\b").unwrap();
    
    // Resolution patterns
    let resolution_regex = Regex::new(r"\b(2160p|1080p|720p|480p|360p|4K|UHD)\b").unwrap();
    
    // Codec patterns
    let codec_regex = Regex::new(r"\b(x264|x265|h264|h265|hevc|avc|xvid|divx|av1)\b").unwrap();
    
    debug!("Classifying video content for: {}", filename);
    
    // 1. First check for TV show patterns (S##E## takes priority)
    if let Some(captures) = season_episode_regex.captures(filename) {
        debug!("Matched SxxEyy pattern: {:?}", captures);
        metadata.category = VideoCategory::TvShow;
        metadata.season = captures.get(1).and_then(|m| m.as_str().parse::<u32>().ok());
        metadata.episode = captures.get(2).and_then(|m| m.as_str().parse::<u32>().ok());
        metadata.title = extract_title_before_pattern(filename, &season_episode_regex);
    } else if let Some(captures) = episode_only_regex.captures(filename) {
        debug!("Matched Eyy pattern: {:?}", captures);
        metadata.category = VideoCategory::TvShow;
        metadata.season = Some(1);
        metadata.episode = captures.get(1).and_then(|m| m.as_str().parse::<u32>().ok());
        metadata.title = extract_title_before_pattern(filename, &episode_only_regex);
    } else if let Some(captures) = season_only_regex.captures(filename) {
        debug!("Matched Sxx pattern: {:?}", captures);
        metadata.category = VideoCategory::TvShow;
        metadata.season = captures.get(1).and_then(|m| m.as_str().parse::<u32>().ok());
        metadata.episode = Some(0);  // Season pack has no specific episode
        metadata.is_boxset = true;   // Season-only pattern indicates a boxset/season pack
        metadata.title = extract_title_before_pattern(filename, &season_only_regex);
    } else if boxset_regex.is_match(filename) {
        debug!("Matched boxset keywords in filename: {}", filename);
        metadata.category = VideoCategory::TvShow;
        metadata.is_boxset = true;
        metadata.season = Some(1);
        metadata.episode = Some(0);
        metadata.title = extract_title_before_pattern(filename, &boxset_regex);
    } else if let Some(date_caps) = full_date_regex.captures(filename) {
        debug!("Matched full date pattern in filename: {}", filename);
        metadata.category = VideoCategory::TvShow;
        metadata.is_dated_tv = true;
        // Use the full year (group 1)
        if let Some(year_str) = date_caps.get(1).map(|m| m.as_str()) {
            if let Ok(year) = year_str.parse::<u32>() {
                metadata.year = Some(year);
                metadata.season = Some(year);
                metadata.episode = Some(0);
            }
        }
        metadata.title = extract_title_before_pattern(filename, &full_date_regex);
    } else if year_regex.is_match(filename) {
        debug!("Matched year pattern in filename: {}", filename);
        metadata.category = VideoCategory::Movie;
        
        // Find all year matches and pick the most likely release year
        let year_matches: Vec<u32> = year_regex.find_iter(filename)
            .filter_map(|m| m.as_str().parse::<u32>().ok())
            .collect();
        
        if !year_matches.is_empty() {
            // Prefer years between 1960 and current year + 1
            let current_year = 2024; // Or use chrono to get actual current year
            let valid_year = year_matches.iter()
                .find(|&&y| y >= 1960 && y <= current_year + 1)
                .or_else(|| year_matches.first());
            
            if let Some(&year) = valid_year {
                metadata.year = Some(year);
            }
        }
        
        metadata.title = extract_title_before_pattern(filename, &year_regex);
    } else {
        // No clear pattern, extract full title
        metadata.title = clean_title(filename);
        
        // For ISO files without clear patterns, check for movie-like titles
        if iso_regex.is_match(filename) && (
            filename.to_lowercase().contains("trilogy") ||
            filename.to_lowercase().contains("collection") ||
            filename.to_lowercase().contains("saga") ||
            bluray_regex.is_match(filename) ||
            dvd_regex.is_match(filename)
        ) {
            metadata.category = VideoCategory::Movie;
        }
    }
    
    // 2. Refine category based on content-specific patterns
    // Check anime first as it often has episode patterns
    if anime_regex.is_match(filename) {
        debug!("Detected anime patterns in filename");
        metadata.category = VideoCategory::Anime;
    } else if documentary_regex.is_match(filename) {
        debug!("Detected documentary patterns in filename");
        metadata.category = VideoCategory::Documentary;
    } else if concert_regex.is_match(filename) {
        debug!("Detected concert patterns in filename");
        metadata.category = VideoCategory::Concert;
    } else if sports_regex.is_match(filename) {
        debug!("Detected sports patterns in filename");
        metadata.category = VideoCategory::Sports;
    }
    
    // 3. Determine source type (priority order matters)
    // First check if this is a boxset/season pack
    if metadata.is_boxset {
        metadata.source_type = VideoSourceType::SeasonPack;
    } else if iso_regex.is_match(filename) || full_disc_regex.is_match(filename) {
        metadata.source_type = VideoSourceType::FullDisc;
    } else if uhd_bluray_regex.is_match(filename) {
        metadata.source_type = VideoSourceType::UHDBluRay;
    } else if bluray_regex.is_match(filename) {
        metadata.source_type = VideoSourceType::BluRay;
    } else if dvd_regex.is_match(filename) {
        metadata.source_type = VideoSourceType::DVD;
    } else if remux_regex.is_match(filename) {
        metadata.source_type = VideoSourceType::Remux;
    } else if web_dl_regex.is_match(filename) {
        metadata.source_type = VideoSourceType::WebDL;
    } else if web_rip_regex.is_match(filename) {
        metadata.source_type = VideoSourceType::WebRip;
    } else if hdtv_regex.is_match(filename) {
        metadata.source_type = VideoSourceType::HDTV;
    } else if pdtv_regex.is_match(filename) {
        metadata.source_type = VideoSourceType::PDTV;
    } else if sdtv_regex.is_match(filename) {
        metadata.source_type = VideoSourceType::SDTV;
    } else if upscale_regex.is_match(filename) {
        metadata.source_type = VideoSourceType::Upscale;
    } else if encode_regex.is_match(filename) {
        // Only set to Encode if no other source type was detected
        metadata.source_type = VideoSourceType::Encode;
    } else {
        // Default to Unknown if nothing matches
        metadata.source_type = VideoSourceType::Unknown;
    }
    
    // 4. Extract resolution
    if let Some(res_match) = resolution_regex.find(filename) {
        metadata.resolution = Some(res_match.as_str().to_uppercase());
    }
    
    // 5. Extract codec
    if let Some(codec_match) = codec_regex.find(filename) {
        metadata.codec = Some(codec_match.as_str().to_uppercase());
    }
    
    // Additional check for directories - see if it's a season pack
    if path_obj.is_dir() && metadata.category == VideoCategory::TvShow {
        // Check if directory contains multiple episodes from same season
        if let Ok(entries) = fs::read_dir(path_obj) {
            let mut seasons = std::collections::HashSet::new();
            let mut episodes = std::collections::HashSet::new();
            let mut video_count = 0;
            
            for entry in entries.flatten().take(20) { // Check first 20 files
                if let Some(ext) = entry.path().extension().and_then(|e| e.to_str()) {
                    if VideoType::from_extension(ext).is_some() {
                        video_count += 1;
                        let file_path = entry.path();
                        if let Some(file_path_str) = file_path.to_str() {
                            let file_metadata = classify_video_content(file_path_str);
                            if let Some(season) = file_metadata.season {
                                seasons.insert(season);
                            }
                            if let Some(episode) = file_metadata.episode {
                                if episode > 0 {
                                    episodes.insert(episode);
                                }
                            }
                        }
                    }
                }
            }
            
            // If we have multiple episodes from the same season, it's a boxset
            if seasons.len() == 1 && episodes.len() > 1 && video_count > 1 {
                metadata.is_boxset = true;
                if metadata.episode.is_none() || metadata.episode == Some(0) {
                    // Keep episode as 0 for season packs
                    metadata.episode = Some(0);
                }
            }
        }
    }
    
    debug!("Video classification result: {:?}", metadata);
    metadata
}

/// Extract title before a regex pattern match
fn extract_title_before_pattern(filename: &str, pattern: &Regex) -> String {
    if let Some(pattern_match) = pattern.find(filename) {
        clean_title(&filename[..pattern_match.start()])
    } else {
        clean_title(filename)
    }
}

/// Clean up title by only replacing separators, preserving technical info
fn clean_title(title: &str) -> String {
    let cleaned = title
        .trim()
        .replace('.', " ")
        .replace('_', " ")
        .replace('-', " ");
    
    // Only clean up extra whitespace, preserve all technical indicators
    let whitespace_regex = Regex::new(r"\s+").unwrap();
    whitespace_regex.replace_all(&cleaned, " ").trim().to_string()
}

/// Classify video content for upload pipeline
pub fn classify_for_upload(input_path: &str, metadata: &serde_json::Value) -> Result<(Option<String>, Option<String>, serde_json::Value), String> {
    // If we already have classification data in metadata, use it
    if metadata.get("category").is_some() {
        let category = metadata.get("category")
            .and_then(|c| c.as_str())
            .map(|c| format!("VideoCategory::{}", c.replace("VideoCategory::", "")));
            
        let source_type = metadata.get("source_type")
            .and_then(|s| s.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| format!("VideoSourceType::{}", s.replace("VideoSourceType::", "")));
        
        return Ok((category, source_type, metadata.clone()));
    }
    
    // Otherwise, run classification
    let video_metadata = classify_video_content(input_path);
    
    let category = Some(format!("VideoCategory::{:?}", video_metadata.category));
    let source_type = Some(format!("VideoSourceType::{:?}", video_metadata.source_type));
    
    // Manually create JSON metadata
    let mut json_metadata = serde_json::json!({
        "title": video_metadata.title,
        "year": video_metadata.year,
        "season": video_metadata.season,
        "episode": video_metadata.episode,
        "category": format!("{:?}", video_metadata.category),
        "source_type": format!("{:?}", video_metadata.source_type),
        "is_boxset": video_metadata.is_boxset,
        "is_dated_tv": video_metadata.is_dated_tv,
        "resolution": video_metadata.resolution,
        "codec": video_metadata.codec,
    });
    
    // Merge with existing metadata
    if let (Some(json_obj), Some(existing_obj)) = (json_metadata.as_object_mut(), metadata.as_object()) {
        for (key, value) in existing_obj {
            json_obj.entry(key.clone()).or_insert(value.clone());
        }
    }
    
    Ok((category, source_type, json_metadata))
}
