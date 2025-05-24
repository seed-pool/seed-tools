use std::path::Path;
use log::info;
use reqwest::blocking::multipart::Form;
use seed_tools::utils::{create_torrent, add_torrent_to_all_qbittorrent_instances};
use seed_tools::types::PathsConfig; // Import PathsConfig
use crate::{QbittorrentConfig, SeedpoolConfig, TorrentLeechConfig, DelugeConfig};
use std::collections::HashMap;
use reqwest::blocking::Client;
use serde_json::Value;
use regex::Regex;

#[allow(dead_code)]
pub trait Tracker {
    fn requires_screenshots(&self) -> bool;
    fn requires_sample(&self) -> bool;
    fn requires_tmdb_id(&self) -> bool;
    fn requires_remote_path(&self) -> bool;
    fn upload(
        &self,
        torrent_file: &str,
        release_name: &str,
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
    ) -> Result<(), String>;
    fn generate_metadata(&self, torrent_file: &str) -> Result<HashMap<String, String>, String>;
}

pub fn process_custom_upload(
    input_path: &str,
    category_id: u32,
    type_id: u32,
    qbittorrent_configs: &[QbittorrentConfig],
    deluge_config: &DelugeConfig, // Deluge configuration
    tracker: &str, // Determines which tracker is being used
    seedpool_config: Option<&SeedpoolConfig>,
    torrentleech_config: Option<&TorrentLeechConfig>,
    mkbrr_path: &str,
    paths_config: &PathsConfig, // Add this parameter
) -> Result<(), String> {
    let base_name = Path::new(input_path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    info!(
        "Processing custom upload with category_id={} and type_id={} for tracker={}",
        category_id, type_id, tracker
    );

    // Determine the announce and upload URLs based on the tracker
    let (announce_url, upload_url) = match tracker {
        "seedpool" => {
            let config = seedpool_config.ok_or("Seedpool configuration is missing")?;
            (config.settings.announce_url.clone(), config.settings.upload_url.clone())
        }
        "torrentleech" => {
            let config = torrentleech_config.ok_or("TorrentLeech configuration is missing")?;
            (config.general.announce_url_1.clone(), config.settings.upload_url.clone())
        }
        _ => return Err("Invalid tracker specified".to_string()),
    };

    let torrent_file = create_torrent(
        input_path, // Pass the input path directly as a &str
        "./torrents", // Output directory for torrents
        &announce_url,
        mkbrr_path, // Path to mkbrr binary
        false, // Disable filtering for non-Standard Upload Mode
    )?;

    // Check for an .nfo file
    let nfo_file = if Path::new(input_path).is_file() {
        // If input_path is a file, check for a sibling .nfo file
        let nfo_path = Path::new(input_path).with_extension("nfo");
        if nfo_path.exists() {
            Some(nfo_path.to_string_lossy().to_string())
        } else {
            None
        }
    } else {
        // If input_path is a directory, look for any .nfo file inside it
        std::fs::read_dir(input_path)
            .ok()
            .and_then(|mut entries| {
                entries.find_map(|entry| {
                    let entry = entry.ok()?;
                    let path = entry.path();
                    if path.extension().map(|ext| ext.eq_ignore_ascii_case("nfo")).unwrap_or(false) {
                        Some(path.to_string_lossy().to_string())
                    } else {
                        None
                    }
                })
            })
    };
    // Prepare the upload form
    let client = reqwest::blocking::Client::new();
    let mut form = Form::new()
        .file("torrent", &torrent_file)
        .map_err(|e| format!("Failed to attach torrent file: {}", e))?
        .text("name", base_name)
        .text("category_id", category_id.to_string())
        .text("type_id", type_id.to_string())
        .text("tmdb", "0")
        .text("imdb", "0")
        .text("tvdb", "0")
        .text("anonymous", "0")
        .text("description", "Custom upload")
        .text("mal", "0")
        .text("igdb", "0")
        .text("stream", "0")
        .text("sd", "0");

    if let Some(nfo) = nfo_file {
        form = form.file("nfo", nfo).map_err(|e| format!("Failed to attach NFO file: {}", e))?;
    }
    
    // Send the upload request
    let response = client
        .post(&upload_url)
        .multipart(form)
        .send()
        .map_err(|e| format!("Failed to send upload request: {}", e))?;
    
    let status = response.status();
    let response_text = response.text().unwrap_or_else(|_| "Failed to read response body".to_string());
    info!("Upload response: HTTP {}: {}", status, response_text);
    
    if !status.is_success() {
        return Err(format!(
            "Failed to upload torrent. HTTP Status: {}. Response: {}",
            status, response_text
        ));
    }

    // Inject the torrent into qBittorrent
    add_torrent_to_all_qbittorrent_instances(
        &[torrent_file], // Use the single torrent file wrapped in a slice
        qbittorrent_configs, // Ensure this is passed correctly
        deluge_config, // Pass the DelugeConfig
        input_path, // Pass the input_path argument
        paths_config, // Use paths_config directly
    )?;

    Ok(())
}

pub fn igdb_lookup_id(game_title: &str, client_id: &str, bearer_token: &str) -> Result<Option<u64>, String> {
    let client = Client::new();

    // Step 1: Search for candidate game IDs
    let search_url = "https://api.igdb.com/v4/search";
    let search_body = format!("fields game; search \"{}\"; limit 10;", game_title);

    let search_resp = client
        .post(search_url)
        .header("Client-ID", client_id)
        .header("Authorization", format!("Bearer {}", bearer_token))
        .header("Accept", "application/json")
        .body(search_body)
        .send()
        .map_err(|e| format!("IGDB search request failed: {}", e))?;

    let search_json: serde_json::Value = search_resp.json().map_err(|e| format!("IGDB search response parse failed: {}", e))?;
    let mut game_ids: Vec<u64> = vec![];
    if let Some(arr) = search_json.as_array() {
        for item in arr {
            if let Some(id) = item.get("game").and_then(|id| id.as_u64()) {
                game_ids.push(id);
            }
        }
    }

    // If no results, try again with the last word stripped (if possible)
    if game_ids.is_empty() {
        if let Some(pos) = game_title.trim().rfind(' ') {
            let shorter = &game_title[..pos];
            if !shorter.trim().is_empty() {
                return igdb_lookup_id(shorter.trim(), client_id, bearer_token);
            }
        }
        return Ok(Some(14591)); // Default to 1 if no results and nothing left to strip
    }

    // ...rest of your function unchanged...
    // Step 2: Query /games for details (request more fields for better matching)
    let games_url = "https://api.igdb.com/v4/games";
    let ids_str = game_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(",");
    let games_body = format!(
        "fields id, name, slug, alternative_names.name, first_release_date; where id = ({}); limit 10;",
        ids_str
    );

    let games_resp = client
        .post(games_url)
        .header("Client-ID", client_id)
        .header("Authorization", format!("Bearer {}", bearer_token))
        .header("Accept", "application/json")
        .body(games_body)
        .send()
        .map_err(|e| format!("IGDB games request failed: {}", e))?;

    let games_json: serde_json::Value = games_resp.json().map_err(|e| format!("IGDB games response parse failed: {}", e))?;

    // Handle both array and single-object responses
    let games: Vec<serde_json::Value> = if let Some(arr) = games_json.as_array() {
        arr.clone()
    } else if games_json.is_object() {
        vec![games_json]
    } else {
        vec![]
    };

    // Step 3: Try to find the best match
    let sanitized_query = sanitize_game_title(game_title).to_lowercase();
    let mut best_match: Option<u64> = None;

    for game in &games {
        let id = game.get("id").and_then(|v| v.as_u64());
        let name = game.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let slug = game.get("slug").and_then(|v| v.as_str()).unwrap_or("");
        let alt_names = game.get("alternative_names")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|n| n.get("name").and_then(|n| n.as_str())).collect::<Vec<_>>())
            .unwrap_or_default();

        // 1. Exact match on sanitized name
        if sanitize_game_title(name).to_lowercase() == sanitized_query {
            best_match = id;
            break;
        }
        // 2. Exact match on slug (replace dashes with spaces for comparison)
        if slug.replace("-", " ").to_lowercase() == sanitized_query.replace("-", " ") {
            best_match = id;
            break;
        }
        // 3. Match on any alternative name
        if alt_names.iter().any(|alt| sanitize_game_title(alt).to_lowercase() == sanitized_query) {
            best_match = id;
            break;
        }
    }

    // 4. Fallback to first result
    if best_match.is_none() {
        best_match = games.get(0).and_then(|game| game.get("id").and_then(|v| v.as_u64()));
    }
    Ok(best_match.or(Some(14591)))
}

