use reqwest::blocking::multipart::Form;
use std::collections::HashMap;
use std::path::Path;
use crate::{Config, SeedpoolConfig, Tracker};
use seed_tools::utils::{generate_release_name, find_video_files, create_torrent, generate_mediainfo, generate_sample, generate_screenshots, fetch_tmdb_id, fetch_external_ids, generate_description, add_torrent_to_all_qbittorrent_instances};
use regex::Regex;
use log::{info, error};

pub struct Seedpool {
    pub upload_url: String,
    pub api_key: String,
}

pub fn process_seedpool_release(
    input_path: &str,
    _sanitized_name: &str,
    config: &mut Config,
    seedpool_config: &SeedpoolConfig,
    ffmpeg_path: &Path,
    ffprobe_path: &Path,
    mkbrr_path: &Path,
    mediainfo_path: &Path,
) -> Result<(), String> {
    log::debug!("Calling determine_release_type_and_title with input_path: {}", input_path);

    let (mut release_type, title, year, season_number, mut episode_number) =
        determine_release_type_and_title(input_path);

    log::debug!(
        "process_seedpool_release: release_type={}, title={}, year={:?}, season_number={:?}, episode_number={:?}",
        release_type, title, year, season_number, episode_number
    );

    if episode_number.is_none() {
        log::warn!("Episode number is None. Adjusting episode_number to 0.");
        episode_number = Some(0);
    }

    // Determine category_id and type_id based on release type
    let (mut category_id, mut type_id) = match release_type.as_str() {
        "tv" => (2, 24),
        "movie" => (1, 22),
        "boxset" => (13, 26),
        _ => (0, 0),
    };

    // Only override category_id and type_id for boxsets
    if release_type == "boxset" && episode_number == Some(0) {
        category_id = 13; // Boxset category
        type_id = 26;     // Boxset type
    }

    log::debug!("process_seedpool_release: category_id={}, type_id={}", category_id, type_id);

    let tmdb_id = fetch_tmdb_id(&title, year, &config.general.tmdb_api_key, &release_type)?;
    log::debug!("process_seedpool_release: tmdb_id={}", tmdb_id);

    let base_name = Path::new(input_path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string(); // Extract only the base name

    let release_name = generate_release_name(&base_name); // Pass only the base name

    let (video_files, nfo_file) = find_video_files(input_path, &config.paths, &seedpool_config.settings)?;
    if video_files.is_empty() {
        return Err("No valid video files detected.".to_string());
    }

    let torrent_files = vec![create_torrent(
        &video_files,
        &config.paths.torrent_dir,
        &seedpool_config.settings.announce_url,
        &mkbrr_path.to_string_lossy(),
    )?];

    let mediainfo_output = generate_mediainfo(&video_files[0], &mediainfo_path.to_string_lossy())?;

    let sample_url = generate_sample(
        &video_files[0],
        &config.paths.screenshots_dir,
        &seedpool_config.screenshots.remote_path,
        &ffmpeg_path.to_string_lossy(),
        &base_name, // Use the base name
    )?;

    let (screenshots, thumbnails) = generate_screenshots(
        &video_files[0],
        &config.paths.screenshots_dir,
        &ffmpeg_path.to_string_lossy(),
        &ffprobe_path.to_string_lossy(),
        &seedpool_config.screenshots.remote_path,
        &base_name, // Use the base name
    )?;

    let (imdb_id, tvdb_id) = match fetch_external_ids(tmdb_id, &release_type, &config.general.tmdb_api_key) {
        Ok(ids) => ids,
        Err(e) => {
            error!("Failed to fetch external IDs: {}. Defaulting to 0 for IMDb and TVDB IDs.", e);
            (None, None)
        }
    };

    let resolution_id = get_seedpool_resolution_id(input_path);

    log::debug!(
        "process_seedpool_release: resolution_id={}, imdb_id={:?}, tvdb_id={:?}",
        resolution_id, imdb_id, tvdb_id
    );

    let description = generate_description(
        &screenshots,
        &thumbnails,
        &sample_url,
        &chrono::Utc::now().format("%A the %dth of %B %Y at %I:%M %p UTC").to_string(),
        Some(&seedpool_config.settings.custom_description),
        None,
        &seedpool_config.screenshots.image_path,
        &release_name,
    );

    Seedpool {
        upload_url: seedpool_config.settings.upload_url.clone(),
        api_key: seedpool_config.general.api_key.clone(),
    }
    .upload(
        &torrent_files[0],
        &release_name, // Pass the release name explicitly
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

    add_torrent_to_all_qbittorrent_instances(
        &torrent_files,
        &config.qbittorrent,
        &config.deluge, // Pass the DelugeConfig
        input_path, // Pass the input_path argument
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

    let cleaned_title = title.replace('.', " ").replace('_', " ");

    let year = year_regex
        .captures(&base_name)
        .and_then(|caps| caps.get(0).map(|m| m.as_str().to_string()));

    log::debug!(
        "determine_release_type_and_title: release_type={}, title={}, year={:?}, season_number={:?}, episode_number={:?}",
        release_type, cleaned_title, year, season_number, episode_number
    );

    (release_type, cleaned_title, year, season_number, episode_number)
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

    if let Ok(entries) = std::fs::read_dir(input_path) {
        for entry in entries.flatten() {
            if let Some(file_name) = entry.file_name().to_str() {
                if let Some(captures) = resolution_regex.captures(file_name) {
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
            }
        }
    }

    log::warn!("No resolution detected in input path or filenames: {}", input_path);
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