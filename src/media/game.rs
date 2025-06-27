use crate::types::{GameFile, GameType, MediaFile, MediaType};
use std::path::Path;
use log::{info, warn};
use regex::Regex;
use crate::extraction::process_and_extract_archives;
use chrono;

/// Game metadata extracted from filename and structure
#[derive(Debug, Clone)]
pub struct GameMetadata {
    pub title: String,
    pub platform: Option<String>,
    pub year: Option<u32>,
    pub version: Option<String>,
    pub developer: Option<String>,
    pub publisher: Option<String>,
    pub genre: Option<String>,
    pub language: Option<String>,
    pub release_group: Option<String>,
    pub is_gog: bool,
    pub is_steam: bool,
    pub is_repack: bool,
}

impl Default for GameMetadata {
    fn default() -> Self {
        Self {
            title: String::new(),
            platform: None,
            year: None,
            version: None,
            developer: None,
            publisher: None,
            genre: None,
            language: None,
            release_group: None,
            is_gog: false,
            is_steam: false,
            is_repack: false,
        }
    }
}

/// Process game file(s) from a path (file or directory) and classify content
pub fn process_game(
    input_path: &str,
    _config: &crate::types::Config,
    _dry_run: bool,
) -> Result<Vec<(GameFile, GameMetadata)>, String> {
    let path = Path::new(input_path);
    
    if !path.exists() {
        return Err(format!("Path not found: {}", input_path));
    }
    
    // Extract any archives first and get the path to process
    let processing_path = process_and_extract_archives(input_path)?;
    
    let mut results = Vec::new();
    
    // Update path to use the processing path
    let path = Path::new(&processing_path);
    
    if path.is_file() {
        // Single file case
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .ok_or_else(|| "Could not determine file extension".to_string())?;

        let game_type = GameType::from_extension(extension)
            .ok_or_else(|| format!("Unsupported game file type: {}", extension))?;
        
        let game_file = GameFile {
            path: path.to_path_buf(),
            game_type,
        };
        
        let filename = path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        
        let metadata = classify_game_content(filename);
        
        results.push((game_file, metadata));
        
    } else if path.is_dir() {
        // Check if directory itself is a game
        if looks_like_game_directory(path) {
            let game_file = GameFile {
                path: path.to_path_buf(),
                game_type: GameType::Directory,
            };
            
            let dirname = path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");
                
            let metadata = classify_game_content(dirname);
            
            results.push((game_file, metadata));
        } else {
            // Search for game files in directory
            for entry in std::fs::read_dir(path)
                .map_err(|e| format!("Failed to read directory: {}", e))? 
            {
                let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
                let file_path = entry.path();
                
                if file_path.is_file() {
                    if let Some(extension) = file_path.extension().and_then(|ext| ext.to_str()) {
                        if let Some(game_type) = GameType::from_extension(extension) {
                            let game_file = GameFile {
                                path: file_path.clone(),
                                game_type,
                            };
                            
                            let filename = file_path.file_name()
                                .and_then(|name| name.to_str())
                                .unwrap_or("");
                            
                            let metadata = classify_game_content(filename);
                            
                            info!("Processed game: {} -> Platform: {:?}", 
                                  filename, metadata.platform);
                            
                            results.push((game_file, metadata));
                        }
                    }
                }
            }
        }
    }
    
    if results.is_empty() {
        return Err("No game files found in the specified path".to_string());
    }
    
    // After we have the results, build the upload data if we have game files
    if !results.is_empty() {
        use crate::upload::UploadBuilder;
        use std::sync::Arc;
        
        let (_game_file, metadata) = &results[0];
        
        // Build upload data directly using UploadBuilder
        use crate::description::DescriptionConfig;
        use crate::types::ImageLayout;
        
        // Configure description for games
        let mut desc_config = DescriptionConfig::default();
        desc_config.image_layout = ImageLayout::Gallery; // Games use gallery layout for screenshots
        desc_config.max_images = 8; // Show game screenshots
        desc_config.image_width = 600;
        
        // Create the upload builder with game-specific components
        let mut builder = UploadBuilder::new(
            &processing_path,
            MediaType::Game(GameType::Directory), // Default to directory type
            Arc::new((*_config).clone())
        )
        .with_extensions(GameType::all_extensions())
        .with_description_config(desc_config)
        .dry_run(_dry_run);
        
        // Add title info
        builder = builder.with_title_info(
            &metadata.title, 
            metadata.year.map(|y| y.to_string()).as_deref()
        );
        
        // Add game-specific metadata
        let mut game_metadata = std::collections::HashMap::new();
        game_metadata.insert("title".to_string(), metadata.title.clone());
        if let Some(platform) = &metadata.platform {
            game_metadata.insert("platform".to_string(), platform.clone());
        }
        if let Some(version) = &metadata.version {
            game_metadata.insert("version".to_string(), version.clone());
        }
        if let Some(developer) = &metadata.developer {
            game_metadata.insert("developer".to_string(), developer.clone());
        }
        if let Some(publisher) = &metadata.publisher {
            game_metadata.insert("publisher".to_string(), publisher.clone());
        }
        if let Some(genre) = &metadata.genre {
            game_metadata.insert("genre".to_string(), genre.clone());
        }
        if let Some(language) = &metadata.language {
            game_metadata.insert("language".to_string(), language.clone());
        }
        if let Some(release_group) = &metadata.release_group {
            game_metadata.insert("release_group".to_string(), release_group.clone());
        }
        if metadata.is_gog {
            game_metadata.insert("store".to_string(), "GOG".to_string());
        } else if metadata.is_steam {
            game_metadata.insert("store".to_string(), "Steam".to_string());
        }
        if metadata.is_repack {
            game_metadata.insert("repack".to_string(), "true".to_string());
        }
        
        // Fetch IGDB data if credentials are available
        if !_config.general.igdb_client_id.is_empty() && !_config.general.igdb_bearer_token.is_empty() {
            info!("Looking up game on IGDB: {}", metadata.title);
            match crate::utils::search_igdb_game(
                &metadata.title,
                &_config.general.igdb_client_id,
                &_config.general.igdb_bearer_token,
                _config
            ) {
                Ok(games) if !games.is_empty() => {
                    // Take the first result
                    if let Some(game) = games.first() {
                        if let Some(igdb_id) = game["id"].as_u64() {
                            info!("Found IGDB ID: {} for game: {}", igdb_id, metadata.title);
                            game_metadata.insert("igdb_id".to_string(), igdb_id.to_string());
                            
                            // Get detailed game information
                            match crate::utils::get_igdb_game_details(
                                igdb_id,
                                &_config.general.igdb_client_id,
                                &_config.general.igdb_bearer_token
                            ) {
                                Ok(details) => {
                                    // Extract and store additional metadata from IGDB
                                    
                                    // Genres
                                    if let Some(genres) = details["genres"].as_array() {
                                        let genre_names: Vec<String> = genres.iter()
                                            .filter_map(|g| g["name"].as_str())
                                            .map(|s| s.to_string())
                                            .collect();
                                        if !genre_names.is_empty() {
                                            game_metadata.insert("igdb_genres".to_string(), genre_names.join(", "));
                                        }
                                    }
                                    
                                    // Developer/Publisher from involved_companies
                                    if let Some(companies) = details.get("involved_companies") {
                                        let (developers, publishers) = crate::utils::extract_igdb_companies(companies);
                                        if !developers.is_empty() && metadata.developer.is_none() {
                                            game_metadata.insert("igdb_developer".to_string(), developers.join(", "));
                                        }
                                        if !publishers.is_empty() && metadata.publisher.is_none() {
                                            game_metadata.insert("igdb_publisher".to_string(), publishers.join(", "));
                                        }
                                    }
                                    
                                    // Summary
                                    if let Some(summary) = details["summary"].as_str() {
                                        game_metadata.insert("igdb_summary".to_string(), summary.to_string());
                                    }
                                    
                                    // Rating
                                    if let Some(rating) = details["total_rating"].as_f64() {
                                        game_metadata.insert("igdb_rating".to_string(), format!("{:.1}", rating));
                                    }
                                    
                                    // Release date
                                    if let Some(release_date) = details["first_release_date"].as_i64() {
                                        // Convert Unix timestamp to year
                                        let year = chrono::DateTime::from_timestamp(release_date, 0)
                                            .map(|dt| dt.format("%Y").to_string());
                                        if let Some(year_str) = year {
                                            game_metadata.insert("igdb_release_year".to_string(), year_str);
                                        }
                                    }
                                    
                                    info!("Successfully fetched IGDB game details");
                                }
                                Err(e) => {
                                    warn!("Failed to fetch IGDB game details: {}", e);
                                }
                            }
                        }
                    }
                }
                Ok(_) => {
                    info!("No games found on IGDB for: {}", metadata.title);
                }
                Err(e) => {
                    warn!("Failed to search IGDB: {}", e);
                }
            }
        }
        
        builder = builder
            .with_nfo()
            .with_duplicate_check()
            .with_screenshots(4) // Game screenshots
            .with_custom_component("game_metadata", crate::types::UploadComponent::Metadata(game_metadata));
        
        let _upload_data = builder.build()?;
        
        info!("Built upload data for game processing");
        
        // Create the upload processor - it will auto-detect the active tracker
        let mut processor = crate::upload::UploadProcessor::new(
            _upload_data,
            std::sync::Arc::new(_config.clone()),
        )
        .dry_run(_dry_run);
        
        // Get media classification for mapping
        if !results.is_empty() {
            let (_, metadata) = &results[0];
            
            // Check if it's software based on platform
            let category_str = if let Some(platform) = &metadata.platform {
                if platform.ends_with("_SOFTWARE") {
                    // It's software, keep it under GameCategory with platform info
                    format!("GameCategory::Software_{}", platform)
                } else {
                    // It's a game
                    format!("GameCategory::{}", platform)
                }
            } else {
                "GameCategory::PC".to_string()
            };
            
            processor = processor.with_media_classification(
                Some(category_str),
                None, // Games don't have source types
            );
        }
        
        // Process the upload - it handles tracker detection and mapping internally
        let upload_result = processor.process()?;
        
        if upload_result.success {
            info!("Upload completed successfully to {}", upload_result.tracker);
            if let Some(torrent_id) = upload_result.torrent_id {
                info!("Torrent ID: {}", torrent_id);
            }
        } else {
            warn!("Upload failed: {}", upload_result.message);
        }
    }
    
    Ok(results)
}

