/// TorrentLeech-specific torrent categories
#[derive(Debug, Clone, PartialEq)]
pub enum TorrentLeechCategory {
    Movies4K = 47,
    MoviesBluRay = 13,
    MoviesDVDRip = 12,
    MoviesWebDL = 37,
    MoviesHD = 14,
    TvShows = 32,
    TvSports = 30,
    TvAnimation = 34,
    Music = 17,
    Games = 42,
    GamesPS = 40,
    GamesXbox = 41,
    GamesNintendo = 39,
    Ebooks = 45,
    AudioBooks = 35,
    Apps = 23,
    Mobile = 46,
    Other = 0,
}

impl TorrentLeechCategory {
    pub fn from_code(code: u32) -> Option<Self> {
        match code {
            47 => Some(TorrentLeechCategory::Movies4K),
            13 => Some(TorrentLeechCategory::MoviesBluRay),
            12 => Some(TorrentLeechCategory::MoviesDVDRip),
            37 => Some(TorrentLeechCategory::MoviesWebDL),
            14 => Some(TorrentLeechCategory::MoviesHD),
            32 => Some(TorrentLeechCategory::TvShows),
            30 => Some(TorrentLeechCategory::TvSports),
            34 => Some(TorrentLeechCategory::TvAnimation),
            17 => Some(TorrentLeechCategory::Music),
            42 => Some(TorrentLeechCategory::Games),
            40 => Some(TorrentLeechCategory::GamesPS),
            41 => Some(TorrentLeechCategory::GamesXbox),
            39 => Some(TorrentLeechCategory::GamesNintendo),
            45 => Some(TorrentLeechCategory::Ebooks),
            35 => Some(TorrentLeechCategory::AudioBooks),
            23 => Some(TorrentLeechCategory::Apps),
            46 => Some(TorrentLeechCategory::Mobile),
            0 => Some(TorrentLeechCategory::Other),
            _ => None,
        }
    }

    pub fn to_code(&self) -> u32 {
        match self {
            TorrentLeechCategory::Movies4K => 47,
            TorrentLeechCategory::MoviesBluRay => 13,
            TorrentLeechCategory::MoviesDVDRip => 12,
            TorrentLeechCategory::MoviesWebDL => 37,
            TorrentLeechCategory::MoviesHD => 14,
            TorrentLeechCategory::TvShows => 32,
            TorrentLeechCategory::TvSports => 30,
            TorrentLeechCategory::TvAnimation => 34,
            TorrentLeechCategory::Music => 17,
            TorrentLeechCategory::Games => 42,
            TorrentLeechCategory::GamesPS => 40,
            TorrentLeechCategory::GamesXbox => 41,
            TorrentLeechCategory::GamesNintendo => 39,
            TorrentLeechCategory::Ebooks => 45,
            TorrentLeechCategory::AudioBooks => 35,
            TorrentLeechCategory::Apps => 23,
            TorrentLeechCategory::Mobile => 46,
            TorrentLeechCategory::Other => 0,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            TorrentLeechCategory::Movies4K => "Movies 4K",
            TorrentLeechCategory::MoviesBluRay => "Movies BluRay",
            TorrentLeechCategory::MoviesDVDRip => "Movies DVDRip",
            TorrentLeechCategory::MoviesWebDL => "Movies Web-DL",
            TorrentLeechCategory::MoviesHD => "Movies HD",
            TorrentLeechCategory::TvShows => "TV Shows",
            TorrentLeechCategory::TvSports => "TV Sports",
            TorrentLeechCategory::TvAnimation => "TV Animation",
            TorrentLeechCategory::Music => "Music",
            TorrentLeechCategory::Games => "Games",
            TorrentLeechCategory::GamesPS => "Games PS",
            TorrentLeechCategory::GamesXbox => "Games Xbox",
            TorrentLeechCategory::GamesNintendo => "Games Nintendo",
            TorrentLeechCategory::Ebooks => "E-Books",
            TorrentLeechCategory::AudioBooks => "Audio Books",
            TorrentLeechCategory::Apps => "Apps",
            TorrentLeechCategory::Mobile => "Mobile",
            TorrentLeechCategory::Other => "Other",
        }
    }
}

use crate::tracker_mappings::{CategoryMapping, TrackerMappingEngine};
use crate::mapping;

