use reqwest::blocking::{multipart::Form, Client};
use std::path::Path;
use regex::Regex;
use log::info;
use serde_json::Value;
use std::fs;
use walkdir::WalkDir;
use crate::types::{SeedpoolConfig, Config};

/// Filter files in a directory by accepted file extensions
/// 
/// Takes an input path (file or directory) and returns only files with accepted extensions.
/// If input is a file, returns it only if it has an accepted extension.
/// If input is a directory, recursively finds all files with accepted extensions.
/// 
/// # Arguments
/// * `input_path` - Path to file or directory to search
/// * `accepted_extensions` - Array of accepted file extensions (without dots, e.g., ["mp4", "mkv"])
/// 
/// # Returns
/// Vector of paths to files with accepted extensions
pub fn filter_files_by_extension(input_path: &str, accepted_extensions: &[&str]) -> Result<Vec<std::path::PathBuf>, String> {
    let path = Path::new(input_path);
    
    if !path.exists() {
        return Err(format!("Path not found: {}", input_path));
    }
    
    let mut matching_files = Vec::new();
    
    // Convert extensions to lowercase for case-insensitive comparison
    let accepted_exts: Vec<String> = accepted_extensions
        .iter()
        .map(|ext| ext.to_lowercase())
        .collect();
    
    if path.is_file() {
        // Check if single file has accepted extension
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if accepted_exts.contains(&ext.to_lowercase()) {
                matching_files.push(path.to_path_buf());
            }
        }
    } else if path.is_dir() {
        // Keywords to exclude from paths
        let excluded_keywords = ["sample", "samples", "screen", "screens", "screenshot", "screenshots", 
                                "extra", "extras", "proof", "proofs"];
        
        // Recursively search directory for files with accepted extensions
        for entry in WalkDir::new(path)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let entry_path = entry.path();
            
            // Check if path contains any excluded keywords
            let path_str = entry_path.to_string_lossy().to_lowercase();
            let should_exclude = excluded_keywords.iter().any(|keyword| {
                path_str.contains(&format!("/{}/", keyword)) || 
                path_str.contains(&format!("\\{}\\", keyword)) ||
                path_str.ends_with(&format!("/{}", keyword)) ||
                path_str.ends_with(&format!("\\{}", keyword))
            });
            
            if should_exclude {
                continue;
            }
            
            if entry_path.is_file() {
                if let Some(ext) = entry_path.extension().and_then(|e| e.to_str()) {
                    if accepted_exts.contains(&ext.to_lowercase()) {
                        matching_files.push(entry_path.to_path_buf());
                    }
                }
            }
        }
    }
    
    Ok(matching_files)
}

