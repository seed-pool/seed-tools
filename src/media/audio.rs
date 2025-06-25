use crate::types::{AudioFile, AudioType, MediaFile, MediaType, AudioCategory, AudioSourceType};
use std::path::Path;
use regex::Regex;
use log::{info, warn, debug};
use crate::extraction::process_and_extract_archives;

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
    _config: &crate::types::Config,
    _dry_run: bool,
) -> Result<Vec<(AudioFile, AudioMetadata)>, String> {
    let path = Path::new(input_path);
    
    if !path.exists() {
        return Err(format!("Path not found: {}", input_path));
    }
    
    // Extract any archives first and get the path to process
    let processing_path = process_and_extract_archives(input_path)?;
    
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
        use crate::upload::UploadBuilder;
        use std::sync::Arc;
        
        let (_audio_file, metadata) = &results[0];
        
        // Build upload data directly using UploadBuilder
        use crate::description::DescriptionConfig;
        use crate::types::ImageLayout;
        
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
        .with_extensions(crate::types::AudioType::all_extensions())
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
            .with_custom_component("audio_metadata", crate::types::UploadComponent::Metadata(audio_metadata));
        
        // Add cover art extraction for audio (if applicable)
        // Audio files often have embedded cover art that could be extracted
        
        let _upload_data = builder.build()?;
        
        info!("Built upload data for audio processing");
        
        // Create the upload processor - it will auto-detect the active tracker
        let mut processor = crate::upload::UploadProcessor::new(
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
    let search_path = Path::new(path);
    
    if search_path.is_file() {
        let extension = search_path
            .extension()
            .and_then(|ext| ext.to_str())
            .ok_or_else(|| "Could not determine file extension".to_string())?;

        if let Some(audio_type) = AudioType::from_extension(extension) {
            audio_files.push(AudioFile {
                path: search_path.to_path_buf(),
                audio_type,
            });
        }
    } else if search_path.is_dir() {
        for entry in std::fs::read_dir(search_path)
            .map_err(|e| format!("Failed to read directory: {}", e))? 
        {
            let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
            let file_path = entry.path();
            
            if file_path.is_file() {
                if let Some(extension) = file_path.extension().and_then(|ext| ext.to_str()) {
                    if let Some(audio_type) = AudioType::from_extension(extension) {
                        audio_files.push(AudioFile {
                            path: file_path,
                            audio_type,
                        });
                    }
                }
            }
        }
    }
    
    Ok(audio_files)
}

/// Check if this is a lossless audio collection
pub fn is_lossless_collection(audio_files: &[AudioFile]) -> bool {
    !audio_files.is_empty() && audio_files.iter().all(|f| f.audio_type.is_lossless())
}

/// Convert AudioFile to MediaFile
pub fn to_media_file(audio_file: &AudioFile) -> MediaFile {
    MediaFile {
        path: audio_file.path.clone(),
        media_type: MediaType::Audio(audio_file.audio_type.clone()),
    }
}

/// Process audio files with enhanced categorization
pub fn process_audio_with_metadata(
    input_path: &str,
    config: &crate::types::Config,
    dry_run: bool,
) -> Result<(AudioFile, AudioMetadata), String> {
    let results = process_audio(input_path, config, dry_run)?;
    
    // For single file processing, return the first result
    if results.len() == 1 {
        Ok(results.into_iter().next().unwrap())
    } else if results.is_empty() {
        Err("No audio files found".to_string())
    } else {
        // Return the first valid audio file
        Ok(results.into_iter().next().unwrap())
    }
}

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
    let disc_regex = Regex::new(r"(?i)\b(?:CD|Disc)[\s_-]?(\d+)\b").unwrap();
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