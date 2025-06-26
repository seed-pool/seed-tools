use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Supported video file types
#[derive(Debug, Clone, PartialEq)]
pub enum VideoType {
    Mp4,
    Mkv,
    Avi,
    Mov,
    Wmv,
    Flv,
    Webm,
    M4v,
    Ts,
    Mpg,
    Mpeg,
}

impl VideoType {
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "mp4" => Some(VideoType::Mp4),
            "mkv" => Some(VideoType::Mkv),
            "avi" => Some(VideoType::Avi),
            "mov" => Some(VideoType::Mov),
            "wmv" => Some(VideoType::Wmv),
            "flv" => Some(VideoType::Flv),
            "webm" => Some(VideoType::Webm),
            "m4v" => Some(VideoType::M4v),
            "ts" => Some(VideoType::Ts),
            "mpg" | "mpeg" => Some(VideoType::Mpg),
            _ => None,
        }
    }
    
    pub fn all_extensions() -> Vec<&'static str> {
        vec!["mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v", "ts", "mpg", "mpeg"]
    }
}

/// Video content categories
#[derive(Debug, Clone, PartialEq)]
pub enum VideoCategory {
    Movie,
    TvShow,
    Anime,
    Sports,
    Documentary,
    Concert,
    Unknown,
}

/// Video source types based on common scene naming conventions
#[derive(Debug, Clone, PartialEq)]
pub enum VideoSourceType {
    // Disc-based sources
    BluRay,
    UHDBluRay,
    DVD,
    Remux,
    FullDisc,
    
    // Web sources
    WebDL,
    WebRip,
    
    // TV sources
    HDTV,
    PDTV,
    SDTV,
    
    // Other sources
    Encode,
    Upscale,
    
    Unknown,
}

/// Supported audio file types
#[derive(Debug, Clone, PartialEq)]
pub enum AudioType {
    Mp3,
    Flac,
    Wav,
    Aac,
    Ogg,
    M4a,
    Wma,
    Aiff,
    Ape,
    Opus,
}

impl AudioType {
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "mp3" => Some(AudioType::Mp3),
            "flac" => Some(AudioType::Flac),
            "wav" => Some(AudioType::Wav),
            "aac" => Some(AudioType::Aac),
            "ogg" => Some(AudioType::Ogg),
            "m4a" => Some(AudioType::M4a),
            "wma" => Some(AudioType::Wma),
            "aiff" => Some(AudioType::Aiff),
            "ape" => Some(AudioType::Ape),
            "opus" => Some(AudioType::Opus),
            _ => None,
        }
    }

    pub fn is_lossless(&self) -> bool {
        matches!(self, AudioType::Flac | AudioType::Wav | AudioType::Aiff | AudioType::Ape)
    }
    
    pub fn all_extensions() -> Vec<&'static str> {
        vec!["mp3", "flac", "wav", "aac", "ogg", "m4a", "wma", "aiff", "ape", "opus"]
    }
}

/// Audio content categories
#[derive(Debug, Clone, PartialEq)]
pub enum AudioCategory {
    Album,           // Standard music album
    Single,          // Single track release
    EP,              // Extended Play
    Compilation,     // Various artists compilation
    Soundtrack,      // Movie/TV/Game soundtrack
    Live,            // Live performance/concert
    Bootleg,         // Unofficial recording
    Podcast,         // Podcast episode(s)
    Audiobook,       // Audiobook content
    Mix,             // DJ mix/mixtape
    Demo,            // Demo recording
    Remix,           // Remix album/collection
    Classical,       // Classical music
    Unknown,
}

/// Audio source/quality types
#[derive(Debug, Clone, PartialEq)]
pub enum AudioSourceType {
    CD,              // CD rip
    Vinyl,           // Vinyl rip
    Web,             // Web download (iTunes, Amazon, etc.)
    FM,              // FM radio recording
    DAB,             // Digital radio
    Cassette,        // Cassette tape
    Stream,          // Stream rip
    SBD,             // Soundboard recording
    AUD,             // Audience recording
    Studio,          // Studio recording
    Remaster,        // Remastered version
    Unknown,
}

