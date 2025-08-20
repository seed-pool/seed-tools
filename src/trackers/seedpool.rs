/// Seedpool-specific torrent categories
#[derive(Debug, Clone, PartialEq)]
pub enum SeedpoolCategory {
    Movie = 1,
    TvShow = 2,
    Music = 5,
    Anime = 6,
    Ebook = 7,
    Sports = 8,
    Audiobook = 9,
    Movie4K = 10,
    Other = 11,
    Hobby = 12,
    Games = 14,
    Education = 15,
    WindowsApps = 16,
    LinuxApps = 17,
    MacApps = 18,
    Retro = 19,
    JPTV = 20,
}

impl SeedpoolCategory {
    pub fn from_code(code: u8) -> Option<Self> {
        match code {
            1 => Some(SeedpoolCategory::Movie),
            2 => Some(SeedpoolCategory::TvShow),
            3 => Some(SeedpoolCategory::Games), // Alternative Games category code
            4 => Some(SeedpoolCategory::Games), // Alternative Games category code
            5 => Some(SeedpoolCategory::Music),
            6 => Some(SeedpoolCategory::Anime),
            7 => Some(SeedpoolCategory::Ebook),
            8 => Some(SeedpoolCategory::Sports),
            9 => Some(SeedpoolCategory::Audiobook),
            10 => Some(SeedpoolCategory::Movie4K),
            11 => Some(SeedpoolCategory::Other),
            12 => Some(SeedpoolCategory::Hobby),
            14 => Some(SeedpoolCategory::Games),
            15 => Some(SeedpoolCategory::Education),
            16 => Some(SeedpoolCategory::WindowsApps),
            17 => Some(SeedpoolCategory::LinuxApps),
            18 => Some(SeedpoolCategory::MacApps),
            19 => Some(SeedpoolCategory::Retro),
            20 => Some(SeedpoolCategory::JPTV),
            _ => None,
        }
    }

    pub fn to_code(&self) -> u8 {
        match self {
            SeedpoolCategory::Movie => 1,
            SeedpoolCategory::TvShow => 2,
            SeedpoolCategory::Music => 5,
            SeedpoolCategory::Anime => 6,
            SeedpoolCategory::Ebook => 7,
            SeedpoolCategory::Sports => 8,
            SeedpoolCategory::Audiobook => 9,
            SeedpoolCategory::Movie4K => 10,
            SeedpoolCategory::Other => 11,
            SeedpoolCategory::Hobby => 12,
            SeedpoolCategory::Games => 14,
            SeedpoolCategory::Education => 15,
            SeedpoolCategory::WindowsApps => 16,
            SeedpoolCategory::LinuxApps => 17,
            SeedpoolCategory::MacApps => 18,
            SeedpoolCategory::Retro => 19,
            SeedpoolCategory::JPTV => 20,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            SeedpoolCategory::Movie => "Movie",
            SeedpoolCategory::TvShow => "TV Show",
            SeedpoolCategory::Music => "Music",
            SeedpoolCategory::Anime => "Anime",
            SeedpoolCategory::Ebook => "E-Book",
            SeedpoolCategory::Sports => "Sports",
            SeedpoolCategory::Audiobook => "Audiobook",
            SeedpoolCategory::Movie4K => "4K Movie",
            SeedpoolCategory::Other => "Other",
            SeedpoolCategory::Hobby => "Hobby",
            SeedpoolCategory::Games => "Games",
            SeedpoolCategory::Education => "Education",
            SeedpoolCategory::WindowsApps => "Windows Apps",
            SeedpoolCategory::LinuxApps => "Linux Apps",
            SeedpoolCategory::MacApps => "Mac Apps",
            SeedpoolCategory::Retro => "Retro",
            SeedpoolCategory::JPTV => "JPTV",
        }
    }

    pub fn all_categories() -> Vec<(u8, &'static str)> {
        vec![
            (1, "Movie"),
            (2, "TV Show"),
            (3, "Games"),
            (4, "NSW Games"),
            (5, "Music"),
            (6, "Anime"),
            (7, "E-Book"),
            (8, "Sports"),
            (9, "Audiobook"),
            (10, "4K Movie"),
            (11, "Other"),
            (12, "Hobby"),
            (14, "Games"),
            (15, "Education"),
            (16, "Windows Apps"),
            (17, "Linux Apps"),
            (18, "Mac Apps"),
            (19, "Retro"),
            (20, "JPTV"),
        ]
    }
}

