use reqwest::blocking::multipart::Form;
use std::collections::HashMap;
use std::path::Path;
use std::ffi::OsStr;
use std::process::Command;
use std::os::unix::fs::PermissionsExt;
use std::fs;
use crate::{Config, Client, SeedpoolConfig, Tracker};
use seed_tools::utils::{
    generate_release_name, find_video_files, create_torrent, generate_mediainfo, generate_sample,
    generate_screenshots, fetch_tmdb_id, generate_screenshots_imgbb, default_non_video_description, fetch_external_ids, generate_description,
    add_torrent_to_all_qbittorrent_instances,
};
use tui::text::Spans;
use tui::text::Span;
use tui::style::{Color, Style};
use regex::Regex;
use log::info;
use seed_tools::types::PreflightCheckResult;
pub struct Seedpool {
    pub upload_url: String,
    pub api_key: String,
}
use walkdir::WalkDir;
pub fn process_seedpool_release(
    input_path: &str,
    _sanitized_name: &str,
    config: &mut Config,
    seedpool_config: &SeedpoolConfig,
    ffmpeg_path: &Path,
    ffprobe_path: &Path,
    mkbrr_path: &Path,
    mediainfo_path: &Path,
    imgbb_api_key: Option<&str>, // Optional ImgBB API key
) -> Result<(), String> {
    log::debug!("Processing release for input_path: {}", input_path);

    // Check for music files early
    let music_extensions = ["mp3", "flac"];
    let mut type_id = 0;
    let mut found_music_file = false;

    for entry in WalkDir::new(input_path).into_iter().filter_map(|e| e.ok()) {
        if let Some(ext) = entry.path().extension().and_then(|e| e.to_str()) {
            if music_extensions.contains(&ext.to_lowercase().as_str()) {
                found_music_file = true;
                match ext.to_lowercase().as_str() {
                    "mp3" => {
                        type_id = 13; // MP3 type
                    }
                    "flac" => {
                        type_id = 11; // FLAC type
                    }
                    _ => {}
                }
                break; // Exit the loop once a valid music file is found
            }
        }
    }

    if found_music_file {
        log::debug!("Music release detected: {}", input_path);
        return process_music_release(input_path, config, seedpool_config, mkbrr_path, ffmpeg_path);
    }

    // Determine release type and title
    let (mut release_type, title, year, season_number, mut episode_number) =
        determine_release_type_and_title(input_path);
    let base_name = Path::new(input_path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    // Check for duplicates
    if let Some(download_link) = check_seedpool_dupes(&base_name, &seedpool_config.general.api_key)? {
        log::info!("Duplicate found for '{}'. Downloading and adding to clients.", base_name);

        let client = reqwest::blocking::Client::new();
        let response = client
            .get(&download_link)
            .send()
            .map_err(|e| format!("Failed to download torrent: {}", e))?;
        if !response.status().is_success() {
            return Err(format!("Failed to download torrent. HTTP Status: {}", response.status()));
        }

        let torrent_data = response
            .bytes()
            .map_err(|e| format!("Failed to read torrent data: {}", e))?;
        let torrent_file_path = Path::new(&config.paths.torrent_dir).join(format!("{}.torrent", base_name));
        std::fs::write(&torrent_file_path, &torrent_data)
            .map_err(|e| format!("Failed to save torrent file: {}", e))?;

        add_torrent_to_all_qbittorrent_instances(
            &[torrent_file_path.to_string_lossy().to_string()],
            &config.qbittorrent,
            &config.deluge,
            input_path,
            &config.paths,
        )?;
        return Ok(());
    }

    // Adjust episode number if none
    if episode_number.is_none() {
        log::warn!("Episode number is None. Adjusting to 0.");
        episode_number = Some(0);
    }

    // Determine category and type IDs
    let (mut category_id, mut type_id) = match release_type.as_str() {
        "tv" => (2, 24),
        "movie" => (1, 22),
        "boxset" => (13, 26),
        _ => (0, 0),
    };
    if release_type == "boxset" && episode_number == Some(0) {
        category_id = 13;
        type_id = 26;
    }

    // Fetch TMDB ID and find video files
    let tmdb_id = fetch_tmdb_id(&title, year, &config.general.tmdb_api_key, &release_type)?;
    let (video_files, nfo_file) = find_video_files(input_path, &config.paths, &seedpool_config.settings)?;
    if video_files.is_empty() {
        return Err("No valid video files detected.".to_string());
    }

    let stripshit_from_videos = seedpool_config.settings.stripshit_from_videos;

    // Generate torrent file
    let torrent_files = vec![create_torrent(
        input_path,
        &config.paths.torrent_dir,
        &seedpool_config.settings.announce_url,
        &mkbrr_path.to_string_lossy(),
        stripshit_from_videos,
    )?];

    // Generate mediainfo
    let mediainfo_output = generate_mediainfo(&video_files[0], &mediainfo_path.to_string_lossy())?;

    // Generate screenshots using ImgBB or Seedpool CDN
    let (screenshots, thumbnails) = if let Some(api_key) = imgbb_api_key {
        if api_key.is_empty() {
            log::warn!("ImgBB API key is empty. Falling back to Seedpool CDN for screenshots.");
            generate_screenshots(
                &video_files[0],
                &config.paths.screenshots_dir,
                &ffmpeg_path.to_string_lossy(),
                &ffprobe_path.to_string_lossy(),
                &seedpool_config.screenshots.remote_path,
                &seedpool_config.screenshots.image_path,
                &_sanitized_name,
            )?
        } else {
            generate_screenshots_imgbb(&video_files[0], ffmpeg_path, ffprobe_path, api_key)?
        }
    } else {
        generate_screenshots(
            &video_files[0],
            &config.paths.screenshots_dir,
            &ffmpeg_path.to_string_lossy(),
            &ffprobe_path.to_string_lossy(),
            &seedpool_config.screenshots.remote_path,
            &seedpool_config.screenshots.image_path,
            &_sanitized_name,
        )?
    };

    let sample_url = if imgbb_api_key.is_some() && !imgbb_api_key.unwrap_or("").is_empty() {
        String::new()
    } else {
        generate_sample(
            &video_files[0],
            &config.paths.screenshots_dir,
            &seedpool_config.screenshots.remote_path,
            &seedpool_config.screenshots.image_path,
            &ffmpeg_path.to_string_lossy(),
            &base_name,
        )?
    };

    // Fetch external IDs
    let (imdb_id, tvdb_id) = fetch_external_ids(tmdb_id, &release_type, &config.general.tmdb_api_key)
        .unwrap_or((None, None));
    let resolution_id = get_seedpool_resolution_id(input_path);

    // Generate description
    let description = generate_description(
        &screenshots,
        &thumbnails,
        &sample_url,
        &chrono::Utc::now().to_string(),
        Some(&seedpool_config.settings.custom_description),
        None,
        &seedpool_config.screenshots.image_path,
        &generate_release_name(&base_name),
    );

    // Upload to Seedpool
    Seedpool {
        upload_url: seedpool_config.settings.upload_url.clone(),
        api_key: seedpool_config.general.api_key.clone(),
    }
    .upload(
        &torrent_files[0],
        &generate_release_name(&base_name),
        Some(&description),
        Some(&mediainfo_output),
        &nfo_file,
        category_id,
        Some(type_id),
        Some(tmdb_id),
        imdb_id,
        tvdb_id,
        season_number,
        episode_number,
        Some(resolution_id),
    )?;

    // Add torrent to clients
    add_torrent_to_all_qbittorrent_instances(
        &torrent_files,
        &config.qbittorrent,
        &config.deluge,
        input_path,
        &config.paths,
    )?;

    Ok(())
}

fn determine_release_type_and_title(input_path: &str) -> (String, String, Option<String>, Option<u32>, Option<u32>) {
    let base_name = Path::new(input_path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    log::debug!("Base name extracted: {}", base_name);

    let season_episode_regex = Regex::new(r"(?i)S(\d{2})E(\d{2})").unwrap();
    let season_only_regex = Regex::new(r"(?i)S(\d{2})").unwrap();
    let boxset_regex = Regex::new(r"(?i)\b(boxset|complete|collection)\b").unwrap();
    let year_regex = Regex::new(r"\b(19|20)\d{2}\b").unwrap();

    let mut release_type = "unknown".to_string();
    let mut season_number = None;
    let mut episode_number = None;

    if let Some(captures) = season_episode_regex.captures(&base_name) {
        log::debug!("Matched SxxEyy pattern: {:?}", captures);
        release_type = "tv".to_string();
        season_number = captures.get(1).and_then(|m| m.as_str().parse::<u32>().ok());
        episode_number = captures.get(2).and_then(|m| m.as_str().parse::<u32>().ok());
    } else if let Some(captures) = season_only_regex.captures(&base_name) {
        log::debug!("Matched Sxx pattern: {:?}", captures);
        release_type = "tv".to_string();
        season_number = captures.get(1).and_then(|m| m.as_str().parse::<u32>().ok());
    } else if boxset_regex.is_match(&base_name) {
        log::debug!("Matched boxset keywords in base_name: {}", base_name);
        release_type = "boxset".to_string();
        season_number = Some(1);
        episode_number = Some(0);
    } else if year_regex.is_match(&base_name) {
        log::debug!("Matched year pattern in base_name: {}", base_name);
        release_type = "movie".to_string();
    }

    let title = if let Some(season_match) = season_episode_regex.find(&base_name) {
        base_name[..season_match.start()].trim().to_string()
    } else if let Some(season_match) = season_only_regex.find(&base_name) {
        base_name[..season_match.start()].trim().to_string()
    } else if let Some(boxset_match) = boxset_regex.find(&base_name) {
        base_name[..boxset_match.start()].trim().to_string()
    } else if let Some(year_match) = year_regex.find(&base_name) {
        base_name[..year_match.start()].trim().to_string()
    } else {
        base_name.trim().to_string()
    };

    let cleaned_title = title.replace('.', " ").replace('_', " ").trim().to_string();

    let year = year_regex
        .captures(&base_name)
        .and_then(|caps| caps.get(0).map(|m| m.as_str().to_string()));

    log::debug!(
        "determine_release_type_and_title: release_type={}, title={}, year={:?}, season_number={:?}, episode_number={:?}",
        release_type, cleaned_title, year, season_number, episode_number
    );

    (release_type, cleaned_title, year, season_number, episode_number)
}

pub fn process_music_release(
    input_path: &str,
    config: &Config,
    seedpool_config: &SeedpoolConfig,
    mkbrr_path: &Path,
    ffmpeg_path: &Path,
) -> Result<(), String> {
    log::debug!("Processing music release for input_path: {}", input_path);

    // Determine category_id and type_id
    let mut category_id = 5; // Music category
    let mut type_id = 0;

    let music_extensions = ["mp3", "flac"];
    let mut found_music_file = false;

    // Use WalkDir to recursively search for music files
    for entry in WalkDir::new(input_path).into_iter().filter_map(|e| e.ok()) {
        if let Some(ext) = entry.path().extension().and_then(|e| e.to_str()) {
            if music_extensions.contains(&ext.to_lowercase().as_str()) {
                found_music_file = true;
                match ext.to_lowercase().as_str() {
                    "mp3" => {
                        type_id = 13; // MP3 type
                    }
                    "flac" => {
                        type_id = 11; // FLAC type
                    }
                    _ => {}
                }
                break; // Exit the loop once a valid music file is found
            }
        }
    }

    if !found_music_file {
        return Err("No valid music files detected (mp3 or flac).".to_string());
    }

    // Find the first audio file in the folder or subfolders
    let first_file = WalkDir::new(input_path)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path().to_path_buf())
        .find(|path| {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                ext.eq_ignore_ascii_case("mp3") || ext.eq_ignore_ascii_case("flac")
            } else {
                false
            }
        })
        .ok_or_else(|| "No valid music files found in the folder.".to_string())?;

    // Extract metadata from the first file
    let metadata = parse_mediainfo_log(&first_file);

    let artist_global = metadata.get("Performer").cloned().unwrap_or_else(|| "Unknown Artist".to_string());
    let album_meta = metadata.get("Album").cloned().unwrap_or_else(|| "Unknown Album".to_string());
    let genre = metadata.get("Genre").cloned().unwrap_or_else(|| "Unknown Genre".to_string());

    let recorded_date = metadata.get("Recorded date").cloned().unwrap_or_default();
    let extracted_year = recorded_date
        .chars()
        .filter(|c| c.is_numeric())
        .collect::<String>()
        .get(0..4)
        .map(|s| s.to_string())
        .unwrap_or_else(|| "Unknown".to_string());

    let audio_format = metadata.get("Format").cloned().unwrap_or_else(|| "Unknown Format".to_string());
    let bit_depth = metadata.get("Bit depth").cloned().unwrap_or_else(|| "Unknown".to_string());
    let sampling_rate = metadata.get("Sampling rate").cloned().unwrap_or_else(|| "Unknown".to_string());

    let sampling_rate_khz = if sampling_rate.ends_with("kHz") {
        sampling_rate.clone() // Already in kHz format
    } else if let Ok(rate) = sampling_rate.parse::<f64>() {
        format!("{:.1} kHz", rate / 1000.0) // Convert Hz to kHz
    } else {
        "Unknown".to_string()
    };

    let audio_info = if bit_depth == "Unknown" || sampling_rate_khz == "Unknown" {
        format!("{} / {}", audio_format, sampling_rate_khz)
    } else {
        format!("{} {} bit / {}", audio_format, bit_depth, sampling_rate_khz)
    };

    // Find the largest image in the folder or subfolders
    let largest_image = WalkDir::new(input_path)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            if let Some(ext) = entry.path().extension().and_then(|e| e.to_str()) {
                ext.eq_ignore_ascii_case("jpg") || ext.eq_ignore_ascii_case("png")
            } else {
                false
            }
        })
        .max_by_key(|entry| entry.metadata().map(|m| m.len()).unwrap_or(0));

    let (album_cover_path, album_cover_url) = if let Some(image) = largest_image {
        // Sanitize the input folder/file name to make it URL-friendly
        let sanitized_name = Path::new(input_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .replace(' ', "_")
            .replace(|c: char| !c.is_alphanumeric() && c != '-' && c != '_', "");

        let album_cover_name = format!("{}.jpg", sanitized_name);
        let album_cover_path = Path::new(input_path).join(&album_cover_name);
        fs::copy(image.path(), &album_cover_path)
            .map_err(|e| format!("Failed to copy album cover: {}", e))?;

        // Set permissions to 777 for the album cover
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            fs::set_permissions(&album_cover_path, fs::Permissions::from_mode(0o777))
                .map_err(|e| format!("Failed to set permissions for album cover '{}': {}", album_cover_path.display(), e))?;
        }

        // Upload the album cover via SCP
        let scp_command = Command::new("scp")
            .arg(album_cover_path.to_str().expect("Failed to convert album cover path to string"))
            .arg(&seedpool_config.screenshots.remote_path)
            .output()
            .map_err(|e| format!("Failed to upload album cover via SCP: {}", e))?;

        if !scp_command.status.success() {
            log::warn!("Failed to upload album cover via SCP.");
        }

        // Generate the public-facing URL for the album cover
        let album_cover_url = format!(
            "{}/{}",
            seedpool_config.screenshots.image_path, // Base URL
            album_cover_name
        );

        (Some(album_cover_path), Some(album_cover_url))
    } else {
        log::warn!("No valid album cover found in the folder.");
        (None, None) // Proceed without an album cover
    };

    // Generate the torrent file
    let base_name = Path::new(input_path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let torrent_file = create_torrent(
        input_path, // Pass the input path directly
        &config.paths.torrent_dir,
        &seedpool_config.settings.announce_url,
        &mkbrr_path.to_string_lossy(),
        true, // Enable filtering for Standard Upload Mode
    )?;

    // Generate the BBCode description
    let description = generate_music_bbcode_description(
        input_path,
        &artist_global,
        &album_meta,
        &extracted_year,
        &genre,
        &audio_info,
        album_cover_url.as_deref(),
        Some(seedpool_config.settings.custom_description.as_str()), // Pass the custom description
    )?;

    // Prepare the upload form
    let client = reqwest::blocking::Client::new();
    let mut form = Form::new()
        .file("torrent", &torrent_file)
        .map_err(|e| format!("Failed to attach torrent file: {}", e))?
        .text("name", base_name.clone()) // Clone base_name to satisfy the 'static lifetime
        .text("category_id", category_id.to_string())
        .text("type_id", type_id.to_string())
        .text("tmdb", "0")
        .text("imdb", "0")
        .text("tvdb", "0")
        .text("anonymous", "0")
        .text("description", description) // Add the generated BBCode description
        .text("mal", "0") // Add default value for mal
        .text("igdb", "0") // Add default value for igdb
        .text("stream", "0") // Add default value for stream
        .text("sd", "0"); // Add default value for sd

    // Send the upload request
    let response = client
        .post(&seedpool_config.settings.upload_url)
        .header("Authorization", format!("Bearer {}", seedpool_config.general.api_key))
        .multipart(form)
        .send()
        .map_err(|e| format!("Failed to send request to Seedpool: {}", e))?;

    let status = response.status();
    let response_text = response.text().unwrap_or_else(|_| "Failed to read response body".to_string());
    info!("Seedpool API Response: {}", response_text);

    if !status.is_success() {
        return Err(format!(
            "Failed to upload to Seedpool. HTTP Status: {}. Response: {}",
            status, response_text
        ));
    }

    // Extract the torrent ID from the response
    let torrent_id = extract_torrent_id(&response_text)?;

    // Create a torrent cover using FFmpeg
    if let Some(album_cover_path) = album_cover_path {
        let torrent_cover_path = album_cover_path.with_file_name(format!("torrent-cover_{}.jpg", torrent_id));
        let ffmpeg_command = Command::new(ffmpeg_path)
            .args([
                "-y",
                "-i",
                album_cover_path.to_str().expect("Failed to convert album cover path to string"),
                "-vf",
                "scale=320:-1",
                "-q:v",
                "1",
                torrent_cover_path.to_str().expect("Failed to convert torrent cover path to string"),
            ])
            .output()
            .map_err(|e| format!("Failed to create torrent cover with FFmpeg: {}", e))?;

        if !ffmpeg_command.status.success() {
            return Err("Failed to create torrent cover with FFmpeg.".to_string());
        }

        // Set permissions to 777 for the torrent cover
        #[cfg(unix)]
        {
            fs::set_permissions(&torrent_cover_path, fs::Permissions::from_mode(0o777))
                .map_err(|e| format!("Failed to set permissions for torrent cover '{}': {}", torrent_cover_path.display(), e))?;
        }

        // Upload the torrent cover via SCP
        let remote_albumcovers_path = format!("{}/albumcovers", seedpool_config.screenshots.remote_path);
        let scp_command = Command::new("scp")
            .arg(&torrent_cover_path)
            .arg(&remote_albumcovers_path)
            .output()
            .map_err(|e| format!("Failed to upload torrent cover via SCP: {}", e))?;

        if !scp_command.status.success() {
            return Err("Failed to upload torrent cover via SCP.".to_string());
        }
    } else {
        log::warn!("No album cover path provided. Skipping torrent cover creation.");
    }

    log::info!("Music release successfully uploaded: {}", base_name);

    // Add torrent to all qBittorrent instances
    add_torrent_to_all_qbittorrent_instances(
        &[torrent_file.clone()], // Use the torrent_file directly
        &config.qbittorrent,
        &config.deluge,
        input_path,
        &config.paths,
    )?;

    Ok(())
}