/// Supported ebook file types
#[derive(Debug, Clone, PartialEq)]
pub enum EbookType {
    Epub,
    Pdf,
    Cbz,
    Cbr,
    Mobi,
    Azw,
    Azw3,
    Lit,
    Pdb,
}

/// Ebook content categories
#[derive(Debug, Clone, PartialEq)]
pub enum EbookCategory {
    Novel,
    Comic,
    Magazine,
    Newspaper,
    Technical,
    Educational,
    Biography,
    History,
    Science,
    Religion,
    Cookbook,
    Travel,
    Children,
    Unknown,
}

impl EbookType {
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "epub" => Some(EbookType::Epub),
            "pdf" => Some(EbookType::Pdf),
            "cbz" => Some(EbookType::Cbz),
            "cbr" => Some(EbookType::Cbr),
            "mobi" => Some(EbookType::Mobi),
            "azw" => Some(EbookType::Azw),
            "azw3" => Some(EbookType::Azw3),
            "lit" => Some(EbookType::Lit),
            "pdb" => Some(EbookType::Pdb),
            _ => None,
        }
    }

    pub fn is_comic(&self) -> bool {
        matches!(self, EbookType::Cbz | EbookType::Cbr)
    }

    pub fn needs_renaming(&self) -> bool {
        matches!(self, EbookType::Epub)
    }
    
    pub fn all_extensions() -> Vec<&'static str> {
        vec!["epub", "pdf", "cbz", "cbr", "mobi", "azw", "azw3", "lit", "pdb"]
    }
}

/// Game content categories
#[derive(Debug, Clone, PartialEq)]
pub enum GameCategory {
    PCGame,
    PS4Game,
    PS5Game,
    XboxGame,
    NintendoSwitch,
    Mobile,
    Retro,
    VR,
    Unknown,
}

/// Supported game file types and platforms
#[derive(Debug, Clone, PartialEq)]
pub enum GameType {
    // Archive formats
    Zip,
    Rar,
    SevenZ,
    Tar,
    TarGz,
    
    // Executable formats
    Exe,
    Msi,
    Dmg,
    Pkg,
    Deb,
    Rpm,
    AppImage,
    
    // Console ROMs
    Iso,
    Cso,
    Nsp,
    Xci,
    Cia,
    
    // Platform specific
    Directory, // For games extracted to folders
}

impl GameType {
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "zip" => Some(GameType::Zip),
            "rar" => Some(GameType::Rar),
            "7z" => Some(GameType::SevenZ),
            "tar" => Some(GameType::Tar),
            "tar.gz" | "tgz" => Some(GameType::TarGz),
            "exe" => Some(GameType::Exe),
            "msi" => Some(GameType::Msi),
            "dmg" => Some(GameType::Dmg),
            "pkg" => Some(GameType::Pkg),
            "deb" => Some(GameType::Deb),
            "rpm" => Some(GameType::Rpm),
            "appimage" => Some(GameType::AppImage),
            "iso" => Some(GameType::Iso),
            "cso" => Some(GameType::Cso),
            "nsp" => Some(GameType::Nsp),
            "xci" => Some(GameType::Xci),
            "cia" => Some(GameType::Cia),
            _ => None,
        }
    }

    pub fn is_archive(&self) -> bool {
        matches!(self, GameType::Zip | GameType::Rar | GameType::SevenZ | GameType::Tar | GameType::TarGz)
    }

    pub fn is_executable(&self) -> bool {
        matches!(self, GameType::Exe | GameType::Msi | GameType::Dmg | GameType::Pkg | 
                GameType::Deb | GameType::Rpm | GameType::AppImage)
    }
    
    pub fn all_extensions() -> Vec<&'static str> {
        vec![
            "zip", "rar", "7z", "tar", "tar.gz", "tgz",
            "exe", "msi", "dmg", "pkg", "deb", "rpm", "appimage",
            "iso", "cso", "nsp", "xci", "cia"
        ]
    }
}

/// Supported hobby/miscellaneous file types
#[derive(Debug, Clone, PartialEq)]
pub enum HobbyType {
    // Documents
    Doc,
    Docx,
    Txt,
    Rtf,
    
