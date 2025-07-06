// Cover art extraction utilities for audio files

use std::path::{Path, PathBuf};
use std::process::Command;
use log::{debug, info, warn};

use crate::core::{Config, error::{SeedError, Result}};
use crate::utils::http::{download_file, upload_to_imgbb};

/// Extract cover art from audio files with fallback to MusicBrainz
pub fn extract_cover_art(
    input_path: &str,
    config: &Config,
    release_name: &str,
    metadata: &serde_json::Value,
    dry_run: bool,
) -> Result<Option<String>> {
    info!("Processing cover art for audio files");
    
    let output_dir = PathBuf::from(&config.paths.screenshots_dir);
    std::fs::create_dir_all(&output_dir)
        .map_err(|e| SeedError::Other(format!("Failed to create output directory: {}", e)))?;
    
    // Step 1: Try to extract embedded artwork
    let input_path = Path::new(input_path);
    let mut cover_path: Option<PathBuf> = None;
    
    if input_path.is_file() {
        cover_path = extract_embedded_artwork(input_path, &output_dir, release_name, config)?;
    } else if input_path.is_dir() {
        // For directories, try to find and extract from the first audio file
        let audio_extensions = ["mp3", "flac", "m4a", "ogg", "opus", "wav"];
        
        for entry in std::fs::read_dir(input_path)
            .map_err(|e| SeedError::Other(format!("Failed to read directory: {}", e)))? 
        {
            if let Ok(entry) = entry {
                let path = entry.path();
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if audio_extensions.contains(&ext.to_lowercase().as_str()) {
                        cover_path = extract_embedded_artwork(&path, &output_dir, release_name, config)?;
                        if cover_path.is_some() {
                            break;
                        }
                    }
                }
            }
        }
    }
    
    // Step 2: If no embedded artwork, try MusicBrainz
    if cover_path.is_none() {
        cover_path = fetch_from_musicbrainz(metadata, &output_dir, release_name)?;
    }
    
    // Step 3: Upload the cover art if found
    if let Some(path) = cover_path {
        if !dry_run {
            match upload_cover(&path, config, dry_run) {
                Ok(url) => {
                    info!("✅ Cover art uploaded successfully");
                    return Ok(Some(url));
                }
                Err(e) => {
                    warn!("Failed to upload cover art: {}", e);
                }
            }
        } else {
            info!("Dry run: Would upload cover art from {}", path.display());
            return Ok(Some(format!("file://{}", path.display())));
        }
    }
    
    info!("No cover art found for audio files");
    Ok(None)
}

/// Extract embedded cover art from audio file using ffmpeg
fn extract_embedded_artwork(
    audio_file: &Path,
    output_dir: &Path,
    release_name: &str,
    config: &Config,
) -> Result<Option<PathBuf>> {
    let cover_path = output_dir.join(format!("{}_cover.jpg", release_name));
    
    info!("Attempting to extract embedded artwork from: {}", audio_file.display());
    
    let (ffmpeg_path, _, _, _) = Config::get_binary_paths(config);
    let ffmpeg_path_str = ffmpeg_path.to_str()
        .ok_or_else(|| SeedError::Other("Invalid ffmpeg path".to_string()))?;
    
    let output = Command::new(ffmpeg_path_str)
        .arg("-i")
        .arg(audio_file)
        .arg("-an")  // Disable audio
        .arg("-vcodec")
        .arg("copy")  // Copy video stream (cover art)
        .arg("-y")  // Overwrite output
        .arg(&cover_path)
        .output()
        .map_err(|e| SeedError::Other(format!("Failed to run ffmpeg: {}", e)))?;

    if output.status.success() && cover_path.exists() {
        // Verify the file is a valid image
        let file_size = std::fs::metadata(&cover_path)
            .map(|m| m.len())
            .unwrap_or(0);
        
        if file_size > 0 {
            info!("✅ Successfully extracted embedded cover art: {} bytes", file_size);
            return Ok(Some(cover_path));
        } else {
            // Clean up empty file
            let _ = std::fs::remove_file(&cover_path);
        }
    }
    
    debug!("No embedded artwork found in audio file");
    Ok(None)
}

/// Download cover art from MusicBrainz Cover Art Archive
fn fetch_from_musicbrainz(
    metadata: &serde_json::Value,
    output_dir: &Path,
    release_name: &str,
) -> Result<Option<PathBuf>> {
    // Check if we have a MusicBrainz release ID
    let release_id = metadata.get("musicbrainz_release_id")
        .and_then(|v| v.as_str());
    
    if let Some(mbid) = release_id {
        info!("Attempting to fetch cover art from MusicBrainz for release: {}", mbid);
        
        let cover_path = output_dir.join(format!("{}_mb_cover.jpg", release_name));
        let url = format!("https://coverartarchive.org/release/{}/front", mbid);
        
        match download_file(&url, 60) {
            Ok(image_data) => {
                std::fs::write(&cover_path, image_data)
                    .map_err(|e| SeedError::Other(format!("Failed to save cover image: {}", e)))?;
                
                info!("✅ Successfully downloaded cover art from MusicBrainz");
                return Ok(Some(cover_path));
            }
            Err(e) => {
                debug!("Failed to fetch cover art from MusicBrainz: {}", e);
            }
        }
    } else {
        debug!("No MusicBrainz release ID available for cover art lookup");
    }
    
    Ok(None)
}

/// Upload cover art and return URL
fn upload_cover(cover_path: &Path, config: &Config, dry_run: bool) -> Result<String> {
    let use_imgbb = config.imgbb.is_some();
    
    if use_imgbb {
        // Upload to ImgBB
        let imgbb_api_key = config.imgbb.as_ref()
            .ok_or_else(|| SeedError::Other("ImgBB API key not configured".to_string()))?
            .imgbb_api_key.as_str();
        
        let cover_path_str = cover_path.to_str()
            .ok_or_else(|| SeedError::Other("Invalid cover path".to_string()))?;
        
        info!("Uploading cover art to ImgBB...");
        match upload_to_imgbb(cover_path_str, imgbb_api_key, dry_run) {
            Ok((url, _delete_url)) => {
                info!("✅ Cover art uploaded to ImgBB: {}", url);
                Ok(url)
            }
            Err(e) => {
                warn!("Failed to upload cover art to ImgBB: {}", e);
                Err(e)
            }
        }
    } else if let Some(cdn_paths) = &config.paths.cdnpaths {
        let cdn_path = cdn_paths.remote_path.as_ref()
            .ok_or_else(|| SeedError::Other("CDN remote path not configured".to_string()))?;
        
        // Upload to CDN via SCP
        info!("Uploading cover art to CDN...");
        let remote_path = format!("{}/covers/", cdn_path.trim_end_matches('/'));
        let filename = cover_path.file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| SeedError::Other("Invalid cover filename".to_string()))?;
        
        let output = Command::new("scp")
            .arg("-o")
            .arg("StrictHostKeyChecking=no")
            .arg(cover_path)
            .arg(&format!("{}{}", remote_path, filename))
            .output()
            .map_err(|e| SeedError::Other(format!("Failed to run scp: {}", e)))?;

        if output.status.success() {
            let image_path = cdn_paths.image_path.as_ref()
                .ok_or_else(|| SeedError::Other("CDN image path not configured".to_string()))?;
            let url = format!("{}/covers/{}", image_path.trim_end_matches('/'), filename);
            info!("✅ Cover art uploaded to CDN: {}", url);
            Ok(url)
        } else {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            Err(SeedError::Other(format!("SCP upload failed: {}", error_msg)))
        }
    } else {
        Err(SeedError::Other("No upload destination configured".to_string()))
    }
}