// Helper function to extract the torrent ID from the response
fn extract_torrent_id(response_text: &str) -> Result<String, String> {
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

pub fn generate_music_bbcode_description(
    input_path: &str,
    artist_global: &str,
    album_meta: &str,
    extracted_year: &str,
    genre: &str,
    audio_info: &str,
    album_cover_url: Option<&str>,
    custom_description: Option<&str>, 
) -> Result<String, String> {
    let mut description = String::new();

    // Add the input folder/file name as the first line
    let base_name = Path::new(input_path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    description.push_str(&format!("[b]{}[/b]\n", base_name));

    // Add artist, album, year, genre, and audio info
    description.push_str(&format!(
        "[b]Artist:[/b] {}\n[b]Album:[/b] {}\n[b]Year:[/b] {}\n[b]Genre:[/b] {}\n[b]Audio:[/b] {}\n",
        artist_global, album_meta, extracted_year, genre, audio_info
    ));

    // Start the table with the new "kHz" column
    description.push_str("[table]\n[tr][th]Nr.[/th][th]Artist[/th][th]Title[/th][th]Duration[/th][th]Size[/th][th]Format[/th][th]Bitrate[/th][th]kHz[/th][/tr]\n");

    // Loop through files in the folder and subfolders
    let mut track_number = 1;
    for entry in WalkDir::new(input_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|entry| {
            if let Some(ext) = entry.path().extension().and_then(|e| e.to_str()) {
                ext.eq_ignore_ascii_case("mp3") || ext.eq_ignore_ascii_case("flac")
            } else {
                false
            }
        })
    {
        let path = entry.path();

        // Parse the mediainfo log
        let metadata = parse_mediainfo_log(&path);

        // Extract fields from the metadata
        let title = metadata.get("Track name").cloned().unwrap_or_else(|| "Unknown Title".to_string());
        let artist = metadata.get("Performer").cloned().unwrap_or_else(|| artist_global.to_string());
        let duration = metadata.get("Duration").cloned().unwrap_or_else(|| "Unknown".to_string());
        let size = metadata.get("File size").cloned().unwrap_or_else(|| "Unknown".to_string());
        let format = metadata.get("Format").cloned().unwrap_or_else(|| {
            path.extension()
                .and_then(|e| e.to_str())
                .unwrap_or("Unknown")
                .to_uppercase()
        });
        let bitrate = metadata.get("Overall bit rate").cloned().unwrap_or_else(|| "Unknown".to_string());
        let sampling_rate = metadata.get("Sampling rate").cloned().unwrap_or_else(|| "Unknown".to_string());

        // Add track details to the table
        description.push_str(&format!(
            "[tr][td]{}[/td][td]{}[/td][td]{}[/td][td]{}[/td][td]{}[/td][td]{}[/td][td]{}[/td][td]{}[/td][/tr]\n",
            track_number, artist, title, duration, size, format, bitrate, sampling_rate
        ));

        track_number += 1;
    }

    // Close the table
    description.push_str("[/table]\n");

    // Add album cover if provided
    if let Some(cover_url) = album_cover_url {
        description.push_str(&format!("\n[img]{}[/img]\n", cover_url));
    }

    if let Some(custom_desc) = custom_description {
        description.push_str(custom_desc);
        description.push_str("\n\n");
    }

    // Append the default non-video description wrapped in [note] (not centered)
    description.push_str(&default_non_video_description());

    Ok(description)
}

pub fn parse_metadata(folder: &str) -> Result<(String, String, String, String, String), String> {
    // Find the first audio file in the folder
    let first_file = std::fs::read_dir(folder)
        .map_err(|e| format!("Failed to read directory: {}", e))?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .find(|path| {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                ext.eq_ignore_ascii_case("mp3") || ext.eq_ignore_ascii_case("flac")
            } else {
                false
            }
        })
        .ok_or_else(|| "No valid audio files found in the folder.".to_string())?;

    // Parse the mediainfo log for the first file
    let metadata = parse_mediainfo_log(&first_file);

    // Extract fields from the metadata
    let artist_global = metadata.get("Performer").cloned().unwrap_or_else(|| "Unknown Artist".to_string());
    let album_meta = metadata.get("Album").cloned().unwrap_or_else(|| "Unknown Album".to_string());
    let audio_format = metadata.get("Format").cloned().unwrap_or_else(|| "Unknown Format".to_string());
    let bit_depth = metadata.get("Bit depth").cloned().unwrap_or_else(|| "Unknown".to_string());
    let sampling_rate = metadata.get("Sampling rate").cloned().unwrap_or_else(|| "0".to_string());

    // Return the extracted metadata
    Ok((artist_global, album_meta, audio_format, bit_depth, sampling_rate))
}

fn parse_mediainfo_log(file_path: &Path) -> HashMap<String, String> {
    let output = Command::new("mediainfo")
        .arg(file_path)
        .output();

    let mut metadata = HashMap::new();

    if let Ok(output) = output {
        if output.status.success() {
            let log = String::from_utf8_lossy(&output.stdout);
            for line in log.lines() {
                if let Some((key, value)) = line.split_once(':') {
                    metadata.insert(key.trim().to_string(), value.trim().to_string());
                }
            }
        }
    }

    metadata
}

fn get_seedpool_resolution_id(input_path: &str) -> u32 {
    let resolution_regex = Regex::new(r"(?i)(8640p|4320p|2160p|1440p|1080p|1080i|720p|576p|576i|480p|480i)").unwrap();

    if let Some(captures) = resolution_regex.captures(input_path) {
        if let Some(resolution) = captures.get(1).map(|m| m.as_str().to_lowercase()) {
            return match resolution.as_str() {
                "8640p" => 10,
                "4320p" => 1,
                "2160p" => 2,
                "1440p" => 3,
                "1080p" => 3,
                "1080i" => 4,
                "720p" => 5,
                "576p" => 6,
                "576i" => 7,
                "480p" => 8,
                "480i" => 9,
                _ => 10,
            };
        }
    }

    10
}

impl Tracker for Seedpool {
    fn requires_screenshots(&self) -> bool {
        true
    }

    fn requires_sample(&self) -> bool {
        true
    }

    fn requires_tmdb_id(&self) -> bool {
        true
    }

    fn requires_remote_path(&self) -> bool {
        true
    }

    fn generate_metadata(&self, _: &str) -> Result<HashMap<String, String>, String> {
        Ok(HashMap::from([
            ("category".to_string(), "TV".to_string()),
            ("original_language".to_string(), "en".to_string()),
            ("type".to_string(), "WEB".to_string()),
        ]))
    }

    fn upload(
        &self,
        torrent_file: &str,
        release_name: &str, // Pass the release name explicitly
        description: Option<&str>,
        mediainfo: Option<&str>,
        nfo_file: &Option<String>,
        mut category_id: u32,
        mut type_id: Option<u32>,
        tmdb_id: Option<u32>,
        imdb_id: Option<String>,
        tvdb_id: Option<u32>,
        season_number: Option<u32>,
        episode_number: Option<u32>,
        resolution_id: Option<u32>,
    ) -> Result<(), String> {
        log::debug!(
            "upload: category_id={}, type_id={:?}, tmdb_id={:?}, imdb_id={:?}, tvdb_id={:?}, season_number={:?}, episode_number={:?}, resolution_id={:?}",
            category_id, type_id, tmdb_id, imdb_id, tvdb_id, season_number, episode_number, resolution_id
        );

        // Detect and update category_id and type_id for boxsets before constructing the form
        if category_id == 2 && episode_number == Some(0) {
            log::debug!("Detected season-only release. Setting category_id to 13 (Boxset) and type_id to 26.");
            category_id = 13; // Boxset category
            type_id = Some(26); // Boxset type
        }

        let client = reqwest::blocking::Client::new();

        let mut form = Form::new()
            .file("torrent", torrent_file)
            .map_err(|e| format!("Failed to attach torrent file: {}", e))?
            .text("name", release_name.to_string()) // Use the passed release name
            .text("category_id", category_id.to_string())
            .text("type_id", type_id.unwrap_or(0).to_string())
            .text("resolution_id", resolution_id.unwrap_or(0).to_string())
            .text("anonymous", "0")
            .text("mal", "0")
            .text("igdb", "0")
            .text("stream", "0")
            .text("sd", "0");

        if let Some(desc) = description {
            form = form.text("description", desc.to_string());
        }
        if let Some(media) = mediainfo {
            form = form.text("mediainfo", media.to_string());
        }
        if let Some(nfo) = nfo_file {
            form = form.file("nfo", nfo).map_err(|e| format!("Failed to attach NFO file: {}", e))?;
        }
        form = form
            .text("tmdb", tmdb_id.unwrap_or(0).to_string())
            .text("imdb", imdb_id.unwrap_or_else(|| "0".to_string()))
            .text("tvdb", tvdb_id.unwrap_or(0).to_string());

        // Only include season_number and episode_number if category_id is 2 (TV) or 13 (Boxset)
        if category_id == 2 || category_id == 13 {
            if let Some(season) = season_number {
                form = form.text("season_number", season.to_string());
            }
            if let Some(episode) = episode_number {
                form = form.text("episode_number", episode.to_string());
            }
        }

        let response = client
            .post(&self.upload_url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .multipart(form)
            .send()
            .map_err(|e| format!("Failed to send request to Seedpool: {}", e))?;

        let status = response.status();
        let response_text = response.text().unwrap_or_else(|_| "Failed to read response body".to_string());
        info!("Seedpool API Response: {}", response_text);

        if !status.is_success() {
            return Err(format!(
                "Failed to upload to Seedpool. HTTP Status: {}. Response: {}",
                status, response_text
            ));
        }
        Ok(())
    }
}

fn check_seedpool_dupes(
    name: &str,
    seedpool_api_key: &str,
) -> Result<Option<String>, String> {
    let client = Client::new();

    info!("Checking Seedpool for existing torrent with name: '{}'", name);

    // Use the full input name as the search term
    let search_term = generate_release_name(name);
    info!("Search Term for Seedpool Query: '{}'", search_term);

    let query_url = format!(
        "https://seedpool.org/api/torrents/filter?name={}&perPage=10&sortField=name&sortDirection=asc&api_token={}",
        urlencoding::encode(&search_term),
        seedpool_api_key
    );

    info!("Seedpool API Query URL: {}", query_url);

    let search_response = client
        .get(&query_url)
        .send()
        .map_err(|e| format!("Failed to query Seedpool for '{}': {}", name, e))?;

    if !search_response.status().is_success() {
        return Err(format!(
            "Failed to query Seedpool for '{}': HTTP {}",
            name,
            search_response.status()
        ));
    }

    let raw_response = search_response.text().unwrap_or_else(|_| "Failed to read response body".to_string());
    info!("Seedpool API Response: {}", raw_response);

    let search_results: serde_json::Value = serde_json::from_str(&raw_response)
        .map_err(|e| format!("Failed to parse Seedpool response for '{}': {}", name, e))?;

    let empty_vec = vec![];
    let data = search_results["data"].as_array().unwrap_or(&empty_vec);

    for result in data {
        if let Some(attributes) = result["attributes"].as_object() {
            if let Some(result_title) = attributes.get("name").and_then(|t| t.as_str()) {
                info!("Checking result title: {}", result_title);

                // Check for an exact match with the search term
                if result_title == search_term {
                    if let Some(download_link) = attributes.get("download_link").and_then(|d| d.as_str()) {
                        info!("Duplicate found for '{}'. Download link: {}", name, download_link);
                        return Ok(Some(download_link.to_string()));
                    }
                } else {
                    info!("Skipping result due to mismatched title: {}", result_title);
                }
            }
        }
    }

    info!("No duplicate found for '{}'.", name);
    Ok(None)
}

pub fn preflight_check(
    input_path: &str,
    config: &Config,
    seedpool_config: &SeedpoolConfig,
    ffmpeg_path: &Path,
    ffprobe_path: &Path,
    mediainfo_path: &Path,
) -> Result<PreflightCheckResult, String> {
    log::debug!("Processing release for input_path: {}", input_path);

    // Step 0: Check for music files
    let music_extensions = ["mp3", "flac"];
    let mut found_music_file = false;
    let mut music_type = None;

    for entry in WalkDir::new(input_path).into_iter().filter_map(|e| e.ok()) {
        if let Some(ext) = entry.path().extension().and_then(|e| e.to_str()) {
            if music_extensions.contains(&ext.to_lowercase().as_str()) {
                found_music_file = true;
                music_type = Some(match ext.to_lowercase().as_str() {
                    "mp3" => "🎧 MP3".to_string(), // Add 🎧 icon for MP3
                    "flac" => "🎧 FLAC".to_string(), // Add 🎧 icon for FLAC
                    _ => ext.to_uppercase(),
                });
                break; // Exit the loop once a valid music file is found
            }
        }
    }

    // If music files are found, process as a music release
    if found_music_file {
        log::debug!("Music files detected in input path: {}", input_path);

        // Extract metadata from the first music file
        let first_file = WalkDir::new(input_path)
            .into_iter()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path().to_path_buf())
            .find(|path| {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    ext.eq_ignore_ascii_case("mp3") || ext.eq_ignore_ascii_case("flac")
                } else {
                    false
                }
            })
            .ok_or_else(|| "No valid music files found in the folder.".to_string())?;

        let metadata = parse_mediainfo_log(&first_file);

        let artist = metadata.get("Performer").cloned().unwrap_or_else(|| "Unknown Artist".to_string());
        let album = metadata.get("Album").cloned().unwrap_or_else(|| "Unknown Album".to_string());
        let audio_format = metadata.get("Format").cloned().unwrap_or_else(|| "Unknown Format".to_string());
        let bit_depth = metadata.get("Bit depth").cloned().unwrap_or_else(|| "Unknown".to_string());
        let sampling_rate = metadata.get("Sampling rate").cloned().unwrap_or_else(|| "Unknown".to_string());

        let sampling_rate_khz = if sampling_rate.ends_with("kHz") {
            sampling_rate.clone()
        } else if let Ok(rate) = sampling_rate.parse::<f64>() {
            format!("{:.1} kHz", rate / 1000.0)
        } else {
            "Unknown".to_string()
        };

        let audio_info = if bit_depth == "Unknown" || sampling_rate_khz == "Unknown" {
            format!("{} / {}", audio_format, sampling_rate_khz)
        } else {
            format!("{} {} / {}", audio_format, bit_depth, sampling_rate_khz)
        };

        let title = format!("{} - {}", artist, album);
        let generated_release_name = Path::new(input_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        // Check for album cover (image file) in the input path or subfolders
        let album_cover_available = WalkDir::new(input_path)
            .into_iter()
            .filter_map(|entry| entry.ok())
            .any(|entry| {
                if let Some(ext) = entry.path().extension().and_then(|e| e.to_str()) {
                    ext.eq_ignore_ascii_case("jpg") || ext.eq_ignore_ascii_case("png")
                } else {
                    false
                }
            });

        let album_cover_status = if album_cover_available {
            "Available".to_string()
        } else {
            "Not Available".to_string()
        };

        // Generate and print the log
        println!("Pre-flight Check Results:");
        println!("Title: {}", title);
        println!("Release Name: {}", generated_release_name);
        println!("Dupe Check: N/A");
        println!("Release Type: {}", music_type.as_ref().unwrap());
        println!("Season Number: N/A");
        println!("Episode Number: N/A");
        println!("TMDB ID: 0");
        println!("IMDb ID: N/A");
        println!("TVDB ID: N/A");
        println!("Excluded Files: N/A");
        println!("Album Cover: {}", album_cover_status);
        println!("Audio Languages: [{}]", audio_info);

        return Ok(PreflightCheckResult {
            release_name: title,
            generated_release_name,
            dupe_check: "N/A".to_string(),
            tmdb_id: 0,
            imdb_id: None,
            tvdb_id: None,
            excluded_files: "N/A".to_string(),
            album_cover: album_cover_status,
            audio_languages: vec![audio_info],
            release_type: format!("{} Music", music_type.as_ref().unwrap().to_uppercase()),
            season_number: None,
            episode_number: None,
        });
    }

    // Step 1: Determine release type and title
    let (release_type_raw, title, year, season_number, episode_number) =
        determine_release_type_and_title(input_path);
    log::debug!(
        "Release type: {}, Title: {}, Year: {:?}, Season: {:?}, Episode: {:?}",
        release_type_raw, title, year, season_number, episode_number
    );

    // Add icons for display purposes, but keep the raw release_type for logic
    let release_type_display = if release_type_raw == "tv" && episode_number.is_none() {
        "📺 Boxset".to_string() // Return plain string
    } else {
        match release_type_raw.as_str() {
            "tv" => format!("★  📺 TV Show"), // Include the star as plain text
            "movie" => "🎥 Movie".to_string(),
            "boxset" => "📺 Boxset".to_string(),
            _ => release_type_raw.clone(),
        }
    };

    // Step 2: Generate release name using `generate_release_name`
    let base_name = Path::new(input_path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let generated_release_name = generate_release_name(&base_name);
    // Step 3: Check for duplicates
    if let Some(download_link) = check_seedpool_dupes(&title, &seedpool_config.general.api_key)? {
        log::info!("Duplicate found for '{}'. Downloading and adding to clients.", title);

        let client = reqwest::blocking::Client::new();
        let response = client
            .get(&download_link)
            .send()
            .map_err(|e| format!("Failed to download torrent: {}", e))?;
        if !response.status().is_success() {
            return Err(format!("Failed to download torrent. HTTP Status: {}", response.status()));
        }

        let torrent_data = response
            .bytes()
            .map_err(|e| format!("Failed to read torrent data: {}", e))?;
        let torrent_file_path = Path::new(&config.paths.torrent_dir).join(format!("{}.torrent", title));
        std::fs::write(&torrent_file_path, &torrent_data)
            .map_err(|e| format!("Failed to save torrent file: {}", e))?;

        add_torrent_to_all_qbittorrent_instances(
            &[torrent_file_path.to_string_lossy().to_string()],
            &config.qbittorrent,
            &config.deluge,
            input_path,
            &config.paths,
        )?;

        return Ok(PreflightCheckResult {
            release_name: title.clone(),
            generated_release_name: generated_release_name.clone(),
            dupe_check: "FAIL".to_string(),
            tmdb_id: 0,
            imdb_id: None,
            tvdb_id: None,
            excluded_files: "N/A".to_string(),
            album_cover: "N/A".to_string(),
            audio_languages: vec![],
            release_type: release_type_display,
            season_number,
            episode_number,
        });
    }

    // Step 4: Fetch TMDB ID
    log::info!(
        "Fetching TMDB ID with title: '{}', year: {:?}, release_type: '{}'",
        title,
        year,
        release_type_raw
    );
    let tmdb_id = fetch_tmdb_id(&title, year, &config.general.tmdb_api_key, &release_type_raw)?;
    log::debug!("TMDB ID: {}", tmdb_id);

    // Step 5: Fetch external IDs (IMDb, TVDB)
    let (imdb_id, tvdb_id) = fetch_external_ids(tmdb_id, &release_type_raw, &config.general.tmdb_api_key)
        .unwrap_or((None, None));
    log::debug!("IMDb ID: {:?}, TVDB ID: {:?}", imdb_id, tvdb_id);

    // Step 6: Check the `strip_from_videos` setting
    let excluded_files = if seedpool_config.settings.stripshit_from_videos {
        "Yes".to_string()
    } else {
        "No".to_string()
    };

    // Step 7: Extract audio languages using MediaInfo
    let mut audio_languages = Vec::new();
    let (video_files, _) = find_video_files(input_path, &config.paths, &seedpool_config.settings)?;
    for video_file in &video_files {
        let mediainfo_output = generate_mediainfo(video_file, &mediainfo_path.to_string_lossy())?;
        audio_languages.extend(extract_audio_languages(&mediainfo_output));
    }
    log::debug!("Audio languages: {:?}", audio_languages);

    // Generate and print the log
    println!("Pre-flight Check Results:");
    println!("Title: {}", title);
    println!("Release Name: {}", generated_release_name); // Use the generated release name
    println!("Dupe Check: ✔️ PASS");
    println!("Release Type: {}", release_type_display);
    println!("Season Number: {}", season_number.map_or("N/A".to_string(), |s| s.to_string()));
    println!("Episode Number: {}", episode_number.map_or("N/A".to_string(), |e| e.to_string()));
    println!("TMDB ID: {}", tmdb_id);
    println!("IMDb ID: {}", imdb_id.clone().unwrap_or_else(|| "N/A".to_string()));
    println!("TVDB ID: {}", tvdb_id.map_or("N/A".to_string(), |id| id.to_string()));
    println!("Excluded Files: {}", excluded_files);
    println!("Album Cover: N/A");
    println!("Audio Languages: [{}]", audio_languages.join(", "));

    // Step 8: Return the preflight check result
    Ok(PreflightCheckResult {
        release_name: title.clone(),
        generated_release_name, // Use the generated release name
        dupe_check: "✔️ PASS".to_string(),
        tmdb_id,
        imdb_id,
        tvdb_id,
        excluded_files,
        album_cover: "N/A".to_string(),
        audio_languages,
        release_type: release_type_display,
        season_number,
        episode_number,
    })
}

// Helper function to extract audio languages from MediaInfo output
fn extract_audio_languages(mediainfo_output: &str) -> Vec<String> {
    let mut audio_languages = Vec::new();
    let mut in_audio_section = false;

    for line in mediainfo_output.lines() {
        if line.starts_with("Audio") {
            in_audio_section = true; // Entering an audio section
        } else if line.is_empty() {
            in_audio_section = false; // Exiting the current section
        }

        if in_audio_section && line.contains("Language") {
            if let Some(language) = line.split(':').nth(1) {
                audio_languages.push(language.trim().to_string());
            }
        }
    }

    audio_languages
}