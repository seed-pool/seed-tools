use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use log::{info, error};
use reqwest::blocking::{multipart::Form, Client};
use reqwest::blocking::ClientBuilder;
use reqwest::cookie::Jar;
use serde_json::{Value, json};

use crate::types::{QbittorrentConfig, DelugeConfig, PathsConfig};
use crate::naming::generate_release_name;

/// Create a torrent file using mkbrr
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
            "[X]*,*sample*,*proof*,*screens*,*screenshots*,*.txt,*.jpg,*.jpeg,*.png,*.nfo,*.srr,*.doc,*.sfv,*.r??",
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

pub fn add_torrent_to_qbittorrent(
    torrent_file: &str,
    config: &QbittorrentConfig,
    _input_path: &str,
    _is_folder: bool,
    _paths_config: &PathsConfig,
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
    _input_path: &str,
    _is_folder: bool,
    _paths_config: &PathsConfig,
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

/// Add torrents to all qBittorrent and Deluge instances
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
            if let Some(_executable) = &config.executable {
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