/// Seedpool-specific torrent types
#[derive(Debug, Clone, PartialEq)]
pub enum SeedpoolType {
    FullDisc = 1,
    Remux = 2,
    Encode = 3,
    WebDL = 4,
    WebRip = 5,
    HDTV = 6,
    UHDBluRay = 7,
    BluRay = 8,
    EBook = 9,
    Web = 10,
    Flac = 11,
    Foreign = 12,
    Mp3 = 13,
    Windows = 14,
    NSWGame = 15,
    PCGame = 16,
    Other = 17,
    Sports = 19,
    EPub = 20,
    Audiobook = 21,
    Movie = 22,
    Movie4K = 23,
    Episode = 24,
    LinuxGame = 25,
    Season = 26,
    Anime = 27,
    PS4 = 28,
    MusicPack = 29,
    FlacPack = 30,
    Mp3Pack = 31,
    Education = 32,
    Linux = 33,
    MacOS = 34,
    Xbox = 35,
    Upscale = 36,
    Dubbed = 37,
    Print3D = 38,
    JPTV = 39,
    Comic = 40,
    Magazine = 41,
    Newspaper = 42,
    Karaoke = 43,
    Wii = 44,
    NES = 45,
    MusicVideo = 55,
}

impl SeedpoolType {
    pub fn from_code(code: u8) -> Option<Self> {
        match code {
            1 => Some(SeedpoolType::FullDisc),
            2 => Some(SeedpoolType::Remux),
            3 => Some(SeedpoolType::Encode),
            4 => Some(SeedpoolType::WebDL),
            5 => Some(SeedpoolType::WebRip),
            6 => Some(SeedpoolType::HDTV),
            7 => Some(SeedpoolType::UHDBluRay),
            8 => Some(SeedpoolType::BluRay),
            9 => Some(SeedpoolType::EBook),
            10 => Some(SeedpoolType::Web),
            11 => Some(SeedpoolType::Flac),
            12 => Some(SeedpoolType::Foreign),
            13 => Some(SeedpoolType::Mp3),
            14 => Some(SeedpoolType::Windows),
            15 => Some(SeedpoolType::NSWGame),
            16 => Some(SeedpoolType::PCGame),
            17 => Some(SeedpoolType::Other),
            19 => Some(SeedpoolType::Sports),
            20 => Some(SeedpoolType::EPub),
            21 => Some(SeedpoolType::Audiobook),
            22 => Some(SeedpoolType::Movie),
            23 => Some(SeedpoolType::Movie4K),
            24 => Some(SeedpoolType::Episode),
            25 => Some(SeedpoolType::LinuxGame),
            26 => Some(SeedpoolType::Season),
            27 => Some(SeedpoolType::Anime),
            28 => Some(SeedpoolType::PS4),
            29 => Some(SeedpoolType::MusicPack),
            30 => Some(SeedpoolType::FlacPack),
            31 => Some(SeedpoolType::Mp3Pack),
            32 => Some(SeedpoolType::Education),
            33 => Some(SeedpoolType::Linux),
            34 => Some(SeedpoolType::MacOS),
            35 => Some(SeedpoolType::Xbox),
            36 => Some(SeedpoolType::Upscale),
            37 => Some(SeedpoolType::Dubbed),
            38 => Some(SeedpoolType::Print3D),
            39 => Some(SeedpoolType::JPTV),
            40 => Some(SeedpoolType::Comic),
            41 => Some(SeedpoolType::Magazine),
            42 => Some(SeedpoolType::Newspaper),
            43 => Some(SeedpoolType::Karaoke),
            44 => Some(SeedpoolType::Wii),
            45 => Some(SeedpoolType::NES),
            55 => Some(SeedpoolType::MusicVideo),
            _ => None,
        }
    }