/// Find and read NFO file content from a directory
/// 
/// Searches for .nfo files in the specified directory and returns the content
/// of the first NFO file found. This can be used by any media processing function
/// to include NFO data in uploads.
/// 
/// # Arguments
/// * `working_path` - Path to directory to search for NFO files
/// 
/// # Returns
/// * `Ok(Some((path, content)))` - Path to NFO file and its content as bytes
/// * `Ok(None)` - No NFO file found
/// * `Err(String)` - Error reading NFO file
pub fn find_and_read_nfo(working_path: &str) -> Result<Option<(String, Vec<u8>)>, String> {
    let path = Path::new(working_path);
    
    if !path.exists() {
        return Err(format!("Path not found: {}", working_path));
    }
    
    // If it's a single file, check if it's an NFO
    if path.is_file() {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if ext.eq_ignore_ascii_case("nfo") {
                let content = fs::read(path)
                    .map_err(|e| format!("Failed to read NFO file '{}': {}", path.display(), e))?;
                return Ok(Some((path.to_string_lossy().to_string(), content)));
            }
        }
        return Ok(None);
    }
    
    // Search directory for NFO files
    if path.is_dir() {
        for entry in fs::read_dir(path)
            .map_err(|e| format!("Failed to read directory '{}': {}", working_path, e))?
        {
            let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
            let entry_path = entry.path();
            
            if entry_path.is_file() {
                if let Some(ext) = entry_path.extension().and_then(|e| e.to_str()) {
                    if ext.eq_ignore_ascii_case("nfo") {
                        info!("Found NFO file: {}", entry_path.display());
                        let content = fs::read(&entry_path)
                            .map_err(|e| format!("Failed to read NFO file '{}': {}", entry_path.display(), e))?;
                        return Ok(Some((entry_path.to_string_lossy().to_string(), content)));
                    }
                }
            }
        }
    }
    
    Ok(None)
}

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
pub fn generate_mediainfo(input_path: &str, config: &Config) -> Result<String, String> {
    use std::process::Command;
    use rand::seq::SliceRandom;
    use crate::types::{VideoType, AudioType};
    
    let mediainfo_path = &config.paths.mediainfo;
    let path = Path::new(input_path);
    
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
                return Err(format!("File '{}' is not a supported media file", input_path));
            }
        } else {
            return Err(format!("File '{}' has no extension", input_path));
        }
    } else if path.is_dir() {
        // Directory - first try to find video files, then audio files
        let mut media_files = filter_files_by_extension(input_path, &video_extensions)?;
        
        if media_files.is_empty() {
            // No video files found, try audio files
            media_files = filter_files_by_extension(input_path, &audio_extensions)?;
        }
        
        if media_files.is_empty() {
            return Err(format!("No media files found in directory '{}' or its subdirectories", input_path));
        }
        
        // Pick a random media file
        let mut rng = rand::thread_rng();
        media_files.choose(&mut rng)
            .ok_or_else(|| "Failed to select random media file".to_string())?
            .clone()
    } else {
        return Err(format!("Path '{}' is neither a file nor a directory", input_path));
    };
    
    info!("Running mediainfo on: {}", media_file.display());
    
    let output = Command::new(mediainfo_path)
        .args(&["--Output=TEXT", &media_file.to_string_lossy()])
        .output()
        .map_err(|e| format!("Failed to run mediainfo: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "Mediainfo command failed with status: {}",
            output.status
        ));
    }

    let mut result = String::from_utf8(output.stdout)
        .map_err(|e| format!("Failed to parse mediainfo output: {}", e))?;

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


// Re-export validation functions for backward compatibility
pub use crate::validation::{validate_file_path, validate_api_key, validate_url};

// Re-export naming functions for backward compatibility
pub use crate::naming::generate_release_name;

// Re-export torrent functions for backward compatibility
pub use crate::torrent::{create_torrent, add_torrent_to_all_qbittorrent_instances, add_torrent_to_qbittorrent, add_torrent_to_deluge};

// Re-export archive functions for backward compatibility
pub use crate::archive::{extract_rar_archives, extract_archives_in_directory};

// Re-export ebook functions for backward compatibility
pub use crate::media::ebook::{process_ebook_upload, generate_ebook_description};

// Re-export video functions for backward compatibility
pub use crate::media::video::{
    find_video_files, generate_sample, get_video_duration, default_non_video_description,
    process_file, contains_excluded_keywords, generate_description
};

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
) -> Result<(Vec<String>, Vec<String>), String> {
    use crate::naming::generate_release_name;
    
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
    let ffmpeg_path = _ffmpeg_path.to_str().ok_or("Invalid ffmpeg path")?;
    let ffprobe_path_str = ffprobe_path.to_str().ok_or("Invalid ffprobe path")?;
    
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
            fs::remove_file(&screenshot_path).map_err(|e| format!("Failed to delete temporary screenshot: {}", e))?;
        }
        
        return Ok((screenshots_list, thumbnails_list));
    }
    
    // Use CDN upload (we already checked has_cdn)
    info!("Using CDN for screenshot upload");
    let remote = remote_path.unwrap(); // Safe because we checked has_cdn
    let image = image_path.unwrap(); // Safe because we checked has_cdn
    
    // Ensure the output directory exists
    let output_dir = &config.paths.screenshots_dir;
    fs::create_dir_all(output_dir).map_err(|e| format!("Failed to create output directory: {}", e))?;
    
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
                .map_err(|e| format!("Failed to set permissions for {}: {}", screenshot_file, e))?;
            fs::set_permissions(&thumbnail_file, fs::Permissions::from_mode(0o777))
                .map_err(|e| format!("Failed to set permissions for {}: {}", thumbnail_file, e))?;
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
fn generate_screenshot(video_file: &str, ffmpeg_path: &str, timestamp: &u32, output_file: &str) -> Result<(), String> {
    use std::process::Command;
    
    Command::new(ffmpeg_path)
        .args(&[
            "-y", "-loglevel", "error", "-ss", &timestamp.to_string(),
            "-i", video_file, "-vframes", "1", "-qscale:v", "2", output_file,
        ])
        .status()
        .map_err(|e| format!("Failed to run ffmpeg for screenshot: {}", e))?;
    Ok(())
}

