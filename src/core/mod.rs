// Core module - Essential shared code and types

pub mod types;
pub mod config;
pub mod error;

// Re-export commonly used types
pub use types::{
    Config, GeneralConfig, PathsConfig, QbittorrentConfig, DelugeConfig, ImgBBConfig,
    MediaType, VideoType, AudioType, EbookType, GameType, HobbyType,
    PreflightCheckResult, VideoSettings, FastResumeData,
    SeedpoolConfig, SeedpoolGeneralConfig, SeedpoolSettings, SeedpoolScreenshots,
    TorrentLeechConfig, TorrentLeechGeneralConfig, TorrentLeechSettings,
    ImageLayout, SectionFormat, DescriptionComponent, UploadComponent,
    CDNPaths,
    // File types
    MediaFile, VideoFile, AudioFile, EbookFile, GameFile, HobbyFile,
    // Category types
    VideoCategory, AudioCategory, EbookCategory, GameCategory, HobbyCategory,
    VideoSourceType, AudioSourceType,
};

pub use config::load_config;
pub use error::{SeedError, Result};

// Import TorrentInfo from trackers module since it's defined there
pub use crate::trackers::TorrentInfo;