// MediaInfo utilities

use crate::core::types::{AudioType, VideoType};
use crate::core::{
    error::{Result, SeedError},
    Config,
};
use crate::utils::fs::filter_files_by_extension;
use log::info;
use rand::seq::SliceRandom;
use std::process::Command;

/// Generate mediainfo output for media file(s)
///
/// Runs mediainfo command on a media file (video or audio). If input_path is a directory,
/// selects a random media file from it. If it's a single file, uses that file.
/// Automatically detects whether to look for video or audio files based on content.
/// If the path doesn't exist (for preflight with just names), returns "N/A".
///
/// # Arguments
/// * `input_path` - Path to a media file or directory containing media files
/// * `config` - Configuration containing the mediainfo path
///
/// # Returns
/// * `Ok(String)` - Mediainfo output as text or "N/A" if path doesn't exist
/// * `Err(String)` - Error message if mediainfo fails or no media files found
pub fn generate_mediainfo(input_path: &str, config: &Config) -> Result<String> {
    let mediainfo_path = &config.paths.mediainfo;
    let path = std::path::Path::new(input_path);

    // Check if path exists - if not, return "N/A" for preflight mode with just names
    if !path.exists() {
        info!("Path '{}' does not exist, returning N/A for mediainfo", input_path);
        return Ok("N/A".to_string());
    }

    // Get all valid video and audio extensions from the type definitions
    let video_extensions = VideoType::all_extensions();
    let audio_extensions = AudioType::all_extensions();

    let media_file = if path.is_file() {
        // Single file - verify it's a media file (video or audio)
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let ext_lower = ext.to_lowercase();
            if VideoType::from_extension(&ext_lower).is_some()
                || AudioType::from_extension(&ext_lower).is_some()
            {
                path.to_path_buf()
            } else {
                return Err(SeedError::Validation(format!(
                    "File '{}' is not a supported media file",
                    input_path
                )));
            }
        } else {
            return Err(SeedError::Validation(format!(
                "File '{}' has no extension",
                input_path
            )));
        }
    } else if path.is_dir() {
        // Directory - first try to find video files, then audio files
        let mut media_files = filter_files_by_extension(input_path, &video_extensions)?;

        if media_files.is_empty() {
            // No video files found, try audio files
            media_files = filter_files_by_extension(input_path, &audio_extensions)?;
        }

        if media_files.is_empty() {
            return Err(SeedError::Validation(format!(
                "No media files found in directory '{}' or its subdirectories",
                input_path
            )));
        }

        // Pick a random media file
        let mut rng = rand::thread_rng();
        media_files
            .choose(&mut rng)
            .ok_or_else(|| SeedError::Validation("Failed to select random media file".to_string()))?
            .clone()
    } else {
        return Err(SeedError::Validation(format!(
            "Path '{}' is neither a file nor a directory",
            input_path
        )));
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

/// Generate mock mediainfo output for preflight checks when no actual files are present
///
/// Creates a placeholder mediainfo output based on the filename/path provided.
/// This allows preflight checks to run without requiring actual media files.
///
/// # Arguments
/// * `file_name` - Name of the file or folder to generate mock mediainfo for
/// * `media_type` - Type of media (Video, Audio, etc.) to determine format
///
/// # Returns
/// * `String` - Mock mediainfo output as text
pub fn generate_mock_mediainfo(file_name: &str, media_type: &crate::core::MediaType) -> String {
    use std::path::Path;
    
    let path = Path::new(file_name);
    let display_name = path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    
    match media_type {
        crate::core::MediaType::Video(_) => {
            format!(
r#"General
Complete name                            : {}
Format                                   : Matroska
Format version                           : Version 4
File size                               : [To be calculated from actual file]
Duration                                : [To be detected from actual file]
Overall bit rate mode                   : Variable
Overall bit rate                        : [To be detected from actual file]
Movie name                              : {}
Encoded date                            : UTC [To be detected]
Writing application                     : [To be detected]
Writing library                         : [To be detected]

Video
ID                                      : 1
Format                                  : AVC
Format/Info                             : Advanced Video Codec
Format profile                          : High@L4.1
Format settings                         : CABAC / 4 Ref Frames
Format settings, CABAC                  : Yes
Format settings, Reference frames       : 4 frames
Codec ID                                : V_MPEG4/ISO/AVC
Duration                                : [To be detected]
Bit rate                                : [To be detected]
Width                                   : [To be detected]
Height                                  : [To be detected]
Display aspect ratio                    : [To be detected]
Frame rate mode                         : Constant
Frame rate                              : [To be detected]
Color space                             : YUV
Chroma subsampling                      : 4:2:0
Bit depth                               : 8 bits
Scan type                               : Progressive
Language                                : English
Default                                 : Yes
Forced                                  : No

Audio
ID                                      : 2
Format                                  : AAC LC
Format/Info                             : Advanced Audio Codec Low Complexity
Codec ID                                : A_AAC-2
Duration                                : [To be detected]
Bit rate mode                           : Variable
Bit rate                                : [To be detected]
Channel(s)                              : [To be detected]
Channel layout                          : [To be detected]
Sampling rate                           : 48.0 kHz
Frame rate                              : 46.875 FPS (1024 SPF)
Compression mode                        : Lossy
Language                                : English
Default                                 : Yes
Forced                                  : No"#,
                display_name, display_name
            )
        },
        crate::core::MediaType::Audio(_) => {
            format!(
r#"General
Complete name                            : {}
Format                                   : [To be detected from actual file]
File size                               : [To be calculated from actual file]
Duration                                : [To be detected from actual file]
Overall bit rate mode                   : [To be detected]
Overall bit rate                        : [To be detected]
Album                                   : [To be detected]
Track name                              : {}
Performer                               : [To be detected]
Genre                                   : [To be detected]
Recorded date                           : [To be detected]

Audio
Format                                  : [To be detected]
Format/Info                             : [To be detected]
Duration                                : [To be detected]
Bit rate mode                           : [To be detected]
Bit rate                                : [To be detected]
Channel(s)                              : [To be detected]
Sampling rate                           : [To be detected]
Compression mode                        : [To be detected]"#,
                display_name, display_name
            )
        },
        _ => {
            format!(
r#"General
Complete name                            : {}
Format                                   : [To be detected from actual file]
File size                               : [To be calculated from actual file]
Duration                                : [To be detected if applicable]
Overall bit rate                        : [To be detected if applicable]"#,
                display_name
            )
        }
    }
}
