use crate::core::types::{MediaFile, MediaType};
use log::info;

use super::{audio, ebook, game, hobby, video};

/// Comprehensive media type detection for any file or directory
pub fn detect_media_type(path: &str) -> Result<Vec<MediaFile>, String> {
    info!("detect_media_type: Starting detection for path: {}", path);
    let mut media_files = Vec::new();

    // Check if path exists, if not try to infer from filename
    if !std::path::Path::new(path).exists() {
        info!("Path doesn't exist, trying to infer media type from filename: {}", path);
        return detect_media_type_from_name(path);
    }

    // Try each media type detector in priority order
    // First, check for actual file extensions to avoid false positives from name patterns

    // Check for audio files first (they're often misidentified as video by name patterns)
    if let Ok(audio_files) = audio::detect_audio_files(path) {
        for audio_file in audio_files {
            media_files.push(audio::to_media_file(&audio_file));
        }
    }

    // Check for ebook files
    if let Ok(ebook_files) = ebook::detect_ebook_files(path) {
        for ebook_file in ebook_files {
            media_files.push(ebook::to_media_file(&ebook_file));
        }
    }

    // Check for video files (after audio to avoid music album misidentification)
    if let Ok(video_files) = video::detect_video_files(path) {
        for video_file in video_files {
            media_files.push(video::to_media_file(&video_file));
        }
    }

    // Check for game files
    if let Ok(game_files) = game::detect_game_files(path) {
        for game_file in game_files {
            media_files.push(game::to_media_file(&game_file));
        }
    }

    // Check for hobby files (lowest priority, catch-all)
    if media_files.is_empty() {
        if let Ok(hobby_files) = hobby::detect_hobby_files(path) {
            for hobby_file in hobby_files {
                media_files.push(hobby::to_media_file(&hobby_file));
            }
        }
    }

    Ok(media_files)
}

/// Detect media type from filename patterns when file doesn't exist
pub fn detect_media_type_from_name(name: &str) -> Result<Vec<MediaFile>, String> {
    use std::path::PathBuf;
    use crate::core::types::{VideoType, AudioType, MediaFile, MediaType};
    
    info!("Inferring media type from name: {}", name);
    
    let name_lower = name.to_lowercase();
    
    // Video patterns (most common for torrents)
    let video_patterns = [
        // Resolution indicators
        "1080p", "720p", "480p", "2160p", "4k",
        // Video codecs
        "x264", "x265", "h264", "h265", "xvid", "divx",
        // Video sources
        "bluray", "web-dl", "webrip", "dvdrip", "brrip", "hdtv", "cam", "ts",
        // Video containers (as extensions)
        ".mkv", ".mp4", ".avi", ".mov", ".wmv", ".flv", ".webm",
        // TV show patterns
        "s01", "s02", "s03", "season", "episode", "e01", "e02",
        // Movie year patterns (4 digits)
    ];
    
    let audio_patterns = [
        // Audio formats
        "flac", "mp3", "aac", "ogg", "wav", "m4a", "wma",
        // Audio quality indicators  
        "320kbps", "v0", "v2", "lossless",
        // Album indicators
        "album", "discography", "single", "ep", "ost", "soundtrack",
        // Audio containers
        ".flac", ".mp3", ".aac", ".ogg", ".wav", ".m4a", ".wma",
    ];
    
    // Check for video patterns
    for pattern in &video_patterns {
        if name_lower.contains(pattern) {
            info!("Detected video pattern '{}' in name", pattern);
            return Ok(vec![MediaFile {
                path: PathBuf::from(name),
                media_type: MediaType::Video(VideoType::Mkv), // Default to MKV for video
            }]);
        }
    }
    
    // Check for year patterns (common in movies)
    if let Some(_) = regex::Regex::new(r"\b(19|20)\d{2}\b")
        .unwrap()
        .find(&name_lower) 
    {
        if !name_lower.contains("season") && !name_lower.contains("episode") {
            info!("Detected year pattern in name, assuming movie");
            return Ok(vec![MediaFile {
                path: PathBuf::from(name),
                media_type: MediaType::Video(VideoType::Mkv),
            }]);
        }
    }
    
    // Check for audio patterns
    for pattern in &audio_patterns {
        if name_lower.contains(pattern) {
            info!("Detected audio pattern '{}' in name", pattern);
            // Only use FLAC if "flac" is explicitly in the name, otherwise default to MP3
            let audio_type = if name_lower.contains("flac") {
                AudioType::Flac
            } else {
                AudioType::Mp3
            };
            return Ok(vec![MediaFile {
                path: PathBuf::from(name),
                media_type: MediaType::Audio(audio_type),
            }]);
        }
    }
    
    // If no specific patterns found, assume video (most common for torrents)
    info!("No specific patterns found, defaulting to video type");
    Ok(vec![MediaFile {
        path: PathBuf::from(name),
        media_type: MediaType::Video(VideoType::Mkv),
    }])
}

/// Detect the primary media type for a path (returns the most significant type)
pub fn detect_primary_media_type(path: &str) -> Result<MediaType, String> {
    let media_files = detect_media_type(path)?;

    if media_files.is_empty() {
        return Err("No supported media files detected".to_string());
    }

    // Return the first detected type (priority order)
    Ok(media_files[0].media_type.clone())
}

/// Check if path contains mixed media types
pub fn is_mixed_media_collection(path: &str) -> Result<bool, String> {
    let media_files = detect_media_type(path)?;

    if media_files.len() < 2 {
        return Ok(false);
    }

    let first_category = media_files[0].media_type.category();
    Ok(media_files
        .iter()
        .any(|f| f.media_type.category() != first_category))
}

/// Get media statistics for a collection
pub fn get_media_statistics(path: &str) -> Result<MediaStatistics, String> {
    let media_files = detect_media_type(path)?;

    let mut stats = MediaStatistics::default();
    stats.total_files = media_files.len();

    for media_file in &media_files {
        match &media_file.media_type {
            MediaType::Video(_) => stats.video_count += 1,
            MediaType::Audio(_) => stats.audio_count += 1,
            MediaType::Ebook(_) => stats.ebook_count += 1,
            MediaType::Game(_) => stats.game_count += 1,
            MediaType::Hobby(_) => stats.hobby_count += 1,
        }
    }

    // Determine dominant type
    let counts = [
        ("video", stats.video_count),
        ("audio", stats.audio_count),
        ("ebook", stats.ebook_count),
        ("game", stats.game_count),
        ("hobby", stats.hobby_count),
    ];

    stats.dominant_type = counts
        .iter()
        .max_by_key(|(_, count)| *count)
        .map(|(name, _)| name.to_string())
        .unwrap_or_else(|| "unknown".to_string());

    Ok(stats)
}

#[derive(Debug, Default)]
pub struct MediaStatistics {
    pub total_files: usize,
    pub video_count: usize,
    pub audio_count: usize,
    pub ebook_count: usize,
    pub game_count: usize,
    pub hobby_count: usize,
    pub dominant_type: String,
}

impl MediaStatistics {
    pub fn is_pure_collection(&self) -> bool {
        let non_zero_counts = [
            self.video_count,
            self.audio_count,
            self.ebook_count,
            self.game_count,
            self.hobby_count,
        ]
        .iter()
        .filter(|&&count| count > 0)
        .count();

        non_zero_counts <= 1
    }
}
