use crate::media::{
    audio::process_audio,
    video::process_video,
    ebook::process_ebook,
    game::process_game,
    hobby::process_hobby,
};
use crate::types::{Config, MediaType, PreflightCheckResult};
use crate::utils::{check_all_duplicates, fetch_tmdb_id, generate_mediainfo};
use crate::naming::generate_release_name;
use std::path::Path;
use log::{info, debug};

/// Perform a preflight check on the input path to analyze what would happen during upload
pub fn preflight_check(
    input_path: &str,
    config: &Config,
    dry_run: bool,
) -> Result<PreflightCheckResult, String> {
    info!("Running preflight check for: {}", input_path);
    
    // Step 1: Detect media type and process using the appropriate module
    let (media_type, title, metadata) = detect_and_analyze_media(input_path, config, dry_run)?;
    
    // Step 2: Generate release name
    let base_name = Path::new(input_path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let generated_release_name = generate_release_name(&base_name);
    
    // Step 3: Check for duplicates
    let dupe_check_result = match check_all_duplicates(&title) {
        Ok(duplicates) if !duplicates.is_empty() => {
            let dupe_list = duplicates.iter()
                .map(|(tracker, _)| tracker.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            format!("FAIL (found on: {})", dupe_list)
        }
        _ => "✔️ PASS".to_string()
    };
    
    // Step 4: Extract additional metadata based on media type
    let mut result = PreflightCheckResult {
        release_name: title.clone(),
        generated_release_name,
        dupe_check: dupe_check_result,
        tmdb_id: 0,
        imdb_id: None,
        tvdb_id: None,
        excluded_files: "N/A".to_string(),
        album_cover: "N/A".to_string(),
        audio_languages: Vec::new(),
        release_type: format_release_type(&media_type),
        season_number: None,
        episode_number: None,
    };
    
    // Media-specific metadata extraction
    match media_type {
        MediaType::Video(_) => {
            extract_video_metadata(input_path, config, &metadata, &mut result)?;
        }
        MediaType::Audio(_) => {
            extract_audio_metadata(input_path, &metadata, &mut result)?;
        }
        MediaType::Ebook(_) => {
            result.album_cover = check_for_cover_image(input_path);
        }
        _ => {}
    }
    
    Ok(result)
}

/// Detect media type and analyze using the appropriate processor
fn detect_and_analyze_media(
    input_path: &str,
    config: &Config,
    dry_run: bool,
) -> Result<(MediaType, String, serde_json::Value), String> {
    // Try each media processor to see which one can handle the input
    
    // Try video first
    if let Ok(video_results) = process_video(input_path, config, dry_run) {
        if !video_results.is_empty() {
            let (video_file, metadata) = &video_results[0];
            let title = metadata.title.clone();
            let media_type = MediaType::Video(video_file.video_type.clone());
            
            // Convert metadata to JSON for flexible handling
            let metadata_json = serde_json::json!({
                "title": metadata.title,
                "year": metadata.year,
                "category": format!("{:?}", metadata.category),
                "season": metadata.season,
                "episode": metadata.episode,
                "is_boxset": metadata.is_boxset,
                "is_dated_tv": metadata.is_dated_tv,
            });
            
            return Ok((media_type, title, metadata_json));
        }
    }
    
    // Try audio
    if let Ok(audio_results) = process_audio(input_path, config, dry_run) {
        if !audio_results.is_empty() {
            let (audio_file, metadata) = &audio_results[0];
            let title = format!("{} - {}", 
                metadata.artist.as_deref().unwrap_or("Unknown Artist"),
                metadata.album.as_deref().unwrap_or("Unknown Album")
            );
            let media_type = MediaType::Audio(audio_file.audio_type.clone());
            
            let metadata_json = serde_json::json!({
                "artist": metadata.artist,
                "album": metadata.album,
                "year": metadata.year,
                "format": format!("{:?}", audio_file.audio_type),
            });
            
            return Ok((media_type, title, metadata_json));
        }
    }
    
    // Try ebook
    if let Ok(ebook_results) = process_ebook(input_path, config, dry_run) {
        if !ebook_results.is_empty() {
            let (ebook_file, metadata) = &ebook_results[0];
            let title = if metadata.title.is_empty() {
                base_name_from_path(input_path)
            } else {
                metadata.title.clone()
            };
            let media_type = MediaType::Ebook(ebook_file.ebook_type.clone());
            
            let metadata_json = serde_json::json!({
                "title": metadata.title,
                "author": metadata.author,
                "category": format!("{:?}", metadata.category),
            });
            
            return Ok((media_type, title, metadata_json));
        }
    }
    
    // Try game
    if let Ok(game_results) = process_game(input_path, config, dry_run) {
        if !game_results.is_empty() {
            let (game_file, metadata) = &game_results[0];
            let title = metadata.title.clone();
            let media_type = MediaType::Game(game_file.game_type.clone());
            
            let metadata_json = serde_json::json!({
                "title": metadata.title,
                "platform": metadata.platform,
            });
            
            return Ok((media_type, title, metadata_json));
        }
    }
    
    // Try hobby
    if let Ok(hobby_results) = process_hobby(input_path, config, dry_run) {
        if !hobby_results.is_empty() {
            let (hobby_file, metadata) = &hobby_results[0];
            let title = metadata.title.clone();
            let media_type = MediaType::Hobby(hobby_file.hobby_type.clone());
            
            let metadata_json = serde_json::json!({
                "title": metadata.title,
                "category": format!("{:?}", metadata.category),
            });
            
            return Ok((media_type, title, metadata_json));
        }
    }
    
    Err("Unable to determine media type for input path".to_string())
}

/// Extract video-specific metadata
fn extract_video_metadata(
    input_path: &str,
    config: &Config,
    metadata: &serde_json::Value,
    result: &mut PreflightCheckResult,
) -> Result<(), String> {
    // Extract season/episode info
    result.season_number = metadata["season"].as_u64().map(|n| n as u32);
    result.episode_number = metadata["episode"].as_u64().map(|n| n as u32);
    
    // Fetch TMDB info if it's a movie or TV show
    let category = metadata["category"].as_str().unwrap_or("");
    let is_movie_or_tv = category.contains("Movie") || category.contains("TvShow");
    
    if is_movie_or_tv && !config.general.tmdb_api_key.is_empty() {
        let title = metadata["title"].as_str().unwrap_or("");
        let year = metadata["year"].as_u64().map(|y| y.to_string());
        let release_type = if category.contains("Movie") { "movie" } else { "tv" };
        
        match fetch_tmdb_id(title, year, &config.general.tmdb_api_key, release_type) {
            Ok(tmdb_id) => {
                result.tmdb_id = tmdb_id;
                // TODO: Fetch external IDs (IMDb, TVDB) if we bring back fetch_external_ids
            }
            Err(e) => {
                debug!("Failed to fetch TMDB ID: {}", e);
            }
        }
    }
    
    // Extract audio languages
    let video_extensions = crate::types::VideoType::all_extensions();
    if let Ok(files) = crate::utils::filter_files_by_extension(input_path, &video_extensions) {
        for file in files.iter().take(1) { // Just check the first file for performance
            if let Ok(mediainfo) = generate_mediainfo(file.to_str().unwrap_or(""), config) {
                result.audio_languages = extract_audio_languages(&mediainfo);
            }
        }
    }
    
    // Check exclusion settings - default to "No" since we don't have this config field
    result.excluded_files = "No".to_string();
    
    Ok(())
}

/// Extract audio-specific metadata
fn extract_audio_metadata(
    input_path: &str,
    metadata: &serde_json::Value,
    result: &mut PreflightCheckResult,
) -> Result<(), String> {
    // Check for album cover
    result.album_cover = check_for_cover_image(input_path);
    
    // Format audio info
    if let Some(format) = metadata["format"].as_str() {
        let audio_info = format.to_string();
        result.audio_languages = vec![audio_info];
    }
    
    Ok(())
}

/// Check if cover image exists in the path
fn check_for_cover_image(input_path: &str) -> String {
    use walkdir::WalkDir;
    
    let has_cover = WalkDir::new(input_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .any(|entry| {
            if let Some(ext) = entry.path().extension().and_then(|e| e.to_str()) {
                matches!(ext.to_lowercase().as_str(), "jpg" | "jpeg" | "png")
            } else {
                false
            }
        });
    
    if has_cover {
        "Available".to_string()
    } else {
        "Not Available".to_string()
    }
}

/// Format release type for display
fn format_release_type(media_type: &MediaType) -> String {
    match media_type {
        MediaType::Video(vtype) => {
            match format!("{:?}", vtype).as_str() {
                "Mkv" | "Mp4" | "Avi" => "🎥 Movie".to_string(),
                _ => "📺 TV Show".to_string(),
            }
        }
        MediaType::Audio(atype) => {
            format!("🎧 {}", format!("{:?}", atype).to_uppercase())
        }
        MediaType::Ebook(_) => "📚 Ebook".to_string(),
        MediaType::Game(_) => "🎮 Game".to_string(),
        MediaType::Hobby(_) => "🎨 Hobby".to_string(),
    }
}

/// Extract audio languages from mediainfo output
fn extract_audio_languages(mediainfo_output: &str) -> Vec<String> {
    let mut audio_languages = Vec::new();
    let mut in_audio_section = false;

    for line in mediainfo_output.lines() {
        if line.starts_with("Audio") {
            in_audio_section = true;
        } else if line.is_empty() {
            in_audio_section = false;
        }

        if in_audio_section && line.contains("Language") {
            if let Some(language) = line.split(':').nth(1) {
                audio_languages.push(language.trim().to_string());
            }
        }
    }

    audio_languages
}

/// Get base name from path
fn base_name_from_path(path: &str) -> String {
    Path::new(path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
}

/// Print preflight check results in a formatted way
pub fn print_preflight_results(result: &PreflightCheckResult) {
    println!("Pre-flight Check Results:");
    println!("Title: {}", result.release_name);
    println!("Release Name: {}", result.generated_release_name);
    println!("Dupe Check: {}", result.dupe_check);
    println!("Release Type: {}", result.release_type);
    println!(
        "Season Number: {}",
        result.season_number.map_or("N/A".to_string(), |s| s.to_string())
    );
    println!(
        "Episode Number: {}",
        result.episode_number.map_or("N/A".to_string(), |e| e.to_string())
    );
    println!("TMDB ID: {}", result.tmdb_id);
    println!("IMDb ID: {}", result.imdb_id.as_deref().unwrap_or("N/A"));
    println!("TVDB ID: {}", result.tvdb_id.map_or("N/A".to_string(), |id| id.to_string()));
    println!("Excluded Files: {}", result.excluded_files);
    println!("Album Cover: {}", result.album_cover);
    println!("Audio Languages: [{}]", result.audio_languages.join(", "));
}