/// Generate a thumbnail from a screenshot
fn generate_thumbnail(ffmpeg_path: &str, input_file: &str, output_file: &str) -> Result<(), String> {
    use std::process::Command;
    
    Command::new(ffmpeg_path)
        .args(&[
            "-y", "-loglevel", "error", "-i", input_file,
            "-vf", "scale=720:-1", output_file,
        ])
        .status()
        .map_err(|e| format!("Failed to run ffmpeg for thumbnail: {}", e))?;
    Ok(())
}

pub fn fetch_tmdb_id(title: &str, year: Option<String>, tmdb_api_key: &str, release_type: &str) -> Result<u32, String> {
    let sanitized_title = if release_type == "TvShow" {
        // Extract everything before the SXX* pattern
        let season_regex = Regex::new(r"(?i)(S\d{2}.*)").unwrap();
        let cleaned_title = season_regex.replace(title, "").trim().to_string();

        // Remove the year if present
        let year_regex = Regex::new(r"\b(19|20)\d{2}\b").unwrap();
        year_regex.replace(&cleaned_title, "").trim().to_string()
    } else {
        // For movies, extract everything before the year
        let year_regex = Regex::new(r"\b(19|20)\d{2}\b").unwrap();
        year_regex.replace(title, "").trim().to_string()
    };

    let encoded_title = urlencoding::encode(&sanitized_title);

    let url = if release_type == "tv" {
        format!(
            "https://api.themoviedb.org/3/search/tv?query={}&first_air_date_year={}&api_key={}",
            encoded_title,
            year.unwrap_or_default(),
            tmdb_api_key
        )
    } else {
        format!(
            "https://api.themoviedb.org/3/search/movie?query={}&year={}&api_key={}",
            encoded_title,
            year.unwrap_or_default(),
            tmdb_api_key
        )
    };

    info!("TMDB API URL: {}", url);

    let client = Client::new();
    let response = client
        .get(&url)
        .send()
        .map_err(|e| format!("Failed to query TMDB for '{}': {}", title, e))?;

    if !response.status().is_success() {
        return Err(format!(
            "TMDB API request failed with status: {}",
            response.status()
        ));
    }

    let json: Value = response
        .json()
        .map_err(|e| format!("Failed to parse TMDB response for '{}': {}", title, e))?;

    let tmdb_id = json["results"]
        .as_array()
        .and_then(|results| results.get(0))
        .and_then(|result| result["id"].as_u64())
        .unwrap_or(0) as u32;

    if tmdb_id == 0 {
        info!("No TMDB ID found for '{}'.", title);
    }

    Ok(tmdb_id)
}

pub fn fetch_youtube_trailer(title: &str, year: Option<&str>, youtube_api_key: &str) -> Result<String, String> {
    let client = Client::new();

    // Construct the search query
    let query = if let Some(year) = year {
        format!("{} {} trailer", title, year)
    } else {
        format!("{} trailer", title)
    };

    // Construct the YouTube Data API URL
    let url = format!(
        "https://www.googleapis.com/youtube/v3/search?part=snippet&q={}&type=video&key={}&maxResults=1",
        urlencoding::encode(&query),
        youtube_api_key
    );

    // Send the API request
    let response = client
        .get(&url)
        .send()
        .map_err(|e| format!("Failed to send request to YouTube API: {}", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "YouTube API request failed with status: {}",
            response.status()
        ));
    }

    // Parse the JSON response
    let response_body = response.text().map_err(|e| format!("Failed to read YouTube API response: {}", e))?;
    let json: Value = serde_json::from_str(&response_body)
        .map_err(|e| format!("Failed to parse YouTube API response: {}", e))?;

    // Extract the video ID of the first result
    if let Some(video_id) = json["items"]
        .as_array()
        .and_then(|items| items.get(0))
        .and_then(|item| item["id"]["videoId"].as_str())
    {
        let video_url = format!("https://www.youtube.com/watch?v={}", video_id);
        Ok(video_url)
    } else {
        Err("No trailer found on YouTube.".to_string())
    }
}



