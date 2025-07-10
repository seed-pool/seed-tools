// MusicBrainz API integration for audio metadata enrichment

use crate::core::error::{Result, SeedError};
use log::info;
use reqwest::blocking::Client;
use serde_json::Value;

/// Search for release on MusicBrainz
pub fn search_musicbrainz_release(artist: &str, album: &str) -> Result<Vec<Value>> {
    search_musicbrainz_release_with_year(artist, album, None)
}

/// Search for release on MusicBrainz with optional year
pub fn search_musicbrainz_release_with_year(
    artist: &str,
    album: &str,
    year: Option<&str>,
) -> Result<Vec<Value>> {
    let client = Client::new();

    // Clean the artist and album names
    let clean_artist = artist
        .trim()
        .replace('\u{2019}', "'")
        .replace('\u{2018}', "'");

    let clean_album = album
        .trim()
        .replace('\u{2019}', "'")
        .replace('\u{2018}', "'");

    // Try different search strategies
    let search_strategies = vec![
        // Strategy 1: Exact search with AND
        (
            format!(
                "artist:{} AND release:{}",
                urlencoding::encode(&clean_artist),
                urlencoding::encode(&clean_album)
            ),
            "exact with AND",
        ),
        // Strategy 2: Remove parenthetical content
        (
            format!(
                "artist:{} AND release:{}",
                urlencoding::encode(&clean_artist),
                urlencoding::encode(&clean_album.split('(').next().unwrap_or(&clean_album).trim())
            ),
            "without parentheses",
        ),
        // Strategy 3: Just the base album name (no Remixed, Remastered, etc)
        (
            format!(
                "artist:{} AND release:{}",
                urlencoding::encode(&clean_artist),
                urlencoding::encode(
                    &clean_album
                        .replace("(Remixed)", "")
                        .replace("(Remastered)", "")
                        .replace("(Deluxe)", "")
                        .replace("(Expanded)", "")
                        .trim()
                )
            ),
            "base album name",
        ),
        // Strategy 4: Loose search with just artist and album words
        (format!("{} {}", clean_artist, clean_album), "loose search"),
        // Strategy 5: Artist only, then filter results
        (
            format!("artist:{}", urlencoding::encode(&clean_artist)),
            "artist only",
        ),
    ];

    for (query_base, strategy) in search_strategies {
        // Add year if provided (except for loose search)
        let query =
            if year.is_some() && !strategy.contains("loose") && !strategy.contains("artist only") {
                format!("{} AND date:{}", query_base, year.unwrap())
            } else {
                query_base
            };

        let url = format!(
            "https://musicbrainz.org/ws/2/release/?query={}&fmt=json&limit=10",
            urlencoding::encode(&query)
        );

        info!(
            "MusicBrainz search strategy '{}': {} - {}",
            strategy, artist, album
        );

        let response = client
            .get(&url)
            .header(
                "User-Agent",
                "seedbrr/1.0 (https://github.com/seed-pool/seed-tools)",
            )
            .send()
            .map_err(|e| SeedError::ApiError(format!("Failed to query MusicBrainz: {}", e)))?;

        if !response.status().is_success() {
            continue; // Try next variation
        }

        let json: Value = response.json().map_err(|e| {
            SeedError::ApiError(format!("Failed to parse MusicBrainz response: {}", e))
        })?;

        let mut releases = json["releases"].as_array().unwrap_or(&vec![]).clone();

        info!(
            "Found {} MusicBrainz releases using strategy '{}'",
            releases.len(),
            strategy
        );

        // For broader searches, filter results to find best match
        if strategy.contains("loose") || strategy.contains("artist only") {
            releases = releases
                .into_iter()
                .filter(|release| {
                    // Check if artist matches
                    if let Some(artist_credit) = release["artist-credit"].as_array() {
                        let release_artist = artist_credit
                            .iter()
                            .filter_map(|ac| ac["artist"]["name"].as_str())
                            .collect::<Vec<_>>()
                            .join(" ");

                        if !release_artist
                            .to_lowercase()
                            .contains(&clean_artist.to_lowercase())
                        {
                            return false;
                        }
                    }

                    // For artist-only search, also check album name
                    if strategy.contains("artist only") {
                        if let Some(title) = release["title"].as_str() {
                            let title_lower = title.to_lowercase();
                            let album_lower = clean_album.to_lowercase();

                            // Check if the release title contains key words from our album
                            let album_words: Vec<&str> = album_lower
                                .split(|c: char| !c.is_alphanumeric())
                                .filter(|w| w.len() > 2) // Skip short words
                                .collect();

                            let matching_words = album_words
                                .iter()
                                .filter(|word| title_lower.contains(*word))
                                .count();

                            // Require at least 50% of words to match
                            if matching_words < album_words.len() / 2 {
                                return false;
                            }
                        }
                    }

                    true
                })
                .collect();

            info!("After filtering: {} releases remain", releases.len());
        }

        if !releases.is_empty() {
            // Sort by relevance (prefer exact matches)
            releases.sort_by(|a, b| {
                let a_title = a["title"].as_str().unwrap_or("");
                let b_title = b["title"].as_str().unwrap_or("");

                // Prefer exact matches
                let a_exact = a_title.eq_ignore_ascii_case(&clean_album);
                let b_exact = b_title.eq_ignore_ascii_case(&clean_album);

                match (a_exact, b_exact) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => {
                        // Then prefer matches without parentheses
                        let a_base = a_title.eq_ignore_ascii_case(
                            &clean_album.split('(').next().unwrap_or(&clean_album).trim(),
                        );
                        let b_base = b_title.eq_ignore_ascii_case(
                            &clean_album.split('(').next().unwrap_or(&clean_album).trim(),
                        );

                        match (a_base, b_base) {
                            (true, false) => std::cmp::Ordering::Less,
                            (false, true) => std::cmp::Ordering::Greater,
                            _ => std::cmp::Ordering::Equal,
                        }
                    }
                }
            });

            // Log the top matches
            for (i, release) in releases.iter().take(3).enumerate() {
                if let Some(title) = release["title"].as_str() {
                    info!("  Match {}: {}", i + 1, title);
                }
            }

            return Ok(releases);
        }
    }

    // If no strategies found results, return empty vec
    info!("No MusicBrainz releases found after trying all strategies");
    Ok(vec![])
}

