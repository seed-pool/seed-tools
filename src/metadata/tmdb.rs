// The Movie Database (TMDB) API integration

use crate::core::error::{Result, SeedError};
use log::info;
use regex::Regex;
use reqwest::blocking::Client;
use serde_json::Value;

/// Fetch TMDB ID for a movie or TV show
pub fn fetch_tmdb_id(
    title: &str,
    year: Option<String>,
    tmdb_api_key: &str,
    release_type: &str,
) -> Result<u32> {
    info!(
        "🎬 Starting TMDB lookup for '{}' (type: {}, year: {:?})",
        title, release_type, year
    );

    let sanitized_title = if release_type == "TvShow" {
        // Extract everything before the SXX* pattern
        let season_regex = Regex::new(r"(?i)(S\d{2}.*)").unwrap();
        let cleaned_title = season_regex.replace(title, "").trim().to_string();

        // Remove the year if present
        let year_regex = Regex::new(r"\b(19|20)\d{2}\b").unwrap();
        year_regex.replace(&cleaned_title, "").trim().to_string()
    } else {
        // For movies, extract everything before the year
        let year_regex = Regex::new(r"\b(19|20)\d{2}\b").unwrap();
        year_regex.replace(title, "").trim().to_string()
    };

    info!(
        "🧹 Cleaned TMDB title: '{}' -> '{}'",
        title, sanitized_title
    );
    let encoded_title = urlencoding::encode(&sanitized_title);

    let url = if release_type.to_lowercase() == "tv" || release_type == "TvShow" {
        format!(
            "https://api.themoviedb.org/3/search/tv?query={}&first_air_date_year={}&api_key={}",
            encoded_title,
            year.unwrap_or_default(),
            tmdb_api_key
        )
    } else {
        format!(
            "https://api.themoviedb.org/3/search/movie?query={}&year={}&api_key={}",
            encoded_title,
            year.unwrap_or_default(),
            tmdb_api_key
        )
    };

    info!("TMDB API URL: {}", url);

    let client = Client::new();
    let response = client
        .get(&url)
        .send()
        .map_err(|e| SeedError::ApiError(format!("Failed to query TMDB for '{}': {}", title, e)))?;

    if !response.status().is_success() {
        return Err(SeedError::ApiError(format!(
            "TMDB API request failed with status: {}",
            response.status()
        )));
    }

    let json: Value = response.json().map_err(|e| {
        SeedError::ApiError(format!(
            "Failed to parse TMDB response for '{}': {}",
            title, e
        ))
    })?;

    // Log response status instead of full JSON to avoid breaking UI
    if let Some(total_results) = json["total_results"].as_u64() {
        info!("📊 TMDB API returned {} total results", total_results);
    }

    let empty_vec = vec![];
    let results = json["results"].as_array().unwrap_or(&empty_vec);
    info!("🔍 Found {} TMDB results", results.len());

    let tmdb_id = results
        .get(0)
        .and_then(|result| {
            if let Some(title) = result["title"].as_str().or_else(|| result["name"].as_str()) {
                info!("📽️  First result: '{}'", title);
            }
            result["id"].as_u64()
        })
        .unwrap_or(0) as u32;

    if tmdb_id == 0 {
        info!("❌ No TMDB ID found for '{}'.", title);
    } else {
        info!("✅ Found TMDB ID: {} for '{}'", tmdb_id, title);
    }

    Ok(tmdb_id)
}

/// Fetch external IDs (IMDB, TVDB) from TMDB
pub fn fetch_external_ids(
    tmdb_id: u32,
    release_type: &str,
    tmdb_api_key: &str,
) -> Result<(Option<String>, Option<u32>)> {
    if tmdb_id == 0 {
        return Ok((None, None));
    }

    let tmdb_type = if release_type == "boxset" {
        "tv"
    } else {
        release_type
    };
    let url = format!(
        "https://api.themoviedb.org/3/{}/{}/external_ids?api_key={}",
        tmdb_type, tmdb_id, tmdb_api_key
    );

    info!("TMDB External IDs API URL: {}", url);

    let client = Client::new();
    let response = client
        .get(&url)
        .send()
        .map_err(|e| SeedError::ApiError(format!("Failed to fetch external IDs: {}", e)))?;

    if !response.status().is_success() {
        return Err(SeedError::ApiError(format!(
            "Failed to fetch external IDs: HTTP {}",
            response.status()
        )));
    }

    let json: Value = response.json().map_err(|e| {
        SeedError::ApiError(format!("Failed to parse external IDs response: {}", e))
    })?;

    let imdb_id = json["imdb_id"].as_str().map(|s| s.to_string());
    let tvdb_id = json["tvdb_id"].as_u64().map(|id| id as u32);

    info!("Fetched IMDb ID: {:?}", imdb_id);
    info!("Fetched TVDB ID: {:?}", tvdb_id);

    Ok((imdb_id, tvdb_id))
}

