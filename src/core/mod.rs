// Core module - Essential shared code and types

pub mod config;
pub mod error;
pub mod types;

// Re-export commonly used types
pub use types::{
    AudioCategory,
    AudioFile,
    AudioSourceType,
    AudioType,
    Config,
    DelugeConfig,
    DescriptionComponent,
    EbookCategory,
    EbookFile,
    EbookType,
    FastResumeData,
    GameCategory,
    GameFile,
    GameType,
    GeneralConfig,
    HobbyCategory,
    HobbyFile,
    HobbyType,
    ImageLayout,
    ImgBBConfig,
    // File types
    MediaFile,
    MediaType,
    PathsConfig,
    PreflightCheckResult,
    QbittorrentConfig,
    Screenshots,
    SectionFormat,
    SeedpoolConfig,
    SeedpoolGeneralConfig,
    SeedpoolScreenshots,
    SeedpoolSettings,
    TorrentLeechConfig,
    TorrentLeechGeneralConfig,
    TorrentLeechSettings,
    UploadComponent,
    // Category types
    VideoCategory,
    VideoFile,
    VideoSettings,
    VideoSourceType,
    VideoType,
};

pub use config::load_config;
pub use error::{Result, SeedError};

// Import TorrentInfo from trackers module since it's defined there
pub use crate::trackers::TorrentInfo;
