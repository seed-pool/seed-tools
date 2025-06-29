use crate::core::types::{AudioFile, AudioType, MediaFile, MediaType, AudioCategory, AudioSourceType};
use std::path::Path;
use regex::Regex;
use log::{info, warn, debug};
use crate::processing::extraction::process_and_extract_archives;
use crate::processing::description::{DescriptionBuilder, DescriptionConfig};
use crate::core::DescriptionComponent;

/// Metadata extracted from audio filename and folder structure
#[derive(Debug, Clone)]
pub struct AudioMetadata {
    pub artist: Option<String>,
    pub album: Option<String>,
    pub title: Option<String>,
    pub year: Option<u32>,
    pub track_number: Option<String>,
    pub disc_number: Option<String>,
    pub category: AudioCategory,
    pub source_type: AudioSourceType,
    pub format: AudioType,
    pub is_lossless: bool,
    pub is_24bit: bool,
    pub sample_rate: Option<String>,
    pub is_various_artists: bool,
    pub label: Option<String>,
    pub catalog_number: Option<String>,
}

impl Default for AudioMetadata {
    fn default() -> Self {
        Self {
            artist: None,
            album: None,
            title: None,
            year: None,
            track_number: None,
            disc_number: None,
            category: AudioCategory::Unknown,
            source_type: AudioSourceType::Unknown,
            format: AudioType::Mp3,
            is_lossless: false,
            is_24bit: false,
            sample_rate: None,
            is_various_artists: false,
            label: None,
            catalog_number: None,
        }
    }
}

