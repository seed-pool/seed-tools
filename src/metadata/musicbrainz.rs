// MusicBrainz API integration for audio metadata enrichment

use reqwest::blocking::Client;
use serde_json::Value;
use log::info;
use crate::core::error::{SeedError, Result};

/// Search for release on MusicBrainz
pub fn search_musicbrainz_release(
    artist: &str,
    album: &str,
) -> Result<Vec<Value>> {
    let client = Client::new();
    
    // Build search query
    let query = format!("artist:{} AND release:{}", 
        urlencoding::encode(artist),
        urlencoding::encode(album)
    );
    
    let url = format!(
        "https://musicbrainz.org/ws/2/release/?query={}&fmt=json&limit=5",
        urlencoding::encode(&query)
    );
    
    info!("Searching MusicBrainz for: {} - {}", artist, album);
    
    let response = client
        .get(&url)
        .header("User-Agent", "seedbrr/1.0 (https://github.com/seed-pool/seed-tools)")
        .send()
        .map_err(|e| SeedError::ApiError(format!("Failed to query MusicBrainz: {}", e)))?;
    
    if !response.status().is_success() {
        return Err(SeedError::ApiError(format!(
            "MusicBrainz API request failed with status: {}",
            response.status()
        )));
    }
    
    let json: Value = response
        .json()
        .map_err(|e| SeedError::ApiError(format!("Failed to parse MusicBrainz response: {}", e)))?;
    
    let releases = json["releases"].as_array()
        .unwrap_or(&vec![])
        .clone();
    
    info!("Found {} MusicBrainz releases", releases.len());
    Ok(releases)
}

/// Get detailed release information from MusicBrainz
pub fn get_musicbrainz_release_details(
    release_id: &str,
) -> Result<Value> {
    let client = Client::new();
    
    let url = format!(
        "https://musicbrainz.org/ws/2/release/{}?fmt=json&inc=artist-credits+labels+recordings+release-groups",
        release_id
    );
    
    info!("Fetching MusicBrainz release details for ID: {}", release_id);
    
    let response = client
        .get(&url)
        .header("User-Agent", "seedbrr/1.0 (https://github.com/seed-pool/seed-tools)")
        .send()
        .map_err(|e| SeedError::ApiError(format!("Failed to fetch MusicBrainz details: {}", e)))?;
    
    if !response.status().is_success() {
        return Err(SeedError::ApiError(format!(
            "MusicBrainz details API request failed with status: {}",
            response.status()
        )));
    }
    
    let json: Value = response
        .json()
        .map_err(|e| SeedError::ApiError(format!("Failed to parse MusicBrainz details: {}", e)))?;
    
    info!("✅ Successfully fetched MusicBrainz details for ID: {}", release_id);
    Ok(json)
}