/// Get detailed release information from MusicBrainz
pub fn get_musicbrainz_release_details(release_id: &str) -> Result<Value> {
    let client = Client::new();

    let url = format!(
        "https://musicbrainz.org/ws/2/release/{}?fmt=json&inc=artist-credits+labels+recordings+release-groups",
        release_id
    );

    info!(
        "Fetching MusicBrainz release details for ID: {}",
        release_id
    );

    let response = client
        .get(&url)
        .header(
            "User-Agent",
            "seedbrr/1.0 (https://github.com/seed-pool/seed-tools)",
        )
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

    info!(
        "✅ Successfully fetched MusicBrainz details for ID: {}",
        release_id
    );
    Ok(json)
}

/// Extract comprehensive metadata from MusicBrainz release details
pub fn extract_musicbrainz_metadata(
    mb_details: &Value,
) -> std::collections::HashMap<String, String> {
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
        let artists: Vec<String> = artist_credits
            .iter()
            .filter_map(|ac| ac["artist"]["name"].as_str())
            .map(|s| s.to_string())
            .collect();
        if !artists.is_empty() {
            metadata.insert("musicbrainz_artist".to_string(), artists.join(", "));
        }

        // Also get artist MBIDs
        let artist_ids: Vec<String> = artist_credits
            .iter()
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
            metadata.insert(
                "musicbrainz_release_group_id".to_string(),
                rg_id.to_string(),
            );
        }

        if let Some(rg_type) = release_group["primary-type"].as_str() {
            metadata.insert("musicbrainz_primary_type".to_string(), rg_type.to_string());
        }

        if let Some(secondary_types) = release_group["secondary-types"].as_array() {
            let types: Vec<String> = secondary_types
                .iter()
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
        let labels: Vec<String> = label_info
            .iter()
            .filter_map(|li| li["label"]["name"].as_str())
            .map(|s| s.to_string())
            .collect();
        if !labels.is_empty() {
            metadata.insert("musicbrainz_labels".to_string(), labels.join(", "));
        }

        let catalog_numbers: Vec<String> = label_info
            .iter()
            .filter_map(|li| li["catalog-number"].as_str())
            .map(|s| s.to_string())
            .collect();
        if !catalog_numbers.is_empty() {
            metadata.insert(
                "musicbrainz_catalog_numbers".to_string(),
                catalog_numbers.join(", "),
            );
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
            metadata.insert(
                "musicbrainz_track_count".to_string(),
                total_tracks.to_string(),
            );
        }

        if total_length > 0 {
            let length_minutes = total_length / 60000; // Convert from ms to minutes
            metadata.insert(
                "musicbrainz_total_length".to_string(),
                format!("{} min", length_minutes),
            );
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
        .header(
            "User-Agent",
            "seedbrr/1.0 (https://github.com/seed-pool/seed-tools)",
        )
        .send()
        .map_err(|e| {
            SeedError::ApiError(format!("Failed to query MusicBrainz for artist: {}", e))
        })?;

    if !response.status().is_success() {
        return Err(SeedError::ApiError(format!(
            "MusicBrainz artist API request failed with status: {}",
            response.status()
        )));
    }

    let json: Value = response.json().map_err(|e| {
        SeedError::ApiError(format!(
            "Failed to parse MusicBrainz artist response: {}",
            e
        ))
    })?;

    let artists = json["artists"].as_array().unwrap_or(&vec![]).clone();

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
        .header(
            "User-Agent",
            "seedbrr/1.0 (https://github.com/seed-pool/seed-tools)",
        )
        .send()
        .map_err(|e| {
            SeedError::ApiError(format!("Failed to fetch MusicBrainz artist details: {}", e))
        })?;

    if !response.status().is_success() {
        return Err(SeedError::ApiError(format!(
            "MusicBrainz artist details API request failed with status: {}",
            response.status()
        )));
    }

    let json: Value = response.json().map_err(|e| {
        SeedError::ApiError(format!("Failed to parse MusicBrainz artist details: {}", e))
    })?;

    info!(
        "✅ Successfully fetched MusicBrainz artist details for ID: {}",
        artist_id
    );
    Ok(json)
}