    pub fn to_code(&self) -> u8 {
        match self {
            SeedpoolType::FullDisc => 1,
            SeedpoolType::Remux => 2,
            SeedpoolType::Encode => 3,
            SeedpoolType::WebDL => 4,
            SeedpoolType::WebRip => 5,
            SeedpoolType::HDTV => 6,
            SeedpoolType::UHDBluRay => 7,
            SeedpoolType::BluRay => 8,
            SeedpoolType::EBook => 9,
            SeedpoolType::Web => 10,
            SeedpoolType::Flac => 11,
            SeedpoolType::Foreign => 12,
            SeedpoolType::Mp3 => 13,
            SeedpoolType::Windows => 14,
            SeedpoolType::NSWGame => 15,
            SeedpoolType::PCGame => 16,
            SeedpoolType::Other => 17,
            SeedpoolType::Sports => 19,
            SeedpoolType::EPub => 20,
            SeedpoolType::Audiobook => 21,
            SeedpoolType::Movie => 22,
            SeedpoolType::Movie4K => 23,
            SeedpoolType::Episode => 24,
            SeedpoolType::LinuxGame => 25,
            SeedpoolType::Season => 26,
            SeedpoolType::Anime => 27,
            SeedpoolType::PS4 => 28,
            SeedpoolType::MusicPack => 29,
            SeedpoolType::FlacPack => 30,
            SeedpoolType::Mp3Pack => 31,
            SeedpoolType::Education => 32,
            SeedpoolType::Linux => 33,
            SeedpoolType::MacOS => 34,
            SeedpoolType::Xbox => 35,
            SeedpoolType::Upscale => 36,
            SeedpoolType::Dubbed => 37,
            SeedpoolType::Print3D => 38,
            SeedpoolType::JPTV => 39,
            SeedpoolType::Comic => 40,
            SeedpoolType::Magazine => 41,
            SeedpoolType::Newspaper => 42,
            SeedpoolType::Karaoke => 43,
            SeedpoolType::Wii => 44,
            SeedpoolType::NES => 45,
            SeedpoolType::MusicVideo => 55,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            SeedpoolType::FullDisc => "Full Disc",
            SeedpoolType::Remux => "Remux",
            SeedpoolType::Encode => "Encode",
            SeedpoolType::WebDL => "WEB-DL",
            SeedpoolType::WebRip => "WEBRip",
            SeedpoolType::HDTV => "HDTV",
            SeedpoolType::UHDBluRay => "UHD.BluRay",
            SeedpoolType::BluRay => "BluRay",
            SeedpoolType::EBook => "E-Book",
            SeedpoolType::Web => "WEB",
            SeedpoolType::Flac => "FLAC",
            SeedpoolType::Foreign => "Foreign",
            SeedpoolType::Mp3 => "MP3",
            SeedpoolType::Windows => "Windows",
            SeedpoolType::NSWGame => "NSW Game",
            SeedpoolType::PCGame => "PC Game",
            SeedpoolType::Other => "Other",
            SeedpoolType::Sports => "Sports",
            SeedpoolType::EPub => "E-Pub",
            SeedpoolType::Audiobook => "Audiobook",
            SeedpoolType::Movie => "Movie",
            SeedpoolType::Movie4K => "4K Movie",
            SeedpoolType::Episode => "Episode",
            SeedpoolType::LinuxGame => "Linux Game",
            SeedpoolType::Season => "Season",
            SeedpoolType::Anime => "Anime",
            SeedpoolType::PS4 => "PS4",
            SeedpoolType::MusicPack => "Music Pack",
            SeedpoolType::FlacPack => "FLAC Pack",
            SeedpoolType::Mp3Pack => "MP3 Pack",
            SeedpoolType::Education => "Education",
            SeedpoolType::Linux => "Linux",
            SeedpoolType::MacOS => "macOS",
            SeedpoolType::Xbox => "Xbox",
            SeedpoolType::Upscale => "Upscale",
            SeedpoolType::Dubbed => "Dubbed",
            SeedpoolType::Print3D => "3D Print",
            SeedpoolType::JPTV => "JPTV",
            SeedpoolType::Comic => "Comic",
            SeedpoolType::Magazine => "Magazine",
            SeedpoolType::Newspaper => "Newspaper",
            SeedpoolType::Karaoke => "Karaoke",
            SeedpoolType::Wii => "Wii",
            SeedpoolType::NES => "NES",
            SeedpoolType::MusicVideo => "Music Video",
        }
    }

    pub fn all_types() -> Vec<(u8, &'static str)> {
        vec![
            (1, "Full Disc"),
            (2, "Remux"),
            (3, "Encode"),
            (4, "WEB-DL"),
            (5, "WEBRip"),
            (6, "HDTV"),
            (7, "UHD.BluRay"),
            (8, "BluRay"),
            (9, "E-Book"),
            (10, "WEB"),
            (11, "FLAC"),
            (12, "Foreign"),
            (13, "MP3"),
            (14, "Windows"),
            (15, "NSW Game"),
            (16, "PC Game"),
            (17, "Other"),
            (19, "Sports"),
            (20, "E-Pub"),
            (21, "Audiobook"),
            (22, "Movie"),
            (23, "4K Movie"),
            (24, "Episode"),
            (25, "Linux Game"),
            (26, "Season"),
            (27, "Anime"),
            (28, "PS4"),
            (29, "Music Pack"),
            (30, "FLAC Pack"),
            (31, "MP3 Pack"),
            (32, "Education"),
            (33, "Linux"),
            (34, "macOS"),
            (35, "Xbox"),
            (36, "Upscale"),
            (37, "Dubbed"),
            (38, "3D Print"),
            (39, "JPTV"),
            (40, "Comic"),
            (41, "Magazine"),
            (42, "Newspaper"),
            (43, "Karaoke"),
            (44, "Wii"),
            (45, "NES"),
            (55, "Music Video"),
        ]
    }
}

/// Complete Seedpool torrent classification
#[derive(Debug, Clone)]
pub struct SeedpoolTorrentInfo {
    pub category: SeedpoolCategory,
    pub torrent_type: SeedpoolType,
    pub original_category_code: Option<u8>,
    pub original_type_code: Option<u8>,
}

impl SeedpoolTorrentInfo {
    pub fn new(category: SeedpoolCategory, torrent_type: SeedpoolType) -> Self {
        Self {
            category,
            torrent_type,
            original_category_code: None,
            original_type_code: None,
        }
    }

    pub fn new_with_codes(
        category: SeedpoolCategory, 
        torrent_type: SeedpoolType,
        original_category_code: u8,
        original_type_code: u8,
    ) -> Self {
        Self {
            category,
            torrent_type,
            original_category_code: Some(original_category_code),
            original_type_code: Some(original_type_code),
        }
    }

