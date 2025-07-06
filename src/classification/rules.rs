// Classification rules for different media types

use serde_json::Value as JsonValue;
use regex::Regex;
use crate::core::{VideoCategory, VideoSourceType, AudioCategory, AudioSourceType, EbookCategory, HobbyCategory};

/// Classification rules for video content
pub fn classify_video_rules(metadata: &JsonValue) -> Option<String> {
    // Extract filename from metadata - try multiple fields
    let filename = if let Some(f) = metadata.get("filename").and_then(|f| f.as_str()) {
        f
    } else if let Some(t) = metadata.get("title").and_then(|t| t.as_str()) {
        t
    } else if let Some(p) = metadata.get("input_path").and_then(|p| p.as_str()) {
        std::path::Path::new(p).file_name()?.to_str()?
    } else {
        return None;
    };
    
    // Initialize regex patterns for TV show detection
    let season_episode_regex = Regex::new(r"(?i)S(\d{1,2})E(\d{1,3})").unwrap();
    let season_only_regex = Regex::new(r"(?i)S(\d{1,2})").unwrap();
    let episode_only_regex = Regex::new(r"(?i)\bE(\d{1,4})\b").unwrap();
    let boxset_regex = Regex::new(r"(?i)\b(boxset|complete|collection|season\s*\d+.*complete)\b").unwrap();
    let year_regex = Regex::new(r"\b(19|20)\d{2}\b").unwrap();
    let full_date_regex = Regex::new(r"\b((19|20)\d{2})[.\-](0[1-9]|1[0-2])[.\-](0[1-9]|[12][0-9]|3[01])\b").unwrap();
    
    // Content type patterns
    let anime_regex = Regex::new(r"(?i)\b(anime|dubbed|subbed|jpn|japanese|[Ss]ub|[Dd]ub|naruto|one\.piece|attack\.on\.titan|bleach|dragon\.ball|demon\.slayer|jujutsu\.kaisen|my\.hero\.academia|boku\.no\.hero|death\.note|hunter\.x\.hunter|fullmetal\.alchemist|sword\.art\.online|tokyo\.ghoul|steins\.gate|evangelion|cowboy\.bebop|one\.punch\.man|mob\.psycho|chainsaw\.man|spy\.x\.family|vinland\.saga|haikyuu|fairy\.tail|black\.clover|boruto|shippuden|kimetsu\.no\.yaiba)\b").unwrap();
    let sports_regex = Regex::new(r"(?i)\b(nba|nfl|nhl|mlb|uefa|fifa|premier\.league|bundesliga|la\.liga|serie\.a|ligue\.1|championship|tournament|vs\.|boxing|mma|ufc|wwe|aew|f1|formula\.1|formula\.one|olympics?|world\.cup|super\.bowl|wrestlemania|summerslam|grand\.prix|tennis|wimbledon|golf|pga|cricket|rugby)\b").unwrap();
    let documentary_regex = Regex::new(r"(?i)\b(documentary|docu|national\.geographic|discovery|history|nature|wildlife|science|biography|bio)\b").unwrap();
    let concert_regex = Regex::new(r"(?i)\b(concert|live\.at|tour|festival|acoustic|unplugged|live\.from)\b").unwrap();
    
    // Check for ISO/disc files
    let iso_regex = Regex::new(r"(?i)\.iso$").unwrap();
    let bluray_regex = Regex::new(r"(?i)\b(blu.?ray|bd|m2ts)\b").unwrap();
    let dvd_regex = Regex::new(r"(?i)\b(dvd|dvdrip)\b").unwrap();
    
    // Determine category based on patterns
    let category = if season_episode_regex.is_match(filename) || 
                      episode_only_regex.is_match(filename) || 
                      season_only_regex.is_match(filename) || 
                      boxset_regex.is_match(filename) ||
                      full_date_regex.is_match(filename) {
        // TV Show patterns take priority
        if anime_regex.is_match(filename) {
            VideoCategory::Anime
        } else {
            VideoCategory::TvShow
        }
    } else if year_regex.is_match(filename) {
        // Year pattern suggests movie
        VideoCategory::Movie
    } else if iso_regex.is_match(filename) && (
        filename.to_lowercase().contains("trilogy") ||
        filename.to_lowercase().contains("collection") ||
        filename.to_lowercase().contains("saga") ||
        bluray_regex.is_match(filename) ||
        dvd_regex.is_match(filename)
    ) {
        // ISO files with movie indicators
        VideoCategory::Movie
    } else if anime_regex.is_match(filename) {
        VideoCategory::Anime
    } else if documentary_regex.is_match(filename) {
        VideoCategory::Documentary
    } else if concert_regex.is_match(filename) {
        VideoCategory::Concert
    } else if sports_regex.is_match(filename) {
        VideoCategory::Sports
    } else {
        // Check if metadata already has category information
        if let Some(existing_category) = metadata.get("category").and_then(|c| c.as_str()) {
            match existing_category {
                "Movie" => VideoCategory::Movie,
                "TvShow" => VideoCategory::TvShow,
                "Anime" => VideoCategory::Anime,
                "Sports" => VideoCategory::Sports,
                "Documentary" => VideoCategory::Documentary,
                "Concert" => VideoCategory::Concert,
                _ => VideoCategory::Unknown,
            }
        } else {
            VideoCategory::Unknown
        }
    };
    
    Some(format!("VideoCategory::{:?}", category))
}

