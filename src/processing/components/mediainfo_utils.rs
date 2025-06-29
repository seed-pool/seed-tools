// MediaInfo utilities

use std::process::Command;
use rand::seq::SliceRandom;
use log::info;
use crate::core::{Config, error::{SeedError, Result}};
use crate::core::types::{VideoType, AudioType};
use crate::utils::fs::filter_files_by_extension;

/// Generate mediainfo output for media file(s)
/// 
/// Runs mediainfo command on a media file (video or audio). If input_path is a directory,
/// selects a random media file from it. If it's a single file, uses that file.
/// Automatically detects whether to look for video or audio files based on content.
/// 
/// # Arguments
/// * `input_path` - Path to a media file or directory containing media files
/// * `config` - Configuration containing the mediainfo path
/// 
/// # Returns
/// * `Ok(String)` - Mediainfo output as text
/// * `Err(String)` - Error message if mediainfo fails or no media files found
pub fn generate_mediainfo(input_path: &str, config: &Config) -> Result<String> {
    let mediainfo_path = &config.paths.mediainfo;
    let path = std::path::Path::new(input_path);
    
    // Get all valid video and audio extensions from the type definitions
    let video_extensions = VideoType::all_extensions();
    let audio_extensions = AudioType::all_extensions();
    
    let media_file = if path.is_file() {
        // Single file - verify it's a media file (video or audio)
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let ext_lower = ext.to_lowercase();
            if VideoType::from_extension(&ext_lower).is_some() || AudioType::from_extension(&ext_lower).is_some() {
                path.to_path_buf()
            } else {
                return Err(SeedError::Validation(format!("File '{}' is not a supported media file", input_path)));
            }
        } else {
            return Err(SeedError::Validation(format!("File '{}' has no extension", input_path)));
        }
    } else if path.is_dir() {
        // Directory - first try to find video files, then audio files
        let mut media_files = filter_files_by_extension(input_path, &video_extensions)?;
        
        if media_files.is_empty() {
            // No video files found, try audio files
            media_files = filter_files_by_extension(input_path, &audio_extensions)?;
        }
        
        if media_files.is_empty() {
            return Err(SeedError::Validation(format!("No media files found in directory '{}' or its subdirectories", input_path)));
        }
        
        // Pick a random media file
        let mut rng = rand::thread_rng();
        media_files.choose(&mut rng)
            .ok_or_else(|| SeedError::Validation("Failed to select random media file".to_string()))?
            .clone()
    } else {
        return Err(SeedError::Validation(format!("Path '{}' is neither a file nor a directory", input_path)));
    };
    
    info!("Running mediainfo on: {}", media_file.display());
    
    let output = Command::new(mediainfo_path)
        .args(&["--Output=TEXT", &media_file.to_string_lossy()])
        .output()
        .map_err(|e| SeedError::Other(format!("Failed to run mediainfo: {}", e)))?;

    if !output.status.success() {
        return Err(SeedError::Validation(format!(
            "Mediainfo command failed with status: {}",
            output.status
        )));
    }

    let mut result = String::from_utf8(output.stdout)
        .map_err(|e| SeedError::Parse(format!("Failed to parse mediainfo output: {}", e)))?;

    // Sanitize the "Complete name" field
    if let Some(start) = result.find("Complete name") {
        if let Some(end) = result[start..].find('\n') {
            let full_line = &result[start..start + end];
            if let Some(_separator) = full_line.find(':') {
                let sanitized_line = format!(
                    "Complete name                            : {}",
                    media_file.file_name().unwrap_or_default().to_string_lossy()
                );
                result = result.replace(full_line, &sanitized_line);
            }
        }
    }

    Ok(result)
}