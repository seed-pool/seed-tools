use crate::types::{MediaFile, MediaType};

use super::{audio, ebook, game, hobby, video};

/// Comprehensive media type detection for any file or directory
pub fn detect_media_type(path: &str) -> Result<Vec<MediaFile>, String> {
    let mut media_files = Vec::new();
    
    // Try each media type detector in priority order
    
    // Check for video files first (highest priority for torrenting)
    if let Ok(video_files) = video::detect_video_files(path) {
        for video_file in video_files {
            media_files.push(video::to_media_file(&video_file));
        }
    }
    
    // Check for ebook files
    if let Ok(ebook_files) = ebook::detect_ebook_files(path) {
        for ebook_file in ebook_files {
            media_files.push(ebook::to_media_file(&ebook_file));
        }
    }
    
    // Check for audio files
    if let Ok(audio_files) = audio::detect_audio_files(path) {
        for audio_file in audio_files {
            media_files.push(audio::to_media_file(&audio_file));
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
    Ok(media_files.iter().any(|f| f.media_type.category() != first_category))
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
        ].iter().filter(|&&count| count > 0).count();
        
        non_zero_counts <= 1
    }
}