/// Determine video source type from metadata
pub fn classify_video_source_type(metadata: &JsonValue) -> Option<String> {
    let filename = if let Some(f) = metadata.get("filename").and_then(|f| f.as_str()) {
        f
    } else if let Some(t) = metadata.get("title").and_then(|t| t.as_str()) {
        t
    } else if let Some(p) = metadata.get("input_path").and_then(|p| p.as_str()) {
        std::path::Path::new(p).file_name()?.to_str()?
    } else {
        return None;
    };
    
    // Check if it's a boxset or season pack
    let is_boxset = metadata.get("is_boxset")
        .and_then(|b| b.as_bool())
        .unwrap_or(false);
    
    // Source type patterns - order matters for proper detection
    let iso_regex = Regex::new(r"(?i)\.iso$").unwrap();
    let full_disc_regex = Regex::new(r"(?i)\b(full\.?disc|complete\.?disc|bdmv|disc\.?image)\b").unwrap();
    let uhd_bluray_regex = Regex::new(r"(?i)\b(uhd\.?blu.?ray|4k\.?blu.?ray)\b").unwrap();
    let bluray_regex = Regex::new(r"(?i)\b(blu.?ray|bd|m2ts)\b").unwrap();
    let dvd_regex = Regex::new(r"(?i)\b(dvd|dvdrip)\b").unwrap();
    let remux_regex = Regex::new(r"(?i)\b(remux)\b").unwrap();
    let web_dl_regex = Regex::new(r"(?i)\b(web[\.\-]?dl|webdl|amzn|nf|hmax|dsnp|atvp|hulu|pcok|pmtp)\b").unwrap();
    let web_rip_regex = Regex::new(r"(?i)\b(web[\.\-]?rip|webrip)\b").unwrap();
    let hdtv_regex = Regex::new(r"(?i)\b(hdtv)\b").unwrap();
    let pdtv_regex = Regex::new(r"(?i)\b(pdtv)\b").unwrap();
    let sdtv_regex = Regex::new(r"(?i)\b(sdtv)\b").unwrap();
    let encode_regex = Regex::new(r"(?i)\b(encode|x264|x265|h264|h265|hevc|xvid|divx)\b").unwrap();
    let upscale_regex = Regex::new(r"(?i)\b(upscale|upscaled|ai.?upscale)\b").unwrap();
    
    let source_type = if is_boxset {
        VideoSourceType::SeasonPack
    } else if iso_regex.is_match(filename) || full_disc_regex.is_match(filename) {
        VideoSourceType::FullDisc
    } else if uhd_bluray_regex.is_match(filename) {
        VideoSourceType::UHDBluRay
    } else if bluray_regex.is_match(filename) {
        VideoSourceType::BluRay
    } else if dvd_regex.is_match(filename) {
        VideoSourceType::DVD
    } else if remux_regex.is_match(filename) {
        VideoSourceType::Remux
    } else if web_dl_regex.is_match(filename) {
        VideoSourceType::WebDL
    } else if web_rip_regex.is_match(filename) {
        VideoSourceType::WebRip
    } else if hdtv_regex.is_match(filename) {
        VideoSourceType::HDTV
    } else if pdtv_regex.is_match(filename) {
        VideoSourceType::PDTV
    } else if sdtv_regex.is_match(filename) {
        VideoSourceType::SDTV
    } else if upscale_regex.is_match(filename) {
        VideoSourceType::Upscale
    } else if encode_regex.is_match(filename) {
        VideoSourceType::Encode
    } else {
        VideoSourceType::Unknown
    };
    
    Some(format!("VideoSourceType::{:?}", source_type))
}