/// Create the complete mapping array for TorrentLeech
pub fn create_torrentleech_mappings() -> TrackerMappingEngine {
    let mappings = vec![
        // Video Category Mappings - TorrentLeech only uses category IDs
        // Movies with quality distinctions
        mapping!("VideoCategory::Movie", "VideoSourceType::UHDBluRay" => 47),  // Movies 4K
        mapping!("VideoCategory::Movie", "VideoSourceType::BluRay" => 13),    // Movies BluRay
        mapping!("VideoCategory::Movie", "VideoSourceType::DVD" => 12),       // Movies DVDRip
        mapping!("VideoCategory::Movie", "VideoSourceType::WebDL" => 37),     // Movies Web-DL
        mapping!("VideoCategory::Movie", "VideoSourceType::WebRip" => 37),    // Movies Web-DL
        mapping!("VideoCategory::Movie", "VideoSourceType::HDTV" => 14),      // Movies HD
        mapping!("VideoCategory::Movie", "VideoSourceType::Remux" => 13),     // Movies BluRay
        mapping!("VideoCategory::Movie", "VideoSourceType::Encode" => 14),    // Movies HD
        mapping!("VideoCategory::Movie" => 14),                               // Movies HD (default)
        
        // TV Shows
        mapping!("VideoCategory::TvShow" => 32),                              // TV Shows
        mapping!("VideoCategory::Sports" => 30),                              // TV Sports
        mapping!("VideoCategory::Anime" => 34),                               // TV Animation
        mapping!("VideoCategory::Documentary" => 32),                         // TV Shows
        mapping!("VideoCategory::Concert" => 17),                             // Music
        mapping!("VideoCategory::Unknown" => 0),                              // Other
        
        // Audio Category Mappings
        mapping!("AudioCategory::Album" => 17),                               // Music
        mapping!("AudioCategory::Single" => 17),                              // Music
        mapping!("AudioCategory::EP" => 17),                                  // Music
        mapping!("AudioCategory::Compilation" => 17),                         // Music
        mapping!("AudioCategory::Soundtrack" => 17),                          // Music
        mapping!("AudioCategory::Live" => 17),                                // Music
        mapping!("AudioCategory::Bootleg" => 17),                             // Music
        mapping!("AudioCategory::Mix" => 17),                                 // Music
        mapping!("AudioCategory::Demo" => 17),                                // Music
        mapping!("AudioCategory::Remix" => 17),                               // Music
        mapping!("AudioCategory::Classical" => 17),                           // Music
        mapping!("AudioCategory::Audiobook" => 35),                           // Audio Books
        mapping!("AudioCategory::Podcast" => 35),                             // Audio Books
        
        // Ebook Category Mappings
        mapping!("EbookCategory::Novel" => 45),                               // E-Books
        mapping!("EbookCategory::Comic" => 45),                               // E-Books
        mapping!("EbookCategory::Magazine" => 45),                            // E-Books
        mapping!("EbookCategory::Newspaper" => 45),                           // E-Books
        mapping!("EbookCategory::Technical" => 45),                           // E-Books
        mapping!("EbookCategory::Educational" => 45),                         // E-Books
        mapping!("EbookCategory::Biography" => 45),                           // E-Books
        mapping!("EbookCategory::History" => 45),                             // E-Books
        mapping!("EbookCategory::Science" => 45),                             // E-Books
        mapping!("EbookCategory::Religion" => 45),                            // E-Books
        mapping!("EbookCategory::Cookbook" => 45),                            // E-Books
        mapping!("EbookCategory::Travel" => 45),                              // E-Books
        mapping!("EbookCategory::Children" => 45),                            // E-Books
        mapping!("EbookCategory::Unknown" => 45),                             // E-Books
        
        // Game Category Mappings
        mapping!("GameCategory::PCGame" => 42),                               // Games
        mapping!("GameCategory::PS4Game" => 40),                              // Games PS
        mapping!("GameCategory::PS5Game" => 40),                              // Games PS
        mapping!("GameCategory::XboxGame" => 41),                             // Games Xbox
        mapping!("GameCategory::NintendoSwitch" => 39),                       // Games Nintendo
        mapping!("GameCategory::Mobile" => 46),                               // Mobile
        mapping!("GameCategory::Retro" => 42),                                // Games
        mapping!("GameCategory::VR" => 42),                                   // Games
        mapping!("GameCategory::Unknown" => 42),                              // Games
        
        // Hobby Category Mappings
        mapping!("HobbyCategory::Documents" => 45),                           // E-Books
        mapping!("HobbyCategory::Images" => 0),                               // Other
        mapping!("HobbyCategory::CAD3D" => 0),                                // Other
        mapping!("HobbyCategory::Archives" => 0),                             // Other
        mapping!("HobbyCategory::DataFiles" => 0),                            // Other
        mapping!("HobbyCategory::Fonts" => 0),                                // Other
        mapping!("HobbyCategory::Collection" => 0),                           // Other
        mapping!("HobbyCategory::Tutorial" => 45),                            // E-Books
        mapping!("HobbyCategory::Template" => 0),                             // Other
        mapping!("HobbyCategory::Resource" => 0),                             // Other
        mapping!("HobbyCategory::Unknown" => 0),                              // Other
    ];
    
    TrackerMappingEngine::new(mappings)
}

/// Get the default category for unmapped content
pub fn get_torrentleech_defaults() -> (u32, Option<u32>) {
    (0, None) // Other category, no type field
}

/// Create field mappings for TorrentLeech upload forms
pub fn create_torrentleech_field_mapping() -> crate::upload::TrackerFieldMapping {
    let mut mapping = crate::upload::TrackerFieldMapping::new();
    
    // Field mappings (internal name -> TorrentLeech form field name)
    mapping
        .add_mapping("name", "name")
        .add_mapping("description", "descr")
        .add_mapping("category", "categoryID")
        .add_mapping("torrent", "torrent")
        .add_mapping("nfo", "nfo")
        .add_mapping("imdb", "imdbID")
        .add_mapping("anonymous", "anonymous");
    
    // Required fields for TorrentLeech
    mapping
        .add_required("name")
        .add_required("categoryID")
        .add_required("torrent");
    
    // Optional fields
    mapping
        .add_optional("descr")
        .add_optional("nfo")
        .add_optional("imdbID")
        .add_optional("anonymous");
    
    mapping
}