/// Fetch comprehensive movie/TV details from TMDB
pub fn fetch_tmdb_details(tmdb_id: u32, release_type: &str, tmdb_api_key: &str) -> Result<Value> {
    if tmdb_id == 0 {
        return Err(SeedError::ApiError("Invalid TMDB ID".to_string()));
    }

    let tmdb_type = if release_type == "tv" || release_type == "TvShow" {
        "tv"
    } else {
        "movie"
    };
    let url = format!(
        "https://api.themoviedb.org/3/{}/{}?api_key={}&append_to_response=credits,videos,images,keywords",
        tmdb_type, tmdb_id, tmdb_api_key
    );

    info!(
        "Fetching TMDB details for ID: {} (type: {})",
        tmdb_id, tmdb_type
    );

    let client = Client::new();
    let response = client
        .get(&url)
        .send()
        .map_err(|e| SeedError::ApiError(format!("Failed to fetch TMDB details: {}", e)))?;

    if !response.status().is_success() {
        return Err(SeedError::ApiError(format!(
            "TMDB details API request failed with status: {}",
            response.status()
        )));
    }

    let json: Value = response.json().map_err(|e| {
        SeedError::ApiError(format!("Failed to parse TMDB details response: {}", e))
    })?;

    info!("✅ Successfully fetched TMDB details for ID: {}", tmdb_id);
    Ok(json)
}

/// Extract comprehensive metadata from TMDB details response
pub fn extract_tmdb_metadata(
    tmdb_details: &Value,
    release_type: &str,
) -> std::collections::HashMap<String, String> {
    info!(
        "🎬 Extracting TMDB metadata for release type: {}",
        release_type
    );
    info!(
        "📊 TMDB details received: {}",
        serde_json::to_string_pretty(tmdb_details).unwrap_or_else(|_| "Invalid JSON".to_string())
    );

    let mut metadata = std::collections::HashMap::new();

    // Basic info
    if let Some(overview) = tmdb_details["overview"].as_str() {
        metadata.insert("tmdb_overview".to_string(), overview.to_string());
    }

    if let Some(rating) = tmdb_details["vote_average"].as_f64() {
        metadata.insert("tmdb_rating".to_string(), format!("{:.1}", rating));
        metadata.insert("tmdb_vote_average".to_string(), format!("{:.1}", rating));
    }

    if let Some(vote_count) = tmdb_details["vote_count"].as_u64() {
        metadata.insert("tmdb_vote_count".to_string(), vote_count.to_string());
    }

    // Title and year
    if let Some(title) = tmdb_details
        .get("title")
        .or(tmdb_details.get("name"))
        .and_then(|v| v.as_str())
    {
        metadata.insert("tmdb_title".to_string(), title.to_string());
    }

    // Release date
    if let Some(release_date) = tmdb_details
        .get("release_date")
        .or(tmdb_details.get("first_air_date"))
        .and_then(|v| v.as_str())
    {
        metadata.insert("tmdb_release_date".to_string(), release_date.to_string());
        // Extract year from date
        if let Some(year) = release_date.split('-').next() {
            metadata.insert("tmdb_year".to_string(), year.to_string());
        }
    }

    // Original language
    if let Some(lang) = tmdb_details["original_language"].as_str() {
        metadata.insert("tmdb_original_language".to_string(), lang.to_uppercase());
    }

    // Genres
    if let Some(genres) = tmdb_details["genres"].as_array() {
        let genre_names: Vec<String> = genres
            .iter()
            .filter_map(|g| g["name"].as_str())
            .map(|s| s.to_string())
            .collect();
        if !genre_names.is_empty() {
            metadata.insert("tmdb_genres".to_string(), genre_names.join(", "));
        }
    }

    // Runtime/Episode runtime
    if release_type == "tv" || release_type == "TvShow" {
        if let Some(runtime) = tmdb_details["episode_run_time"]
            .as_array()
            .and_then(|arr| arr.get(0))
            .and_then(|v| v.as_u64())
        {
            metadata.insert("tmdb_runtime".to_string(), format!("{} min", runtime));
        }

        if let Some(status) = tmdb_details["status"].as_str() {
            metadata.insert("tmdb_status".to_string(), status.to_string());
        }

        if let Some(networks) = tmdb_details["networks"].as_array() {
            let network_names: Vec<String> = networks
                .iter()
                .filter_map(|n| n["name"].as_str())
                .map(|s| s.to_string())
                .collect();
            if !network_names.is_empty() {
                metadata.insert("tmdb_networks".to_string(), network_names.join(", "));
            }
        }
    } else {
        if let Some(runtime) = tmdb_details["runtime"].as_u64() {
            metadata.insert("tmdb_runtime".to_string(), format!("{} min", runtime));
        }

        if let Some(budget) = tmdb_details["budget"].as_u64() {
            if budget > 0 {
                metadata.insert("tmdb_budget".to_string(), format!("${}", budget));
            }
        }

        if let Some(revenue) = tmdb_details["revenue"].as_u64() {
            if revenue > 0 {
                metadata.insert("tmdb_revenue".to_string(), format!("${}", revenue));
            }
        }
    }

    // Cast and crew
    if let Some(credits) = tmdb_details["credits"].as_object() {
        if let Some(cast) = credits["cast"].as_array() {
            let main_cast: Vec<String> = cast
                .iter()
                .take(5) // Top 5 cast members
                .filter_map(|c| c["name"].as_str())
                .map(|s| s.to_string())
                .collect();
            if !main_cast.is_empty() {
                metadata.insert("tmdb_cast".to_string(), main_cast.join(", "));
            }
        }

        if let Some(crew) = credits["crew"].as_array() {
            let directors: Vec<String> = crew
                .iter()
                .filter(|c| c["job"].as_str() == Some("Director"))
                .filter_map(|c| c["name"].as_str())
                .map(|s| s.to_string())
                .collect();
            if !directors.is_empty() {
                metadata.insert("tmdb_directors".to_string(), directors.join(", "));
            }

            let writers: Vec<String> = crew
                .iter()
                .filter(|c| c["department"].as_str() == Some("Writing"))
                .filter_map(|c| c["name"].as_str())
                .map(|s| s.to_string())
                .collect();
            if !writers.is_empty() {
                metadata.insert("tmdb_writers".to_string(), writers.join(", "));
            }
        }
    }

    // Videos (trailers)
    if let Some(videos) = tmdb_details["videos"]["results"].as_array() {
        let trailers: Vec<String> = videos
            .iter()
            .filter(|v| {
                v["type"].as_str() == Some("Trailer") && v["site"].as_str() == Some("YouTube")
            })
            .filter_map(|v| v["key"].as_str())
            .map(|key| format!("https://www.youtube.com/watch?v={}", key))
            .collect();
        if !trailers.is_empty() {
            metadata.insert("tmdb_trailer_url".to_string(), trailers[0].clone());
        }
    }

    // Production companies
    if let Some(companies) = tmdb_details["production_companies"].as_array() {
        let company_names: Vec<String> = companies
            .iter()
            .filter_map(|c| c["name"].as_str())
            .map(|s| s.to_string())
            .collect();
        if !company_names.is_empty() {
            metadata.insert(
                "tmdb_production_companies".to_string(),
                company_names.join(", "),
            );
        }
    }

    // Keywords
    let keywords_key = if release_type == "tv" || release_type == "TvShow" {
        "results"
    } else {
        "keywords"
    };
    if let Some(keywords) = tmdb_details["keywords"][keywords_key].as_array() {
        let keyword_names: Vec<String> = keywords
            .iter()
            .filter_map(|k| k["name"].as_str())
            .map(|s| s.to_string())
            .collect();
        if !keyword_names.is_empty() {
            metadata.insert("tmdb_keywords".to_string(), keyword_names.join(", "));
        }
    }

    info!("✅ Extracted {} TMDB metadata fields:", metadata.len());
    for (key, value) in &metadata {
        info!("  📌 {}: {}", key, value);
    }

    metadata
}

