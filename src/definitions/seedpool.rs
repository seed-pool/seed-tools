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
    TvShow = 24,
    LinuxGame = 25,
    BoxSet = 26,
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
            24 => Some(SeedpoolType::TvShow),
            25 => Some(SeedpoolType::LinuxGame),
            26 => Some(SeedpoolType::BoxSet),
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
            SeedpoolType::TvShow => 24,
            SeedpoolType::LinuxGame => 25,
            SeedpoolType::BoxSet => 26,
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
            SeedpoolType::TvShow => "TV Show",
            SeedpoolType::LinuxGame => "Linux Game",
            SeedpoolType::BoxSet => "BoxSet",
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
        }
    }

    pub fn all_types() -> Vec<(u8, &'static str)> {
        vec![
            (1, "Full Disc"), (2, "Remux"), (3, "Encode"), (4, "WEB-DL"), (5, "WEBRip"),
            (6, "HDTV"), (7, "UHD.BluRay"), (8, "BluRay"), (9, "E-Book"), (10, "WEB"),
            (11, "FLAC"), (12, "Foreign"), (13, "MP3"), (14, "Windows"), (15, "NSW Game"),
            (16, "PC Game"), (17, "Other"), (19, "Sports"), (20, "E-Pub"), (21, "Audiobook"),
            (22, "Movie"), (23, "4K Movie"), (24, "TV Show"), (25, "Linux Game"), (26, "BoxSet"),
            (27, "Anime"), (28, "PS4"), (29, "Music Pack"), (30, "FLAC Pack"), (31, "MP3 Pack"),
            (32, "Education"), (33, "Linux"), (34, "macOS"), (35, "Xbox"), (36, "Upscale"),
            (37, "Dubbed"), (38, "3D Print"), (39, "JPTV"), (40, "Comic"), (41, "Magazine"),
            (42, "Newspaper"), (43, "Karaoke"), (44, "Wii"), (45, "NES"),
        ]
    }
}

/// Complete Seedpool torrent classification
#[derive(Debug, Clone)]
pub struct SeedpoolTorrentInfo {
    pub category: SeedpoolCategory,
    pub torrent_type: SeedpoolType,
}

impl SeedpoolTorrentInfo {
    pub fn new(category: SeedpoolCategory, torrent_type: SeedpoolType) -> Self {
        Self { category, torrent_type }
    }

    pub fn category_code(&self) -> u8 {
        self.category.to_code()
    }

    pub fn type_code(&self) -> u8 {
        self.torrent_type.to_code()
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

    let category_code: u8 = arg[0..2].parse()
        .map_err(|_| format!("Invalid category code in argument: {}", &arg[0..2]))?;
    let type_code: u8 = arg[2..4].parse()
        .map_err(|_| format!("Invalid type code in argument: {}", &arg[2..4]))?;

    let category = SeedpoolCategory::from_code(category_code)
        .ok_or_else(|| format!("Unknown Seedpool category code: {:02}", category_code))?;
    let torrent_type = SeedpoolType::from_code(type_code)
        .ok_or_else(|| format!("Unknown Seedpool type code: {:02}", type_code))?;

    Ok(SeedpoolTorrentInfo::new(category, torrent_type))
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
        let line = chunk.iter()
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
        matches!(self.category, SeedpoolCategory::Games | SeedpoolCategory::Retro)
    }
    
    fn is_audio_category(&self) -> bool {
        matches!(self.category, SeedpoolCategory::Music)
    }
    
    fn is_video_category(&self) -> bool {
        matches!(self.category, 
            SeedpoolCategory::Movie | 
            SeedpoolCategory::TvShow | 
            SeedpoolCategory::Movie4K | 
            SeedpoolCategory::Anime | 
            SeedpoolCategory::JPTV
        )
    }
    
    fn is_audiobook_category(&self) -> bool {
        matches!(self.category, SeedpoolCategory::Audiobook)
    }
    
    fn is_hobby_category(&self) -> bool {
        matches!(self.category, SeedpoolCategory::Hobby | SeedpoolCategory::Education)
    }
    
    fn is_sports_category(&self) -> bool {
        matches!(self.category, SeedpoolCategory::Sports)
    }
    
    fn is_application_category(&self) -> bool {
        matches!(self.category, 
            SeedpoolCategory::WindowsApps | 
            SeedpoolCategory::LinuxApps | 
            SeedpoolCategory::MacApps
        )
    }
    
    fn is_other_category(&self) -> bool {
        matches!(self.category, SeedpoolCategory::Other)
    }
}

use reqwest::blocking::Client;
use log::info;
use crate::utils::generate_release_name;
use crate::tracker_mappings::{CategoryMapping, TrackerMappingEngine};
use crate::mapping;

