// Duplicate checking utilities

use std::fs;
use log::info;
use crate::core::{
    types::{SeedpoolConfig, TorrentLeechConfig, Config},
    error::{SeedError, Result},
};

/// Check for duplicates across all configured trackers
/// 
/// This function loads configurations and checks all enabled trackers for duplicates.
/// It's useful when you need to check duplicates without knowing which tracker will be used.
/// 
/// # Arguments
/// * `title` - The title/name to check for duplicates
/// 
/// # Returns
/// * `Ok(Vec<(tracker_name, download_link)>)` - List of trackers where duplicates were found
/// * `Err(String)` - If an error occurs
pub fn check_all_duplicates(title: &str) -> Result<Vec<(String, String)>> {
    let mut duplicates = Vec::new();
    
    // Try to load configurations
    let config_path = "config/config.yaml";
    let config_content = fs::read_to_string(config_path)
        .map_err(|e| SeedError::Other(format!("Failed to read config file: {}", e)))?;
    let _config: Config = serde_yaml::from_str(&config_content)
        .map_err(|e| SeedError::Parse(format!("Failed to parse config: {}", e)))?;
    
    // Check Seedpool if configured
    if let Ok(seedpool_content) = fs::read_to_string("config/trackers/seedpool.yaml") {
        if let Ok(seedpool_config) = serde_yaml::from_str::<SeedpoolConfig>(&seedpool_content) {
            if seedpool_config.general.enabled && seedpool_config.settings.dupe_checks {
                match check_duplicates(title, "seedpool", Some(&seedpool_config), None) {
                    Ok(Some(download_link)) => {
                        duplicates.push(("seedpool".to_string(), download_link));
                    }
                    Ok(None) => {}
                    Err(e) => {
                        info!("Error checking Seedpool duplicates: {}", e);
                    }
                }
            }
        }
    }
    
    // Check TorrentLeech if configured
    if let Ok(tl_content) = fs::read_to_string("config/trackers/torrentleech.yaml") {
        if let Ok(tl_config) = serde_yaml::from_str::<TorrentLeechConfig>(&tl_content) {
            if tl_config.general.enabled {
                match check_duplicates(title, "torrentleech", None, Some(&tl_config)) {
                    Ok(Some(download_link)) => {
                        duplicates.push(("torrentleech".to_string(), download_link));
                    }
                    Ok(None) => {}
                    Err(e) => {
                        info!("Error checking TorrentLeech duplicates: {}", e);
                    }
                }
            }
        }
    }
    
    Ok(duplicates)
}

/// Check for duplicates across different trackers
/// 
/// This function dispatches to tracker-specific duplicate checking functions
/// based on the tracker parameter.
/// 
/// # Arguments
/// * `title` - The title/name to check for duplicates
/// * `tracker` - The tracker to check ("seedpool", "sp", "torrentleech", "tl", etc.)
/// * `seedpool_config` - Optional Seedpool configuration (required for Seedpool)
/// * `torrentleech_config` - Optional TorrentLeech configuration (required for TorrentLeech)
/// 
/// # Returns
/// * `Ok(Some(download_link))` - If a duplicate is found, returns the download link
/// * `Ok(None)` - If no duplicate is found
/// * `Err(String)` - If an error occurs
/// 
/// # Example
/// ```rust
/// let tracker = if cli_args.sp { "seedpool" } else if cli_args.tl { "torrentleech" } else { "unknown" };
/// match check_duplicates("Movie.Title.2023.1080p", tracker, Some(&seedpool_config), None)? {
///     Some(download_link) => println!("Duplicate found: {}", download_link),
///     None => println!("No duplicate found"),
/// }
/// ```
pub fn check_duplicates(
    title: &str,
    tracker: &str,
    seedpool_config: Option<&SeedpoolConfig>,
    torrentleech_config: Option<&TorrentLeechConfig>,
) -> Result<Option<String>> {
    match tracker.to_lowercase().as_str() {
        "seedpool" | "sp" => {
            let seedpool_cfg = seedpool_config
                .ok_or(SeedError::Config("Seedpool configuration is required for Seedpool duplicate checks".to_string()))?;
            
            // Check if duplicate checks are enabled
            if !seedpool_cfg.settings.dupe_checks {
                info!("Duplicate checks are disabled for Seedpool");
                return Ok(None);
            }
            
            // Call the seedpool-specific duplicate check
            use crate::trackers::seedpool::check_seedpool_dupes;
            check_seedpool_dupes(title, &seedpool_cfg.general.api_key)
                .map_err(|e| SeedError::Other(e))
        }
        "torrentleech" | "tl" => {
            let _tl_cfg = torrentleech_config
                .ok_or(SeedError::Config("TorrentLeech configuration is required for TorrentLeech duplicate checks".to_string()))?;
            
            // TODO: Implement TorrentLeech duplicate checking
            info!("TorrentLeech duplicate checking not yet implemented");
            Ok(None)
        }
        _ => {
            Err(SeedError::Config(format!("Unknown tracker: {}", tracker)))
        }
    }
}

/// Load tracker configuration from YAML file
pub fn load_tracker_config<T: serde::de::DeserializeOwned>(tracker_name: &str) -> Result<T> {
    let config_path = format!("config/trackers/{}.yaml", tracker_name);
    let config_contents = fs::read_to_string(&config_path)
        .map_err(|e| SeedError::Other(format!("Failed to read {} config: {}", tracker_name, e)))?;
    
    serde_yaml::from_str(&config_contents)
        .map_err(|e| SeedError::Parse(format!("Failed to parse {} config: {}", tracker_name, e)))
}