use crate::types::{Config, PreflightCheckResult};
use crate::process_builder::preflight_builder;
use std::sync::Arc;
use log::info;

/// Perform a preflight check on the input path to analyze what would happen during upload
pub fn preflight_check(
    input_path: &str,
    config: &Config,
    dry_run: bool,
) -> Result<PreflightCheckResult, String> {
    info!("Running preflight check for: {}", input_path);
    
    // Use the process builder to get all the data we need
    let process_result = preflight_builder(input_path, Arc::new(config.clone()))
        .dry_run(dry_run)
        .build()?;
    
    // The process builder already generated all preflight data for us
    process_result.preflight_data
        .ok_or_else(|| "Process builder did not generate preflight data".to_string())
}


/// Print preflight check results in a formatted way
pub fn print_preflight_results(result: &PreflightCheckResult) {
    println!("Pre-flight Check Results:");
    println!("Title: {}", result.release_name);
    println!("Release Name: {}", result.generated_release_name);
    println!("Dupe Check: {}", result.dupe_check);
    println!("Release Type: {}", result.release_type);
    
    // Game-specific display
    if result.release_type.contains("Game") {
        // IGDB Data
        if result.igdb_id.is_some() {
            println!("\nIGDB Information:");
            println!("  IGDB ID: {}", result.igdb_id.map_or("N/A".to_string(), |id| id.to_string()));
            if let Some(developer) = &result.igdb_developer {
                println!("  Developer: {}", developer);
            }
            if let Some(publisher) = &result.igdb_publisher {
                println!("  Publisher: {}", publisher);
            }
            if let Some(genres) = &result.igdb_genres {
                println!("  Genres: {}", genres);
            }
            if let Some(rating) = result.igdb_rating {
                println!("  Rating: {:.1}/100", rating);
            }
            if let Some(platforms) = &result.igdb_platforms {
                println!("  Platforms: {}", platforms.join(", "));
            }
            if let Some(summary) = &result.igdb_summary {
                println!("  Summary: {}", summary);
            }
        }
        
        println!("\nGame Details:");
        println!("  Cover Art: {}", result.album_cover);
        if !result.audio_languages.is_empty() {
            println!("  Detected Platforms: [{}]", result.audio_languages.join(", "));
        }
    } else {
        // Non-game display (TV/Movie)
        if result.is_boxset {
            println!("Type: Season Pack/Boxset");
        } else if result.episode_number.is_some() && result.episode_number != Some(0) {
            println!("Type: Single Episode");
        }
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
        println!("Audio Languages: [{}]", result.audio_languages.join(", "));
    }
    
    println!("Excluded Files: {}", result.excluded_files);
    
    if !result.tracker_categories.is_empty() {
        println!("\nTracker Category Mappings:");
        for (tracker, category) in &result.tracker_categories {
            println!("  {}: {}", tracker, category);
        }
    }
}