/// Create the complete mapping array for Seedpool
pub fn create_seedpool_mappings() -> TrackerMappingEngine {
    let mappings = vec![
        // Video Category Mappings
        // Movies with specific source types
        mapping!("VideoCategory::Movie", "VideoSourceType::UHDBluRay" => 1, 7),  // Movie category, UHD BluRay type
        mapping!("VideoCategory::Movie", "VideoSourceType::BluRay" => 1, 8),     // Movie category, BluRay type
        mapping!("VideoCategory::Movie", "VideoSourceType::Remux" => 1, 2),      // Movie category, Remux type
        mapping!("VideoCategory::Movie", "VideoSourceType::WebDL" => 1, 4),      // Movie category, WebDL type
        mapping!("VideoCategory::Movie", "VideoSourceType::WebRip" => 1, 5),     // Movie category, WebRip type
        mapping!("VideoCategory::Movie", "VideoSourceType::HDTV" => 1, 6),       // Movie category, HDTV type
        mapping!("VideoCategory::Movie", "VideoSourceType::DVD" => 1, 22),       // Movie category, Movie type (default)
        mapping!("VideoCategory::Movie", "VideoSourceType::Encode" => 1, 3),     // Movie category, Encode type
        mapping!("VideoCategory::Movie" => 1, 22),                               // Movie category, Movie type (default)
        
        // TV Shows with specific source types
        ///Season packs will map to boxset automatically, should we map it some other way?
        mapping!("VideoCategory::TvShow", "VideoSourceType::BluRay" => 2, 8),    // TV Show category, BluRay type
        mapping!("VideoCategory::TvShow", "VideoSourceType::WebDL" => 2, 4),     // TV Show category, WebDL type
        mapping!("VideoCategory::TvShow", "VideoSourceType::WebRip" => 2, 5),    // TV Show category, WebRip type
        mapping!("VideoCategory::TvShow", "VideoSourceType::HDTV" => 2, 6),      // TV Show category, HDTV type
        mapping!("VideoCategory::TvShow", "VideoSourceType::DVD" => 2, 8),       // TV Show category, BluRay type
        mapping!("VideoCategory::TvShow" => 2, 24),                              // TV Show category, TvShow type (default)
        
        
        // Other video categories
        mapping!("VideoCategory::Anime" => 6, 27),                               // Anime category, Anime type
        mapping!("VideoCategory::Sports" => 8, 19),                              // Sports category, Sports type
        mapping!("VideoCategory::Documentary" => 1, 22),                         // Movie category, Movie type
        mapping!("VideoCategory::Concert" => 5, 10),                             // Music category, Web type
        mapping!("VideoCategory::Unknown" => 11, 17),                            // Other category, Other type
        
        // Audio Category Mappings
        // Music with source types
        // Audio will only put flace ext in flac and mp3 in mp3 regardless of these mappings unless it is a PACK
        mapping!("AudioCategory::Album", "AudioSourceType::CD" => 5, 11),        // Music category, FLAC type
        mapping!("AudioCategory::Album", "AudioSourceType::Web" => 5, 10),       // Music category, Web type
        mapping!("AudioCategory::Album", "AudioSourceType::Vinyl" => 5, 11),     // Music category, FLAC type
        mapping!("AudioCategory::Album" => 5, 11),                               // Music category, FLAC type (default)
        
        mapping!("AudioCategory::Single", "AudioSourceType::CD" => 5, 11),       // Music category, FLAC type
        mapping!("AudioCategory::Single", "AudioSourceType::Web" => 5, 13),      // Music category, MP3 type
        mapping!("AudioCategory::Single" => 5, 13),                              // Music category, MP3 type (default)
        
        mapping!("AudioCategory::EP", "AudioSourceType::CD" => 5, 11),           // Music category, FLAC type
        mapping!("AudioCategory::EP", "AudioSourceType::Web" => 5, 13),          // Music category, MP3 type
        mapping!("AudioCategory::EP" => 5, 13),                                  // Music category, MP3 type (default)
        
        mapping!("AudioCategory::Compilation" => 5, 29),                         // Music category, MusicPack type
        mapping!("AudioCategory::Soundtrack" => 5, 10),                          // Music category, Web type
        mapping!("AudioCategory::Live" => 5, 10),                                // Music category, Web type
        mapping!("AudioCategory::Bootleg" => 5, 10),                             // Music category, Web type
        mapping!("AudioCategory::Mix" => 5, 29),                                 // Music category, MusicPack type
        mapping!("AudioCategory::Demo" => 5, 10),                                // Music category, Web type
        mapping!("AudioCategory::Remix" => 5, 29),                               // Music category, MusicPack type
        mapping!("AudioCategory::Classical" => 5, 11),                           // Music category, FLAC type

        // Audiobooks
        mapping!("AudioCategory::Audiobook" => 9, 21),                           // Audiobook category, Audiobook type
        mapping!("AudioCategory::Podcast" => 9, 21),                             // Audiobook category, Audiobook type
        
        // Ebook Category Mappings
        // Filetype routing supersedes this mapping
        mapping!("EbookCategory::Novel" => 7, 20),                               // E-Book category, EPub type
        mapping!("EbookCategory::Comic" => 7, 9),                                // E-Book category, EBook type
        mapping!("EbookCategory::Magazine" => 7, 9),                             // E-Book category, EBook type
        mapping!("EbookCategory::Newspaper" => 7, 9),                            // E-Book category, EBook type
        mapping!("EbookCategory::Technical" => 15, 32),                          // Education category, Education type
        mapping!("EbookCategory::Educational" => 15, 32),                        // Education category, Education type
        mapping!("EbookCategory::Biography" => 7, 20),                           // E-Book category, EPub type
        mapping!("EbookCategory::History" => 7, 20),                             // E-Book category, EPub type
        mapping!("EbookCategory::Science" => 15, 32),                            // Education category, Education type
        mapping!("EbookCategory::Religion" => 7, 9),                             // E-Book category, EBook type
        mapping!("EbookCategory::Cookbook" => 12, 17),                           // Hobby category, Other type
        mapping!("EbookCategory::Travel" => 12, 17),                             // Hobby category, Other type
        mapping!("EbookCategory::Children" => 7, 20),                            // E-Book category, EPub type
        mapping!("EbookCategory::Unknown" => 7, 9),                              // E-Book category, EBook type
        
        // Game Category Mappings
        mapping!("GameCategory::PCGame" => 14, 16),                              // Games category, PCGame type
        mapping!("GameCategory::PS4Game" => 14, 28),                             // Games category, PS4 type
        mapping!("GameCategory::PS5Game" => 14, 28),                             // Games category, PS4 type (no PS5 yet)
        mapping!("GameCategory::XboxGame" => 14, 35),                            // Games category, Xbox type
        mapping!("GameCategory::NintendoSwitch" => 14, 15),                      // Games category, NSWGame type
        mapping!("GameCategory::Mobile" => 14, 17),                              // Games category, Other type
        mapping!("GameCategory::Retro" => 19, 17),                               // Games category, Other type
        mapping!("GameCategory::VR" => 14, 16),                                  // Games category, PCGame type
        mapping!("GameCategory::Unknown" => 14, 17),                             // Games category, Other type
        
        // Hobby Category Mappings
        mapping!("HobbyCategory::Documents" => 12, 17),                          // Hobby category, Other type
        mapping!("HobbyCategory::Images" => 12, 17),                             // Hobby category, Other type
        mapping!("HobbyCategory::CAD3D" => 12, 38),                              // Hobby category, 3D Print Type
        mapping!("HobbyCategory::Archives" => 12, 17),                           // Hobby category, Other type
        mapping!("HobbyCategory::DataFiles" => 12, 17),                          // Hobby category, Other type
        mapping!("HobbyCategory::Fonts" => 12, 17),                              // Hobby category, Other type
        mapping!("HobbyCategory::Collection" => 12, 17),                         // Hobby category, Other type
        mapping!("HobbyCategory::Tutorial" => 15, 32),                           // Education category, Education type
        mapping!("HobbyCategory::Template" => 12, 17),                           // Hobby category, Other type
        mapping!("HobbyCategory::Resource" => 12, 17),                           // Hobby category, Other type
        mapping!("HobbyCategory::Unknown" => 11, 17),                            // Other category, Other type
    ];
    
    TrackerMappingEngine::new(mappings)
}