    pub fn category_code(&self) -> u8 {
        self.original_category_code.unwrap_or_else(|| self.category.to_code())
    }

    pub fn type_code(&self) -> u8 {
        self.original_type_code.unwrap_or_else(|| self.torrent_type.to_code())
    }

    pub fn description(&self) -> String {
        format!("{} - {}", self.category.name(), self.torrent_type.name())
    }
}

/// Parse a 4-digit Seedpool category/type argument (e.g., "0740")
pub fn parse_seedpool_category_type(arg: &str) -> Result<SeedpoolTorrentInfo, String> {
    if arg.len() != 4 || !arg.chars().all(|c| c.is_digit(10)) {
        return Err(format!(
            "Invalid format for Seedpool category/type. Expected 4 digits (e.g., 0740), got: {}",
            arg
        ));
    }

    let category_code: u8 = arg[0..2]
        .parse()
        .map_err(|_| format!("Invalid category code in argument: {}", &arg[0..2]))?;
    let type_code: u8 = arg[2..4]
        .parse()
        .map_err(|_| format!("Invalid type code in argument: {}", &arg[2..4]))?;

    let category = SeedpoolCategory::from_code(category_code)
        .ok_or_else(|| format!("Unknown Seedpool category code: {:02}", category_code))?;
    let torrent_type = SeedpoolType::from_code(type_code)
        .ok_or_else(|| format!("Unknown Seedpool type code: {:02}", type_code))?;

    Ok(SeedpoolTorrentInfo::new_with_codes(category, torrent_type, category_code, type_code))
}

/// Print all available Seedpool categories and types
pub fn print_seedpool_categories_and_types() {
    println!("Available Seedpool Categories:");
    for (code, name) in SeedpoolCategory::all_categories() {
        println!("  {:02} = {}", code, name);
    }

    println!("\nAvailable Seedpool Types:");
    let types = SeedpoolType::all_types();
    for chunk in types.chunks(5) {
        let line = chunk
            .iter()
            .map(|(code, name)| format!("{:02} = {}", code, name))
            .collect::<Vec<_>>()
            .join(", ");
        println!("  {}", line);
    }

    println!("\nExamples:");
    println!("  -c 0740 = E-Book (07) + Comic (40)");
    println!("  -c 1416 = Games (14) + PC Game (16)");
    println!("  -c 0511 = Music (05) + FLAC (11)");
}

/// Implementation of generic TorrentInfo trait for Seedpool
impl super::TorrentInfo for SeedpoolTorrentInfo {
    fn category_code(&self) -> u8 {
        self.category.to_code()
    }

    fn type_code(&self) -> u8 {
        self.torrent_type.to_code()
    }

    fn description(&self) -> String {
        format!("{} - {}", self.category.name(), self.torrent_type.name())
    }

    fn category_name(&self) -> &'static str {
        self.category.name()
    }

    fn type_name(&self) -> &'static str {
        self.torrent_type.name()
    }

    fn is_ebook_category(&self) -> bool {
        matches!(self.category, SeedpoolCategory::Ebook)
    }

    fn is_game_category(&self) -> bool {
        matches!(
            self.category,
            SeedpoolCategory::Games | SeedpoolCategory::Retro
        )
    }

    fn is_audio_category(&self) -> bool {
        matches!(self.category, SeedpoolCategory::Music)
    }

    fn is_video_category(&self) -> bool {
        // Check if category is a video category
        let is_video_category = matches!(
            self.category,
            SeedpoolCategory::Movie
                | SeedpoolCategory::TvShow
                | SeedpoolCategory::Movie4K
                | SeedpoolCategory::Anime
                | SeedpoolCategory::JPTV
        );
        
        // Also check if type is MusicVideo (which should go through video pipeline)
        let is_music_video_type = matches!(self.torrent_type, SeedpoolType::MusicVideo);
        
        is_video_category || is_music_video_type
    }

    fn is_audiobook_category(&self) -> bool {
        matches!(self.category, SeedpoolCategory::Audiobook)
    }

    fn is_hobby_category(&self) -> bool {
        matches!(
            self.category,
            SeedpoolCategory::Hobby | SeedpoolCategory::Education
        )
    }

    fn is_sports_category(&self) -> bool {
        matches!(self.category, SeedpoolCategory::Sports)
    }

    fn is_application_category(&self) -> bool {
        matches!(
            self.category,
            SeedpoolCategory::WindowsApps | SeedpoolCategory::LinuxApps | SeedpoolCategory::MacApps
        )
    }

    fn is_other_category(&self) -> bool {
        matches!(self.category, SeedpoolCategory::Other)
    }
}

use crate::processing::naming::generate_release_name;
use crate::trackers::common::{
    TrackerApi, TrackerConfig, UploadData as TrackerUploadData, UploadResponse,
};
use async_trait::async_trait;
use log::info;
use reqwest::blocking::Client;

