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

/// Create field mappings for TorrentLeech upload forms
pub fn create_torrentleech_field_mapping() -> crate::processing::upload::TrackerFieldMapping {
    let mut mapping = crate::processing::upload::TrackerFieldMapping::new();

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

/// Get TorrentLeech category code from media classification strings
pub fn get_category_from_media_strings(
    media_category: Option<&str>,
    media_source_type: Option<&str>,
) -> Result<u8, String> {
    match media_category {
        Some(cat_str) if cat_str.starts_with("VideoCategory::") => {
            let cat_name = cat_str.strip_prefix("VideoCategory::").unwrap();
            let source_name = media_source_type
                .and_then(|s| s.strip_prefix("VideoSourceType::"))
                .unwrap_or("");

            match (cat_name, source_name) {
                // Movies are restricted to only FullDisc, Remux, and Encode types
                ("Movie", "FullDisc") => Ok(13),                    // Movies BluRay (FullDisc maps to BluRay)
                ("Movie", "Remux") => Ok(13),                       // Movies BluRay
                ("Movie", "Encode") => Ok(14),                      // Movies HD
                ("Movie", _) => {
                    // For movies, convert all other source types to Encode
                    eprintln!("Info: Converting movie source type '{}' to Encode", source_name);
                    Ok(14) // Movies HD (Encode)
                }
                ("TvShow", _) | ("Documentary", _) => Ok(32),       // TV Shows
                ("Sports", _) => Ok(30),                            // TV Sports
                ("Anime", _) => Ok(34),                             // TV Animation
                ("Concert", _) => Ok(17),                           // Music
                _ => Ok(0),                                         // Other
            }
        }
        Some(cat_str) if cat_str.starts_with("AudioCategory::") => {
            let cat_name = cat_str.strip_prefix("AudioCategory::").unwrap();
            match cat_name {
                "Audiobook" | "Podcast" => Ok(35), // Audio Books
                _ => Ok(17),                       // Music
            }
        }
        Some(cat_str) if cat_str.starts_with("EbookCategory::") => {
            Ok(45) // All ebooks go to E-Books
        }
        Some(cat_str) if cat_str.starts_with("GameCategory::") => {
            let cat_name = cat_str.strip_prefix("GameCategory::").unwrap();
            // All games go to category 3 (Games)
            match cat_name {
                "NintendoSwitch" => {
                    eprintln!("Info: Nintendo Switch game going to Games category (3) with Nintendo type");
                    Ok(3) // Games category
                }
                _ => {
                    eprintln!("Info: Game '{}' going to Games category (3)", cat_name);
                    Ok(3) // Games category for all other games
                }
            }
        }
        Some(cat_str) if cat_str.starts_with("HobbyCategory::") => {
            let cat_name = cat_str.strip_prefix("HobbyCategory::").unwrap();
            match cat_name {
                "Documents" | "Tutorial" => Ok(45), // E-Books
                _ => Ok(0),                         // Other
            }
        }
        _ => Ok(0), // Other
    }
}
