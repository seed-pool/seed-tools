use reqwest::blocking::{multipart::Form, Client};
use reqwest::blocking::ClientBuilder;
use reqwest::cookie::Jar;
use std::path::Path;
use std::sync::Arc;
use regex::Regex;
use log::{info, error};
use std::process::Command;
use serde_json::{Value, json};
use rand::Rng;
use std::os::unix::fs::PermissionsExt;
use std::fs::{self, Permissions};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use crate::types::{PathsConfig, QbittorrentConfig, VideoSettings, DelugeConfig};
pub fn generate_release_name(base_name: &str) -> String {
    let mut release_name = base_name.to_string();
    release_name = Regex::new(r"\.(mkv|mp4|m4b|avi|mov|flv|wmv|ts)$")
        .unwrap()
        .replace(&release_name, "")
        .to_string(); // Remove file extensions
    release_name = Regex::new(r"[^A-Za-z0-9+\-]")
        .unwrap()
        .replace_all(&release_name, ".")
        .to_string(); // Replace non-alphanumeric characters with dots
    release_name = Regex::new(r"\.\.+")
        .unwrap()
        .replace_all(&release_name, ".")
        .to_string(); // Replace multiple dots with a single dot
    release_name = Regex::new(r"-\.+|\.-+")
        .unwrap()
        .replace_all(&release_name, "-")
        .to_string(); // Replace mixed dot-dash patterns
    release_name = Regex::new(r"\.$")
        .unwrap()
        .replace(&release_name, "")
        .to_string(); // Remove trailing dots
    release_name.trim_start_matches('.').to_string() // Remove leading dots
}

pub fn find_video_files<T>(
    input_path: &str,
    _paths: &PathsConfig,
    settings: &T,
) -> Result<(Vec<String>, Option<String>), String>
where
    T: VideoSettings,
{
    let supported_extensions = ["mkv", "mp4", "ts", "avi", "mov", "flv", "wmv"];
    let path = Path::new(input_path);

    let mut video_files = Vec::new();
    let mut nfo_file: Option<String> = None;

    let exclusions_enabled = settings.stripshit_from_videos();
    info!("Exclusions enabled: {}", exclusions_enabled);

    if path.is_file() {
        log::debug!("Processing file: {}", path.display());
        process_file(path, &mut video_files, &mut nfo_file, &supported_extensions, exclusions_enabled)?;
    } else if path.is_dir() {
        for entry in fs::read_dir(path).map_err(|e| format!("Failed to read directory: {}", e))? {
            let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
            let file_path = entry.path();
            log::debug!("Processing file: {}", file_path.display());
            process_file(&file_path, &mut video_files, &mut nfo_file, &supported_extensions, exclusions_enabled)?;
        }
    }

    if video_files.is_empty() {
        error!("No valid video files detected after exclusions.");
        return Err("No valid video files detected.".to_string());
    }

    info!("Final NFO file: {:?}", nfo_file);

    Ok((video_files, nfo_file))
}

