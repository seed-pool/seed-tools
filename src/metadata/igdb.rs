// Internet Game Database (IGDB) API integration

use crate::core::{
    error::{Result, SeedError},
    Config,
};
use log::{error, info};
use regex::Regex;
use reqwest::blocking::Client;
use serde_json::Value;

/// Clean up a game title for IGDB search by removing version numbers, release groups, etc.
pub fn clean_game_title_for_search(title: &str, config: &Config) -> String {
    let title = title.trim();

    // First normalize periods to spaces (common in release names)
    let mut cleaned = title.replace('.', " ");

    // Build release group patterns from config
    let release_groups = config
        .general
        .release_groups
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<&str>>()
        .join("|");

    // First, remove common patterns that won't be in IGDB
    let mut patterns_to_remove = vec![
        // Version patterns
        r"(?i)\s+v?\d+[\.\s]\d+[\.\s]\d+[\.\s]\d+\w*".to_string(), // v1.0.2.31110s
        r"(?i)\s+v?\d+[\.\s]\d+[\.\s]\d+\w*".to_string(),          // v1.0.2
        r"(?i)\s+v?\d+[\.\s]\d+\w*".to_string(),                   // v1.0
        r"(?i)\s+v\d+\w*".to_string(),                             // v1

        // Build/Update patterns
        r"(?i)\s+build\s*\d+".to_string(),
        r"(?i)\s+build\d+".to_string(),     // Build911 (no space)
        r"(?i)\s+update\s*\d+".to_string(),
        r"(?i)\s+patch\s*\d+".to_string(),
        r"(?i)\s+hotfix\s*\d+".to_string(),

        // Platform/Store patterns
        r"(?i)\s*[\(\[]?(steam|epic|origin|uplay|battle\.net|gog)[\s\-]?(rip|version)?[\)\]]?".to_string(),

        // Edition patterns (but keep some like "Game of the Year")
        r"(?i)\s+digital\s+deluxe(\s+edition)?".to_string(),
        r"(?i)\s+collectors?(\s+edition)?".to_string(),
        r"(?i)\s+premium(\s+edition)?".to_string(),
        r"(?i)\s+ultimate(\s+edition)?".to_string(),
        r"(?i)\s+complete(\s+edition)?".to_string(),
        r"(?i)\s+definitive(\s+edition)?".to_string(),
        r"(?i)\s+enhanced(\s+edition)?".to_string(),

        // DLC patterns
        r"(?i)\s*[\+\-]\s*DLC\s*.*$".to_string(),
        r"(?i)\s+DLC\s+Pack.*$".to_string(),
        r"(?i)\s+Season\s+Pass.*$".to_string(),

        // Language patterns
        r"(?i)\s*[\(\[]?(multi\d*|english|spanish|french|german|italian|russian|japanese|chinese)[\)\]]?".to_string(),

        // Other common suffixes
        r"(?i)\s+shipping".to_string(),
        r"(?i)\s+incl[\.\s]".to_string(),
        r"(?i)\s+including".to_string(),
        r"(?i)\s+proper".to_string(),
        r"(?i)\s+internal".to_string(),
        r"(?i)\s+cracked".to_string(),
    ];

    // Add release group patterns from config
    if !release_groups.is_empty() {
        // Match release groups with dash (keep the dash format: -GROUP)
        patterns_to_remove.push(format!(r"(?i)\s*-({}).*$", release_groups));
        // Match release groups with just space
        patterns_to_remove.push(format!(r"(?i)\s+({}).*$", release_groups));
    }

    // Apply all removal patterns
    for pattern in &patterns_to_remove {
        if let Ok(re) = Regex::new(pattern) {
            cleaned = re.replace_all(&cleaned, "").to_string();
        }
    }

    // Clean up any remaining artifacts
    cleaned = cleaned
        .trim_matches(|c: char| !c.is_alphanumeric()) // Remove trailing punctuation
        .replace("  ", " ") // Remove double spaces
        .trim()
        .to_string();

    // If we've removed too much, fall back to a simpler approach
    if cleaned.is_empty() || cleaned.len() < 3 {
        // Just take the first few words before any version/group markers
        let simple_markers = [" v", " V", " -", " REPACK", " Update", " Build"];
        let mut simple_title = title.to_string();
        for marker in &simple_markers {
            if let Some(pos) = simple_title.find(marker) {
                simple_title = simple_title[..pos].to_string();
                break;
            }
        }
        cleaned = simple_title.trim().to_string();
    }

    info!(
        "Cleaned game title for IGDB search: '{}' -> '{}'",
        title, cleaned
    );
    cleaned
}

