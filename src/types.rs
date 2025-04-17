use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize)]
pub struct PathsConfig {
    pub torrent_dir: String,
    pub screenshots_dir: String,
    pub ffmpeg: String,
    pub ffprobe: String,
    pub mktorrent: String,
}

#[derive(Deserialize)]
pub struct QbittorrentConfig {
    pub webui_url: String,
    pub username: String,
    pub password: String,
    pub category: Option<String>,
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
    pub passkey: String,
    pub api_key: String,
}

#[derive(Deserialize)]
pub struct SeedpoolScreenshots {
    pub remote_path: String,
    pub image_path: String,
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