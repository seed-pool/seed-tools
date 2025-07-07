// qBittorrent client implementation

use super::{TorrentClient, TorrentInfo};
use crate::core::QbittorrentConfig;
use log::info;
use reqwest::blocking::{multipart::Form, Client};
use std::path::Path;

pub struct QBittorrentClient {
    config: QbittorrentConfig,
    client: Client,
}

impl QBittorrentClient {
    pub fn new(config: QbittorrentConfig) -> Result<Self, String> {
        let client = Client::builder()
            .cookie_store(true)
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

        let qb_client = Self { config, client };

        // Test login during initialization
        qb_client.login()?;
        Ok(qb_client)
    }

    /// Login to qBittorrent Web UI
    fn login(&self) -> Result<(), String> {
        let login_url = format!("{}/api/v2/auth/login", self.config.webui_url);
        info!("Logging in to qBittorrent at {}...", login_url);

        let login_response = self
            .client
            .post(&login_url)
            .form(&[
                ("username", self.config.username.as_str()),
                ("password", self.config.password.as_str()),
            ])
            .send()
            .map_err(|e| format!("Failed to send login request to qBittorrent: {}", e))?;

        let login_status = login_response.status();
        let login_body = login_response
            .text()
            .map_err(|e| format!("Failed to read login response body: {}", e))?;

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
        Ok(())
    }

    /// Add torrent from file path
    pub fn add_torrent_file(&self, torrent_file: &str) -> Result<(), String> {
        // Ensure we're logged in
        self.login()?;

        if !Path::new(torrent_file).exists() {
            return Err(format!("Torrent file does not exist: {}", torrent_file));
        }

        let mut form = Form::new()
            .file("torrents", torrent_file)
            .map_err(|e| format!("Failed to attach torrent file: {}", e))?
            .text("paused", "false")
            .text("skip_checking", "true");

        if let Some(category) = &self.config.category {
            info!("Using category for qBittorrent: {}", category);
            form = form.text("category", category.clone());
        }

        let add_url = format!("{}/api/v2/torrents/add", self.config.webui_url);
        info!("Adding torrent to qBittorrent at {}...", add_url);

        let upload_response = self
            .client
            .post(&add_url)
            .multipart(form)
            .send()
            .map_err(|e| format!("Failed to send add torrent request to qBittorrent: {}", e))?;

        let status = upload_response.status();
        let response_body = upload_response
            .text()
            .unwrap_or_else(|_| "Failed to read response body".to_string());
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
}

impl TorrentClient for QBittorrentClient {
    fn add_torrent(&self, torrent_data: &[u8], _save_path: &str) -> Result<String, String> {
        // Ensure we're logged in
        self.login()?;

        let mut form = Form::new()
            .part(
                "torrents",
                reqwest::blocking::multipart::Part::bytes(torrent_data.to_vec())
                    .file_name("upload.torrent")
                    .mime_str("application/x-bittorrent")
                    .map_err(|e| format!("Failed to set torrent file mime type: {}", e))?,
            )
            .text("paused", "false")
            .text("skip_checking", "true");

        if let Some(category) = &self.config.category {
            form = form.text("category", category.clone());
        }

        let add_url = format!("{}/api/v2/torrents/add", self.config.webui_url);

        let upload_response = self
            .client
            .post(&add_url)
            .multipart(form)
            .send()
            .map_err(|e| format!("Failed to send add torrent request to qBittorrent: {}", e))?;

        let status = upload_response.status();
        let response_body = upload_response
            .text()
            .unwrap_or_else(|_| "Failed to read response body".to_string());

        if !status.is_success() || response_body.to_lowercase().contains("fail") {
            return Err(format!(
                "Failed to upload torrent to qBittorrent: {}. Response: {}",
                status, response_body
            ));
        }

        // qBittorrent returns "Ok." on success, we'll return a placeholder hash
        Ok("qbittorrent_success".to_string())
    }

    fn get_torrent_info(&self, _hash: &str) -> Result<TorrentInfo, String> {
        // TODO: Implement get torrent info
        unimplemented!("Get torrent info from qBittorrent")
    }

    fn remove_torrent(&self, _hash: &str, _delete_files: bool) -> Result<(), String> {
        // TODO: Implement remove torrent
        unimplemented!("Remove torrent from qBittorrent")
    }

    fn list_torrents(&self) -> Result<Vec<TorrentInfo>, String> {
        // TODO: Implement list torrents
        unimplemented!("List torrents from qBittorrent")
    }
}