/// Search for a game on IGDB
pub fn search_igdb_game(
    game_name: &str,
    client_id: &str,
    bearer_token: &str,
    config: &Config,
) -> Result<Vec<Value>> {
    let client = Client::new();

    // Clean the game name before searching
    let cleaned_name = clean_game_title_for_search(game_name, config);

    info!(
        "Searching IGDB for game: {} (cleaned: {})",
        game_name, cleaned_name
    );

    // IGDB uses a special query language for searching
    let query = format!(
        r#"search "{}"; fields id,name,first_release_date,summary,genres.name,platforms.name,involved_companies.company.name,involved_companies.developer,involved_companies.publisher,cover.url,screenshots.url; limit 5;"#,
        cleaned_name.replace('"', r#"\""#)
    );

    info!("IGDB query: {}", query);

    let response = client
        .post("https://api.igdb.com/v4/games")
        .header("Client-ID", client_id)
        .header("Authorization", format!("Bearer {}", bearer_token))
        .header("Content-Type", "text/plain")
        .body(query.clone())
        .send()
        .map_err(|e| SeedError::ApiError(format!("Failed to search IGDB: {}", e)))?;

    info!("IGDB API Response status: {}", response.status());

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .unwrap_or_else(|_| "Unable to read response".to_string());
        error!("IGDB API error {}: {}", status, error_text);
        return Err(SeedError::ApiError(format!(
            "IGDB API error: {} - {}",
            status, error_text
        )));
    }

    let response_text = response
        .text()
        .map_err(|e| SeedError::ApiError(format!("Failed to read IGDB response: {}", e)))?;

    let games: Vec<Value> = serde_json::from_str(&response_text).map_err(|e| {
        SeedError::ApiError(format!(
            "Failed to parse IGDB response: {} - Response was: {}",
            e, response_text
        ))
    })?;

    info!("Found {} games on IGDB", games.len());

    // Log game results in a more readable format
    if !games.is_empty() {
        for (i, game) in games.iter().take(3).enumerate() {
            if let Some(name) = game["name"].as_str() {
                let id = game["id"].as_u64().unwrap_or(0);
                let release_date = game["first_release_date"]
                    .as_u64()
                    .map(|ts| {
                        format!(
                            "{}",
                            chrono::DateTime::<chrono::Utc>::from_timestamp(ts as i64, 0)
                                .map(|dt| dt.format("%Y-%m-%d").to_string())
                                .unwrap_or_else(|| "Unknown".to_string())
                        )
                    })
                    .unwrap_or_else(|| "Unknown".to_string());
                info!(
                    "  {}. {} (ID: {}, Released: {})",
                    i + 1,
                    name,
                    id,
                    release_date
                );
            }
        }
        if games.len() > 3 {
            info!("  ... and {} more results", games.len() - 3);
        }
    } else {
        info!("IGDB raw response: {}", response_text);
    }
    Ok(games)
}