    // Images
    Jpg,
    Png,
    Gif,
    Bmp,
    Tiff,
    Svg,
    
    // CAD/3D files
    Dwg,
    Dxf,
    Stl,
    Obj,
    Ply,
    
    // Archives
    Zip,
    Rar,
    SevenZ,
    
    // Data files
    Csv,
    Json,
    Xml,
    Sql,
    
    // Fonts
    Ttf,
    Otf,
    Woff,
    
    Directory, // For collections in folders
}

impl HobbyType {
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "doc" => Some(HobbyType::Doc),
            "docx" => Some(HobbyType::Docx),
            "txt" => Some(HobbyType::Txt),
            "rtf" => Some(HobbyType::Rtf),
            "jpg" | "jpeg" => Some(HobbyType::Jpg),
            "png" => Some(HobbyType::Png),
            "gif" => Some(HobbyType::Gif),
            "bmp" => Some(HobbyType::Bmp),
            "tiff" | "tif" => Some(HobbyType::Tiff),
            "svg" => Some(HobbyType::Svg),
            "dwg" => Some(HobbyType::Dwg),
            "dxf" => Some(HobbyType::Dxf),
            "stl" => Some(HobbyType::Stl),
            "obj" => Some(HobbyType::Obj),
            "ply" => Some(HobbyType::Ply),
            "zip" => Some(HobbyType::Zip),
            "rar" => Some(HobbyType::Rar),
            "7z" => Some(HobbyType::SevenZ),
            "csv" => Some(HobbyType::Csv),
            "json" => Some(HobbyType::Json),
            "xml" => Some(HobbyType::Xml),
            "sql" => Some(HobbyType::Sql),
            "ttf" => Some(HobbyType::Ttf),
            "otf" => Some(HobbyType::Otf),
            "woff" => Some(HobbyType::Woff),
            _ => None,
        }
    }

    pub fn is_image(&self) -> bool {
        matches!(self, HobbyType::Jpg | HobbyType::Png | HobbyType::Gif | 
                HobbyType::Bmp | HobbyType::Tiff | HobbyType::Svg)
    }

    pub fn is_document(&self) -> bool {
        matches!(self, HobbyType::Doc | HobbyType::Docx | HobbyType::Txt | HobbyType::Rtf)
    }
    
    pub fn all_extensions() -> Vec<&'static str> {
        vec![
            "doc", "docx", "txt", "rtf",
            "jpg", "jpeg", "png", "gif", "bmp", "tiff", "tif", "svg",
            "dwg", "dxf", "stl", "obj", "ply",
            "zip", "rar", "7z",
            "csv", "json", "xml", "sql",
            "ttf", "otf", "woff"
        ]
    }
}

/// Hobby content categories
#[derive(Debug, Clone, PartialEq)]
pub enum HobbyCategory {
    Documents,
    Images,
    CAD3D,
    Archives,
    DataFiles,
    Fonts,
    Collection,
    Tutorial,
    Template,
    Resource,
    Unknown,
}

/// Unified media type enum
#[derive(Debug, Clone, PartialEq)]
pub enum MediaType {
    Video(VideoType),
    Audio(AudioType),
    Ebook(EbookType),
    Game(GameType),
    Hobby(HobbyType),
}


impl MediaType {
    pub fn from_extension(ext: &str) -> Option<Self> {
        if let Some(video_type) = VideoType::from_extension(ext) {
            Some(MediaType::Video(video_type))
        } else if let Some(audio_type) = AudioType::from_extension(ext) {
            Some(MediaType::Audio(audio_type))
        } else if let Some(ebook_type) = EbookType::from_extension(ext) {
            Some(MediaType::Ebook(ebook_type))
        } else if let Some(game_type) = GameType::from_extension(ext) {
            Some(MediaType::Game(game_type))
        } else if let Some(hobby_type) = HobbyType::from_extension(ext) {
            Some(MediaType::Hobby(hobby_type))
        } else {
            None
        }
    }

    pub fn category(&self) -> &'static str {
        match self {
            MediaType::Video(_) => "video",
            MediaType::Audio(_) => "audio", 
            MediaType::Ebook(_) => "ebook",
            MediaType::Game(_) => "game",
            MediaType::Hobby(_) => "hobby",
        }
    }
}

