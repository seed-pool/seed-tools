use std::{
    fs,
    env,
    path::Path,
    collections::HashMap,
    thread,
    time::Duration,
};
use regex::Regex;
use serde::Deserialize;
use serde_yaml;
use log::{info, error};
use simplelog::{Config as SimpleLogConfig, CombinedLogger, WriteLogger, LevelFilter};
use std::fs::File;
use seed_tools::utils::generate_release_name;
use seed_tools::types::{Config, SeedpoolConfig, TorrentLeechConfig, QbittorrentConfig, DelugeConfig};
use trackers::common::process_custom_upload;
use reqwest::blocking::Client;
use seed_tools::sync; 

mod trackers {
    pub mod seedpool;
    pub mod torrentleech;
    pub mod common;
}

#[derive(Deserialize)]
struct GeneralConfig {
    pub tmdb_api_key: String,
}

trait Tracker {
    fn requires_screenshots(&self) -> bool;
    fn requires_sample(&self) -> bool;
    fn requires_tmdb_id(&self) -> bool;
    fn requires_remote_path(&self) -> bool;
    fn upload(
        &self,
        torrent_file: &str,
        release_name: &str,
        description: Option<&str>,
        mediainfo: Option<&str>,
        nfo_file: &Option<String>,
        category_id: u32,
        type_id: Option<u32>,
        tmdb_id: Option<u32>,
        imdb_id: Option<String>,
        tvdb_id: Option<u32>,
        season_number: Option<u32>,
        episode_number: Option<u32>,
        resolution_id: Option<u32>,
    ) -> Result<(), String>;
    fn generate_metadata(&self, torrent_file: &str) -> Result<HashMap<String, String>, String>;
}

fn load_yaml_config<T: serde::de::DeserializeOwned>(path: &str) -> T {
    serde_yaml::from_str(&fs::read_to_string(path).expect("Failed to read config file"))
        .expect("Failed to parse YAML config")
}

fn extract_binaries(config_path: &str) -> Result<String, String> {
    // Load the configuration file
    let config: serde_yaml::Value = serde_yaml::from_str(
        &fs::read_to_string(config_path).map_err(|e| format!("Failed to read config file: {}", e))?,
    )
    .map_err(|e| format!("Failed to parse config file: {}", e))?;

    // Extract the paths field
    let paths = config["paths"]
        .as_mapping()
        .ok_or("Missing or invalid 'paths' field in config")?;

    // Define the required binaries
    let required_binaries = ["ffmpeg", "ffprobe", "mkbrr", "mediainfo"];

    // Check if all required binaries exist in the paths
    for binary in &required_binaries {
        if !paths.contains_key(binary) {
            return Err(format!("Missing '{}' in 'paths' field of config", binary));
        }

        let binary_path = paths[binary]
            .as_str()
            .ok_or(format!("Invalid path for '{}'", binary))?;

        if !Path::new(binary_path).exists() {
            return Err(format!("Binary '{}' not found at '{}'", binary, binary_path));
        }
    }

    // Return the bin directory path (assumes binaries are in the same directory)
    let bin_dir = Path::new(
        paths["ffmpeg"]
            .as_str()
            .ok_or("Invalid path for 'ffmpeg'")?,
    )
    .parent()
    .ok_or("Failed to determine bin directory")?
    .to_string_lossy()
    .to_string();

    Ok(bin_dir)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    CombinedLogger::init(vec![WriteLogger::new(
        LevelFilter::Debug,
        SimpleLogConfig::default(),
        File::create("seed-tools.log")?,
    )])?;

    let args: Vec<String> = env::args().collect();

    // Provide the path to the configuration file
    let config_path = "config/config.yaml";
    let binaries_dir = extract_binaries(config_path).unwrap_or_else(|e| {
        error!("Failed to extract binaries: {}", e);
        std::process::exit(1);
    });

    let ffmpeg_path = Path::new(&binaries_dir).join("ffmpeg");
    let ffprobe_path = Path::new(&binaries_dir).join("ffprobe");
    let mkbrr_path = Path::new(&binaries_dir).join("mkbrr");
    let mediainfo_path = Path::new(&binaries_dir).join("mediainfo");

    // Load configurations
    let mut main_config: Config = load_yaml_config(config_path);
    let seedpool_config: SeedpoolConfig = load_yaml_config("config/trackers/seedpool.yaml");
    let torrentleech_config: TorrentLeechConfig = load_yaml_config("config/trackers/torrentleech.yaml");

    if args.len() < 2 {
        error!("Usage: seedtool <input_path> or seedtool -sync or seedtool <input_path> -SP/-TL");
        return Ok(());
    }

    log::debug!("Raw command-line arguments: {:?}", args);
    let input_path = &args[1];
    let sanitized_name = generate_release_name(
        &Path::new(input_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string(),
    );

    if args.len() == 2 && args[1] == "-sync" {
        // Delegate sync functionality to sync.rs
        if let Err(e) = sync::sync_qbittorrent(&main_config.qbittorrent, &seedpool_config.general.api_key) {
            error!("Error syncing qBittorrent: {}", e);
        }
        return Ok(());
    }

    let mut errors = Vec::new();

    if args.iter().any(|arg| arg.starts_with('-') && arg.len() == 5 && arg[1..].chars().all(|c| c.is_digit(10))) {
        let category_type_arg = args.iter().find(|arg| arg.starts_with('-') && arg.len() == 5).unwrap();
        let category_id: u32 = category_type_arg[1..3].trim_start_matches('0').parse().unwrap_or(0);
        let type_id: u32 = category_type_arg[3..5].trim_start_matches('0').parse().unwrap_or(0);

        let tracker = if args.contains(&"-SP".to_string()) {
            "seedpool"
        } else if args.contains(&"-TL".to_string()) {
            "torrentleech"
        } else {
            error!("No valid tracker specified for custom upload");
            return Ok(());
        };

        if let Err(e) = process_custom_upload(
            input_path,
            category_id,
            type_id,
            &main_config.qbittorrent,
            &main_config.deluge,
            tracker,
            Some(&seedpool_config),
            Some(&torrentleech_config),
            mkbrr_path.to_str().ok_or("Invalid mkbrr_path")?,
            &main_config.paths,
        ) {
            error!("Error processing custom upload: {}", e);
        }
        return Ok(());
    }

    if args.contains(&"-SP".to_string()) {
        log::debug!("Input path passed to process_seedpool_release: {}", input_path);
        if let Err(e) = trackers::seedpool::process_seedpool_release(
            input_path,
            &sanitized_name,
            &mut main_config,
            &seedpool_config,
            &ffmpeg_path,
            &ffprobe_path,
            &mkbrr_path,
            &mediainfo_path,
        ) {
            error!("Error processing Seedpool release: {}", e);
            errors.push(format!("Seedpool: {}", e));
        }
    }

    if args.contains(&"-TL".to_string()) {
        if let Err(e) = trackers::torrentleech::process_torrentleech_release(
            input_path,
            &sanitized_name,
            &mut main_config,
            &torrentleech_config,
            &mkbrr_path,
            &mediainfo_path,
        ) {
            error!("Error processing TorrentLeech release: {}", e);
            errors.push(format!("TorrentLeech: {}", e));
        }
    }

    if errors.is_empty() {
        info!("Upload completed successfully for all specified trackers.");
    } else {
        error!("Upload completed with errors: {:?}", errors);
    }

    Ok(())
}