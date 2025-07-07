// Configuration loading and validation

use super::error::{Result, SeedError};
use super::types::Config;
use serde::de::DeserializeOwned;
use std::fs;
use std::path::Path;

/// Load a YAML configuration file
pub fn load_yaml_config<T: DeserializeOwned>(path: &str) -> Result<T> {
    let content = fs::read_to_string(path)
        .map_err(|e| SeedError::Config(format!("Failed to read config file '{}': {}", path, e)))?;

    serde_yaml::from_str(&content)
        .map_err(|e| SeedError::Config(format!("Failed to parse YAML config '{}': {}", path, e)))
}

/// Load the main configuration
pub fn load_config(config_path: &Path) -> Result<Config> {
    let config_str = config_path
        .to_str()
        .ok_or_else(|| SeedError::Config("Invalid config path".to_string()))?;

    load_yaml_config(config_str)
}

/// Validate configuration
pub fn validate_config(config: &Config) -> Result<()> {
    // Validate binary paths exist
    let binaries = [
        ("ffmpeg", &config.paths.ffmpeg),
        ("ffprobe", &config.paths.ffprobe),
        ("mkbrr", &config.paths.mkbrr),
        ("mediainfo", &config.paths.mediainfo),
    ];

    for (name, path) in &binaries {
        if !Path::new(path).exists() {
            return Err(SeedError::Config(format!(
                "Binary '{}' not found at '{}'",
                name, path
            )));
        }
    }

    // Validate directories exist
    if !Path::new(&config.paths.torrent_dir).exists() {
        return Err(SeedError::Config(format!(
            "Torrent directory not found: {}",
            config.paths.torrent_dir
        )));
    }

    if !Path::new(&config.paths.screenshots_dir).exists() {
        return Err(SeedError::Config(format!(
            "Screenshots directory not found: {}",
            config.paths.screenshots_dir
        )));
    }

    Ok(())
}
