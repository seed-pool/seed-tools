use reqwest::blocking::multipart::Form;
use std::collections::HashMap;
use std::path::Path;
use crate::{Config, Client, SeedpoolConfig, Tracker};
use seed_tools::utils::{
    generate_release_name, find_video_files, create_torrent, generate_mediainfo, generate_sample,
    generate_screenshots, fetch_tmdb_id, fetch_external_ids, generate_description,
    add_torrent_to_all_qbittorrent_instances,
};
use regex::Regex;
use log::info;

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
    log::debug!("Processing release for input_path: {}", input_path);

    // determine release type and title
    let (mut release_type, title, year, season_number, mut episode_number) =
        determine_release_type_and_title(input_path);
    let base_name = Path::new(input_path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    // check for duplicates
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

    // adjust episode number if none
    if episode_number.is_none() {
        log::warn!("Episode number is None. Adjusting to 0.");
        episode_number = Some(0);
    }

    // determine category and type ids
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

    // fetch tmdb id and find video files
    let tmdb_id = fetch_tmdb_id(&title, year, &config.general.tmdb_api_key, &release_type)?;
    let (video_files, nfo_file) = find_video_files(input_path, &config.paths, &seedpool_config.settings)?;
    if video_files.is_empty() {
        return Err("No valid video files detected.".to_string());
    }

    // create torrent and generate metadata
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
        &seedpool_config.screenshots.image_path,
        &ffmpeg_path.to_string_lossy(),
        &base_name,
    )?;
    let (screenshots, thumbnails) = generate_screenshots(
        &video_files[0],
        &config.paths.screenshots_dir,
        &ffmpeg_path.to_string_lossy(),
        &ffprobe_path.to_string_lossy(),
        &seedpool_config.screenshots.remote_path,
        &seedpool_config.screenshots.image_path,
        &base_name,
    )?;
    let (imdb_id, tvdb_id) = fetch_external_ids(tmdb_id, &release_type, &config.general.tmdb_api_key)
        .unwrap_or((None, None));
    let resolution_id = get_seedpool_resolution_id(input_path);

    // generate description
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

    // upload to seedpool
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

    // add torrent to clients
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

    let normalized_name = generate_release_name(name);
    info!("Normalized Name for Seedpool Query: '{}'", normalized_name);

    let season_episode_regex = Regex::new(r"S(\d{2})E(\d{2})").unwrap();
    let season_episode = season_episode_regex.captures(name).map(|caps| {
        (
            caps.get(1).unwrap().as_str().parse::<u32>().unwrap_or(0),
            caps.get(2).unwrap().as_str().parse::<u32>().unwrap_or(0),
        )
    });
    if let Some((season, episode)) = &season_episode {
        info!("Detected Season/Episode: S{}E{}", season, episode);
    }

    let mut query_url = format!(
        "https://seedpool.org/api/torrents/filter?name={}&perPage=10&sortField=name&sortDirection=asc&api_token={}",
        urlencoding::encode(&normalized_name),
        seedpool_api_key
    );

    if let Some((season, episode)) = &season_episode {
        query_url = format!(
            "{}&seasonNumber={}&episodeNumber={}",
            query_url, season, episode
        );
    }

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
            if let Some(title) = attributes.get("name").and_then(|t| t.as_str()) {
                info!("Checking result title: {}", title);

                if let Some((season, episode)) = &season_episode {
                    if !title.contains(&format!("S{:02}E{:02}", season, episode)) {
                        info!("Skipping result due to mismatched season/episode: {}", title);
                        continue;
                    }
                }

                if let Some(download_link) = attributes.get("download_link").and_then(|d| d.as_str()) {
                    info!("Duplicate found for '{}'. Download link: {}", name, download_link);
                    return Ok(Some(download_link.to_string()));
                }
            }
        }
    }

    info!("No duplicate found for '{}'.", name);
    Ok(None)
}