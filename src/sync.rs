use std::fs;
use std::{path::Path, thread, time::Duration};
use log::{info, error};
// use regex::Regex; // No longer needed after removing check_seedpool
use serde_bencode::de;
use reqwest::blocking::Client;
use serde_json;
// use crate::utils::generate_release_name; // No longer needed after removing check_seedpool
use crate::types::{QbittorrentConfig, FastResumeData}; 


// check_seedpool function has been replaced by ProcessBuilder duplicate checking
// Original function archived in old_functions.txt

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
            error!(
                "Failed to log in to qBittorrent at {}: {}",
                config.webui_url,
                login_response.status()
            );
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
            let torrent_hash = torrent["hash"].as_str().unwrap_or("");
            let default_save_path = torrent["save_path"].as_str().unwrap_or("");

            // Attempt to get the save path from the .fastresume file
            let save_path = match get_save_path_from_fastresume(torrent_hash, &config.fastresumes) {
                Ok(path) => {
                    info!("Save path for '{}' determined from .fastresume: {}", name, path);
                    path
                }
                Err(e) => {
                    error!(
                        "Failed to get save path from .fastresume for '{}': {}. Falling back to default save path.",
                        name, e
                    );
                    default_save_path.to_string()
                }
            };

            // Ensure the save path exists
            if let Err(e) = std::fs::create_dir_all(&save_path) {
                error!("Failed to create save path '{}': {}", save_path, e);
                continue;
            }

            info!("Using save path for '{}': {}", name, save_path);

            info!("Checking for duplicate on Seedpool for '{}'", name);
            // TODO: Replace with ProcessBuilder duplicate checking
            match crate::definitions::seedpool::check_seedpool_dupes(name, seedpool_api_key) {
                Ok(Some(download_link)) => {
                    info!("Found duplicate for '{}'. Adding to qBittorrent.", name);

                    // Add the torrent to qBittorrent with the determined save path
                    let add_torrent_response = client
                        .post(format!("{}/api/v2/torrents/add", config.webui_url))
                        .form(&[
                            ("urls", download_link.as_str()),
                            ("savepath", &save_path),
                            ("category", config.category.as_deref().unwrap_or("")),
                            ("paused", "false"),
                            ("skip_checking", "true"),
                        ])
                        .send()
                        .map_err(|e| format!("Failed to add torrent to qBittorrent: {}", e))?;

                    if !add_torrent_response.status().is_success() {
                        error!(
                            "Failed to add torrent '{}' to qBittorrent: {}",
                            name,
                            add_torrent_response.status()
                        );
                    } else {
                        info!(
                            "Successfully added torrent '{}' to qBittorrent with save path '{}'.",
                            name, save_path
                        );
                    }
                }
                Ok(None) => {
                    info!("No duplicate found for '{}'.", name);
                }
                Err(e) => {
                    error!("Error checking for duplicate for '{}': {}", name, e);
                }
            }

            thread::sleep(Duration::from_secs(3));
        }
    }

    Ok(())
}

fn get_save_path_from_fastresume(torrent_hash: &str, fastresume_dir: &str) -> Result<String, String> {
    let fastresume_path = Path::new(fastresume_dir).join(format!("{}.fastresume", torrent_hash));
    info!("Reading .fastresume file: {}", fastresume_path.display());

    let fastresume_data = fs::read(&fastresume_path)
        .map_err(|e| format!("Failed to read .fastresume file: {}", e))?;

    let data: Result<FastResumeData, _> = de::from_bytes(&fastresume_data);
    
    let (qb_save_path, save_path) = match data {
        Ok(resume_data) => {
            let qb_save_path = resume_data.qbt_save_path
                .map(|bytes| String::from_utf8_lossy(&bytes).to_string());
            let save_path = resume_data.save_path
                .map(|bytes| String::from_utf8_lossy(&bytes).to_string());
            (qb_save_path, save_path)
        }
        Err(_) => {
            // Fallback to simple parsing if structured parsing fails
            (None, None)
        }
    };

    // Use qBt-savePath if available, otherwise fallback to save_path
    qb_save_path
        .or(save_path)
        .ok_or_else(|| "Neither qBt-savePath nor save_path found in .fastresume file".to_string())
}