/// Represents a media file with its type
#[derive(Debug, Clone)]
pub struct MediaFile {
    pub path: PathBuf,
    pub media_type: MediaType,
}

/// Legacy ebook file struct for backwards compatibility
#[derive(Debug, Clone)]
pub struct EbookFile {
    pub path: PathBuf,
    pub ebook_type: EbookType,
}

/// Audio file representation
#[derive(Debug, Clone)]
pub struct AudioFile {
    pub path: PathBuf,
    pub audio_type: AudioType,
}

/// Video file representation  
#[derive(Debug, Clone)]
pub struct VideoFile {
    pub path: PathBuf,
    pub video_type: VideoType,
}

/// Game file representation
#[derive(Debug, Clone)]
pub struct GameFile {
    pub path: PathBuf,
    pub game_type: GameType,
}

/// Hobby file representation
#[derive(Debug, Clone)]
pub struct HobbyFile {
    pub path: PathBuf,
    pub hobby_type: HobbyType,
}

/// Fast resume data for qBittorrent
#[derive(Deserialize)]
pub struct FastResumeData {
    #[serde(rename = "qBt-savePath")]
    pub qbt_save_path: Option<Vec<u8>>,
    pub save_path: Option<Vec<u8>>,
}

#[derive(Deserialize, Clone)]
pub struct GeneralConfig {
    pub tmdb_api_key: String,
    pub youtube_api_key: Option<String>,
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

#[derive(Deserialize, Clone)]
pub struct PathsConfig {
    pub torrent_dir: String,
    pub screenshots_dir: String,
    pub ffmpeg: String,
    pub ffprobe: String,
    pub mkbrr: String,
    pub mediainfo: String,
}

#[derive(Deserialize, Clone)]
pub struct QbittorrentConfig {
    pub webui_url: String,
    pub username: String,
    pub password: String,
    pub category: Option<String>,
    pub default_save_path: String,
    pub executable: Option<String>,
    pub fastresumes: String,
}

#[derive(Deserialize, Clone)]
pub struct DelugeConfig {
    pub webui_url: String,
    pub daemon_port: u16,
    pub username: String,
    pub password: String,
    pub label: Option<String>,
    pub default_save_path: String,
}

#[derive(Deserialize, Clone)]
pub struct SeedpoolSettings {
    pub stripshit_from_videos: bool,
    pub announce_url: String,
    pub upload_url: String,
    pub custom_description: String,
    #[serde(default = "default_true")]
    pub dupe_checks: bool,
    // Upload component settings
    #[serde(default = "default_true")]
    pub enable_mediainfo: bool,
    #[serde(default = "default_true")]
    pub enable_screenshots: bool,
    #[serde(default = "default_true")]
    pub enable_sample: bool,
    #[serde(default = "default_true")]
    pub enable_nfo: bool,
    #[serde(default = "default_true")]
    pub enable_tmdb: bool,
    #[serde(default = "default_false")]
    pub enable_torrent_creation: bool,
    #[serde(default = "default_duplicate_check_setting")]
    pub enable_duplicate_check: bool,
    #[serde(default = "default_screenshot_count")]
    pub screenshot_count: usize,
}

/// Helper function to provide default value of true for serde
fn default_true() -> bool {
    true
}

/// Helper function to provide default value of false for serde
fn default_false() -> bool {
    false
}

/// Helper function for duplicate check setting (uses dupe_checks field)
fn default_duplicate_check_setting() -> bool {
    true // Default to true, but will be overridden by dupe_checks if present
}

/// Helper function for default screenshot count
fn default_screenshot_count() -> usize {
    4
}

#[derive(Deserialize, Clone)]
pub struct TorrentLeechSettings {
    pub stripshit_from_videos: bool,
    pub tl_key: String,
    pub upload_url: String,
    pub custom_description: String,
    // Upload component settings
    #[serde(default = "default_true")]
    pub enable_mediainfo: bool,
    #[serde(default = "default_false")]
    pub enable_screenshots: bool,
    #[serde(default = "default_false")]
    pub enable_sample: bool,
    #[serde(default = "default_true")]
    pub enable_nfo: bool,
    #[serde(default = "default_true")]
    pub enable_tmdb: bool,
    #[serde(default = "default_false")]
    pub enable_torrent_creation: bool,
    #[serde(default = "default_true")]
    pub enable_duplicate_check: bool,
    #[serde(default = "default_screenshot_count")]
    pub screenshot_count: usize,
}

#[derive(Deserialize, Clone)]
pub struct TorrentLeechConfig {
    pub general: TorrentLeechGeneralConfig,
    pub settings: TorrentLeechSettings,
    pub categories: HashMap<String, u32>,
}

#[derive(Deserialize, Clone)]
pub struct TorrentLeechGeneralConfig {
    pub enabled: bool,
    pub announce_url_1: String,
    pub announce_url_2: String,
}

#[derive(Deserialize, Clone)]
pub struct SeedpoolConfig {
    pub general: SeedpoolGeneralConfig,
    pub settings: SeedpoolSettings,
    pub screenshots: SeedpoolScreenshots,
}

#[derive(Deserialize, Clone)]
pub struct SeedpoolGeneralConfig {
    pub enabled: bool,
    pub username: String,
    pub passkey: String,
    pub api_key: String,
}

#[derive(Deserialize, Clone)]
pub struct SeedpoolScreenshots {
    pub remote_path: String,
    pub image_path: String,
    pub imgbb_api_key: Option<String>,
}

#[derive(Deserialize, Clone)]
pub struct Config {
    pub general: GeneralConfig,
    pub paths: PathsConfig,
    pub qbittorrent: Vec<QbittorrentConfig>,
    pub deluge: DelugeConfig,
    pub imgbb: Option<ImgBBConfig>, // Add this field
}

impl Config {
    /// Get binary paths from a config instance
    /// Returns (ffmpeg, ffprobe, mkbrr, mediainfo)
    pub fn get_binary_paths(config: &Config) -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
        (
            std::path::PathBuf::from(&config.paths.ffmpeg),
            std::path::PathBuf::from(&config.paths.ffprobe),
            std::path::PathBuf::from(&config.paths.mkbrr),
            std::path::PathBuf::from(&config.paths.mediainfo),
        )
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            general: GeneralConfig {
                tmdb_api_key: String::new(),
                youtube_api_key: None,
                igdb_client_id: String::new(),
                igdb_bearer_token: String::new(),
            },
            paths: PathsConfig {
                torrent_dir: String::new(),
                screenshots_dir: String::new(),
                ffmpeg: String::new(),
                ffprobe: String::new(),
                mkbrr: String::new(),
                mediainfo: String::new(),
            },
            qbittorrent: Vec::new(),
            deluge: DelugeConfig {
                webui_url: String::new(),
                daemon_port: 0,
                username: String::new(),
                password: String::new(),
                label: None,
                default_save_path: String::new(),
            },
            imgbb: None,
        }
    }
}