/// Classify game content based on filename patterns
pub fn classify_game_content(filename: &str) -> GameMetadata {
    let mut metadata = GameMetadata::default();
    
    // Initialize regex patterns
    let year_regex = Regex::new(r"\b(19|20)\d{2}\b").unwrap();
    let version_regex = Regex::new(r"(?i)\b(?:v|ver|version)\.?\s*(\d+(?:\.\d+)*)\b").unwrap();
    let platform_regex = Regex::new(r"(?i)\b(pc|windows|linux|mac|macos|ps[1-5]|xbox|switch|android|ios)\b").unwrap();
    let gog_regex = Regex::new(r"(?i)\bgog\b").unwrap();
    let steam_regex = Regex::new(r"(?i)\bsteam[\s-]?rip\b").unwrap();
    let repack_regex = Regex::new(r"(?i)\brepack\b").unwrap();
    let language_regex = Regex::new(r"(?i)\b(multi\d*|english|spanish|french|german|italian|russian|japanese|chinese)\b").unwrap();
    let release_group_regex = Regex::new(r"-([A-Z][A-Za-z0-9]+)(?:\s*$|\s*\()").unwrap();
    
    // Clean filename for processing
    let clean_name = filename
        .trim()
        .trim_end_matches(|c: char| c == '.' || c.is_numeric());
    
    // Extract title (everything before year, version, or platform indicators)
    let mut title = clean_name.to_string();
    
    // Extract year
    if let Some(year_match) = year_regex.find(clean_name) {
        metadata.year = year_match.as_str().parse::<u32>().ok();
        // Title is everything before the year
        if let Some(pos) = clean_name.find(year_match.as_str()) {
            title = clean_name[..pos].trim().to_string();
        }
    }
    
    // Extract version
    if let Some(version_match) = version_regex.captures(clean_name) {
        if let Some(ver) = version_match.get(1) {
            metadata.version = Some(ver.as_str().to_string());
        }
    }
    
    // Extract platform - but check if it's actually a game or software
    if let Some(platform_match) = platform_regex.find(clean_name) {
        let detected_platform = platform_match.as_str().to_uppercase();
        
        // Check if this is software vs game based on content
        let filename_lower = filename.to_lowercase();
        
        // Software patterns
        let software_keywords = [
            "office", "photoshop", "adobe", "microsoft", "autodesk", "vmware",
            "antivirus", "norton", "kaspersky", "avast", "malwarebytes",
            "driver", "utility", "tool", "converter", "editor", "manager",
            "professional", "enterprise", "business", "suite", "studio",
            "windows", "macos", "linux", "ubuntu", "debian", "fedora"
        ];
        
        // Game patterns
        let game_keywords = [
            "game", "games", "steam", "gog", "epic", "origin", "uplay", "battle.net",
            "repack", "rip", "crack", "codex", "plaza", "skidrow", "reloaded",
            "fps", "rpg", "mmo", "rts", "moba", "dlc", "expansion",
            "edition", "goty", "deluxe", "ultimate", "gold"
        ];
        
        let has_software_keyword = software_keywords.iter().any(|&kw| filename_lower.contains(kw));
        let has_game_keyword = game_keywords.iter().any(|&kw| filename_lower.contains(kw));
        
        // Set platform differently for software vs games
        if has_software_keyword && !has_game_keyword {
            // It's software - use a special platform indicator
            metadata.platform = Some(format!("{}_SOFTWARE", detected_platform));
        } else {
            // It's a game or uncertain - use normal platform
            metadata.platform = Some(detected_platform);
        }
    }
    
    // Check for store/distribution
    metadata.is_gog = gog_regex.is_match(clean_name);
    metadata.is_steam = steam_regex.is_match(clean_name);
    metadata.is_repack = repack_regex.is_match(clean_name);
    
    // Extract language
    if let Some(lang_match) = language_regex.find(clean_name) {
        metadata.language = Some(lang_match.as_str().to_string());
    }
    
    // Extract release group (usually at the end after a dash)
    if let Some(group_match) = release_group_regex.captures(clean_name) {
        if let Some(group) = group_match.get(1) {
            metadata.release_group = Some(group.as_str().to_string());
        }
    }
    
    // Clean up title
    metadata.title = title
        .replace('.', " ")
        .replace('_', " ")
        .replace('-', " ")
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ");
    
    metadata
}