/// Get the default category and type for unmapped content
pub fn get_seedpool_defaults() -> (u32, Option<u32>) {
    (11, Some(17)) // Other category, Other type
}

/// Create field mappings for Seedpool upload forms
pub fn create_seedpool_field_mapping() -> crate::upload::TrackerFieldMapping {
    let mut mapping = crate::upload::TrackerFieldMapping::new();
    
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
        .add_mapping("resolution", "resolution_id");
    
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
        .add_optional("resolution_id");
    
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
pub fn check_seedpool_dupes(
    name: &str,
    seedpool_api_key: &str,
) -> Result<Option<String>, String> {
    let client = Client::new();

    info!("Checking Seedpool for existing torrent with name: '{}'", name);

    // Use the full input name as the search term
    let search_term = generate_release_name(name);
    info!("Search Term for Seedpool Query: '{}'", search_term);

    let query_url = format!(
        "https://seedpool.org/api/torrents/filter?name={}&perPage=10&sortField=name&sortDirection=asc&api_token={}",
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

    let raw_response = search_response.text().unwrap_or_else(|_| "Failed to read response body".to_string());
    info!("Seedpool API Response: {}", raw_response);

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
                    if let Some(download_link) = attributes.get("download_link").and_then(|d| d.as_str()) {
                        info!("Duplicate found for '{}'. Download link: {}", name, download_link);
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