/// Create field mappings for Seedpool upload forms
pub fn create_seedpool_field_mapping() -> crate::processing::upload::TrackerFieldMapping {
    let mut mapping = crate::processing::upload::TrackerFieldMapping::new();

    // Field mappings (internal name -> Seedpool form field name)
    mapping
        .add_mapping("name", "name")
        .add_mapping("description", "description")
        .add_mapping("mediainfo", "mediainfo")
        .add_mapping("category", "category_id")
        .add_mapping("type", "type_id")
        .add_mapping("screenshots", "images")
        .add_mapping("torrent", "torrent")
        .add_mapping("nfo", "nfo")
        .add_mapping("tmdb", "tmdb")
        .add_mapping("imdb", "imdb")
        .add_mapping("tvdb", "tvdb")
        .add_mapping("season", "season_number")
        .add_mapping("episode", "episode_number")
        .add_mapping("resolution", "resolution_id")
        .add_mapping("sticky", "sticky")
        .add_mapping("featured", "featured")
        .add_mapping("internal", "internal");

    // Required fields for Seedpool
    mapping
        .add_required("name")
        .add_required("category_id")
        .add_required("type_id")
        .add_required("torrent");

    // Optional fields
    mapping
        .add_optional("description")
        .add_optional("mediainfo")
        .add_optional("images")
        .add_optional("nfo")
        .add_optional("tmdb")
        .add_optional("imdb")
        .add_optional("tvdb")
        .add_optional("season_number")
        .add_optional("episode_number")
        .add_optional("resolution_id")
        .add_optional("sticky")
        .add_optional("featured")
        .add_optional("internal");

    mapping
}

/// Check for duplicates on Seedpool tracker
///
/// # Arguments
/// * `name` - The release name to check
/// * `seedpool_api_key` - The Seedpool API key
///
/// # Returns
/// * `Ok(Some(download_link))` - If a duplicate is found
/// * `Ok(None)` - If no duplicate is found
/// * `Err(String)` - If an error occurs
pub fn check_seedpool_dupes(name: &str, seedpool_api_key: &str) -> Result<Option<String>, String> {
    let client = Client::new();

    info!(
        "Checking Seedpool for existing torrent with name: '{}'",
        name
    );

    // Load the seedpool config to check for domain override
    let domain = if let Ok(config) =
        crate::utils::load_tracker_config::<crate::core::SeedpoolConfig>("seedpool")
    {
        config
            .settings
            .domain_override
            .unwrap_or_else(|| "seedpool.org".to_string())
    } else {
        "seedpool.org".to_string()
    };

    // Use the full input name as the search term
    let search_term = generate_release_name(name);
    info!("Search Term for Seedpool Query: '{}'", search_term);

    let query_url = format!(
        "https://{}/api/torrents/filter?name={}&perPage=10&sortField=name&sortDirection=asc&api_token={}",
        domain,
        urlencoding::encode(&search_term),
        seedpool_api_key
    );

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

    let raw_response = search_response
        .text()
        .unwrap_or_else(|_| "Failed to read response body".to_string());
    // Seedpool API response logging removed for cleaner output

    let search_results: serde_json::Value = serde_json::from_str(&raw_response)
        .map_err(|e| format!("Failed to parse Seedpool response for '{}': {}", name, e))?;

    let empty_vec = vec![];
    let data = search_results["data"].as_array().unwrap_or(&empty_vec);

    for result in data {
        if let Some(attributes) = result["attributes"].as_object() {
            if let Some(result_title) = attributes.get("name").and_then(|t| t.as_str()) {
                info!("Checking result title: {}", result_title);

                // Check for an exact match with the search term
                if result_title == search_term {
                    if let Some(download_link) =
                        attributes.get("download_link").and_then(|d| d.as_str())
                    {
                        info!(
                            "Duplicate found for '{}'. Download link: {}",
                            name, download_link
                        );
                        return Ok(Some(download_link.to_string()));
                    }
                } else {
                    info!("Skipping result due to mismatched title: {}", result_title);
                }
            }
        }
    }

    info!("No duplicate found for '{}'.", name);
    Ok(None)
}

/// Map resolution string to Seedpool resolution ID
pub fn map_resolution_to_id(resolution: &str) -> Option<String> {
    match resolution.to_uppercase().as_str() {
        "4320P" | "8K" => Some("1".to_string()),
        "2160P" | "4K" | "UHD" => Some("2".to_string()),
        "1080P" => Some("3".to_string()),
        "1080I" => Some("4".to_string()),
        "720P" => Some("5".to_string()),
        "576P" => Some("6".to_string()),
        "576I" => Some("7".to_string()),
        "480P" => Some("8".to_string()),
        "480I" => Some("9".to_string()),
        "OTHER" => Some("10".to_string()),
        _ => Some("11".to_string()), // Unknown
    }
}