#[derive(Deserialize, Clone)]
pub struct ImgBBConfig {
    pub imgbb_api_key: String,
}


/// Archive types supported by the extraction system
#[derive(Debug, Clone, PartialEq)]
pub enum ArchiveType {
    Zip,
    Rar,
    SevenZ,
    Tar,
    TarGz,
    TarBz2,
    TarXz,
    Gz,
    Bz2,
    Xz,
}

/// Layout options for images in descriptions
#[derive(Debug, Clone, PartialEq)]
pub enum ImageLayout {
    Grid2x2,     // For video screenshots (2x2 grid)
    TwoColumn,   // For ebooks/comics (2 column layout)
    SingleColumn, // Single column of images
    Gallery,     // Gallery style layout
}

/// Format options for custom sections
#[derive(Debug, Clone)]
pub enum SectionFormat {
    Plain,
    Quoted,
    Spoiler,
    Colored { color: String },
}

/// Components that can be added to a description
#[derive(Debug, Clone)]
pub enum DescriptionComponent {
    Title { 
        text: String, 
        size: u8, 
        color: String 
    },
    Author { 
        name: String, 
        color: String 
    },
    Images { 
        urls: Vec<String>, 
        layout: ImageLayout, 
        width: u32 
    },
    Synopsis { 
        text: String 
    },
    Sample { 
        url: String, 
        filename: String 
    },
    Trailer { 
        url: String, 
        platform: String 
    },
    CustomSection { 
        title: String, 
        content: String, 
        format: SectionFormat 
    },
    Table { 
        rows: Vec<Vec<String>> 
    },
    Quote { 
        content: String 
    },
    Spoiler { 
        title: String, 
        content: String 
    },
    Raw { 
        content: String 
    },
}