pub fn create_torrent(
    files: &[String],
    torrent_dir: &str,
    announce_url: &str,
    mkbrr_path: &str,
) -> Result<String, String> {
    fs::create_dir_all(torrent_dir)
        .map_err(|e| format!("Failed to create torrent directory '{}': {}", torrent_dir, e))?;

    let input_path = if files.len() == 1 {
        files[0].clone()
    } else {
        let parent_folder = Path::new(&files[0])
            .parent()
            .ok_or("Failed to determine parent folder for multi-file release")?;
        parent_folder.to_string_lossy().to_string()
    };

    // Extract the base name of the folder or file for the release name
    let base_name = Path::new(&input_path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let release_name = generate_release_name(&base_name);
    let torrent_file = format!("{}/{}.torrent", torrent_dir, release_name);

    info!("Creating torrent for input path: {}", input_path);
    info!("Torrent File: {}", torrent_file);

    let mut command = Command::new(mkbrr_path);
    command.args(&[
        "create",
        "-t", announce_url,
        "-o", &torrent_file,
        "--source", "seedpool.org",
        &input_path,
    ]);

    let output = command.output().map_err(|e| format!("Failed to run mkbrr: {}", e))?;

    if !output.stdout.is_empty() {
        info!("mkbrr stdout:\n{}", String::from_utf8_lossy(&output.stdout));
    }
    if !output.stderr.is_empty() {
        error!("mkbrr stderr:\n{}", String::from_utf8_lossy(&output.stderr));
    }

    if !output.status.success() {
        return Err(format!(
            "mkbrr failed to create torrent for input path: {}. Exit code: {}",
            input_path,
            output.status.code().unwrap_or(-1)
        ));
    }

    info!("Created torrent: {}", torrent_file);
    Ok(torrent_file)
}

pub fn generate_mediainfo(video_file: &str, mediainfo_path: &str) -> Result<String, String> {
    let output = Command::new(mediainfo_path)
        .args(&["--Output=TEXT", video_file])
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
            if let Some(separator) = full_line.find(':') {
                let sanitized_line = format!(
                    "Complete name                            : {}",
                    Path::new(video_file).file_name().unwrap_or_default().to_string_lossy()
                );
                result = result.replace(full_line, &sanitized_line);
            }
        }
    }

    Ok(result)
}

pub fn add_torrent_to_all_qbittorrent_instances(
    torrent_files: &[String],
    qbittorrent_configs: &[QbittorrentConfig],
    deluge_config: &DelugeConfig,
    input_path: &str,
    paths_config: &PathsConfig, // Add this parameter
) -> Result<(), String> {
    let is_folder = Path::new(input_path).is_dir();

    // Add torrents to all qBittorrent instances
    for config in qbittorrent_configs {
        for torrent_file in torrent_files {
            if let Some(executable) = &config.executable {
                // Call add_torrent_to_qbittorrent for each instance
                if let Err(e) = add_torrent_to_qbittorrent(
                    torrent_file,
                    config,
                    input_path,
                    is_folder,
                    paths_config, // Pass paths_config here
                ) {
                    error!(
                        "Error adding torrent '{}' to qBittorrent instance '{}': {}",
                        torrent_file, config.webui_url, e
                    );
                } else {
                    info!(
                        "Successfully added torrent '{}' to qBittorrent instance '{}'.",
                        torrent_file, config.webui_url
                    );
                }
            } else {
                error!(
                    "No executable specified for qBittorrent instance '{}'. Skipping.",
                    config.webui_url
                );
            }
        }
    }

    // Add torrents to Deluge
    for torrent_file in torrent_files {
        let save_path = if is_folder {
            Path::new(input_path).to_string_lossy().to_string()
        } else {
            Path::new(input_path)
                .parent()
                .unwrap_or_else(|| Path::new(&deluge_config.webui_url))
                .to_string_lossy()
                .to_string()
        };

        // Pass the `is_folder` argument to `add_torrent_to_deluge`
        if let Err(e) = add_torrent_to_deluge(
            torrent_file,
            deluge_config,
            &save_path,
            is_folder,
            paths_config, // Pass paths_config here
        ) {
            error!("Error adding torrent '{}' to Deluge: {}", torrent_file, e);
        } else {
            info!("Successfully added torrent '{}' to Deluge.", torrent_file);
        }
    }

    Ok(())
}

