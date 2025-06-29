// Screenshot generation and upload utilities

use std::fs;
use std::path::Path;
use log::info;
use crate::core::{Config, error::{SeedError, Result}};
use crate::utils::http::{upload_to_imgbb, upload_to_cdn};
use crate::processing::naming::generate_release_name;

/// Generate screenshots from a video file
/// 
/// This function consolidates screenshot generation with support for both ImgBB and CDN upload.
/// If imgbb_api_key is provided, it uploads to ImgBB. Otherwise, it falls back to CDN upload.
/// 
/// # Arguments
/// * `video_file` - Path to the video file
/// * `config` - Configuration containing paths
/// * `imgbb_api_key` - Optional ImgBB API key
/// * `remote_path` - Optional remote CDN path for SCP upload
/// * `image_path` - Optional base URL for CDN images
/// * `input_name` - Name used for generating screenshot filenames
/// * `dry_run` - If true, skips actual upload
/// 
/// # Returns
/// * `Ok((screenshots, thumbnails))` - Vectors of screenshot and thumbnail URLs
/// * `Err(String)` - Error message
pub fn generate_screenshots(
    video_file: &str,
    config: &Config,
    imgbb_api_key: Option<&str>,
    remote_path: Option<&str>,
    image_path: Option<&str>,
    input_name: &str,
    dry_run: bool,
) -> Result<(Vec<String>, Vec<String>)> {
    // First check if we have any valid upload method
    let has_imgbb = imgbb_api_key.map(|k| !k.is_empty()).unwrap_or(false);
    let has_cdn = remote_path.map(|r| !r.is_empty()).unwrap_or(false) && 
                  image_path.map(|i| !i.is_empty()).unwrap_or(false);
    
    if !has_imgbb && !has_cdn {
        info!("No image upload method available (no ImgBB API key or CDN paths configured). Proceeding without screenshots.");
        return Ok((Vec::new(), Vec::new()));
    }
    
    let mut screenshots_list = Vec::new();
    let mut thumbnails_list = Vec::new();
    
    // Get binary paths from config
    let (_ffmpeg_path, ffprobe_path, _mkbrr_path, _mediainfo_path) = Config::get_binary_paths(config);
    let ffmpeg_path = _ffmpeg_path.to_str().ok_or(SeedError::Validation("Invalid ffmpeg path".to_string()))?;
    let ffprobe_path_str = ffprobe_path.to_str().ok_or(SeedError::Validation("Invalid ffprobe path".to_string()))?;
    
    // Get video duration and generate timestamps
    let duration = get_video_duration(video_file, ffprobe_path_str)?;
    let timestamps = generate_random_timestamps(duration, 4);
    
    // Generate sanitized base name
    let sanitized_input_name = generate_release_name(input_name);
    
    // Decide whether to use ImgBB or CDN
    if has_imgbb {
        info!("Using ImgBB for screenshot upload");
        let api_key = imgbb_api_key.unwrap(); // Safe because we checked has_imgbb
        
        for (i, timestamp) in timestamps.iter().enumerate() {
            // Generate screenshot file name
            let screenshot_name = format!("{}_{}.jpg", sanitized_input_name, i + 1);
            let screenshot_path = format!("/tmp/{}", screenshot_name);
            
            // Generate screenshot
            generate_screenshot(video_file, ffmpeg_path, timestamp, &screenshot_path)?;
            
            // Upload to ImgBB
            let (full_image_url, thumb_url) = upload_to_imgbb(&screenshot_path, api_key, dry_run)?;
            screenshots_list.push(full_image_url);
            thumbnails_list.push(thumb_url);
            
            // Clean up temp file
            fs::remove_file(&screenshot_path).map_err(|e| SeedError::Other(format!("Failed to delete temporary screenshot: {}", e)))?;
        }
        
        return Ok((screenshots_list, thumbnails_list));
    }
    
    // Use CDN upload (we already checked has_cdn)
    info!("Using CDN for screenshot upload");
    let remote = remote_path.unwrap(); // Safe because we checked has_cdn
    let image = image_path.unwrap(); // Safe because we checked has_cdn
    
    // Ensure the output directory exists
    let output_dir = &config.paths.screenshots_dir;
    fs::create_dir_all(output_dir)?;
    
    for (i, timestamp) in timestamps.iter().enumerate() {
        // Generate screenshot and thumbnail filenames
        let screenshot_file = format!("{}/{}_{}.jpg", output_dir, sanitized_input_name, i + 1);
        let thumbnail_file = format!("{}/{}_{}_thumb.jpg", output_dir, sanitized_input_name, i + 1);
        
        // Generate screenshot
        generate_screenshot(video_file, ffmpeg_path, timestamp, &screenshot_file)?;
        generate_thumbnail(ffmpeg_path, &screenshot_file, &thumbnail_file)?;
        
        // Set permissions
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&screenshot_file, fs::Permissions::from_mode(0o777))
                .map_err(|e| SeedError::Other(format!("Failed to set permissions for {}: {}", screenshot_file, e)))?;
            fs::set_permissions(&thumbnail_file, fs::Permissions::from_mode(0o777))
                .map_err(|e| SeedError::Other(format!("Failed to set permissions for {}: {}", thumbnail_file, e)))?;
        }
        
        // Upload files to CDN
        if !dry_run {
            upload_to_cdn(&screenshot_file, &format!("{}/screenshots/", remote.trim_end_matches('/')))?;
            upload_to_cdn(&thumbnail_file, &format!("{}/screenshots/", remote.trim_end_matches('/')))?;
        } else {
            info!("[DRY RUN] Skipping screenshot/thumbnail upload to CDN");
        }
        
        // Add public-facing URLs
        let screenshot_filename = Path::new(&screenshot_file).file_name().unwrap().to_string_lossy();
        let thumbnail_filename = Path::new(&thumbnail_file).file_name().unwrap().to_string_lossy();
        
        screenshots_list.push(format!("{}/{}", image, screenshot_filename));
        thumbnails_list.push(format!("{}/{}", image, thumbnail_filename));
    }
    
    Ok((screenshots_list, thumbnails_list))
}