/// Fetch YouTube trailer URL
pub fn fetch_youtube_trailer(
    title: &str,
    year: Option<&str>,
    youtube_api_key: &str,
) -> Result<String> {
    let client = Client::new();

    // Construct the search query
    let query = if let Some(year) = year {
        format!("{} {} trailer", title, year)
    } else {
        format!("{} trailer", title)
    };

    // Construct the YouTube Data API URL
    let url = format!(
        "https://www.googleapis.com/youtube/v3/search?part=snippet&q={}&type=video&key={}&maxResults=1",
        urlencoding::encode(&query),
        youtube_api_key
    );

    info!("YouTube API URL: {}", url);

    let response = client
        .get(&url)
        .send()
        .map_err(|e| SeedError::ApiError(format!("Failed to query YouTube: {}", e)))?;

    if !response.status().is_success() {
        return Err(SeedError::ApiError(format!(
            "YouTube API request failed with status: {}",
            response.status()
        )));
    }

    let json: Value = response
        .json()
        .map_err(|e| SeedError::ApiError(format!("Failed to parse YouTube response: {}", e)))?;

    // Extract the video ID from the response
    let video_id = json["items"]
        .as_array()
        .and_then(|items| items.get(0))
        .and_then(|item| item["id"]["videoId"].as_str())
        .ok_or_else(|| SeedError::ApiError("No YouTube trailer found".to_string()))?;

    let youtube_url = format!("https://www.youtube.com/watch?v={}", video_id);
    info!("Found YouTube trailer: {}", youtube_url);

    Ok(youtube_url)
}