/// Detect game files in a path
pub fn detect_game_files(path: &str) -> Result<Vec<GameFile>, String> {
    let mut game_files = Vec::new();
    detect_game_files_recursive(Path::new(path), &mut game_files)?;
    Ok(game_files)
}

/// Recursively search for game files in a directory tree
fn detect_game_files_recursive(path: &Path, game_files: &mut Vec<GameFile>) -> Result<(), String> {
    if path.is_file() {
        if let Some(extension) = path.extension().and_then(|ext| ext.to_str()) {
            if let Some(game_type) = GameType::from_extension(extension) {
                game_files.push(GameFile {
                    path: path.to_path_buf(),
                    game_type,
                });
            }
        }
    } else if path.is_dir() {
        // Check if directory itself is a game (like extracted game folders)
        if looks_like_game_directory(path) {
            game_files.push(GameFile {
                path: path.to_path_buf(),
                game_type: GameType::Directory,
            });
            // Don't recurse into game directories
            return Ok(());
        }
        
        // Recursively search subdirectories
        for entry in std::fs::read_dir(path)
            .map_err(|e| format!("Failed to read directory {:?}: {}", path, e))? 
        {
            let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
            let entry_path = entry.path();
            
            // Recursively process subdirectories and files
            detect_game_files_recursive(&entry_path, game_files)?;
        }
    }
    
    Ok(())
}