/// Get detailed game information from IGDB
pub fn get_igdb_game_details(game_id: u64, client_id: &str, bearer_token: &str) -> Result<Value> {
    let client = Client::new();

    info!("Fetching IGDB game details for ID: {}", game_id);

    // Request comprehensive game details
    let query = format!(
        r#"fields id,name,summary,storyline,first_release_date,
        genres.name,platforms.name,game_modes.name,themes.name,
        player_perspectives.name,franchises.name,
        involved_companies.company.name,involved_companies.developer,involved_companies.publisher,
        cover.url,screenshots.url,artworks.url,
        websites.url,websites.category,
        total_rating,total_rating_count,
        version_title,version_parent;
        where id = {};"#,
        game_id
    );

    let response = client
        .post("https://api.igdb.com/v4/games")
        .header("Client-ID", client_id)
        .header("Authorization", format!("Bearer {}", bearer_token))
        .header("Content-Type", "text/plain")
        .body(query)
        .send()
        .map_err(|e| SeedError::ApiError(format!("Failed to fetch IGDB game details: {}", e)))?;

    if !response.status().is_success() {
        return Err(SeedError::ApiError(format!(
            "IGDB API error: {}",
            response.status()
        )));
    }

    let mut games: Vec<Value> = response
        .json()
        .map_err(|e| SeedError::ApiError(format!("Failed to parse IGDB response: {}", e)))?;

    games
        .pop()
        .ok_or_else(|| SeedError::ApiError("Game not found on IGDB".to_string()))
}

/// Extract developer and publisher from IGDB response
pub fn extract_igdb_companies(involved_companies: &Value) -> (Vec<String>, Vec<String>) {
    let mut developers = Vec::new();
    let mut publishers = Vec::new();

    if let Some(companies) = involved_companies.as_array() {
        for company in companies {
            let is_developer = company
                .get("developer")
                .and_then(|d| d.as_bool())
                .unwrap_or(false);
            let is_publisher = company
                .get("publisher")
                .and_then(|p| p.as_bool())
                .unwrap_or(false);

            if let Some(name) = company
                .get("company")
                .and_then(|c| c.get("name"))
                .and_then(|n| n.as_str())
            {
                if is_developer {
                    developers.push(name.to_string());
                }
                if is_publisher {
                    publishers.push(name.to_string());
                }
            }
        }
    }

    (developers, publishers)
}