/// Classification rules for audio content
pub fn classify_audio_rules(metadata: &JsonValue) -> Option<String> {
    // Extract filename and directory information
    let filename = metadata.get("filename")
        .or_else(|| metadata.get("title"))
        .and_then(|f| f.as_str())
        .unwrap_or("");
    
    let parent_dir = metadata.get("parent_dir")
        .and_then(|p| p.as_str())
        .unwrap_or("");
    
    let grandparent_dir = metadata.get("grandparent_dir")
        .and_then(|g| g.as_str())
        .unwrap_or("");
    
    // Combine all levels for pattern matching
    let combined_path = format!("{} {} {}", filename, parent_dir, grandparent_dir);
    
    // Check for Various Artists
    let is_various_artists = metadata.get("is_various_artists")
        .and_then(|v| v.as_bool())
        .unwrap_or_else(|| {
            let va_regex = Regex::new(r"(?i)^VA\b|^Various\s+Artists?\b|^V\.A\.\b").unwrap();
            va_regex.is_match(&combined_path)
        });
    
    // Define regex patterns for audio categories
    let podcast_regex = Regex::new(r"(?i)\b(podcast|episode|experience|#\d{3,})\b").unwrap();
    let audiobook_regex = Regex::new(r"(?i)\b(audiobook|audio\s+book|narrated|unabridged|abridged)\b").unwrap();
    let classical_regex = Regex::new(r"(?i)\b(symphony|sonata|concerto|opus|mozart|beethoven|bach|chopin|orchestra|philharmonic)\b").unwrap();
    let soundtrack_regex = Regex::new(r"(?i)\b(OST|soundtrack|score)\b").unwrap();
    let bootleg_regex = Regex::new(r"(?i)\b(bootleg|unofficial|rare|unreleased|demo)\b").unwrap();
    let live_regex = Regex::new(r"(?i)\b(live\s+at|live\s+from|live\s+in|concert|unplugged|acoustic\s+live|bootleg)\b").unwrap();
    let single_regex = Regex::new(r"(?i)\b(single|maxi[\s\-]?single|CDS|CDM)\b").unwrap();
    let ep_regex = Regex::new(r"(?i)\b(EP|E\.P\.|extended\s+play)\b").unwrap();
    let compilation_regex = Regex::new(r"(?i)\b(compilation|best\s+of|greatest\s+hits|anthology|collection|selected)\b").unwrap();
    let remix_regex = Regex::new(r"(?i)\b(remix|remixed|mixes|mixed\s+by)\b").unwrap();
    
    // Check for existing artist/album metadata
    let has_artist = metadata.get("artist").and_then(|a| a.as_str()).is_some();
    let has_album = metadata.get("album").and_then(|a| a.as_str()).is_some();
    
    // Determine category based on patterns (order matters)
    let category = if podcast_regex.is_match(&combined_path) {
        AudioCategory::Podcast
    } else if audiobook_regex.is_match(&combined_path) {
        AudioCategory::Audiobook
    } else if classical_regex.is_match(&combined_path) {
        AudioCategory::Classical
    } else if soundtrack_regex.is_match(&combined_path) {
        AudioCategory::Soundtrack
    } else if bootleg_regex.is_match(&combined_path) {
        AudioCategory::Bootleg
    } else if live_regex.is_match(&combined_path) {
        AudioCategory::Live
    } else if single_regex.is_match(&combined_path) {
        AudioCategory::Single
    } else if ep_regex.is_match(&combined_path) {
        AudioCategory::EP
    } else if compilation_regex.is_match(&combined_path) || is_various_artists {
        AudioCategory::Compilation
    } else if remix_regex.is_match(&combined_path) {
        AudioCategory::Remix
    } else if has_artist || has_album {
        // Default to Album if we have artist/album info
        AudioCategory::Album
    } else {
        // Check existing category in metadata
        if let Some(existing_category) = metadata.get("category").and_then(|c| c.as_str()) {
            match existing_category {
                "Album" => AudioCategory::Album,
                "Single" => AudioCategory::Single,
                "EP" => AudioCategory::EP,
                "Compilation" => AudioCategory::Compilation,
                "Soundtrack" => AudioCategory::Soundtrack,
                "Live" => AudioCategory::Live,
                "Bootleg" => AudioCategory::Bootleg,
                "Podcast" => AudioCategory::Podcast,
                "Audiobook" => AudioCategory::Audiobook,
                "Mix" => AudioCategory::Mix,
                "Demo" => AudioCategory::Demo,
                "Remix" => AudioCategory::Remix,
                "Classical" => AudioCategory::Classical,
                _ => AudioCategory::Unknown,
            }
        } else {
            AudioCategory::Unknown
        }
    };
    
    Some(format!("AudioCategory::{:?}", category))
}