/// Check if directory looks like a game installation
fn looks_like_game_directory(path: &Path) -> bool {
    if !path.is_dir() {
        return false;
    }
    
    // Check if directory contains any game files based on GameType extensions
    for entry in std::fs::read_dir(path).unwrap_or_else(|_| std::fs::read_dir(".").unwrap()) {
        if let Ok(entry) = entry {
            let file_path = entry.path();
            if file_path.is_file() {
                if let Some(extension) = file_path.extension().and_then(|ext| ext.to_str()) {
                    if GameType::from_extension(extension).is_some() {
                        return true;
                    }
                }
            }
        }
    }
    
    false
}


/// Convert GameFile to MediaFile
pub fn to_media_file(game_file: &GameFile) -> MediaFile {
    MediaFile {
        path: game_file.path.clone(),
        media_type: MediaType::Game(game_file.game_type.clone()),
    }
}

/// Classify game content for upload pipeline
pub fn classify_for_upload(input_path: &str, metadata: &serde_json::Value) -> Result<(Option<String>, Option<String>, serde_json::Value), String> {
    // Check if we have platform info in metadata
    if let Some(platform) = metadata.get("platform").and_then(|p| p.as_str()) {
        let category = if platform.ends_with("_SOFTWARE") {
            Some(format!("GameCategory::Software_{}", platform))
        } else {
            // Map platform to game category
            match platform.to_uppercase().as_str() {
                "NSW" | "NINTENDO SWITCH" | "XCI" | "NSP" => Some("GameCategory::Console".to_string()),
                "3DS" | "CIA" => Some("GameCategory::Console".to_string()),
                "PS4" | "PS5" | "XBOX" => Some("GameCategory::Console".to_string()),
                "WII" | "NES" | "SNES" => Some("GameCategory::Retro".to_string()),
                _ => Some("GameCategory::PC".to_string())
            }
        };
        
        return Ok((category, None, metadata.clone()));
    }
    
    // Otherwise, detect and classify
    if let Ok(game_files) = detect_game_files(input_path) {
        if let Some(game_file) = game_files.first() {
            let filename = game_file.path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            
            let game_metadata = classify_game_content(filename);
            
            let category = if let Some(platform) = &game_metadata.platform {
                if platform.ends_with("_SOFTWARE") {
                    Some(format!("GameCategory::Software_{}", platform))
                } else {
                    match platform.to_uppercase().as_str() {
                        "NSW" | "NINTENDO SWITCH" | "XCI" | "NSP" => Some("GameCategory::Console".to_string()),
                        "3DS" | "CIA" => Some("GameCategory::Console".to_string()),
                        "PS4" | "PS5" | "XBOX" => Some("GameCategory::Console".to_string()),
                        "WII" | "NES" | "SNES" => Some("GameCategory::Retro".to_string()),
                        _ => Some("GameCategory::PC".to_string())
                    }
                }
            } else {
                Some("GameCategory::PC".to_string())
            };
            
            // Manually create JSON metadata
            let json_metadata = serde_json::json!({
                "title": game_metadata.title,
                "platform": game_metadata.platform,
                "year": game_metadata.year,
                "version": game_metadata.version,
                "developer": game_metadata.developer,
                "publisher": game_metadata.publisher,
                "genre": game_metadata.genre,
                "language": game_metadata.language,
                "release_group": game_metadata.release_group,
                "is_gog": game_metadata.is_gog,
                "is_steam": game_metadata.is_steam,
                "is_repack": game_metadata.is_repack,
            });
            
            return Ok((category, None, json_metadata));
        }
    }
    
    // Default to PC game
    Ok((Some("GameCategory::PC".to_string()), None, metadata.clone()))
}