/// Create a SeedpoolTorrentInfo from media classification strings
pub fn create_torrent_info_from_media_strings(
    media_category: Option<&str>,
    media_source_type: Option<&str>,
) -> Result<SeedpoolTorrentInfo, String> {
    create_torrent_info_from_media_strings_with_metadata(media_category, media_source_type, None)
}

/// Create a SeedpoolTorrentInfo from media classification strings with optional metadata
pub fn create_torrent_info_from_media_strings_with_metadata(
    media_category: Option<&str>,
    media_source_type: Option<&str>,
    metadata: Option<&serde_json::Value>,
) -> Result<SeedpoolTorrentInfo, String> {
    // Parse category and type from strings like "VideoCategory::Movie" and "VideoSourceType::BluRay"
    let (category, torrent_type) = match media_category {
        Some(cat_str) if cat_str.starts_with("VideoCategory::") => {
            let cat_name = cat_str.strip_prefix("VideoCategory::").unwrap();
            let seedpool_cat = match cat_name {
                "Movie" => SeedpoolCategory::Movie,
                "TvShow" => SeedpoolCategory::TvShow,
                "Anime" => SeedpoolCategory::Anime,
                "Sports" => SeedpoolCategory::Sports,
                "Documentary" => SeedpoolCategory::Movie, // Map to Movie
                "Concert" => SeedpoolCategory::Music,     // Map to Music
                _ => SeedpoolCategory::Other,
            };

            let seedpool_type = if let Some(type_str) = media_source_type {
                if let Some(type_name) = type_str.strip_prefix("VideoSourceType::") {
                    match (cat_name, type_name) {
                        (_, "SeasonPack") => SeedpoolType::Other, // SeasonPack fallback when no source type detected
                        (_, "BoxSet") => SeedpoolType::Other, // BoxSet fallback when no source type detected
                        ("Movie", "FullDisc") => SeedpoolType::FullDisc,
                        ("Movie", "UHDBluRay") => SeedpoolType::UHDBluRay,
                        ("Movie", "BluRay") => SeedpoolType::BluRay,
                        ("Movie", "Remux") => SeedpoolType::Remux,
                        ("Movie", "WebDL") => SeedpoolType::WebDL,
                        ("Movie", "WebRip") => SeedpoolType::WebRip,
                        ("Movie", "HDTV") => SeedpoolType::HDTV,
                        ("Movie", "DVD") => SeedpoolType::BluRay, // Map DVD to BluRay
                        ("Movie", "Encode") => SeedpoolType::Encode,
                        ("Movie", _) => SeedpoolType::Other, // Fallback to type 17

                        ("TvShow", "FullDisc") => SeedpoolType::FullDisc,
                        ("TvShow", "UHDBluRay") => SeedpoolType::UHDBluRay,
                        ("TvShow", "Remux") => SeedpoolType::Remux,
                        ("TvShow", "BluRay") => SeedpoolType::BluRay,
                        ("TvShow", "WebDL") => SeedpoolType::WebDL,
                        ("TvShow", "WebRip") => SeedpoolType::WebRip,
                        ("TvShow", "HDTV") => SeedpoolType::HDTV,
                        ("TvShow", "DVD") => SeedpoolType::BluRay, // Map DVD to BluRay
                        ("TvShow", "Encode") => SeedpoolType::Encode,
                        ("TvShow", _) => SeedpoolType::Other, // All TV shows use fallback type 17 when no source type detected
                        
                        // Handle other categories with source types
                        (_, "BluRay") => SeedpoolType::BluRay,
                        (_, "Remux") => SeedpoolType::Remux,
                        (_, "WebDL") => SeedpoolType::WebDL,
                        (_, "WebRip") => SeedpoolType::WebRip,
                        (_, "HDTV") => SeedpoolType::HDTV,
                        (_, "DVD") => SeedpoolType::BluRay, // Map DVD to BluRay
                        (_, "Encode") => SeedpoolType::Encode,
                        _ => SeedpoolType::Other,
                    }
                } else {
                    SeedpoolType::Other
                }
            } else {
                match cat_name {
                    "Movie" => SeedpoolType::Other, // Fallback to type 17
                    "TvShow" => SeedpoolType::Other, // All TV shows use fallback type 17 when no source type detected
                    "Anime" => SeedpoolType::Anime,
                    "Sports" => SeedpoolType::Sports,
                    _ => SeedpoolType::Other,
                }
            };

            (seedpool_cat, seedpool_type)
        }
        Some(cat_str) if cat_str.starts_with("AudioCategory::") => {
            let cat_name = cat_str.strip_prefix("AudioCategory::").unwrap();
            match cat_name {
                "Audiobook" | "Podcast" => (SeedpoolCategory::Audiobook, SeedpoolType::Audiobook),
                _ => {
                    // Determine format from metadata, source type, or media type
                    let format_type = if let Some(meta) = metadata {
                        // Check audio_format field (from mediainfo)
                        if let Some(format) = meta.get("audio_format").and_then(|f| f.as_str()) {
                            if format.to_lowercase().contains("flac") {
                                SeedpoolType::Flac
                            } else if format.to_lowercase().contains("mp3")
                                || format.to_lowercase().contains("mpeg")
                            {
                                SeedpoolType::Mp3
                            } else {
                                SeedpoolType::Flac // Default to FLAC for other lossless formats
                            }
                        } else if let Some(format) = meta.get("format").and_then(|f| f.as_str()) {
                            // Check format field (from AudioType enum)
                            if format.contains("Mp3") {
                                SeedpoolType::Mp3
                            } else if format.contains("Flac") {
                                SeedpoolType::Flac
                            } else {
                                SeedpoolType::Flac // Default to FLAC
                            }
                        } else {
                            // Try to infer from file extension in the path
                            if let Some(path) = meta.get("input_path").and_then(|p| p.as_str()) {
                                // Check files in the directory for format
                                if let Ok(entries) = std::fs::read_dir(path) {
                                    let mut has_mp3 = false;
                                    let mut has_flac = false;

                                    for entry in entries.flatten() {
                                        if let Some(ext) =
                                            entry.path().extension().and_then(|e| e.to_str())
                                        {
                                            match ext.to_lowercase().as_str() {
                                                "mp3" => has_mp3 = true,
                                                "flac" => has_flac = true,
                                                _ => {}
                                            }
                                        }
                                    }

                                    if has_mp3 && !has_flac {
                                        SeedpoolType::Mp3
                                    } else {
                                        SeedpoolType::Flac // Default to FLAC
                                    }
                                } else {
                                    SeedpoolType::Flac // Default if can't read directory
                                }
                            } else {
                                SeedpoolType::Flac // Default if no path info
                            }
                        }
                    } else {
                        SeedpoolType::Flac // Default if no metadata
                    };
                    (SeedpoolCategory::Music, format_type)
                }
            }
        }
        Some(cat_str) if cat_str.starts_with("EbookCategory::") => {
            let cat_name = cat_str.strip_prefix("EbookCategory::").unwrap();
            match cat_name {
                "Technical" | "Educational" | "Science" => {
                    (SeedpoolCategory::Education, SeedpoolType::Education)
                }
                "Cookbook" | "Travel" => (SeedpoolCategory::Hobby, SeedpoolType::Other),
                "Comic" => (SeedpoolCategory::Ebook, SeedpoolType::EBook),
                _ => (SeedpoolCategory::Ebook, SeedpoolType::EPub), // Default to EPub
            }
        }
        Some(cat_str) if cat_str.starts_with("GameCategory::") => {
            let cat_name = cat_str.strip_prefix("GameCategory::").unwrap();
            match cat_name {
                "Retro" => (SeedpoolCategory::Retro, SeedpoolType::Other),
                name if name.starts_with("Software_") => {
                    // Software with platform info like Software_WINDOWS_SOFTWARE
                    let platform_part = name.strip_prefix("Software_").unwrap_or("");
                    if platform_part.contains("WINDOWS") || platform_part.contains("PC") {
                        (SeedpoolCategory::WindowsApps, SeedpoolType::Windows)
                    } else if platform_part.contains("LINUX") {
                        (SeedpoolCategory::LinuxApps, SeedpoolType::Linux)
                    } else if platform_part.contains("MAC") || platform_part.contains("MACOS") {
                        (SeedpoolCategory::MacApps, SeedpoolType::MacOS)
                    } else {
                        (SeedpoolCategory::WindowsApps, SeedpoolType::Windows) // Default to Windows
                    }
                }
                "Console" => (SeedpoolCategory::Games, SeedpoolType::Other),
                "PC" => (SeedpoolCategory::Games, SeedpoolType::PCGame),
                _ => (SeedpoolCategory::Games, SeedpoolType::PCGame), // Default to PC game
            }
        }
        Some(cat_str) if cat_str.starts_with("HobbyCategory::") => {
            let cat_name = cat_str.strip_prefix("HobbyCategory::").unwrap();
            match cat_name {
                "Tutorial" => (SeedpoolCategory::Education, SeedpoolType::Education),
                "CAD3D" => (SeedpoolCategory::Hobby, SeedpoolType::Print3D),
                _ => (SeedpoolCategory::Hobby, SeedpoolType::Other),
            }
        }
        _ => (SeedpoolCategory::Other, SeedpoolType::Other),
    };

    Ok(SeedpoolTorrentInfo::new(category, torrent_type))
}

