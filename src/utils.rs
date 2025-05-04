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

    fn process_path(
        file_path: &Path,
        video_files: &mut Vec<String>,
        nfo_file: &mut Option<String>,
        supported_extensions: &[&str],
        exclusions_enabled: bool,
    ) -> Result<(), String> {
        if file_path.is_dir() {
            for entry in fs::read_dir(file_path).map_err(|e| format!("Failed to read directory: {}", e))? {
                let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
                let entry_path = entry.path();
                process_path(&entry_path, video_files, nfo_file, supported_extensions, exclusions_enabled)?;
            }
        } else {
            log::debug!("Processing file: {}", file_path.display());
            process_file(file_path, video_files, nfo_file, supported_extensions, exclusions_enabled)?;
        }
        Ok(())
    }

    process_path(path, &mut video_files, &mut nfo_file, &supported_extensions, exclusions_enabled)?;

    if video_files.is_empty() {
        error!("No valid video files detected after exclusions.");
        return Err("No valid video files detected.".to_string());
    }

    info!("Final NFO file: {:?}", nfo_file);

    Ok((video_files, nfo_file))
}

pub fn create_torrent(
    input_path: &str,
    torrent_dir: &str,
    announce_url: &str,
    mkbrr_path: &str,
    stripshit_from_videos: bool,
) -> Result<String, String> {
    fs::create_dir_all(torrent_dir)
        .map_err(|e| format!("Failed to create torrent directory '{}': {}", torrent_dir, e))?;

    let base_name = Path::new(input_path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let release_name = generate_release_name(&base_name);
    let torrent_file = format!("{}/{}.torrent", torrent_dir, release_name);

    info!("Creating torrent for input path: {}", input_path);
    info!("Torrent File: {}", torrent_file);

    // Build the mkbrr command
    let mut command = Command::new(mkbrr_path);
    command.args(&[
        "create",
        "-t", announce_url,
        "-o", &torrent_file,
        "--source", "seedpool.org",
        input_path,
    ]);

    // Add the --exclude flag to exclude unwanted terms and non-video files
    if stripshit_from_videos {
        command.args(&[
            "--exclude",
            "*sample*,*proof*,*screens*,*screenshots*,*.txt,*.jpg,*.jpeg,*.png,*.nfo,*.srr,*.doc,*.pdf",
        ]);
    }

    // Execute the mkbrr command
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
    paths_config: &PathsConfig,
) -> Result<(), String> {
    info!("Adding torrents to all qBittorrent and Deluge instances.");

    // Add torrents to all qBittorrent instances
    for config in qbittorrent_configs {
        for torrent_file in torrent_files {
            if let Some(executable) = &config.executable {
                // Call add_torrent_to_qbittorrent for each instance
                if let Err(e) = add_torrent_to_qbittorrent(
                    torrent_file,
                    config,
                    input_path,
                    Path::new(input_path).is_dir(),
                    paths_config,
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
        if let Err(e) = add_torrent_to_deluge(
            torrent_file,
            deluge_config,
            input_path,
            Path::new(input_path).is_dir(),
            paths_config,
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
    image_path: &str,
    ffmpeg_path: &str,
    input_name: &str,
) -> Result<String, String> {
    let sanitized_input_name = generate_release_name(input_name);
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
    _thumbnails: &[String],
    sample_url: &str,
    _datestamp: &str,
    custom_description: Option<&str>,
    youtube_trailer_url: Option<&str>,
    _base_url: &str,
    release_name: &str,
) -> String {
    let mut description = String::new();

    // Add screenshots in a 2x2 table pattern
    if !screenshots.is_empty() {
        description.push_str("[center][tr]\n");

        for (i, screenshot) in screenshots.iter().enumerate() {
            description.push_str(&format!(
                "        [td][url={}][img width=720]{}[/img][/url][/td]\n",
                screenshot, screenshot
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

    // Add a blank line after screenshots
    description.push_str("\n");

    // Add sample link if available
    if !sample_url.is_empty() {
        description.push_str(&format!(
            "[b][spoiler=Sample: {}]{}[/spoiler][/b]\n\n",
            Path::new(sample_url).file_name().unwrap_or_default().to_string_lossy(),
            sample_url
        ));
    }

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

    // Append the default non-video description
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
    image_path: &str,
    input_name: &str,
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

pub fn default_non_video_description() -> String {
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

pub fn add_torrent_to_qbittorrent(
    torrent_file: &str,
    config: &QbittorrentConfig,
    input_path: &str,
    is_folder: bool,
    paths_config: &PathsConfig,
) -> Result<(), String> {
    info!("Creating HTTP client with cookie support for qBittorrent.");
    let client = Client::builder()
        .cookie_store(true)
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let login_url = format!("{}/api/v2/auth/login", config.webui_url);
    info!("Logging in to qBittorrent at {}...", login_url);
    let login_response = client
        .post(&login_url)
        .form(&[
            ("username", config.username.as_str()),
            ("password", config.password.as_str()),
        ])
        .send()
        .map_err(|e| format!("Failed to send login request to qBittorrent: {}", e))?;

    let login_status = login_response.status();
    let login_body = login_response.text().map_err(|e| format!("Failed to read login response body: {}", e))?;

    if !login_status.is_success() {
        return Err(format!(
            "qBittorrent login request failed: {} - Body: {}",
            login_status, login_body
        ));
    }

    if login_body.trim() != "Ok." {
        return Err(format!(
            "qBittorrent login failed (unexpected response): {}",
            login_body
        ));
    }
    info!("Logged in to qBittorrent successfully.");

    if !Path::new(torrent_file).exists() {
        return Err(format!("Torrent file does not exist: {}", torrent_file));
    }

    let mut form = Form::new()
        .file("torrents", torrent_file)
        .map_err(|e| format!("Failed to attach torrent file: {}", e))?
        .text("paused", "false")
        .text("skip_checking", "true");

    if let Some(category) = &config.category {
        info!("Using category for qBittorrent: {}", category);
        form = form.text("category", category.clone());
    }

    let add_url = format!("{}/api/v2/torrents/add", config.webui_url);
    info!("Injecting torrent into qBittorrent at {}...", add_url);
    let upload_response = client
        .post(&add_url)
        .multipart(form)
        .send()
        .map_err(|e| format!("Failed to send add torrent request to qBittorrent: {}", e))?;

    let status = upload_response.status();
    let response_body = upload_response.text().unwrap_or_else(|_| "Failed to read response body".to_string());
    info!("qBittorrent API Response [add]: {}", response_body);

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

    let absolute_torrent_file = fs::canonicalize(torrent_file)
        .map_err(|e| format!("Failed to resolve absolute path for torrent file '{}': {}", torrent_file, e))?;

    let cookie_jar = Arc::new(Jar::default());
    let client = ClientBuilder::new()
        .cookie_store(true)
        .cookie_provider(cookie_jar.clone())
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

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

    let add_torrent_payload = json!({
        "method": "web.add_torrents",
        "params": [[{
            "path": absolute_torrent_file.to_string_lossy(),
            "options": {
                "add_paused": false,
                "move_completed": false,
                "skip_checking": true,
                "label": config.label.clone().unwrap_or_default(),
            }
        }]],
        "id": 2
    });

    let add_torrent_response = client
        .post(format!("{}/json", config.webui_url))
        .json(&add_torrent_payload)
        .send()
        .map_err(|e| format!("Failed to add torrent to Deluge: {}", e))?;

    let add_torrent_result: serde_json::Value = add_torrent_response
        .json()
        .map_err(|e| format!("Failed to parse Deluge add torrent response: {}", e))?;

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

pub fn upload_to_imgbb(image_path: &str, imgbb_api_key: &str) -> Result<(String, String), String> {
    let client = Client::new();

    // Log the image path and API key for debugging
    log::debug!("Uploading image to ImgBB: path={}, api_key={}", image_path, imgbb_api_key);

    let form = Form::new()
        .file("image", image_path)
        .map_err(|e| format!("Failed to attach image file: {}", e))?;

    let url = format!("https://api.imgbb.com/1/upload?key={}", imgbb_api_key);
    log::debug!("ImgBB API URL: {}", url);

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

pub fn generate_screenshots_imgbb(
    video_file: &str,
    ffmpeg_path: &Path,
    ffprobe_path: &Path,
    imgbb_api_key: &str,
) -> Result<(Vec<String>, Vec<String>), String> {
    let mut screenshots = Vec::new();
    let mut thumbnails = Vec::new();

    // Get video duration
    let duration = get_video_duration(video_file, ffprobe_path.to_str().unwrap())?;
    let timestamps = generate_random_timestamps(duration, 4);

    // Generate sanitized base name for screenshots
    let base_name = Path::new(video_file)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let sanitized_base_name = generate_release_name(&base_name);

    for (i, timestamp) in timestamps.iter().enumerate() {
        // Generate screenshot file name
        let screenshot_name = format!("{}_{}.jpg", sanitized_base_name, i + 1);
        let screenshot_path = format!("/tmp/{}", screenshot_name);

        // Generate screenshot
        generate_screenshot(video_file, ffmpeg_path.to_str().unwrap(), timestamp, &screenshot_path)?;

        // Upload screenshot to ImgBB
        let (full_image_url, thumb_url) = upload_to_imgbb(&screenshot_path, imgbb_api_key)?;
        screenshots.push(full_image_url); // Use full_image_url for the description
        thumbnails.push(thumb_url);

        // Clean up the local screenshot file
        fs::remove_file(&screenshot_path).map_err(|e| format!("Failed to delete temporary screenshot: {}", e))?;
    }

    Ok((screenshots, thumbnails))
}