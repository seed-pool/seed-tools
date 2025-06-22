use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

/// Supported ebook file types
#[derive(Debug, Clone)]
pub enum EbookType {
    Epub,
    Pdf,
    Cbz,
    Cbr,
}

impl EbookType {
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "epub" => Some(EbookType::Epub),
            "pdf" => Some(EbookType::Pdf),
            "cbz" => Some(EbookType::Cbz),
            "cbr" => Some(EbookType::Cbr),
            _ => None,
        }
    }

    pub fn is_comic(&self) -> bool {
        matches!(self, EbookType::Cbz | EbookType::Cbr)
    }

    pub fn needs_renaming(&self) -> bool {
        matches!(self, EbookType::Epub)
    }
}

/// Represents an ebook file with its type
#[derive(Debug, Clone)]
pub struct EbookFile {
    pub path: PathBuf,
    pub ebook_type: EbookType,
}

/// Fast resume data for qBittorrent
#[derive(Deserialize)]
pub struct FastResumeData {
    #[serde(rename = "qBt-savePath")]
    pub qbt_save_path: Option<Vec<u8>>,
    pub save_path: Option<Vec<u8>>,
}

#[derive(Deserialize)]
pub struct GeneralConfig {
    pub tmdb_api_key: String,
    pub igdb_client_id: String,
    pub igdb_bearer_token: String,
}

pub struct PreflightCheckResult {
    pub release_name: String,
    pub generated_release_name: String,
    pub dupe_check: String,
    pub tmdb_id: u32,
    pub imdb_id: Option<String>,
    pub tvdb_id: Option<u32>,
    pub excluded_files: String,
    pub album_cover: String,
    pub audio_languages: Vec<String>,
    pub release_type: String,
    pub season_number: Option<u32>,
    pub episode_number: Option<u32>,
}

#[derive(Deserialize)]
pub struct PathsConfig {
    pub torrent_dir: String,
    pub screenshots_dir: String,
    pub ffmpeg: String,
    pub ffprobe: String,
    pub mkbrr: String,
    pub mediainfo: String,
}

#[derive(Deserialize)]
pub struct QbittorrentConfig {
    pub webui_url: String,
    pub username: String,
    pub password: String,
    pub category: Option<String>,
    pub default_save_path: String,
    pub executable: Option<String>,
    pub fastresumes: String,
}

#[derive(Deserialize)]
pub struct DelugeConfig {
    pub webui_url: String,
    pub daemon_port: u16,
    pub username: String,
    pub password: String,
    pub label: Option<String>,
    pub default_save_path: String,
}

#[derive(Deserialize)]
pub struct SeedpoolSettings {
    pub stripshit_from_videos: bool,
    pub announce_url: String,
    pub upload_url: String,
    pub custom_description: String,
}

#[derive(Deserialize)]
pub struct TorrentLeechSettings {
    pub stripshit_from_videos: bool,
    pub tl_key: String,
    pub upload_url: String,
    pub custom_description: String,
}

#[derive(Deserialize)]
pub struct TorrentLeechConfig {
    pub general: TorrentLeechGeneralConfig,
    pub settings: TorrentLeechSettings,
    pub categories: HashMap<String, u32>,
}

#[derive(Deserialize)]
pub struct TorrentLeechGeneralConfig {
    pub enabled: bool,
    pub announce_url_1: String,
    pub announce_url_2: String,
}

#[derive(Deserialize)]
pub struct SeedpoolConfig {
    pub general: SeedpoolGeneralConfig,
    pub settings: SeedpoolSettings,
    pub screenshots: SeedpoolScreenshots,
}

#[derive(Deserialize)]
pub struct SeedpoolGeneralConfig {
    pub enabled: bool,
    pub username: String,
    pub passkey: String,
    pub api_key: String,
}

#[derive(Deserialize)]
pub struct SeedpoolScreenshots {
    pub remote_path: String,
    pub image_path: String,
}

#[derive(Deserialize)]
pub struct Config {
    pub general: GeneralConfig,
    pub paths: PathsConfig,
    pub qbittorrent: Vec<QbittorrentConfig>,
    pub deluge: DelugeConfig,
    pub imgbb: Option<ImgBBConfig>, // Add this field
}

#[derive(Deserialize)]
pub struct ImgBBConfig {
    pub imgbb_api_key: String,
}

pub trait VideoSettings {
    fn stripshit_from_videos(&self) -> bool;
}

impl VideoSettings for SeedpoolSettings {
    fn stripshit_from_videos(&self) -> bool {
        self.stripshit_from_videos
    }
}

impl VideoSettings for TorrentLeechSettings {
    fn stripshit_from_videos(&self) -> bool {
        self.stripshit_from_videos
    }
}