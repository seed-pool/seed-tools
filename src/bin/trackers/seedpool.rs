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
    let base_name = Path::new(input_path).file_name().unwrap_or_default().to_string_lossy().to_string();
    let release_name = generate_release_name(&base_name);
    let (release_type, title, year) = determine_release_type_and_title(&release_name);

    log::debug!("process_seedpool_release: release_type={}, title={}, year={:?}", release_type, title, year);

    // Map boxset to tv for consistency
    let mapped_release_type = if release_type == "boxset" { "tv" } else { &release_type };

    // Parse season and episode numbers for TV releases
    let (mut season_number, mut episode_number) = if mapped_release_type == "tv" {
        parse_season_and_episode(&release_name)
    } else {
        (None, None)
    };

    // Handle boxsets
    if release_type == "boxset" {
        episode_number = Some(0); // Boxsets use episode 0
        season_number = season_number.or(Some(1)); // Default to season 1 if not detected
    }

    // Determine category_id and type_id
    let (category_id, type_id) = if release_type == "boxset" {
        (13, 26) // Boxset category and type for Seedpool
    } else if mapped_release_type == "tv" {
        (2, 24) // Regular TV category and type
    } else if mapped_release_type == "movie" {
        (1, 22) // Movie category and type
    } else {
        (0, 0) // Default invalid type
    };

    log::debug!("process_seedpool_release: category_id={}, type_id={}", category_id, type_id);

    log::debug!(
        "process_seedpool_release: season_number={:?}, episode_number={:?}",
        season_number,
        episode_number
    );

    // TMDB lookup
    let tmdb_id = fetch_tmdb_id(&title, year, &config.general.tmdb_api_key, mapped_release_type)?;
    log::debug!("process_seedpool_release: tmdb_id={}", tmdb_id);

    let (video_files, nfo_file) = find_video_files(input_path, &config.paths, &seedpool_config.settings)?;
    if video_files.is_empty() {
        return Err("No valid video files detected.".to_string());
    }

    let torrent_files = vec![create_torrent(&video_files, &config.paths.torrent_dir, &seedpool_config.settings.announce_url, &mkbrr_path.to_string_lossy())?];

    let mediainfo_output = generate_mediainfo(&video_files[0], &mediainfo_path.to_string_lossy())?;
    let sample_url = generate_sample(&video_files[0], &config.paths.screenshots_dir, &seedpool_config.screenshots.remote_path, &ffmpeg_path.to_string_lossy(), &release_name)?;
    let (screenshots, thumbnails) = generate_screenshots(&video_files[0], &config.paths.screenshots_dir, &ffmpeg_path.to_string_lossy(), &ffprobe_path.to_string_lossy(), &seedpool_config.screenshots.remote_path, &seedpool_config.screenshots.image_path, &release_name)?;
    let description = generate_description(&screenshots, &thumbnails, &sample_url, &chrono::Utc::now().format("%A the %dth of %B %Y at %I:%M %p UTC").to_string(), Some(&seedpool_config.settings.custom_description), None, &seedpool_config.screenshots.image_path);

    let (imdb_id, tvdb_id) = match fetch_external_ids(tmdb_id, mapped_release_type, &config.general.tmdb_api_key) {
        Ok(ids) => ids,
        Err(e) => {
            error!("Failed to fetch external IDs: {}. Defaulting to 0 for IMDb and TVDB IDs.", e);
            (None, None)
        }
    };

    let resolution_id = get_seedpool_resolution_id(&release_name);

    log::debug!(
        "process_seedpool_release: resolution_id={}, imdb_id={:?}, tvdb_id={:?}",
        resolution_id, imdb_id, tvdb_id
    );

    Seedpool {
        upload_url: seedpool_config.settings.upload_url.clone(),
        api_key: seedpool_config.general.api_key.clone(),
    }
    .upload(
        &torrent_files[0],
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

    add_torrent_to_all_qbittorrent_instances(&torrent_files, &config.qbittorrent, input_path, Path::new(input_path).is_dir())?;
    Ok(())
}

fn parse_season_and_episode(filename: &str) -> (Option<u32>, Option<u32>) {
    let season_episode_regex = Regex::new(r"S(\d{2})E(\d{2})").unwrap();
    if let Some(captures) = season_episode_regex.captures(filename) {
        (captures.get(1).and_then(|m| m.as_str().parse::<u32>().ok()), captures.get(2).and_then(|m| m.as_str().parse::<u32>().ok()))
    } else {
        (None, None)
    }
}

fn determine_release_type_and_title(input_path: &str) -> (String, String, Option<String>) {
    let base_name = Path::new(input_path).file_name().unwrap_or_default().to_string_lossy().to_string();
    let season_regex = Regex::new(r"(?i)S\d{2}").unwrap();
    let year_regex = Regex::new(r"\b(19|20)\d{2}\b").unwrap();
    let release_type = if season_regex.is_match(&base_name) { "boxset".to_string() } else if year_regex.is_match(&base_name) { "movie".to_string() } else { "unknown".to_string() };
    let title = if let Some(season_match) = season_regex.find(&base_name) { base_name[..season_match.start()].trim().to_string() } else if let Some(year_match) = year_regex.find(&base_name) { base_name[..year_match.start()].trim().to_string() } else { base_name.trim().to_string() };
    let cleaned_title = title.replace('.', " ").replace('_', " ");
    let year = year_regex.captures(&base_name).and_then(|caps| caps.get(0).map(|m| m.as_str().to_string()));
    (release_type, cleaned_title, year)
}

fn get_seedpool_resolution_id(filename: &str) -> u32 {
    let resolution_regex = Regex::new(r"(?i)(8640p|4320p|2160p|1440p|1080p|1080i|720p|576p|576i|480p|480i)").unwrap();
    if let Some(captures) = resolution_regex.captures(filename) {
        match captures.get(1).map(|m| m.as_str().to_lowercase()) {
            Some(resolution) => match resolution.as_str() {
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
            },
            None => 10,
        }
    } else {
        10 // Default to 10 if no resolution is detected
    }
}

impl Tracker for Seedpool {
    fn requires_screenshots(&self) -> bool { true }
    fn requires_sample(&self) -> bool { true }
    fn requires_tmdb_id(&self) -> bool { true }
    fn requires_remote_path(&self) -> bool { true }
    fn generate_metadata(&self, _: &str) -> Result<HashMap<String, String>, String> {
        Ok(HashMap::from([("category".to_string(), "TV".to_string()), ("original_language".to_string(), "en".to_string()), ("type".to_string(), "WEB".to_string())]))
    }
    fn upload(
        &self,
        torrent_file: &str,
        description: Option<&str>,
        mediainfo: Option<&str>,
        nfo_file: &Option<String>,
        category_id: u32,
        type_id: Option<u32>,
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
        let client = reqwest::blocking::Client::new();
        let release_name = generate_release_name(Path::new(torrent_file).file_stem().unwrap_or_default().to_string_lossy().as_ref());
        let mut form = Form::new()
            .file("torrent", torrent_file)
            .map_err(|e| format!("Failed to attach torrent file: {}", e))?
            .text("name", release_name)
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
        if let Some(season) = season_number {
            form = form.text("season_number", season.to_string());
        }
        if let Some(episode) = episode_number {
            form = form.text("episode_number", episode.to_string());
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