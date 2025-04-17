use std::{path::Path, thread, time::Duration};
use log::{info, error};
use regex::Regex;
use reqwest::blocking::Client;
use serde_json;
use seed_tools::utils::generate_release_name;
use seed_tools::types::QbittorrentConfig;

pub fn check_seedpool(
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

pub fn sync_qbittorrent(configs: &[QbittorrentConfig], seedpool_api_key: &str) -> Result<(), String> {
    for config in configs {
        let client = Client::new();

        info!("Logging in to qBittorrent at {}...", config.webui_url);
        let login_response = client
            .post(format!("{}/api/v2/auth/login", config.webui_url))
            .form(&[
                ("username", config.username.as_str()),
                ("password", config.password.as_str()),
            ])
            .send()
            .map_err(|e| format!("Failed to log in to qBittorrent: {}", e))?;

        if !login_response.status().is_success() {
            error!("Failed to log in to qBittorrent at {}: {}", config.webui_url, login_response.status());
            continue;
        }
        info!("Logged in to qBittorrent at {} successfully.", config.webui_url);

        let torrents_response = client
            .get(format!("{}/api/v2/torrents/info", config.webui_url))
            .send()
            .map_err(|e| format!("Failed to fetch torrents info: {}", e))?;

        if !torrents_response.status().is_success() {
            return Err(format!(
                "Failed to fetch torrents info: {}",
                torrents_response.status()
            ));
        }

        let torrents: Vec<serde_json::Value> = torrents_response
            .json()
            .map_err(|e| format!("Failed to parse torrents info: {}", e))?;

        let completed_torrents: Vec<&serde_json::Value> = torrents
            .iter()
            .filter(|torrent| torrent["progress"].as_f64().unwrap_or(0.0) == 1.0)
            .collect();

        info!("Completed Torrents:");
        for torrent in &completed_torrents {
            let name = torrent["name"].as_str().unwrap_or("Unknown");
            let save_path = torrent["save_path"].as_str().unwrap_or("");
            let is_folder = Path::new(save_path).is_dir();
            let torrent_file = format!("/tmp/{}.torrent", name);

            info!("Checking for duplicate on Seedpool for '{}'", name);
            match check_seedpool(name, seedpool_api_key) {
                Ok(Some(download_link)) => {
                    info!("Found duplicate for '{}'. Skipping upload.", name);
                }
                Ok(None) => {
                    info!("No duplicate found for '{}'.", name);
                }
                Err(e) => {
                    error!("Error checking for duplicate for '{}': {}", name, e);
                }
            }

            thread::sleep(Duration::from_secs(1));
        }
    }

    Ok(())
}