/// Process audio file(s) from a path (file or directory) and classify content
pub fn process_audio(
    input_path: &str,
    _config: &crate::core::Config,
    _dry_run: bool,
) -> Result<Vec<(AudioFile, AudioMetadata)>, String> {
    let path = Path::new(input_path);
    
    if !path.exists() {
        return Err(format!("Path not found: {}", input_path));
    }
    
    // Extract any archives first and get the path to process
    let processing_path = process_and_extract_archives(input_path).map_err(|e| format!("{:?}", e))?;
    
    let mut results = Vec::new();
    let mut rejected_files = Vec::new();
    
    // Update path to use the processing path
    let path = Path::new(&processing_path);
    
    if path.is_file() {
        // Single file case (non-archive audio file)
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .ok_or_else(|| "Could not determine file extension".to_string())?;

        let audio_type = AudioType::from_extension(extension)
            .ok_or_else(|| format!("Unsupported audio file type: {}", extension))?;
        
        let audio_file = AudioFile {
            path: path.to_path_buf(),
            audio_type: audio_type.clone(),
        };
        
        let metadata = classify_audio_content(path, &audio_type);
        
        if metadata.category == AudioCategory::Unknown {
            return Err(format!(
                "Unable to determine audio category for '{}'. File must have recognizable patterns like: Artist - Album, soundtrack, live, compilation, EP, single, etc.", 
                path.display()
            ));
        }
        
        results.push((audio_file, metadata));
        
    } else if path.is_dir() {
        // Handle directory - process all audio files (including extracted ones)
        for entry in std::fs::read_dir(path)
            .map_err(|e| format!("Failed to read directory: {}", e))? 
        {
            let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
            let file_path = entry.path();
            
            if file_path.is_file() {
                if let Some(extension) = file_path.extension().and_then(|ext| ext.to_str()) {
                    if let Some(audio_type) = AudioType::from_extension(extension) {
                        let audio_file = AudioFile {
                            path: file_path.clone(),
                            audio_type: audio_type.clone(),
                        };
                        
                        let metadata = classify_audio_content(&file_path, &audio_type);
                        
                        if metadata.category == AudioCategory::Unknown {
                            let filename = file_path.file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("");
                            rejected_files.push(filename.to_string());
                            warn!("Rejected audio file with unknown category: {}", filename);
                            continue;
                        }
                        
                        info!("Processed audio: {} -> Category: {:?}, Source: {:?}", 
                              file_path.display(), metadata.category, metadata.source_type);
                        
                        results.push((audio_file, metadata));
                    }
                }
            }
        }
        
        if results.is_empty() {
            if !rejected_files.is_empty() {
                return Err(format!(
                    "No valid audio files found. {} file(s) rejected due to unrecognizable naming patterns: {}",
                    rejected_files.len(),
                    rejected_files.join(", ")
                ));
            } else {
                return Err("No audio files found in directory".to_string());
            }
        }
        
        if !rejected_files.is_empty() {
            warn!("Processed {} valid audio files, rejected {} files with unknown categories", 
                  results.len(), rejected_files.len());
        }
    } else {
        return Err("Path is neither a file nor a directory".to_string());
    }
    
    // After we have the results, build the upload data if we have audio files
    if !results.is_empty() {
        use crate::processing::upload::UploadBuilder;
        use std::sync::Arc;
        
        let (_audio_file, metadata) = &results[0];
        
        // Build upload data directly using UploadBuilder
        use crate::core::ImageLayout;
        
        // Configure description for audio
        let mut desc_config = DescriptionConfig::default();
        desc_config.image_layout = ImageLayout::SingleColumn; // Audio typically uses single column
        desc_config.max_images = 2; // Cover art front/back
        
        // Create the upload builder with audio-specific components
        let mut builder = UploadBuilder::new(
            &processing_path,
            MediaType::Audio(metadata.format.clone()),
            Arc::new((*_config).clone())
        )
        .with_extensions(crate::core::types::AudioType::all_extensions())
        .with_description_config(desc_config)
        .dry_run(_dry_run);
        
        // Add title info (album and year)
        if let Some(album) = &metadata.album {
            builder = builder.with_title_info(
                album, 
                metadata.year.map(|y| y.to_string()).as_deref()
            );
        }
        
        // Add audio-specific metadata
        let mut audio_metadata = std::collections::HashMap::new();
        if let Some(artist) = &metadata.artist {
            audio_metadata.insert("artist".to_string(), artist.clone());
        }
        if let Some(album) = &metadata.album {
            audio_metadata.insert("album".to_string(), album.clone());
        }
        if let Some(label) = &metadata.label {
            audio_metadata.insert("label".to_string(), label.clone());
        }
        if let Some(catalog) = &metadata.catalog_number {
            audio_metadata.insert("catalog_number".to_string(), catalog.clone());
        }
        audio_metadata.insert("format".to_string(), format!("{:?}", metadata.format));
        audio_metadata.insert("category".to_string(), format!("{:?}", metadata.category));
        audio_metadata.insert("source".to_string(), format!("{:?}", metadata.source_type));
        audio_metadata.insert("lossless".to_string(), metadata.is_lossless.to_string());
        if metadata.is_24bit {
            audio_metadata.insert("24bit".to_string(), "true".to_string());
        }
        if let Some(sample_rate) = &metadata.sample_rate {
            audio_metadata.insert("sample_rate".to_string(), sample_rate.clone());
        }
        
        builder = builder
            .with_nfo()
            .with_mediainfo()
            .with_duplicate_check()
            .with_custom_component("audio_metadata", crate::core::UploadComponent::Metadata(audio_metadata));
        
        // Add cover art extraction for audio (if applicable)
        // Audio files often have embedded cover art that could be extracted
        
        let _upload_data = builder.build()?;
        
        info!("Built upload data for audio processing");
        
        // Create the upload processor - it will auto-detect the active tracker
        let mut processor = crate::processing::upload::UploadProcessor::new(
            _upload_data,
            std::sync::Arc::new(_config.clone()),
        )
        .dry_run(_dry_run);
        
        // Get media classification for mapping
        if !results.is_empty() {
            let (_, metadata) = &results[0];
            let category_str = format!("AudioCategory::{:?}", metadata.category);
            let source_str = Some(format!("AudioSourceType::{:?}", metadata.source_type));
            
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

/// Detect audio files in a path (without classification)
pub fn detect_audio_files(path: &str) -> Result<Vec<AudioFile>, String> {
    let mut audio_files = Vec::new();
    detect_audio_files_recursive(Path::new(path), &mut audio_files)?;
    Ok(audio_files)
}

/// Recursively search for audio files in a directory tree
fn detect_audio_files_recursive(path: &Path, audio_files: &mut Vec<AudioFile>) -> Result<(), String> {
    if path.is_file() {
        if let Some(extension) = path.extension().and_then(|ext| ext.to_str()) {
            if let Some(audio_type) = AudioType::from_extension(extension) {
                audio_files.push(AudioFile {
                    path: path.to_path_buf(),
                    audio_type,
                });
            }
        }
    } else if path.is_dir() {
        for entry in std::fs::read_dir(path)
            .map_err(|e| format!("Failed to read directory {:?}: {}", path, e))? 
        {
            let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
            let entry_path = entry.path();
            
            // Recursively process subdirectories and files
            detect_audio_files_recursive(&entry_path, audio_files)?;
        }
    }
    
    Ok(())
}


/// Convert AudioFile to MediaFile
pub fn to_media_file(audio_file: &AudioFile) -> MediaFile {
    MediaFile {
        path: audio_file.path.clone(),
        media_type: MediaType::Audio(audio_file.audio_type.clone()),
    }
}

/// Process audio files with enhanced categorization

/// Classify audio content based on filename and folder patterns
pub fn classify_audio_content(path: &Path, audio_type: &AudioType) -> AudioMetadata {
    let mut metadata = AudioMetadata::default();
    metadata.format = audio_type.clone();
    metadata.is_lossless = audio_type.is_lossless();
    
    // Get filename and parent directory names (up to 2 levels)
    let filename = path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    
    let parent_dir = path.parent()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .unwrap_or("");
    
    // Get grandparent directory for better context
    let grandparent_dir = path.parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .unwrap_or("");
    
    // Initialize regex patterns
    let year_regex = Regex::new(r"\b(19|20)\d{2}\b").unwrap();
    let track_regex = Regex::new(r"^(\d{1,2})[\s\-._]").unwrap();
    let _disc_regex = Regex::new(r"(?i)\b(?:CD|Disc)[\s_-]?(\d+)\b").unwrap();
    let catalog_regex = Regex::new(r"\[([A-Z]{2,}[\-\s]?\d{3,}[A-Z0-9\-]*)\]").unwrap();
    let bit_depth_regex = Regex::new(r"\b(16|24|32)[\s\-]?bit\b").unwrap();
    let sample_rate_regex = Regex::new(r"\b(\d{2,3})[\s\-]?khz\b").unwrap();
    
    // Common patterns for different categories
    let va_regex = Regex::new(r"(?i)^VA\b|^Various\s+Artists?\b|^V\.A\.\b").unwrap();
    let soundtrack_regex = Regex::new(r"(?i)\b(OST|soundtrack|score)\b").unwrap();
    let live_regex = Regex::new(r"(?i)\b(live\s+at|live\s+from|live\s+in|concert|unplugged|acoustic\s+live|bootleg)\b").unwrap();
    let bootleg_regex = Regex::new(r"(?i)\b(bootleg|unofficial|rare|unreleased|demo)\b").unwrap();
    let compilation_regex = Regex::new(r"(?i)\b(compilation|best\s+of|greatest\s+hits|anthology|collection|selected)\b").unwrap();
    let single_regex = Regex::new(r"(?i)\b(single|maxi[\s\-]?single|CDS|CDM)\b").unwrap();
    let ep_regex = Regex::new(r"(?i)\b(EP|E\.P\.|extended\s+play)\b").unwrap();
    let remix_regex = Regex::new(r"(?i)\b(remix|remixed|mixes|mixed\s+by)\b").unwrap();
    let classical_regex = Regex::new(r"(?i)\b(symphony|sonata|concerto|opus|mozart|beethoven|bach|chopin|orchestra|philharmonic)\b").unwrap();
    let podcast_regex = Regex::new(r"(?i)\b(podcast|episode|experience|#\d{3,})\b").unwrap();
    let audiobook_regex = Regex::new(r"(?i)\b(audiobook|audio\s+book|narrated|unabridged|abridged)\b").unwrap();
    
    // Source type patterns
    let cd_regex = Regex::new(r"(?i)\b(CD[\s\-]?RIP|FLAC[\s\-]?CD|CD[\s\-]?FLAC)\b").unwrap();
    let vinyl_regex = Regex::new(r#"(?i)\b(vinyl|LP|12"|7"|45RPM|33RPM)\b"#).unwrap();
    let web_regex = Regex::new(r"(?i)\b(WEB|iTunes|Amazon|Bandcamp|Beatport|Spotify)\b").unwrap();
    let fm_regex = Regex::new(r"(?i)\b(FM|Radio)\b").unwrap();
    let cassette_regex = Regex::new(r"(?i)\b(cassette|tape)\b").unwrap();
    let remaster_regex = Regex::new(r"(?i)\b(remaster|remastered|anniversary|deluxe)\b").unwrap();
    
    debug!("Classifying audio content for: {}", path.display());
    
    // Check for artist - album pattern (common for music folders)
    let artist_album_regex = Regex::new(r"^([^-]+?)\s*-\s*(.+?)(?:\s*\((\d{4})\))?$").unwrap();
    
    // Try to parse from parent directory first (common for album folders)
    if let Some(captures) = artist_album_regex.captures(parent_dir) {
        if let Some(artist) = captures.get(1) {
            metadata.artist = Some(artist.as_str().trim().to_string());
        }
        if let Some(album) = captures.get(2) {
            metadata.album = Some(album.as_str().trim().to_string());
        }
        if let Some(year) = captures.get(3) {
            metadata.year = year.as_str().parse::<u32>().ok();
        }
    } else if let Some(captures) = artist_album_regex.captures(grandparent_dir) {
        // Try grandparent directory if parent didn't match
        if let Some(artist) = captures.get(1) {
            metadata.artist = Some(artist.as_str().trim().to_string());
        }
        if let Some(album) = captures.get(2) {
            metadata.album = Some(album.as_str().trim().to_string());
        }
        if let Some(year) = captures.get(3) {
            metadata.year = year.as_str().parse::<u32>().ok();
        }
    }
    
    // Extract track number from filename
    if let Some(track_match) = track_regex.captures(filename) {
        if let Some(track_num) = track_match.get(1) {
            metadata.track_number = Some(track_num.as_str().to_string());
        }
    }
    
    // Extract year if not already found (but skip if it's a podcast to avoid episode numbers)
    let combined = format!("{} {} {}", grandparent_dir, parent_dir, filename);
    let is_podcast = podcast_regex.is_match(&combined);
    
    if metadata.year.is_none() && !is_podcast {
        if let Some(year_match) = year_regex.find(&combined) {
            metadata.year = year_match.as_str().parse::<u32>().ok();
        }
    }
    
    // Check for catalog number
    if let Some(catalog_match) = catalog_regex.captures(&combined) {
        if let Some(catalog) = catalog_match.get(1) {
            metadata.catalog_number = Some(catalog.as_str().to_string());
        }
    }
    
    // Check for bit depth
    if let Some(bit_match) = bit_depth_regex.captures(&combined) {
        if let Some(depth) = bit_match.get(1) {
            metadata.is_24bit = depth.as_str() == "24" || depth.as_str() == "32";
        }
    }
    
    // Check for sample rate
    if let Some(sample_match) = sample_rate_regex.captures(&combined) {
        if let Some(rate) = sample_match.get(1) {
            metadata.sample_rate = Some(format!("{}kHz", rate.as_str()));
        }
    }
    
    // Check for VA/Various Artists
    metadata.is_various_artists = va_regex.is_match(parent_dir) || 
                                   va_regex.is_match(filename) ||
                                   (metadata.artist.as_ref().map(|a| va_regex.is_match(a)).unwrap_or(false));
    
    // Determine category
    if podcast_regex.is_match(&combined) {
        metadata.category = AudioCategory::Podcast;
    } else if audiobook_regex.is_match(&combined) {
        metadata.category = AudioCategory::Audiobook;
    } else if classical_regex.is_match(&combined) {
        metadata.category = AudioCategory::Classical;
    } else if soundtrack_regex.is_match(&combined) {
        metadata.category = AudioCategory::Soundtrack;
    } else if bootleg_regex.is_match(&combined) {
        metadata.category = AudioCategory::Bootleg;
    } else if live_regex.is_match(&combined) {
        metadata.category = AudioCategory::Live;
    } else if single_regex.is_match(&combined) {
        metadata.category = AudioCategory::Single;
    } else if ep_regex.is_match(&combined) {
        metadata.category = AudioCategory::EP;
    } else if compilation_regex.is_match(&combined) || metadata.is_various_artists {
        metadata.category = AudioCategory::Compilation;
    } else if remix_regex.is_match(&combined) {
        metadata.category = AudioCategory::Remix;
    } else if metadata.album.is_some() || metadata.artist.is_some() {
        // If we have album or artist info, assume it's an album
        metadata.category = AudioCategory::Album;
    }
    
    // Determine source type
    if remaster_regex.is_match(&combined) {
        metadata.source_type = AudioSourceType::Remaster;
    } else if vinyl_regex.is_match(&combined) {
        metadata.source_type = AudioSourceType::Vinyl;
    } else if cd_regex.is_match(&combined) {
        metadata.source_type = AudioSourceType::CD;
    } else if web_regex.is_match(&combined) {
        metadata.source_type = AudioSourceType::Web;
    } else if fm_regex.is_match(&combined) {
        metadata.source_type = AudioSourceType::FM;
    } else if cassette_regex.is_match(&combined) {
        metadata.source_type = AudioSourceType::Cassette;
    } else if metadata.is_lossless {
        // Default lossless to CD if no other source specified
        metadata.source_type = AudioSourceType::CD;
    }
    
    debug!("Audio classification result: {:?}", metadata);
    metadata
}

/// Classify audio content for upload pipeline
pub fn classify_for_upload(input_path: &str, metadata: &serde_json::Value) -> Result<(Option<String>, Option<String>, serde_json::Value), String> {
    // Check if we already have classification in metadata
    if let Some(category_str) = metadata.get("category").and_then(|c| c.as_str()) {
        let category = match category_str {
            "Audiobook" => Some("AudioCategory::Audiobook".to_string()),
            "Podcast" => Some("AudioCategory::Podcast".to_string()),
            _ => Some("AudioCategory::Music".to_string()),
        };
        
        return Ok((category, None, metadata.clone()));
    }
    
    // Otherwise, detect and classify
    if let Ok(audio_files) = detect_audio_files(input_path) {
        if let Some(audio_file) = audio_files.first() {
            let audio_metadata = classify_audio_content(&audio_file.path, &audio_file.audio_type);
            
            let category = match audio_metadata.category {
                AudioCategory::Audiobook => Some("AudioCategory::Audiobook".to_string()),
                AudioCategory::Podcast => Some("AudioCategory::Podcast".to_string()),
                _ => Some("AudioCategory::Music".to_string()),
            };
            
            // Manually create JSON metadata with all expected fields
            let json_metadata = serde_json::json!({
                "artist": audio_metadata.artist,
                "album": audio_metadata.album,
                "title": audio_metadata.title,
                "year": audio_metadata.year,
                "track_number": audio_metadata.track_number,
                "disc_number": audio_metadata.disc_number,
                "category": format!("{:?}", audio_metadata.category),
                "source_type": format!("{:?}", audio_metadata.source_type),
                "format": format!("{:?}", audio_metadata.format),
                "is_lossless": audio_metadata.is_lossless,
                "is_24bit": audio_metadata.is_24bit,
                "sample_rate": audio_metadata.sample_rate,
                "is_various_artists": audio_metadata.is_various_artists,
                "label": audio_metadata.label,
                "catalog_number": audio_metadata.catalog_number,
                // Placeholder fields that can be populated by enrichment services
                "cover_images": serde_json::Value::Array(vec![]), // Empty array for cover art URLs
                "tracklist": serde_json::Value::Array(vec![]), // Empty array for track listing
                "description": serde_json::Value::Null, // Custom description text
                // MusicBrainz metadata (for future enrichment)
                "musicbrainz_artist_id": serde_json::Value::Null,
                "musicbrainz_album_id": serde_json::Value::Null,
                "musicbrainz_release_group_id": serde_json::Value::Null,
            });
            
            return Ok((category, None, json_metadata));
        }
    }
    
    // Default to music
    Ok((Some("AudioCategory::Music".to_string()), None, metadata.clone()))
}

/// Generate description for audio uploads
pub fn generate_description(
    input_path: &str,
    metadata: &serde_json::Value,
    mediainfo: Option<&str>,
) -> Result<String, String> {
    generate_description_with_enriched_metadata(input_path, metadata, mediainfo, None)
}

/// Generate description using template system
pub fn generate_description_with_template(
    metadata: &serde_json::Value,
    enriched_metadata: Option<&std::collections::HashMap<String, String>>,
    template_name: Option<&str>,
) -> Result<String, String> {
    use crate::templates::TemplateProcessor;
    
    let template_processor = TemplateProcessor::with_defaults()
        .map_err(|e| format!("Failed to initialize template processor: {}", e))?;
    
    let template_to_use = template_name.unwrap_or("default");
    
    if let Some(template) = template_processor.get_template("audio", template_to_use) {
        template_processor.apply_template(template, metadata, enriched_metadata)
    } else {
        // Fallback to traditional description generation
        generate_description_with_enriched_metadata("", metadata, None, enriched_metadata)
    }
}

/// Generate enhanced audio description with MusicBrainz metadata
pub fn generate_description_with_enriched_metadata(
    input_path: &str,
    base_metadata: &serde_json::Value,
    mediainfo: Option<&str>,
    musicbrainz_enrichment: Option<&std::collections::HashMap<String, String>>,
) -> Result<String, String> {
    use crate::processing::description::{DescriptionBuilder, DescriptionConfig};
    use crate::core::{MediaType, AudioType, ImageLayout, SectionFormat, DescriptionComponent};
    
    // Helper function to get value from either enriched metadata or base metadata
    let get_value = |key: &str| -> Option<&str> {
        musicbrainz_enrichment
            .and_then(|enriched| enriched.get(key))
            .map(|s| s.as_str())
            .or_else(|| base_metadata.get(key).and_then(|v| v.as_str()))
    };
    
    // Extract audio metadata from JSON
    let audio_metadata = extract_audio_metadata_from_json(base_metadata);
    
    // Configure description builder for audio
    let mut config = DescriptionConfig::default();
    config.image_layout = ImageLayout::SingleColumn;
    config.max_images = 2; // Front/back cover or album art
    
    let mut builder = DescriptionBuilder::with_config(
        MediaType::Audio(audio_metadata.format.clone()),
        config
    );
    
    // Add title (prefer MusicBrainz data)
    let title = if let Some(mb_title) = get_value("musicbrainz_title") {
        if let Some(mb_artist) = get_value("musicbrainz_artist") {
            format!("{} - {}", mb_artist, mb_title)
        } else {
            mb_title.to_string()
        }
    } else if let Some(album) = &audio_metadata.album {
        if audio_metadata.is_various_artists {
            format!("Various Artists - {}", album)
        } else if let Some(artist) = &audio_metadata.artist {
            format!("{} - {}", artist, album)
        } else {
            album.clone()
        }
    } else if let Some(title) = &audio_metadata.title {
        title.clone()
    } else {
        // Extract from filename as fallback
        std::path::Path::new(input_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Unknown Audio")
            .to_string()
    };
    
    builder = builder.title(&title);
    
    // Add artist if not already in title (prefer MusicBrainz data)
    let artist_to_use = get_value("musicbrainz_artist")
        .or_else(|| audio_metadata.artist.as_deref());
    
    if !audio_metadata.is_various_artists && audio_metadata.album.is_some() {
        if let Some(artist) = artist_to_use {
            builder = builder.author(artist);
        }
    }
    
    // Add album art/cover images if provided in metadata
    let cover_images: Vec<String> = base_metadata.get("cover_images")
        .and_then(|c| c.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).map(String::from).collect())
        .unwrap_or_default();
    
    if !cover_images.is_empty() {
        builder = builder.images(cover_images);
    }
    
    // Create metadata table with enriched data
    let mut metadata_rows = Vec::new();
    
    // Format and quality info
    metadata_rows.push(vec!["Format".to_string(), format!("{:?}", audio_metadata.format)]);
    metadata_rows.push(vec!["Category".to_string(), format!("{:?}", audio_metadata.category)]);
    
    if audio_metadata.is_lossless {
        metadata_rows.push(vec!["Quality".to_string(), "Lossless".to_string()]);
    }
    
    if audio_metadata.is_24bit {
        metadata_rows.push(vec!["Bit Depth".to_string(), "24-bit".to_string()]);
    }
    
    if let Some(sample_rate) = &audio_metadata.sample_rate {
        metadata_rows.push(vec!["Sample Rate".to_string(), sample_rate.clone()]);
    }
    
    // Release info (prefer MusicBrainz data)
    let year_to_use = if let Some(mb_year) = get_value("musicbrainz_year") {
        mb_year
    } else if let Some(year) = &audio_metadata.year {
        &year.to_string()
    } else {
        ""
    };
    if !year_to_use.is_empty() {
        metadata_rows.push(vec!["Year".to_string(), year_to_use.to_string()]);
    }
    
    // Label (prefer MusicBrainz data)
    let label_to_use = get_value("musicbrainz_labels")
        .or_else(|| audio_metadata.label.as_deref());
    if let Some(label) = label_to_use {
        metadata_rows.push(vec!["Label".to_string(), label.to_string()]);
    }
    
    // Catalog number (prefer MusicBrainz data)
    let catalog_to_use = get_value("musicbrainz_catalog_numbers")
        .or_else(|| audio_metadata.catalog_number.as_deref());
    if let Some(catalog) = catalog_to_use {
        metadata_rows.push(vec!["Catalog".to_string(), catalog.to_string()]);
    }
    
    // MusicBrainz specific data
    if let Some(mb_status) = get_value("musicbrainz_status") {
        metadata_rows.push(vec!["Status".to_string(), mb_status.to_string()]);
    }
    
    if let Some(mb_country) = get_value("musicbrainz_country") {
        metadata_rows.push(vec!["Country".to_string(), mb_country.to_string()]);
    }
    
    if let Some(mb_type) = get_value("musicbrainz_primary_type") {
        metadata_rows.push(vec!["Type".to_string(), mb_type.to_string()]);
    }
    
    if let Some(track_count) = get_value("musicbrainz_track_count") {
        metadata_rows.push(vec!["Tracks".to_string(), track_count.to_string()]);
    }
    
    if let Some(total_length) = get_value("musicbrainz_total_length") {
        metadata_rows.push(vec!["Length".to_string(), total_length.to_string()]);
    }
    
    metadata_rows.push(vec!["Source".to_string(), format!("{:?}", audio_metadata.source_type)]);
    
    // Add metadata table
    builder = builder.add_component(DescriptionComponent::Table { rows: metadata_rows });
    
    // Add tracklist if available (prefer MusicBrainz data)
    if let Some(mb_tracklist) = get_value("musicbrainz_tracklist") {
        if !mb_tracklist.is_empty() {
            builder = builder.custom_section("Tracklist", mb_tracklist, SectionFormat::Quoted);
        }
    } else if let Some(tracklist_array) = base_metadata.get("tracklist").and_then(|t| t.as_array()) {
        let tracklist_string = tracklist_array.iter()
            .filter_map(|t| t.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        if !tracklist_string.is_empty() {
            builder = builder.custom_section("Tracklist", &tracklist_string, SectionFormat::Quoted);
        }
    }
    
    // Add mediainfo if available
    if let Some(mi) = mediainfo {
        builder = builder.custom_section("MediaInfo", mi, SectionFormat::Quoted);
    }
    
    // Add any custom description
    if let Some(description) = base_metadata.get("description").and_then(|d| d.as_str()) {
        builder = builder.raw(description);
    }
    
    Ok(builder.build())
}

/// Helper function to extract AudioMetadata from JSON metadata
fn extract_audio_metadata_from_json(metadata: &serde_json::Value) -> AudioMetadata {
    AudioMetadata {
        artist: metadata.get("artist").and_then(|a| a.as_str()).map(String::from),
        album: metadata.get("album").and_then(|a| a.as_str()).map(String::from),
        title: metadata.get("title").and_then(|t| t.as_str()).map(String::from),
        year: metadata.get("year").and_then(|y| y.as_u64()).map(|y| y as u32),
        track_number: metadata.get("track_number").and_then(|t| t.as_str()).map(String::from),
        disc_number: metadata.get("disc_number").and_then(|d| d.as_str()).map(String::from),
        category: metadata.get("category").and_then(|c| c.as_str())
            .and_then(|c| match c {
                "Album" => Some(AudioCategory::Album),
                "Single" => Some(AudioCategory::Single),
                "EP" => Some(AudioCategory::EP),
                "Compilation" => Some(AudioCategory::Compilation),
                "Soundtrack" => Some(AudioCategory::Soundtrack),
                "Live" => Some(AudioCategory::Live),
                "Bootleg" => Some(AudioCategory::Bootleg),
                "Podcast" => Some(AudioCategory::Podcast),
                "Audiobook" => Some(AudioCategory::Audiobook),
                "Mix" => Some(AudioCategory::Mix),
                "Demo" => Some(AudioCategory::Demo),
                "Remix" => Some(AudioCategory::Remix),
                "Classical" => Some(AudioCategory::Classical),
                _ => Some(AudioCategory::Unknown),
            }).unwrap_or(AudioCategory::Unknown),
        source_type: metadata.get("source_type").and_then(|s| s.as_str())
            .and_then(|s| match s {
                "CD" => Some(AudioSourceType::CD),
                "Vinyl" => Some(AudioSourceType::Vinyl),
                "Web" => Some(AudioSourceType::Web),
                "FM" => Some(AudioSourceType::FM),
                "Cassette" => Some(AudioSourceType::Cassette),
                "Remaster" => Some(AudioSourceType::Remaster),
                _ => Some(AudioSourceType::Unknown),
            }).unwrap_or(AudioSourceType::Unknown),
        format: metadata.get("format").and_then(|f| f.as_str())
            .and_then(|f| match f.to_lowercase().as_str() {
                "mp3" => Some(AudioType::Mp3),
                "flac" => Some(AudioType::Flac),
                "wav" => Some(AudioType::Wav),
                "aac" => Some(AudioType::Aac),
                "ogg" => Some(AudioType::Ogg),
                "m4a" => Some(AudioType::M4a),
                "wma" => Some(AudioType::Wma),
                "aiff" => Some(AudioType::Aiff),
                "ape" => Some(AudioType::Ape),
                "opus" => Some(AudioType::Opus),
                _ => Some(AudioType::Mp3),
            }).unwrap_or(AudioType::Mp3),
        is_lossless: metadata.get("is_lossless").and_then(|l| l.as_bool()).unwrap_or(false),
        is_24bit: metadata.get("is_24bit").and_then(|b| b.as_bool()).unwrap_or(false),
        sample_rate: metadata.get("sample_rate").and_then(|s| s.as_str()).map(String::from),
        is_various_artists: metadata.get("is_various_artists").and_then(|v| v.as_bool()).unwrap_or(false),
        label: metadata.get("label").and_then(|l| l.as_str()).map(String::from),
        catalog_number: metadata.get("catalog_number").and_then(|c| c.as_str()).map(String::from),
    }
}