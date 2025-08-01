// Common API traits and structures for all trackers

use crate::core::error::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Common trait for tracker APIs
#[async_trait]
pub trait TrackerApi {
    /// Upload a torrent to the tracker
    async fn upload(&self, upload_data: &UploadData) -> Result<UploadResponse>;

    /// Check for duplicates
    async fn check_duplicate(&self, title: &str) -> Result<bool>;

    /// Get tracker name
    fn name(&self) -> &'static str;

    /// Get tracker configuration
    fn config(&self) -> &TrackerConfig;
}

/// Common upload data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadData {
    pub title: String,
    pub category: String,
    pub type_id: Option<String>,
    pub description: String,
    pub mediainfo: Option<String>,
    pub screenshots: Vec<String>,
    pub torrent_file: Vec<u8>,
    pub nfo: Option<String>,
    pub tmdb_id: Option<u32>,
    pub imdb_id: Option<String>,
    pub tvdb_id: Option<u32>,
    pub igdb_id: Option<u64>,
    pub anonymous: bool,
    // TV show specific fields
    pub resolution_id: Option<String>,
    pub season_number: Option<u32>,
    pub episode_number: Option<u32>,
}

/// Upload response from tracker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadResponse {
    pub success: bool,
    pub torrent_id: Option<u32>,
    pub torrent_url: Option<String>,
    pub error_message: Option<String>,
}

/// Common tracker configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackerConfig {
    pub name: String,
    pub enabled: bool,
    pub api_url: String,
    pub announce_url: String,
    pub api_key: String,
    pub username: String,
    pub passkey: String,
}