/// Determine audio source type from metadata
pub fn classify_audio_source_type(metadata: &JsonValue) -> Option<String> {
    let filename = metadata.get("filename")
        .or_else(|| metadata.get("title"))
        .and_then(|f| f.as_str())
        .unwrap_or("");
    
    let parent_dir = metadata.get("parent_dir")
        .and_then(|p| p.as_str())
        .unwrap_or("");
    
    let combined = format!("{} {}", filename, parent_dir);
    
    // Source type patterns
    let cd_regex = Regex::new(r"(?i)\b(CD[\s\-]?RIP|FLAC[\s\-]?CD|CD[\s\-]?FLAC)\b").unwrap();
    let vinyl_regex = Regex::new(r#"(?i)\b(vinyl|LP|12"|7"|45RPM|33RPM)\b"#).unwrap();
    let web_regex = Regex::new(r"(?i)\b(WEB|iTunes|Amazon|Bandcamp|Beatport|Spotify)\b").unwrap();
    let fm_regex = Regex::new(r"(?i)\b(FM|Radio)\b").unwrap();
    let cassette_regex = Regex::new(r"(?i)\b(cassette|tape)\b").unwrap();
    let remaster_regex = Regex::new(r"(?i)\b(remaster|remastered|anniversary|deluxe)\b").unwrap();
    
    let source_type = if remaster_regex.is_match(&combined) {
        AudioSourceType::Remaster
    } else if vinyl_regex.is_match(&combined) {
        AudioSourceType::Vinyl
    } else if cd_regex.is_match(&combined) {
        AudioSourceType::CD
    } else if web_regex.is_match(&combined) {
        AudioSourceType::Web
    } else if fm_regex.is_match(&combined) {
        AudioSourceType::FM
    } else if cassette_regex.is_match(&combined) {
        AudioSourceType::Cassette
    } else {
        AudioSourceType::Unknown
    };
    
    Some(format!("AudioSourceType::{:?}", source_type))
}

/// Classification rules for ebook content
pub fn classify_ebook_rules(metadata: &JsonValue) -> Option<String> {
    // Extract filename and extension from metadata
    let filename = metadata.get("filename")
        .or_else(|| metadata.get("title"))
        .and_then(|f| f.as_str())
        .unwrap_or("");
    
    let extension = metadata.get("extension")
        .and_then(|e| e.as_str())
        .unwrap_or("");
    
    // Define regex patterns for ebook categories
    let comic_regex = Regex::new(r"(?i)\b(comic|comics|manga|graphic\.novel|cbr|cbz)\b").unwrap();
    let magazine_regex = Regex::new(r"(?i)\b(magazine|mag|periodical|journal|monthly|weekly|quarterly|review|economist|geographic|nature|psychology|car\s+and\s+driver)\b").unwrap();
    let newspaper_regex = Regex::new(r"(?i)\b(newspaper|news|daily|times|post|gazette|herald|tribune)\b").unwrap();
    let technical_regex = Regex::new(r"(?i)\b(programming|coding|software|computer|technology|technical|engineering|mathematics|physics|chemistry|algorithm|python|java|javascript|mit\s+press|introduction\s+to\s+algorithms)\b").unwrap();
    let educational_regex = Regex::new(r"(?i)\b(textbook|course|tutorial|guide|manual|handbook|education|learning|study|exam|test\.prep)\b").unwrap();
    let biography_regex = Regex::new(r"(?i)\b(biography|autobiography|memoir|life\.of|story\.of)\b").unwrap();
    let history_regex = Regex::new(r"(?i)\b(history|historical|ancient|medieval|war|battle|civilization)\b").unwrap();
    let science_regex = Regex::new(r"(?i)\b(science|scientific|biology|astronomy|geology|research|medical|medicine|health|clinic|anatomy|physics|einstein)\b").unwrap();
    let religion_regex = Regex::new(r"(?i)\b(bible|quran|torah|religion|religious|spiritual|theology|buddhism|christianity|islam|hindu)\b").unwrap();
    let cookbook_regex = Regex::new(r"(?i)\b(cookbook|cooking|recipe|recipes|cuisine|culinary|baking|food|crocker|ramsay|oliver)\b").unwrap();
    let travel_regex = Regex::new(r"(?i)\b(travel|guide|lonely\.planet|frommer|tourism|vacation|michelin|rick\s+steves|through\s+the\s+back\s+door)\b").unwrap();
    let children_regex = Regex::new(r"(?i)\b(children|kids|juvenile|young\.adult|ya|picture\.book|seuss)\b").unwrap();
    
    // Check for existing metadata hints
    let has_author = metadata.get("author").and_then(|a| a.as_str()).is_some();
    let has_title = metadata.get("title").and_then(|t| t.as_str()).is_some();
    
    // Determine category based on patterns (order matters for priority)
    let category = if matches!(extension.to_lowercase().as_str(), "cbr" | "cbz") || comic_regex.is_match(filename) {
        EbookCategory::Comic
    } else if newspaper_regex.is_match(filename) {
        EbookCategory::Newspaper
    } else if cookbook_regex.is_match(filename) {
        EbookCategory::Cookbook
    } else if travel_regex.is_match(filename) {
        EbookCategory::Travel
    } else if children_regex.is_match(filename) {
        EbookCategory::Children
    } else if technical_regex.is_match(filename) {
        EbookCategory::Technical
    } else if magazine_regex.is_match(filename) {
        EbookCategory::Magazine
    } else if biography_regex.is_match(filename) {
        EbookCategory::Biography
    } else if history_regex.is_match(filename) {
        EbookCategory::History
    } else if science_regex.is_match(filename) {
        EbookCategory::Science
    } else if religion_regex.is_match(filename) {
        EbookCategory::Religion
    } else if educational_regex.is_match(filename) {
        EbookCategory::Educational
    } else if has_author && has_title {
        // If we have author and title, likely a novel
        EbookCategory::Novel
    } else {
        // Check existing category in metadata or use format hints
        if let Some(format) = metadata.get("format").and_then(|f| f.as_str()) {
            if format.contains("Comic") || format.contains("Cbr") || format.contains("Cbz") {
                EbookCategory::Comic
            } else if format.contains("Magazine") {
                EbookCategory::Magazine
            } else {
                EbookCategory::Unknown
            }
        } else if let Some(existing_category) = metadata.get("category").and_then(|c| c.as_str()) {
            match existing_category {
                "Novel" => EbookCategory::Novel,
                "Comic" => EbookCategory::Comic,
                "Magazine" => EbookCategory::Magazine,
                "Newspaper" => EbookCategory::Newspaper,
                "Technical" => EbookCategory::Technical,
                "Educational" => EbookCategory::Educational,
                "Biography" => EbookCategory::Biography,
                "History" => EbookCategory::History,
                "Science" => EbookCategory::Science,
                "Religion" => EbookCategory::Religion,
                "Cookbook" => EbookCategory::Cookbook,
                "Travel" => EbookCategory::Travel,
                "Children" => EbookCategory::Children,
                _ => EbookCategory::Unknown,
            }
        } else {
            EbookCategory::Unknown
        }
    };
    
    // Map to simplified categories for upload
    let simplified_category = match category {
        EbookCategory::Comic => EbookCategory::Comic,
        EbookCategory::Magazine => EbookCategory::Magazine,
        EbookCategory::Educational | EbookCategory::Technical | EbookCategory::Science => EbookCategory::Educational,
        // Map all other categories to a generic category string
        _ => category,
    };
    
    Some(format!("EbookCategory::{:?}", simplified_category))
}

/// Classification rules for game content
pub fn classify_game_rules(metadata: &JsonValue) -> Option<String> {
    // Extract filename and platform from metadata
    let filename = metadata.get("filename")
        .or_else(|| metadata.get("title"))
        .and_then(|f| f.as_str())
        .unwrap_or("");
    
    let platform = metadata.get("platform")
        .and_then(|p| p.as_str())
        .unwrap_or_else(|| {
            // Auto-detect platform from filename if not in metadata
            let platform_regex = Regex::new(r"(?i)\b(pc|windows|linux|mac|macos|ps[1-5]|xbox|switch|android|ios)\b").unwrap();
            if let Some(captures) = platform_regex.captures(filename) {
                captures.get(1).map(|m| m.as_str()).unwrap_or("PC")
            } else {
                "PC"
            }
        });
    
    // Detect if this is software vs game
    let software_regex = Regex::new(r"(?i)\b(office|photoshop|adobe|microsoft|autodesk|vmware|antivirus|norton|kaspersky|avast|malwarebytes|driver|utility|tool|converter|editor|manager|professional|enterprise|business|suite|studio|windows|macos|linux|ubuntu|debian|fedora)\b").unwrap();
    let game_regex = Regex::new(r"(?i)\b(game|games|steam|gog|epic|origin|uplay|battle\.net|repack|rip|crack|codex|plaza|skidrow|reloaded|fps|rpg|mmo|rts|moba|dlc|expansion|edition|goty|deluxe|ultimate|gold)\b").unwrap();
    
    let is_software = software_regex.is_match(filename) && !game_regex.is_match(filename);
    
    // Normalize platform string
    let normalized_platform = platform.to_uppercase()
        .replace(" ", "_")
        .replace("-", "_");
    
    // Determine category based on platform and type
    let category_str = if is_software || normalized_platform.ends_with("_SOFTWARE") {
        format!("GameCategory::Software_{}", normalized_platform.replace("_SOFTWARE", ""))
    } else {
        match normalized_platform.as_str() {
            "NSW" | "NINTENDO_SWITCH" | "SWITCH" | "XCI" | "NSP" => "GameCategory::Console".to_string(),
            "3DS" | "CIA" => "GameCategory::Console".to_string(),
            "PS4" | "PS5" | "XBOX" => "GameCategory::Console".to_string(),
            "WII" | "NES" | "SNES" => "GameCategory::Retro".to_string(),
            _ => "GameCategory::PC".to_string(),
        }
    };
    
    Some(category_str)
}

/// Classification rules for hobby content
pub fn classify_hobby_rules(metadata: &JsonValue) -> Option<String> {
    // Extract filename and extension from metadata
    let filename = metadata.get("filename")
        .or_else(|| metadata.get("title"))
        .and_then(|f| f.as_str())
        .unwrap_or("");
    
    let extension = metadata.get("extension")
        .and_then(|e| e.as_str())
        .unwrap_or("")
        .to_lowercase();
    
    let file_type = metadata.get("file_type")
        .and_then(|t| t.as_str())
        .unwrap_or("");
    
    // Define regex patterns for content-based classification
    let tutorial_regex = Regex::new(r"(?i)\b(tutorial|guide|howto|manual|instructions)\b").unwrap();
    let template_regex = Regex::new(r"(?i)\b(template|templates|mockup|preset|presets)\b").unwrap();
    let resource_regex = Regex::new(r"(?i)\b(resource|resources|asset|assets|pack|bundle|collection)\b").unwrap();
    
    // First check if we have a category hint in metadata
    if let Some(category) = metadata.get("category").and_then(|c| c.as_str()) {
        return Some(format!("HobbyCategory::{}", category));
    }
    
    // Determine category based on file type/extension
    let mut category = match extension.as_str() {
        "doc" | "docx" | "txt" | "rtf" => HobbyCategory::Documents,
        "jpg" | "jpeg" | "png" | "gif" | "bmp" | "tiff" | "svg" => HobbyCategory::Images,
        "dwg" | "dxf" | "stl" | "obj" | "ply" => HobbyCategory::CAD3D,
        "zip" | "rar" | "7z" => HobbyCategory::Archives,
        "csv" | "json" | "xml" | "sql" => HobbyCategory::DataFiles,
        "ttf" | "otf" | "woff" => HobbyCategory::Fonts,
        _ => {
            // Check file_type for directory
            if file_type == "Directory" {
                HobbyCategory::Collection
            } else {
                HobbyCategory::Unknown
            }
        }
    };
    
    // Refine category based on content patterns (these override file type)
    if tutorial_regex.is_match(filename) {
        category = HobbyCategory::Tutorial;
    } else if template_regex.is_match(filename) {
        category = HobbyCategory::Template;
    } else if resource_regex.is_match(filename) {
        category = HobbyCategory::Resource;
    }
    
    Some(format!("HobbyCategory::{:?}", category))
}