/// Get video duration using ffprobe
pub fn get_video_duration(video_file: &str, ffprobe_path: &str) -> Result<f64> {
    use std::process::Command;
    
    let output = Command::new(ffprobe_path)
        .args(&[
            "-v", "error",
            "-show_entries", "format=duration",
            "-of", "default=noprint_wrappers=1:nokey=1",
            video_file
        ])
        .output()
        .map_err(|e| SeedError::Other(format!("Failed to run ffprobe: {}", e)))?;

    if !output.status.success() {
        return Err(SeedError::Validation(format!(
            "ffprobe command failed with status: {}",
            output.status
        )));
    }

    let duration_str = String::from_utf8(output.stdout)
        .map_err(|e| SeedError::Parse(format!("Failed to parse ffprobe output: {}", e)))?;
    
    let duration = duration_str.trim().parse::<f64>()
        .map_err(|e| SeedError::Parse(format!("Failed to parse duration: {}", e)))?;
    
    Ok(duration)
}

/// Generate random timestamps for screenshots
fn generate_random_timestamps(duration: f64, count: usize) -> Vec<u32> {
    use rand::Rng;
    
    let start_time = (duration * 0.15) as u32;
    let end_time = (duration * 0.85) as u32;
    
    let mut rng = rand::thread_rng();
    let mut timestamps: Vec<u32> = (0..count).map(|_| rng.gen_range(start_time..end_time)).collect();
    timestamps.sort();
    timestamps
}

/// Generate a single screenshot at a specific timestamp
fn generate_screenshot(video_file: &str, ffmpeg_path: &str, timestamp: &u32, output_file: &str) -> Result<()> {
    use std::process::Command;
    
    let status = Command::new(ffmpeg_path)
        .args(&[
            "-y", "-loglevel", "error", "-ss", &timestamp.to_string(),
            "-i", video_file, "-vframes", "1", "-qscale:v", "2", output_file,
        ])
        .status()
        .map_err(|e| SeedError::Other(format!("Failed to run ffmpeg for screenshot: {}", e)))?;
        
    if !status.success() {
        return Err(SeedError::Validation(format!("ffmpeg screenshot generation failed")));
    }
    
    Ok(())
}

/// Generate a thumbnail from a screenshot
fn generate_thumbnail(ffmpeg_path: &str, input_file: &str, output_file: &str) -> Result<()> {
    use std::process::Command;
    
    let status = Command::new(ffmpeg_path)
        .args(&[
            "-y", "-loglevel", "error", "-i", input_file,
            "-vf", "scale=720:-1", output_file,
        ])
        .status()
        .map_err(|e| SeedError::Other(format!("Failed to run ffmpeg for thumbnail: {}", e)))?;
        
    if !status.success() {
        return Err(SeedError::Validation(format!("ffmpeg thumbnail generation failed")));
    }
    
    Ok(())
}