/// Extract comprehensive metadata from MusicBrainz release details
pub fn extract_musicbrainz_metadata(mb_details: &Value) -> std::collections::HashMap<String, String> {
    let mut metadata = std::collections::HashMap::new();
    
    // Basic release info
    if let Some(title) = mb_details["title"].as_str() {
        metadata.insert("musicbrainz_title".to_string(), title.to_string());
    }
    
    if let Some(release_id) = mb_details["id"].as_str() {
        metadata.insert("musicbrainz_release_id".to_string(), release_id.to_string());
    }
    
    if let Some(status) = mb_details["status"].as_str() {
        metadata.insert("musicbrainz_status".to_string(), status.to_string());
    }
    
    if let Some(date) = mb_details["date"].as_str() {
        metadata.insert("musicbrainz_date".to_string(), date.to_string());
        // Extract year from date
        if let Some(year) = date.split('-').next() {
            metadata.insert("musicbrainz_year".to_string(), year.to_string());
        }
    }
    
    if let Some(country) = mb_details["country"].as_str() {
        metadata.insert("musicbrainz_country".to_string(), country.to_string());
    }
    
    if let Some(barcode) = mb_details["barcode"].as_str() {
        metadata.insert("musicbrainz_barcode".to_string(), barcode.to_string());
    }
    
    // Artist credits
    if let Some(artist_credits) = mb_details["artist-credit"].as_array() {
        let artists: Vec<String> = artist_credits.iter()
            .filter_map(|ac| ac["artist"]["name"].as_str())
            .map(|s| s.to_string())
            .collect();
        if !artists.is_empty() {
            metadata.insert("musicbrainz_artist".to_string(), artists.join(", "));
        }
        
        // Also get artist MBIDs
        let artist_ids: Vec<String> = artist_credits.iter()
            .filter_map(|ac| ac["artist"]["id"].as_str())
            .map(|s| s.to_string())
            .collect();
        if !artist_ids.is_empty() {
            metadata.insert("musicbrainz_artist_ids".to_string(), artist_ids.join(","));
        }
    }
    
    // Release group info
    if let Some(release_group) = mb_details["release-group"].as_object() {
        if let Some(rg_id) = release_group["id"].as_str() {
            metadata.insert("musicbrainz_release_group_id".to_string(), rg_id.to_string());
        }
        
        if let Some(rg_type) = release_group["primary-type"].as_str() {
            metadata.insert("musicbrainz_primary_type".to_string(), rg_type.to_string());
        }
        
        if let Some(secondary_types) = release_group["secondary-types"].as_array() {
            let types: Vec<String> = secondary_types.iter()
                .filter_map(|t| t.as_str())
                .map(|s| s.to_string())
                .collect();
            if !types.is_empty() {
                metadata.insert("musicbrainz_secondary_types".to_string(), types.join(", "));
            }
        }
    }
    
    // Label info
    if let Some(label_info) = mb_details["label-info"].as_array() {
        let labels: Vec<String> = label_info.iter()
            .filter_map(|li| li["label"]["name"].as_str())
            .map(|s| s.to_string())
            .collect();
        if !labels.is_empty() {
            metadata.insert("musicbrainz_labels".to_string(), labels.join(", "));
        }
        
        let catalog_numbers: Vec<String> = label_info.iter()
            .filter_map(|li| li["catalog-number"].as_str())
            .map(|s| s.to_string())
            .collect();
        if !catalog_numbers.is_empty() {
            metadata.insert("musicbrainz_catalog_numbers".to_string(), catalog_numbers.join(", "));
        }
    }
    
    // Track info (if available)
    if let Some(media) = mb_details["media"].as_array() {
        let mut total_tracks = 0;
        let mut total_length = 0;
        let mut track_titles = Vec::new();
        
        for medium in media {
            if let Some(tracks) = medium["tracks"].as_array() {
                total_tracks += tracks.len();
                
                for track in tracks {
                    if let Some(title) = track["title"].as_str() {
                        track_titles.push(title.to_string());
                    }
                    if let Some(length) = track["length"].as_u64() {
                        total_length += length;
                    }
                }
            }
        }
        
        if total_tracks > 0 {
            metadata.insert("musicbrainz_track_count".to_string(), total_tracks.to_string());
        }
        
        if total_length > 0 {
            let length_minutes = total_length / 60000; // Convert from ms to minutes
            metadata.insert("musicbrainz_total_length".to_string(), format!("{} min", length_minutes));
        }
        
        if !track_titles.is_empty() {
            metadata.insert("musicbrainz_tracklist".to_string(), track_titles.join("\n"));
        }
    }
    
    metadata
}

/// Search for artist information on MusicBrainz
pub fn search_musicbrainz_artist(artist_name: &str) -> Result<Vec<Value>> {
    let client = Client::new();
    
    let query = format!("artist:{}", urlencoding::encode(artist_name));
    let url = format!(
        "https://musicbrainz.org/ws/2/artist/?query={}&fmt=json&limit=5",
        urlencoding::encode(&query)
    );
    
    info!("Searching MusicBrainz for artist: {}", artist_name);
    
    let response = client
        .get(&url)
        .header("User-Agent", "seedbrr/1.0 (https://github.com/seed-pool/seed-tools)")
        .send()
        .map_err(|e| SeedError::ApiError(format!("Failed to query MusicBrainz for artist: {}", e)))?;
    
    if !response.status().is_success() {
        return Err(SeedError::ApiError(format!(
            "MusicBrainz artist API request failed with status: {}",
            response.status()
        )));
    }
    
    let json: Value = response
        .json()
        .map_err(|e| SeedError::ApiError(format!("Failed to parse MusicBrainz artist response: {}", e)))?;
    
    let artists = json["artists"].as_array()
        .unwrap_or(&vec![])
        .clone();
    
    info!("Found {} MusicBrainz artists", artists.len());
    Ok(artists)
}

/// Get detailed artist information from MusicBrainz
pub fn get_musicbrainz_artist_details(artist_id: &str) -> Result<Value> {
    let client = Client::new();
    
    let url = format!(
        "https://musicbrainz.org/ws/2/artist/{}?fmt=json&inc=genres+tags+ratings",
        artist_id
    );
    
    info!("Fetching MusicBrainz artist details for ID: {}", artist_id);
    
    let response = client
        .get(&url)
        .header("User-Agent", "seedbrr/1.0 (https://github.com/seed-pool/seed-tools)")
        .send()
        .map_err(|e| SeedError::ApiError(format!("Failed to fetch MusicBrainz artist details: {}", e)))?;
    
    if !response.status().is_success() {
        return Err(SeedError::ApiError(format!(
            "MusicBrainz artist details API request failed with status: {}",
            response.status()
        )));
    }
    
    let json: Value = response
        .json()
        .map_err(|e| SeedError::ApiError(format!("Failed to parse MusicBrainz artist details: {}", e)))?;
    
    info!("✅ Successfully fetched MusicBrainz artist details for ID: {}", artist_id);
    Ok(json)
}