/// Seedpool API implementation
pub struct SeedpoolApi {
    config: TrackerConfig,
    client: reqwest::Client,
}

impl SeedpoolApi {
    /// Create a new SeedpoolApi instance
    pub fn new(config: TrackerConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    /// Create SeedpoolApi from SeedpoolConfig
    pub fn from_seedpool_config(seedpool_config: &crate::core::SeedpoolConfig) -> Self {
        let config = TrackerConfig {
            name: "seedpool".to_string(),
            enabled: seedpool_config.general.enabled,
            api_url: seedpool_config.settings.upload_url.clone(),
            announce_url: seedpool_config.settings.announce_url.clone(),
            api_key: seedpool_config.general.api_key.clone(),
            username: seedpool_config.general.username.clone(),
            passkey: seedpool_config.general.passkey.clone(),
        };

        Self::new(config)
    }
}

#[async_trait]
impl TrackerApi for SeedpoolApi {
    async fn upload(
        &self,
        upload_data: &TrackerUploadData,
    ) -> crate::core::error::Result<UploadResponse> {
        use crate::utils::http::extract_torrent_id;
        use reqwest::multipart::Form;

        info!("Uploading torrent to Seedpool: {}", upload_data.title);

        // Build multipart form
        let mut form = Form::new()
            .text("name", upload_data.title.clone())
            .text("category_id", upload_data.category.clone())
            .text("description", upload_data.description.clone())
            .text("anonymous", if upload_data.anonymous { "1" } else { "0" })
            .text(
                "tmdb",
                upload_data
                    .tmdb_id
                    .map(|id| id.to_string())
                    .unwrap_or("0".to_string()),
            )
            .text(
                "imdb",
                upload_data.imdb_id.clone().unwrap_or("0".to_string()),
            )
            .text(
                "tvdb",
                upload_data
                    .tvdb_id
                    .map(|id| id.to_string())
                    .unwrap_or("0".to_string()),
            )
            .text("mal", "0")
            .text("igdb", "0")
            .text("stream", "0")
            .text("sd", "0");

        // Add type_id if present
        if let Some(type_id) = &upload_data.type_id {
            form = form.text("type_id", type_id.clone());
        }

        // Add resolution_id for TV shows
        if let Some(resolution_id) = &upload_data.resolution_id {
            form = form.text("resolution_id", resolution_id.clone());
        }

        // Add season_number for TV shows
        if let Some(season_number) = &upload_data.season_number {
            form = form.text("season_number", season_number.to_string());
        }

        // Add episode_number for TV shows
        if let Some(episode_number) = &upload_data.episode_number {
            form = form.text("episode_number", episode_number.to_string());
        }

        // Add torrent file
        form = form.part(
            "torrent",
            reqwest::multipart::Part::bytes(upload_data.torrent_file.clone())
                .file_name("upload.torrent")
                .mime_str("application/x-bittorrent")?,
        );

        // Add mediainfo if present
        if let Some(mediainfo) = &upload_data.mediainfo {
            form = form.text("mediainfo", mediainfo.clone());
        }

        // Add NFO if present
        if let Some(nfo) = &upload_data.nfo {
            form = form.part(
                "nfo",
                reqwest::multipart::Part::text(nfo.clone()).file_name("release.nfo"),
            );
        }

        // Generate keywords from title
        let keywords = upload_data
            .title
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty() && s.len() > 2)
            .map(|s| s.to_lowercase())
            .collect::<Vec<_>>()
            .join(" ");
        form = form.text("keywords", keywords);