/// Components that can be added to an upload
#[derive(Debug, Clone)]
pub enum UploadComponent {
    NfoData { path: String, content: Vec<u8> },
    Mediainfo(String),
    Screenshots(Vec<String>),
    Thumbnails(Vec<String>),
    Sample { url: String, filename: String },
    TorrentPath(String),
    ReleaseName(String),
    Description(String),
    CoverImage(String),
    DuplicateCheckResults(Vec<(String, String)>), // [(tracker, download_link)]
    TmdbData { 
        tmdb_id: u32,
        imdb_id: Option<String>,
        tvdb_id: Option<u32>,
        title: String,
        year: Option<String>,
    },
    Metadata(HashMap<String, String>), // Flexible metadata storage
    Trailer { url: String, platform: String }, // YouTube/Vimeo trailer URL
}

impl ArchiveType {
    /// Detect archive type from file extension
    pub fn from_extension(ext: &str) -> Option<ArchiveType> {
        match ext.to_lowercase().as_str() {
            "zip" => Some(ArchiveType::Zip),
            "rar" => Some(ArchiveType::Rar),
            "7z" => Some(ArchiveType::SevenZ),
            "tar" => Some(ArchiveType::Tar),
            "gz" => {
                // Check if it's tar.gz
                Some(ArchiveType::Gz)
            },
            "bz2" => Some(ArchiveType::Bz2),
            "xz" => Some(ArchiveType::Xz),
            _ => None,
        }
    }
    
    /// Check if path is tar.gz, tar.bz2, or tar.xz
    pub fn from_path(path: &Path) -> Option<ArchiveType> {
        let filename = path.file_name()?.to_str()?.to_lowercase();
        if filename.ends_with(".tar.gz") || filename.ends_with(".tgz") {
            Some(ArchiveType::TarGz)
        } else if filename.ends_with(".tar.bz2") || filename.ends_with(".tbz2") {
            Some(ArchiveType::TarBz2)
        } else if filename.ends_with(".tar.xz") || filename.ends_with(".txz") {
            Some(ArchiveType::TarXz)
        } else {
            path.extension()?.to_str().and_then(ArchiveType::from_extension)
        }
    }
    
    pub fn as_str(&self) -> &'static str {
        match self {
            ArchiveType::Zip => "ZIP",
            ArchiveType::Rar => "RAR",
            ArchiveType::SevenZ => "7Z",
            ArchiveType::Tar => "TAR",
            ArchiveType::TarGz => "TAR.GZ",
            ArchiveType::TarBz2 => "TAR.BZ2",
            ArchiveType::TarXz => "TAR.XZ",
            ArchiveType::Gz => "GZ",
            ArchiveType::Bz2 => "BZ2",
            ArchiveType::Xz => "XZ",
        }
    }
}

/// High-level content categories for processing
#[derive(Debug, Clone, PartialEq)]
pub enum ContentCategory {
    Video,
    Audio,
    Ebook,
    Game,
    Audiobook,
    Application,
    Hobby,
    Sports,
    Education,
}

/// Specific content types for detailed classification
#[derive(Debug, Clone, PartialEq)]
pub enum ContentType {
    // Video types
    Movie,
    Movie4K,
    MovieRemux,
    MovieWeb,
    TvShow,
    Anime,
    
    // Audio types
    MusicFlac,
    MusicMp3,
    Audiobook,
    
    // E-book types
    Ebook,
    Comic,
    Magazine,
    
    // Game types
    PCGame,
    NSWGame,
    PS4Game,
    
    // Application types
    WindowsApp,
    LinuxApp,
    MacApp,
    
    // Other types
    Sports,
    Educational,
    Hobby,
    Mixed,
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