/// Extract comprehensive metadata from IGDB game details response
pub fn extract_igdb_metadata(igdb_details: &Value) -> std::collections::HashMap<String, String> {
    let mut metadata = std::collections::HashMap::new();

    // Basic game info
    if let Some(name) = igdb_details["name"].as_str() {
        metadata.insert("igdb_title".to_string(), name.to_string());
    }

    if let Some(summary) = igdb_details["summary"].as_str() {
        metadata.insert("igdb_summary".to_string(), summary.to_string());
    }

    if let Some(storyline) = igdb_details["storyline"].as_str() {
        metadata.insert("igdb_storyline".to_string(), storyline.to_string());
    }

    // Rating
    if let Some(rating) = igdb_details["total_rating"].as_f64() {
        metadata.insert("igdb_rating".to_string(), format!("{:.1}", rating));
    }

    if let Some(rating_count) = igdb_details["total_rating_count"].as_u64() {
        metadata.insert("igdb_rating_count".to_string(), rating_count.to_string());
    }

    // Release date
    if let Some(release_date) = igdb_details["first_release_date"].as_i64() {
        let year =
            chrono::DateTime::from_timestamp(release_date, 0).map(|dt| dt.format("%Y").to_string());
        if let Some(year_str) = year {
            metadata.insert("igdb_release_year".to_string(), year_str);
        }
    }

    // Genres
    if let Some(genres) = igdb_details["genres"].as_array() {
        let genre_names: Vec<String> = genres
            .iter()
            .filter_map(|g| g["name"].as_str())
            .map(|s| s.to_string())
            .collect();
        if !genre_names.is_empty() {
            metadata.insert("igdb_genres".to_string(), genre_names.join(", "));
        }
    }

    // Platforms
    if let Some(platforms) = igdb_details["platforms"].as_array() {
        let platform_names: Vec<String> = platforms
            .iter()
            .filter_map(|p| p["name"].as_str())
            .map(|s| s.to_string())
            .collect();
        if !platform_names.is_empty() {
            metadata.insert("igdb_platforms".to_string(), platform_names.join(", "));
        }
    }

    // Game modes
    if let Some(game_modes) = igdb_details["game_modes"].as_array() {
        let mode_names: Vec<String> = game_modes
            .iter()
            .filter_map(|m| m["name"].as_str())
            .map(|s| s.to_string())
            .collect();
        if !mode_names.is_empty() {
            metadata.insert("igdb_game_modes".to_string(), mode_names.join(", "));
        }
    }

    // Themes
    if let Some(themes) = igdb_details["themes"].as_array() {
        let theme_names: Vec<String> = themes
            .iter()
            .filter_map(|t| t["name"].as_str())
            .map(|s| s.to_string())
            .collect();
        if !theme_names.is_empty() {
            metadata.insert("igdb_themes".to_string(), theme_names.join(", "));
        }
    }

    // Player perspectives
    if let Some(perspectives) = igdb_details["player_perspectives"].as_array() {
        let perspective_names: Vec<String> = perspectives
            .iter()
            .filter_map(|p| p["name"].as_str())
            .map(|s| s.to_string())
            .collect();
        if !perspective_names.is_empty() {
            metadata.insert(
                "igdb_player_perspectives".to_string(),
                perspective_names.join(", "),
            );
        }
    }

    // Franchises
    if let Some(franchises) = igdb_details["franchises"].as_array() {
        let franchise_names: Vec<String> = franchises
            .iter()
            .filter_map(|f| f["name"].as_str())
            .map(|s| s.to_string())
            .collect();
        if !franchise_names.is_empty() {
            metadata.insert("igdb_franchises".to_string(), franchise_names.join(", "));
        }
    }

    // Companies (developers and publishers)
    if let Some(companies) = igdb_details.get("involved_companies") {
        let (developers, publishers) = extract_igdb_companies(companies);
        if !developers.is_empty() {
            metadata.insert("igdb_developer".to_string(), developers.join(", "));
        }
        if !publishers.is_empty() {
            metadata.insert("igdb_publisher".to_string(), publishers.join(", "));
        }
    }

    // Cover art
    if let Some(cover) = igdb_details.get("cover") {
        if let Some(cover_url) = extract_igdb_cover_url(cover) {
            metadata.insert("igdb_cover_url".to_string(), cover_url);
        }
    }

    // Screenshots
    if let Some(screenshots) = igdb_details["screenshots"].as_array() {
        let screenshot_urls: Vec<String> = screenshots
            .iter()
            .filter_map(|s| s["url"].as_str())
            .map(|url| format!("https:{}", url.replace("t_thumb", "t_1080p"))) // Get high-res version
            .collect();
        if !screenshot_urls.is_empty() {
            metadata.insert("igdb_screenshots".to_string(), screenshot_urls.join(","));
        }
    }

    // Artworks
    if let Some(artworks) = igdb_details["artworks"].as_array() {
        let artwork_urls: Vec<String> = artworks
            .iter()
            .filter_map(|a| a["url"].as_str())
            .map(|url| format!("https:{}", url.replace("t_thumb", "t_1080p")))
            .collect();
        if !artwork_urls.is_empty() {
            metadata.insert("igdb_artwork_urls".to_string(), artwork_urls.join(","));
        }
    }

    // Websites
    if let Some(websites) = igdb_details["websites"].as_array() {
        let website_urls: Vec<String> = websites
            .iter()
            .filter_map(|w| w["url"].as_str())
            .map(|s| s.to_string())
            .collect();
        if !website_urls.is_empty() {
            metadata.insert("igdb_websites".to_string(), website_urls.join(","));
        }
    }

    metadata
}

/// Extract cover URL from IGDB response
pub fn extract_igdb_cover_url(cover: &Value) -> Option<String> {
    cover.get("url").and_then(|url| url.as_str()).map(|url| {
        // IGDB returns URLs like "//images.igdb.com/...", we need to add https:
        if url.starts_with("//") {
            format!("https:{}", url)
        } else {
            url.to_string()
        }
    })
}