pub fn process_game_upload(
    input_path: &str,
    category_id: u32,
    type_id: u32,
    qbittorrent_configs: &[QbittorrentConfig],
    deluge_config: &DelugeConfig,
    tracker: &str,
    seedpool_config: Option<&SeedpoolConfig>,
    torrentleech_config: Option<&TorrentLeechConfig>,
    mkbrr_path: &str,
    paths_config: &PathsConfig,
    igdb_client_id: &str,
    igdb_bearer_token: &str,
) -> Result<(), String> {
    use seed_tools::utils::{upload_to_cdn, generate_game_description, download_igdb_screenshots};
    use std::path::Path;

    let base_name = Path::new(input_path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let game_title = sanitize_game_title(&base_name);

    let igdb_id = igdb_lookup_id(&game_title, igdb_client_id, igdb_bearer_token)?
        .map(|id| id.to_string())
        .unwrap_or_else(|| "0".to_string());

    info!("IGDB ID for '{}': {}", game_title, igdb_id);

    // --- IGDB screenshots logic ---
    let mut screenshot_urls = Vec::new();
    if tracker == "seedpool" && igdb_id != "0" && igdb_id != "1" {
        if let Some(seedpool) = seedpool_config {
            let image_path = seedpool.screenshots.image_path.trim_end_matches('/');
            let remote_path = seedpool.screenshots.remote_path.trim_end_matches('/');

            // 1. Get screenshot IDs from IGDB
            let client = reqwest::blocking::Client::new();
            let screenshots_body = format!("fields screenshots; where id = {}; limit 1;", igdb_id);
            let resp = client
                .post("https://api.igdb.com/v4/games")
                .header("Client-ID", igdb_client_id)
                .header("Authorization", format!("Bearer {}", igdb_bearer_token))
                .header("Accept", "application/json")
                .body(screenshots_body)
                .send()
                .map_err(|e| format!("IGDB screenshots request failed: {}", e))?;
            let json: serde_json::Value = resp.json().map_err(|e| format!("IGDB screenshots response parse failed: {}", e))?;
            let screenshot_ids: Vec<u64> = json.as_array()
                .and_then(|arr| arr.get(0))
                .and_then(|game| game.get("screenshots"))
                .and_then(|ss| ss.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_u64()).collect())
                .unwrap_or_default();

            // 2. Get image_ids for those screenshots
            if !screenshot_ids.is_empty() {
                let ids_str = screenshot_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(",");
                let screenshots_body = format!("fields id,image_id; where id = ({});", ids_str);
                let resp = client
                    .post("https://api.igdb.com/v4/screenshots")
                    .header("Client-ID", igdb_client_id)
                    .header("Authorization", format!("Bearer {}", igdb_bearer_token))
                    .header("Accept", "application/json")
                    .body(screenshots_body)
                    .send()
                    .map_err(|e| format!("IGDB screenshots image_id request failed: {}", e))?;
                let json: serde_json::Value = resp.json().map_err(|e| format!("IGDB screenshots image_id response parse failed: {}", e))?;
                let image_ids: Vec<String> = json.as_array()
                    .map(|arr| arr.iter().filter_map(|v| v.get("image_id").and_then(|id| id.as_str()).map(|s| s.to_string())).collect())
                    .unwrap_or_default();

                // 3. Download screenshots, set permissions, upload to CDN, collect CDN URLs
                let safe_base_name = url_safe_filename(&base_name);
                let local_paths = download_igdb_screenshots(&image_ids, &safe_base_name, "./screenshots")?;
                for (i, local_path) in local_paths.iter().enumerate() {
                    let file_name = Path::new(local_path).file_name().unwrap().to_string_lossy();
                    let remote_file = format!("{}/{}", remote_path, file_name);
                    upload_to_cdn(local_path, &remote_file)?;
                    let cdn_url = format!("{}/{}", image_path, file_name);
                    screenshot_urls.push(cdn_url);
                }
            }
        }
    }
    // --- End IGDB screenshots logic ---

    let (announce_url, upload_url) = match tracker {
        "seedpool" => {
            let config = seedpool_config.ok_or("Seedpool configuration is missing")?;
            (config.settings.announce_url.clone(), config.settings.upload_url.clone())
        }
        "torrentleech" => {
            let config = torrentleech_config.ok_or("TorrentLeech configuration is missing")?;
            (config.general.announce_url_1.clone(), config.settings.upload_url.clone())
        }
        _ => return Err("Invalid tracker specified".to_string()),
    };

    let torrent_file = create_torrent(
        input_path,
        "./torrents",
        &announce_url,
        mkbrr_path,
        false,
    )?;

    // Check for an .nfo file
    let nfo_file = if Path::new(input_path).is_file() {
        let nfo_path = Path::new(input_path).with_extension("nfo");
        if nfo_path.exists() {
            Some(nfo_path.to_string_lossy().to_string())
        } else {
            None
        }
    } else {
        std::fs::read_dir(input_path)
            .ok()
            .and_then(|mut entries| {
                entries.find_map(|entry| {
                    let entry = entry.ok()?;
                    let path = entry.path();
                    if path.extension().map(|ext| ext.eq_ignore_ascii_case("nfo")).unwrap_or(false) {
                        Some(path.to_string_lossy().to_string())
                    } else {
                        None
                    }
                })
            })
    };

    // Use the new game description generator
    let description = if !screenshot_urls.is_empty() {
        generate_game_description(
            &screenshot_urls,
            seedpool_config.and_then(|c| Some(c.settings.custom_description.as_str())),
            None, // youtube_trailer_url
            &base_name,
        )
    } else {
        base_name.clone()
    };

    let client = reqwest::blocking::Client::new();
    let mut form = Form::new()
        .file("torrent", &torrent_file)
        .map_err(|e| format!("Failed to attach torrent file: {}", e))?
        .text("name", base_name)
        .text("category_id", category_id.to_string())
        .text("type_id", type_id.to_string())
        .text("tmdb", "0")
        .text("imdb", "0")
        .text("tvdb", "0")
        .text("anonymous", "0")
        .text("description", description)
        .text("mal", "0")
        .text("igdb", igdb_id)
        .text("stream", "0")
        .text("sd", "0");

    if let Some(nfo) = nfo_file {
        form = form.file("nfo", nfo).map_err(|e| format!("Failed to attach NFO file: {}", e))?;
    }

    let response = client
        .post(&upload_url)
        .multipart(form)
        .send()
        .map_err(|e| format!("Failed to send upload request: {}", e))?;

    let status = response.status();
    let response_text = response.text().unwrap_or_else(|_| "Failed to read response body".to_string());
    info!("Upload response: HTTP {}: {}", status, response_text);

    if !status.is_success() {
        return Err(format!(
            "Failed to upload torrent. HTTP Status: {}. Response: {}",
            status, response_text
        ));
    }

    add_torrent_to_all_qbittorrent_instances(
        &[torrent_file],
        qbittorrent_configs,
        deluge_config,
        input_path,
        paths_config,
    )?;

    Ok(())
}