pub fn extract_torrent_id(response_text: &str) -> Result<String, String> {
    // Unescape any escaped slashes
    let response_text = response_text.replace(r"\/", "/");

    // Updated regex to match the numeric ID followed by a dot and a 32-character hash
    let re = regex::Regex::new(r#"/download/(\d+)\.[a-fA-F0-9]{32}"#).map_err(|e| format!("Failed to compile regex: {}", e))?;
    if let Some(captures) = re.captures(&response_text) {
        if let Some(torrent_id) = captures.get(1) {
            return Ok(torrent_id.as_str().to_string());
        }
    }
    Err("Failed to extract torrent ID from response.".to_string())
}

pub fn upload_to_cdn(file_path: &str, remote_path: &str) -> Result<(), String> {
    use std::process::Command;

    info!("Uploading file to CDN: {}", file_path);

    let status = Command::new("scp")
        .arg(file_path)
        .arg(remote_path)
        .status()
        .map_err(|e| format!("Failed to execute scp: {}", e))?;

    if !status.success() {
        return Err(format!("Failed to upload file to CDN: {}", file_path));
    }

    Ok(())
}

pub fn upload_to_imgbb(image_path: &str, imgbb_api_key: &str, dry_run: bool) -> Result<(String, String), String> {
    let client = Client::new();

    // Log the image path and API key for debugging
    log::debug!("Uploading image to ImgBB: path={}, api_key={}", image_path, imgbb_api_key);

    let form = Form::new()
        .file("image", image_path)
        .map_err(|e| format!("Failed to attach image file: {}", e))?;

    let url = format!("https://api.imgbb.com/1/upload?key={}", imgbb_api_key);
    log::debug!("ImgBB API URL: {}", url);

    if dry_run {
        info!("[DRY RUN] Would upload image to ImgBB: {}", url);
        info!("[DRY RUN] Would generate ImgBB URLs: https://i.ibb.co/fake-url and https://i.ibb.co/fake-thumb");
        return Ok(("https://i.ibb.co/fake-url".to_string(), "https://i.ibb.co/fake-thumb".to_string()));
    }

    let response = client
        .post(&url)
        .multipart(form)
        .send()
        .map_err(|e| format!("Failed to upload image to ImgBB: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let response_body = response.text().unwrap_or_else(|_| "Failed to read response body".to_string());
        log::error!("ImgBB API Error: HTTP Status: {}, Response: {}", status, response_body);
        return Err(format!(
            "Failed to upload image to ImgBB. HTTP Status: {}. Response: {}",
            status, response_body
        ));
    }

    let json: serde_json::Value = response
        .json()
        .map_err(|e| format!("Failed to parse ImgBB response: {}", e))?;

    let full_image_url = json["data"]["image"]["url"]
        .as_str()
        .ok_or("Failed to extract full image URL from ImgBB response")?
        .to_string();
    let thumb_url = json["data"]["thumb"]["url"]
        .as_str()
        .ok_or("Failed to extract thumbnail URL from ImgBB response")?
        .to_string();

    log::info!("ImgBB Upload Successful: full_image_url={}, thumb_url={}", full_image_url, thumb_url);

    Ok((full_image_url, thumb_url))
}

/// Check for duplicates across all configured trackers
/// 
/// This function loads configurations and checks all enabled trackers for duplicates.
/// It's useful when you need to check duplicates without knowing which tracker will be used.
/// 
/// # Arguments
/// * `title` - The title/name to check for duplicates
/// 
/// # Returns
/// * `Ok(Vec<(tracker_name, download_link)>)` - List of trackers where duplicates were found
/// * `Err(String)` - If an error occurs
pub fn check_all_duplicates(title: &str) -> Result<Vec<(String, String)>, String> {
    let mut duplicates = Vec::new();
    
    // Try to load configurations
    let config_path = "config/config.yaml";
    let config_content = fs::read_to_string(config_path)
        .map_err(|e| format!("Failed to read config file: {}", e))?;
    let _config: Config = serde_yaml::from_str(&config_content)
        .map_err(|e| format!("Failed to parse config: {}", e))?;
    
    // Check Seedpool if configured
    if let Ok(seedpool_content) = fs::read_to_string("config/trackers/seedpool.yaml") {
        if let Ok(seedpool_config) = serde_yaml::from_str::<SeedpoolConfig>(&seedpool_content) {
            if seedpool_config.general.enabled && seedpool_config.settings.dupe_checks {
                match check_duplicates(title, "seedpool", Some(&seedpool_config), None) {
                    Ok(Some(download_link)) => {
                        duplicates.push(("seedpool".to_string(), download_link));
                    }
                    Ok(None) => {}
                    Err(e) => {
                        info!("Error checking Seedpool duplicates: {}", e);
                    }
                }
            }
        }
    }
    
    // Check TorrentLeech if configured
    if let Ok(tl_content) = fs::read_to_string("config/trackers/torrentleech.yaml") {
        if let Ok(tl_config) = serde_yaml::from_str::<crate::types::TorrentLeechConfig>(&tl_content) {
            if tl_config.general.enabled {
                match check_duplicates(title, "torrentleech", None, Some(&tl_config)) {
                    Ok(Some(download_link)) => {
                        duplicates.push(("torrentleech".to_string(), download_link));
                    }
                    Ok(None) => {}
                    Err(e) => {
                        info!("Error checking TorrentLeech duplicates: {}", e);
                    }
                }
            }
        }
    }
    
    Ok(duplicates)
}

/// Check for duplicates across different trackers
/// 
/// This function dispatches to tracker-specific duplicate checking functions
/// based on the tracker parameter.
/// 
/// # Arguments
/// * `title` - The title/name to check for duplicates
/// * `tracker` - The tracker to check ("seedpool", "sp", "torrentleech", "tl", etc.)
/// * `seedpool_config` - Optional Seedpool configuration (required for Seedpool)
/// * `torrentleech_config` - Optional TorrentLeech configuration (required for TorrentLeech)
/// 
/// # Returns
/// * `Ok(Some(download_link))` - If a duplicate is found, returns the download link
/// * `Ok(None)` - If no duplicate is found
/// * `Err(String)` - If an error occurs
/// 
/// # Example
/// ```rust
/// let tracker = if cli_args.sp { "seedpool" } else if cli_args.tl { "torrentleech" } else { "unknown" };
/// match check_duplicates("Movie.Title.2023.1080p", tracker, Some(&seedpool_config), None)? {
///     Some(download_link) => println!("Duplicate found: {}", download_link),
///     None => println!("No duplicate found"),
/// }
/// ```
pub fn check_duplicates(
    title: &str,
    tracker: &str,
    seedpool_config: Option<&SeedpoolConfig>,
    torrentleech_config: Option<&crate::types::TorrentLeechConfig>,
) -> Result<Option<String>, String> {
    match tracker.to_lowercase().as_str() {
        "seedpool" | "sp" => {
            let seedpool_cfg = seedpool_config
                .ok_or("Seedpool configuration is required for Seedpool duplicate checks")?;
            
            // Check if duplicate checks are enabled
            if !seedpool_cfg.settings.dupe_checks {
                info!("Duplicate checks are disabled for Seedpool");
                return Ok(None);
            }
            
            // Call the seedpool-specific duplicate check
            use crate::definitions::seedpool::check_seedpool_dupes;
            check_seedpool_dupes(title, &seedpool_cfg.general.api_key)
        }
        "torrentleech" | "tl" => {
            let _tl_cfg = torrentleech_config
                .ok_or("TorrentLeech configuration is required for TorrentLeech duplicate checks")?;
            
            // TODO: Implement TorrentLeech duplicate checking
            info!("TorrentLeech duplicate checking not yet implemented");
            Ok(None)
        }
        _ => {
            Err(format!("Unknown tracker: {}", tracker))
        }
    }
}

/// Load tracker configuration from YAML file
pub fn load_tracker_config<T: serde::de::DeserializeOwned>(tracker_name: &str) -> Result<T, String> {
    let config_path = format!("config/trackers/{}.yaml", tracker_name);
    let config_contents = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read {} config: {}", tracker_name, e))?;
    
    serde_yaml::from_str(&config_contents)
        .map_err(|e| format!("Failed to parse {} config: {}", tracker_name, e))
}