        // Send the upload request
        let response = self
            .client
            .post(&self.config.api_url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .multipart(form)
            .send()
            .await?;

        let status = response.status();
        let response_text = response.text().await?;

        if !status.is_success() {
            return Ok(UploadResponse {
                success: false,
                torrent_id: None,
                torrent_url: None,
                error_message: Some(format!(
                    "Upload failed with status {}: {}",
                    status, response_text
                )),
            });
        }

        // Extract torrent ID from response
        match extract_torrent_id(&response_text) {
            Ok(torrent_id_str) => {
                let torrent_id: u32 = torrent_id_str.parse().map_err(|e| {
                    crate::core::error::SeedError::Parse(format!("Invalid torrent ID: {}", e))
                })?;

                info!(
                    "Successfully uploaded to Seedpool. Torrent ID: {}",
                    torrent_id
                );

                Ok(UploadResponse {
                    success: true,
                    torrent_id: Some(torrent_id),
                    torrent_url: Some(format!("https://seedpool.org/torrents/{}", torrent_id)),
                    error_message: None,
                })
            }
            Err(e) => {
                log::warn!("Upload succeeded but failed to extract torrent ID: {}", e);
                Ok(UploadResponse {
                    success: true,
                    torrent_id: None,
                    torrent_url: None,
                    error_message: Some(
                        "Upload successful but couldn't extract torrent ID".to_string(),
                    ),
                })
            }
        }
    }

    async fn check_duplicate(&self, title: &str) -> crate::core::error::Result<bool> {
        info!("Checking Seedpool for duplicates of: {}", title);

        // Use the existing duplicate checking function
        match check_seedpool_dupes(title, &self.config.api_key) {
            Ok(Some(_download_link)) => Ok(true),
            Ok(None) => Ok(false),
            Err(e) => Err(crate::core::error::SeedError::Other(e)),
        }
    }

    fn name(&self) -> &'static str {
        "seedpool"
    }

    fn config(&self) -> &TrackerConfig {
        &self.config
    }
}