pub fn process_file(
    file_path: &Path,
    video_files: &mut Vec<String>,
    nfo_file: &mut Option<String>,
    supported_extensions: &[&str],
    exclusions_enabled: bool,
) -> Result<(), String> {
    let file_name = file_path.file_name().unwrap_or_default().to_string_lossy().to_string();

    if let Some(ext) = file_path.extension() {
        let ext = ext.to_string_lossy().to_lowercase();
        if supported_extensions.contains(&ext.as_str()) {
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
    image_path: &str, // Public-facing URL base
    ffmpeg_path: &str,
    input_name: &str, // Use the complete input name
) -> Result<String, String> {
    let sanitized_input_name = generate_release_name(input_name); // Sanitize the input name
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
    upload_to_cdn(&sample_file, remote_path)?;

    // Return the public-facing URL for the sample
    Ok(format!("{}/{}.sample.mkv", image_path, sanitized_input_name))
}

pub fn generate_description(
    screenshots: &[String],
    thumbnails: &[String],
    sample_url: &str,
    _datestamp: &str,
    custom_description: Option<&str>,
    youtube_trailer_url: Option<&str>,
    base_url: &str, // Base URL for assets (e.g., screenshots, thumbnails, etc.)
    release_name: &str, // Release name for generating sanitized links
) -> String {
    let mut description = String::new();


    // Add screenshots in a 2x2 table pattern
    if !screenshots.is_empty() && !thumbnails.is_empty() {
        description.push_str("    [center][tr]\n");

        for (i, (screenshot, thumbnail)) in screenshots.iter().zip(thumbnails.iter()).enumerate() {
            description.push_str(&format!(
                "        [td][url={}][img width=720]{}[/img][/url][/td]\n",
                screenshot, thumbnail
            ));

            // Add a new row every 2 images
            if (i + 1) % 2 == 0 {
                description.push_str("    [/tr]\n    [tr]\n");
            }
        }

        // Close the last row properly
        if screenshots.len() % 2 != 0 {
            description.push_str("    [/center][/tr]\n");
        }
    }

    // Add sample link if available
    if !sample_url.is_empty() {
        description.push_str(&format!(
            "[b][spoiler=Sample: {}]{}[/spoiler][/b]\n\n",
            Path::new(sample_url).file_name().unwrap_or_default().to_string_lossy(),
            sample_url
        ));
    }


    description.push_str("\n");
    // Add YouTube trailer link if available
    if let Some(trailer_url) = youtube_trailer_url {
        description.push_str(&format!(
            "[center][b][url={}][Trailer on YouTube][/url][/b][/center]\n\n",
            trailer_url
        ));
    }

    // Add custom description (not centered)
    if let Some(custom_desc) = custom_description {
        description.push_str(custom_desc);
        description.push_str("\n\n");
    }

    // Append the default non-video description wrapped in [note] (not centered)

    description.push_str(&default_non_video_description());


    description
}

pub fn fetch_tmdb_id(title: &str, year: Option<String>, tmdb_api_key: &str, release_type: &str) -> Result<u32, String> {
    let sanitized_title = if release_type == "tv" {
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

pub fn fetch_external_ids(tmdb_id: u32, release_type: &str, tmdb_api_key: &str) -> Result<(Option<String>, Option<u32>), String> {
    if tmdb_id == 0 {
        return Ok((None, None));
    }

    let tmdb_type = if release_type == "boxset" { "tv" } else { release_type };
    let url = format!(
        "https://api.themoviedb.org/3/{}/{}/external_ids?api_key={}",
        tmdb_type, tmdb_id, tmdb_api_key
    );

    log::info!("TMDB External IDs API URL: {}", url);

    let client = reqwest::blocking::Client::new();
    let response = client.get(&url).send().map_err(|e| format!("Failed to fetch external IDs: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("Failed to fetch external IDs: HTTP {}", response.status()));
    }

    let json: serde_json::Value = response.json().map_err(|e| format!("Failed to parse external IDs response: {}", e))?;
    let imdb_id = json["imdb_id"].as_str().map(|s| s.trim_start_matches("tt").to_string());
    let tvdb_id = json["tvdb_id"].as_u64().map(|id| id as u32);

    log::info!("Fetched IMDb ID: {:?}", imdb_id);
    log::info!("Fetched TVDB ID: {:?}", tvdb_id);

    Ok((imdb_id, tvdb_id))
}

pub fn generate_screenshots(
    video_file: &str,
    output_dir: &str,
    ffmpeg_path: &str,
    ffprobe_path: &str,
    remote_path: &str,
    image_path: &str, // Public-facing URL base
    input_name: &str, // Use the complete input name
) -> Result<(Vec<String>, Vec<String>), String> {
    let mut screenshots_list = Vec::new();
    let mut thumbnails_list = Vec::new();

    // Ensure the output directory exists
    fs::create_dir_all(output_dir).map_err(|e| format!("Failed to create output directory: {}", e))?;

    let sanitized_input_name = generate_release_name(input_name); // Sanitize the input name
    let duration = get_video_duration(video_file, ffprobe_path)?;
    let timestamps = generate_random_timestamps(duration, 4);

    for (i, shot_time) in timestamps.iter().enumerate() {
        // Generate sanitized filenames for screenshots and thumbnails
        let screenshot_file = format!("{}/{}_{}.jpg", output_dir, sanitized_input_name, i + 1);
        let thumbnail_file = format!("{}/{}_{}_thumb.jpg", output_dir, sanitized_input_name, i + 1);

        // Generate screenshot
        generate_screenshot(video_file, ffmpeg_path, shot_time, &screenshot_file)?;
        generate_thumbnail(ffmpeg_path, &screenshot_file, &thumbnail_file)?;

        // Set permissions to 777 for the screenshot and thumbnail locally
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&screenshot_file, fs::Permissions::from_mode(0o777))
                .map_err(|e| format!("Failed to set permissions for {}: {}", screenshot_file, e))?;
            fs::set_permissions(&thumbnail_file, fs::Permissions::from_mode(0o777))
                .map_err(|e| format!("Failed to set permissions for {}: {}", thumbnail_file, e))?;
        }

        // Upload files to the CDN
        upload_to_cdn(&screenshot_file, remote_path)?;
        upload_to_cdn(&thumbnail_file, remote_path)?;

        // Add public-facing URLs to the lists
        screenshots_list.push(format!("{}/{}", image_path, Path::new(&screenshot_file).file_name().unwrap().to_string_lossy()));
        thumbnails_list.push(format!("{}/{}", image_path, Path::new(&thumbnail_file).file_name().unwrap().to_string_lossy()));
    }

    Ok((screenshots_list, thumbnails_list))
}

fn get_video_duration(video_file: &str, ffprobe_path: &str) -> Result<f64, String> {
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

fn generate_random_timestamps(duration: f64, count: usize) -> Vec<u32> {
    let start_time = (duration * 0.15) as u32;
    let end_time = (duration * 0.85) as u32;

    let mut rng = rand::thread_rng();
    let mut timestamps: Vec<u32> = (0..count).map(|_| rng.gen_range(start_time..end_time)).collect();
    timestamps.sort();
    timestamps
}

fn generate_screenshot(video_file: &str, ffmpeg_path: &str, timestamp: &u32, output_file: &str) -> Result<(), String> {
    Command::new(ffmpeg_path)
        .args(&[
            "-y", "-loglevel", "error", "-ss", &timestamp.to_string(),
            "-i", video_file, "-vframes", "1", "-qscale:v", "2", output_file,
        ])
        .status()
        .map_err(|e| format!("Failed to run ffmpeg for screenshot: {}", e))?;
    Ok(())
}

fn generate_thumbnail(ffmpeg_path: &str, input_file: &str, output_file: &str) -> Result<(), String> {
    Command::new(ffmpeg_path)
        .args(&[
            "-y", "-loglevel", "error", "-i", input_file,
            "-vf", "scale=720:-1", output_file,
        ])
        .status()
        .map_err(|e| format!("Failed to run ffmpeg for thumbnail: {}", e))?;
    Ok(())
}

fn upload_to_cdn(file_path: &str, remote_path: &str) -> Result<(), String> {
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

fn default_non_video_description() -> String {
    format!(
        "[b][size=12][color=#757575]Created with mkbrr, ffmpeg, and mediainfo. Posted to this fine tracker with seed-tools.[/color][/size][/b]
        
        [url=https://seedpool.org][img]https://cdn.seedpool.org/sp.png[/img][/url]  \
        [url=https://github.com/autobrr][img]https://cdn.seedpool.org/autobrr.png[/img][/url]  \
        [url=https://www.rust-lang.org][img]https://cdn.seedpool.org/rust.png[/img][/url]"
    )
}

fn extract_rar_archives(folder_path: &str) -> Result<Option<String>, String> {
    info!("Checking for RAR archives in folder: {}", folder_path);

    let path = Path::new(folder_path);
    if !path.is_dir() {
        return Err(format!("Provided path is not a directory: {}", folder_path));
    }

    let rar_files: Vec<_> = fs::read_dir(path)
        .map_err(|e| format!("Failed to read directory: {}", e))?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let file_path = entry.path();
            if file_path.extension().map_or(false, |ext| ext.eq_ignore_ascii_case("rar")) {
                Some(file_path)
            } else {
                None
            }
        })
        .collect();

    if rar_files.is_empty() {
        info!("No RAR archives found in folder: {}", folder_path);
        return Ok(None); // No extraction occurred
    }

    info!("Found RAR archives: {:?}", rar_files);

    for rar_file in rar_files {
        info!("Extracting RAR archive: {}", rar_file.display());

        let output = Command::new("unrar")
            .args(&["x", "-o+", rar_file.to_str().unwrap(), folder_path]) // Extract directly into the input folder
            .output()
            .map_err(|e| format!("Failed to execute unrar command: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "Failed to extract RAR archive: {}. Error: {}",
                rar_file.display(),
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        info!("Successfully extracted RAR archive: {}", rar_file.display());
    }

    info!("Extraction completed. Extracted files are in: {}", folder_path);
    Ok(Some(folder_path.to_string()))
}

pub fn determine_save_path(
    input_path: &str,
    default_save_path: &str,
    is_folder: bool,
    is_single_file_torrent: bool,
    torrent_file: &str, // Path to the .torrent file
    mkbrr_path: &str,   // Path to the mkbrr binary
) -> Result<(String, String), String> {
    let input = Path::new(input_path);

    // Fallback for empty default_save_path
    let default_save_path = if default_save_path.is_empty() {
        log::warn!("default_save_path is empty. Falling back to '/home/beholder/files'.");
        "/home/beholder/files".to_string()
    } else {
        default_save_path.to_string()
    };

    // Explicitly check if the input is a folder and contains only a single file
    let contains_single_file = if input.is_dir() {
        let entries: Vec<_> = fs::read_dir(input)
            .map_err(|e| format!("Failed to read directory '{}': {}", input_path, e))?
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.path().is_file())
            .collect();

        entries.len() == 1
    } else {
        false
    };

    // Use mkbrr to inspect the .torrent file if the input is a folder
    let mkbrr_contains_single_file = if is_folder {
        let output = Command::new(mkbrr_path)
            .args(&["inspect", torrent_file])
            .output()
            .map_err(|e| format!("Failed to run mkbrr: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "mkbrr failed to inspect torrent file '{}': {}",
                torrent_file,
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        // Parse the mkbrr output
        let stdout = String::from_utf8_lossy(&output.stdout);
        log::debug!("mkbrr output: {}", stdout);

        // Check if the output contains a single file
        stdout.lines().any(|line| line.trim_start().starts_with("Name:"))
            && !stdout.lines().any(|line| line.trim_start().starts_with("Files:"))
    } else {
        false
    };

    // Combine both checks
    let contains_single_file = contains_single_file || mkbrr_contains_single_file;

    // Determine the base save path
    let base_save_path = if is_folder {
        if is_single_file_torrent || contains_single_file {
            // If the input is a folder and the torrent contains a single file, create a subfolder
            Path::new(&default_save_path)
                .join(input.file_name().unwrap_or_default())
                .to_string_lossy()
                .to_string()
        } else {
            // For folder-to-folder torrents, use the default save path directly
            default_save_path.clone()
        }
    } else {
        // For single-file inputs, use the default save path directly
        default_save_path.clone()
    };

    // Determine if a subfolder should be created
    let create_subfolder = if is_folder && (is_single_file_torrent || contains_single_file) {
        "true" // Create a subfolder if the input is a folder and the torrent contains a single file
    } else {
        "false" // Do not create a subfolder for other cases
    };

    // Debugging logs
    log::debug!("determine_save_path:");
    log::debug!("  input_path: {}", input_path);
    log::debug!("  default_save_path: {}", default_save_path);
    log::debug!("  is_folder: {}", is_folder);
    log::debug!("  is_single_file_torrent: {}", is_single_file_torrent);
    log::debug!("  contains_single_file: {}", contains_single_file);
    log::debug!("  base_save_path: {}", base_save_path);
    log::debug!("  create_subfolder: {}", create_subfolder);

    // Ensure the save path exists
    if let Err(e) = std::fs::create_dir_all(&base_save_path) {
        return Err(format!("Failed to create save path '{}': {}", base_save_path, e));
    }

    Ok((base_save_path, create_subfolder.to_string()))
}

pub fn add_torrent_to_qbittorrent(
    torrent_file: &str,
    config: &QbittorrentConfig,
    input_path: &str,
    is_folder: bool,
    paths_config: &PathsConfig,
) -> Result<(), String> {
    // 1. Create a client with cookie support ENABLED
    info!("Creating HTTP client with cookie support for qBittorrent.");
    let client = Client::builder()
        .cookie_store(true) // <-- Key change: Enable cookie persistence
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    // 2. Log in to qBittorrent (using the cookie-enabled client)
    let login_url = format!("{}/api/v2/auth/login", config.webui_url);
    info!("Logging in to qBittorrent at {}...", login_url);
    let login_response = client // Use the SAME client instance
        .post(&login_url)
        .form(&[
            ("username", config.username.as_str()),            ("password", config.password.as_str()),
        ])
        .send()
        .map_err(|e| format!("Failed to send login request to qBittorrent: {}", e))?;

    // 3. Check Login Response Status AND Body
    let login_status = login_response.status();
    let login_body = login_response.text().map_err(|e| format!("Failed to read login response body: {}", e))?;

    if !login_status.is_success() {
        return Err(format!(
            "qBittorrent login request failed: {} - Body: {}",
            login_status, login_body
        ));
    }
    // qBittorrent API v2 returns "Ok." on successful login
    if login_body.trim() != "Ok." {
        return Err(format!(
            "qBittorrent login failed (unexpected response): {}",
            login_body
        ));
    }
    info!("Logged in to qBittorrent successfully (SID cookie received and stored).");

    // --- Existing Logic (Verify torrent file, determine save path) ---
    if !Path::new(torrent_file).exists() {
        return Err(format!("Torrent file does not exist: {}", torrent_file));
    }

    // Determine if the torrent being added is a folder or a single file
    let is_single_file_torrent = Path::new(input_path).is_file();

    // Determine the save path and subfolder creation logic
    let (save_path, create_subfolder) = determine_save_path(
        input_path,
        &config.default_save_path, // Assuming default_save_path exists on config
        is_folder,
        is_single_file_torrent,
        torrent_file,
        &paths_config.mkbrr, // Assuming mkbrr exists on paths_config
    )?;
    info!("savepath set to: {}", save_path);
    info!("createSubfolder parameter set to: {}", create_subfolder); // Keep original case maybe? Check API docs. Assuming String/bool works.

    // --- Construct the multipart form (Exactly as before) ---
    let mut form = Form::new()
        .file("torrents", torrent_file)
        .map_err(|e| format!("Failed to attach torrent file: {}", e))?
        // API expects boolean as string "true" or "false"
        .text("createSubfolder", create_subfolder.to_string()) // Convert bool to string
        .text("autoTMM", "false")
        .text("paused", "false")
        .text("skip_checking", "true")
        .text("savepath", save_path.clone());

    // Add the category if specified in the config
    if let Some(category) = &config.category {
        info!("Using category for qBittorrent: {}", category);
        form = form.text("category", category.clone());
    }

    // --- Upload the torrent file (using the SAME cookie-enabled client) ---
    let add_url = format!("{}/api/v2/torrents/add", config.webui_url);
    info!("Injecting torrent into qBittorrent at {}...", add_url);
    let upload_response = client // Use the SAME client instance again
        .post(&add_url)
        .multipart(form)
        .send()
        .map_err(|e| format!("Failed to send add torrent request to qBittorrent: {}", e))?;

    // --- Check Upload Response (Exactly as before) ---
    let status = upload_response.status();    // Read body before checking status in case body contains useful error info
    let response_body = upload_response.text().unwrap_or_else(|_| "Failed to read response body".to_string());
    info!("qBittorrent API Response [add]: {}", response_body); // Clarify log message

    // Check status *after* reading body. Check if body contains "fail" as before.
    if !status.is_success() || response_body.to_lowercase().contains("fail") {
        return Err(format!(
            "Failed to upload torrent to qBittorrent: {}. Response: {}",
            status, response_body
        ));
    }

    info!("Torrent added to qBittorrent successfully.");
    Ok(())
}

pub fn add_torrent_to_deluge(
    torrent_file: &str,
    config: &DelugeConfig,
    input_path: &str,
    is_folder: bool,
    paths_config: &PathsConfig,
) -> Result<(), String> {
    info!("Adding torrent '{}' to Deluge at '{}'", torrent_file, config.webui_url);

    // Determine if the torrent being added is a folder or a single file
    let is_single_file_torrent = Path::new(input_path).is_file();

    // Determine the save path and subfolder creation logic
    let (base_save_path, _) = determine_save_path(
        input_path,
        &config.default_save_path,
        is_folder,
        is_single_file_torrent,
        torrent_file,
        &paths_config.mkbrr,
    )?;
    info!("savepath set to: {}", base_save_path);

    // Convert the save path to an absolute path
    let absolute_save_path = fs::canonicalize(&base_save_path)
        .map_err(|e| format!("Failed to resolve absolute path for save path '{}': {}", base_save_path, e))?;

    // Convert the torrent file path to an absolute path
    let absolute_torrent_file = fs::canonicalize(torrent_file)
        .map_err(|e| format!("Failed to resolve absolute path for torrent file '{}': {}", torrent_file, e))?;

    // Create a client with cookie storage
    let cookie_jar = Arc::new(Jar::default());
    let client = ClientBuilder::new()
        .cookie_store(true)
        .cookie_provider(cookie_jar.clone())
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    // Log in to Deluge
    let login_payload = json!({
        "method": "auth.login",
        "params": [config.password],
        "id": 1
    });

    let login_response = client
        .post(format!("{}/json", config.webui_url))
        .json(&login_payload)
        .send()
        .map_err(|e| format!("Failed to log in to Deluge: {}", e))?;

    let login_result: serde_json::Value = login_response
        .json()
        .map_err(|e| format!("Failed to parse Deluge login response: {}", e))?;

    if !login_result["result"].as_bool().unwrap_or(false) {
        return Err("Failed to log in to Deluge: Invalid credentials".to_string());
    }

    info!("Logged in to Deluge successfully.");

    // Add the torrent
    let add_torrent_payload = json!({
        "method": "web.add_torrents",
        "params": [[{
            "path": absolute_torrent_file.to_string_lossy(),
            "options": {
                "download_location": absolute_save_path.to_string_lossy(),
                "add_paused": false,
                "move_completed": false,
                "skip_checking": true, // Skip hash check
                "label": config.label.clone().unwrap_or_default(),
            }
        }]],
        "id": 2
    });

    info!("Deluge add torrent payload: {:?}", add_torrent_payload);

    let add_torrent_response = client
        .post(format!("{}/json", config.webui_url))
        .json(&add_torrent_payload)
        .send()
        .map_err(|e| format!("Failed to add torrent to Deluge: {}", e))?;

    let add_torrent_result: serde_json::Value = add_torrent_response
        .json()
        .map_err(|e| format!("Failed to parse Deluge add torrent response: {}", e))?;

    info!("Deluge add torrent response: {:?}", add_torrent_result);

    // Check for errors in the response
    if let Some(error) = add_torrent_result.get("error") {
        if !error.is_null() {
            return Err(format!(
                "Deluge returned an error while adding torrent: {:?}",
                error
            ));
        }
    }

    info!("Torrent added to Deluge successfully.");
    Ok(())
}