pub fn sanitize_game_title(raw: &str) -> String {
    // Remove extension if present
    let mut name = Regex::new(r"\.[a-z0-9]{2,4}$").unwrap().replace(raw, "").to_string();

    // Always remove everything after the last dash (including the dash)
    if let Some(idx) = name.rfind('-') {
        name = name[..idx].to_string();
    }

    // Remove everything after v1, v2, v3, ... (case-insensitive)
    name = Regex::new(r"(?i)[ _.-]?v\d[\w.]*.*").unwrap().replace(&name, "").to_string();

    // Remove group in brackets (e.g. [GROUP])
    name = Regex::new(r"\[.*?\]$").unwrap().replace(&name, "").to_string();

    // Replace dots, underscores, and multiple spaces with a single space
    name = Regex::new(r"[._]+").unwrap().replace_all(&name, " ").to_string();
    name = Regex::new(r"\s+").unwrap().replace_all(&name, " ").to_string();

    // Remove year (e.g. 2023, 1999)
    name = Regex::new(r"\b(19|20)\d{2}\b").unwrap().replace(&name, "").to_string();

    // Remove common tags (add more as needed)
    name = Regex::new(r"(?i)\b(REPACK|PROPER|MULTI\d+|FULL|NSW|Unlocker|Update|UPDATE|Pack|RELOADED|FLT|GOG|CODEX|SKIDROW|PLAZA|CPY|Razor1911|FitGirl|ElAmigos|DODI|GoldBerg|DOGE|P2P|SteamRip|Switch|XCI|NSP|PC|ISO|DARKSiDERS|Chronos|TiNYiSO|Unleashed|GOG|FIX)\b")
        .unwrap()
        .replace_all(&name, "")
        .to_string();

    // Remove extra spaces again after tag removal
    name = Regex::new(r"\s+").unwrap().replace_all(&name, " ").to_string();

    // Trim whitespace
    name.trim().to_string()
}

fn url_safe_filename(name: &str) -> String {
    use regex::Regex;
    // Replace spaces and consecutive whitespace with underscores
    let name = Regex::new(r"\s+").unwrap().replace_all(name, "_");
    // Remove any character that is not alphanumeric, underscore, dash, or dot
    let name = Regex::new(r"[^A-Za-z0-9_\-\.]").unwrap().replace_all(&name, "");
    name.to_string()
}