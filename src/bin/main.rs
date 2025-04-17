use std::{fs, env, thread, time::Duration, path::Path, collections::HashMap};
use regex::Regex;
use serde::Deserialize;
use serde_yaml;
use reqwest::blocking::Client;
use log::{info, error};
use simplelog::{Config as SimpleLogConfig, CombinedLogger, WriteLogger, LevelFilter};
use std::fs::File;
use seed_tools::utils::generate_release_name;
use seed_tools::types::{PathsConfig, QbittorrentConfig, TorrentLeechConfig, SeedpoolConfig};
mod trackers {
    pub mod seedpool;
    pub mod torrentleech;
}

#[derive(Deserialize)]
struct Config {
    general: GeneralConfig,
    paths: PathsConfig,
    qbittorrent: Vec<QbittorrentConfig>,
}

#[derive(Deserialize)]
struct GeneralConfig {
    tmdb_api_key: String,
}

trait Tracker {
    fn requires_screenshots(&self) -> bool;
    fn requires_sample(&self) -> bool;
    fn requires_tmdb_id(&self) -> bool;
    fn requires_remote_path(&self) -> bool;
    fn upload(
        &self,
        torrent_file: &str,
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
        resolution_id: Option<u32>, // Ensure this is included
    ) -> Result<(), String>;
    fn generate_metadata(&self, torrent_file: &str) -> Result<HashMap<String, String>, String>;
}

fn load_yaml_config<T: serde::de::DeserializeOwned>(path: &str) -> T {
    serde_yaml::from_str(&fs::read_to_string(path).expect("Failed to read config file")).expect("Failed to parse YAML config")
}

fn extract_binaries() -> Result<String, String> {
    let bin_dir = env::current_dir().map_err(|e| format!("Failed to get current directory: {}", e))?.join("bin");
    if bin_dir.exists() && ["ffmpeg", "ffprobe", "mkbrr", "mediainfo"].iter().all(|b| bin_dir.join(b).exists()) {
        return Ok(bin_dir.to_string_lossy().to_string());
    }
    fs::create_dir_all(&bin_dir).map_err(|e| format!("Failed to create bin directory: {}", e))?;
    Ok(bin_dir.to_string_lossy().to_string())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    CombinedLogger::init(vec![WriteLogger::new(LevelFilter::Debug, SimpleLogConfig::default(), File::create("seed-tools.log")?)])?;
    let args: Vec<String> = env::args().collect();
    let binaries_dir = extract_binaries().unwrap_or_default();
    let ffmpeg_path = Path::new(&binaries_dir).join("ffmpeg");
    let ffprobe_path = Path::new(&binaries_dir).join("ffprobe");
    let mkbrr_path = Path::new(&binaries_dir).join("mkbrr");
    let mediainfo_path = Path::new(&binaries_dir).join("mediainfo");
    let mut main_config: Config = load_yaml_config("config/config.yaml");
    let torrentleech_config: TorrentLeechConfig = load_yaml_config("config/trackers/torrentleech.yaml");
    let seedpool_config: SeedpoolConfig = load_yaml_config("config/trackers/seedpool.yaml");

    if args.len() < 2 {
        error!("Usage: seedtool <input_path> or seedtool -sync or seedtool <input_path> -SP/-TL");
        return Ok(());
    }

    let input_path = &args[1];
    let sanitized_name = generate_release_name(
        &Path::new(input_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string(),
    );

    if args.len() == 2 && args[1] == "-sync" {
        if let Err(e) = sync_qbittorrent(&main_config.qbittorrent, &main_config.general.tmdb_api_key) {
            error!("Error syncing qBittorrent: {}", e);
        }
        return Ok(());
    }

    let mut errors = Vec::new();

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

    if args.contains(&"-SP".to_string()) {
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

    if errors.is_empty() {
        info!("Upload completed successfully for all specified trackers.");
    } else {
        error!("Upload completed with errors: {:?}", errors);
    }

    Ok(())
}

fn check_seedpool(
    name: &str,
    seedpool_api_key: &str,
) -> Result<Option<String>, String> {
    let client = Client::new();

    info!("Checking Seedpool for existing torrent with name: '{}'", name);

    let normalized_name = generate_release_name(name);
    info!("Normalized Name for Seedpool Query: '{}'", normalized_name);

    let season_episode_regex = Regex::new(r"S(\d{2})E(\d{2})").unwrap();
    let season_episode = season_episode_regex.captures(name).map(|caps| {
        (
            caps.get(1).unwrap().as_str().parse::<u32>().unwrap_or(0),
            caps.get(2).unwrap().as_str().parse::<u32>().unwrap_or(0),
        )
    });
    if let Some((season, episode)) = &season_episode {
        info!("Detected Season/Episode: S{}E{}", season, episode);
    }

    let mut query_url = format!(
        "https://seedpool.org/api/torrents/filter?name={}&perPage=10&sortField=name&sortDirection=asc&api_token={}",
        urlencoding::encode(&normalized_name),
        seedpool_api_key
    );

    if let Some((season, episode)) = &season_episode {
        query_url = format!(
            "{}&seasonNumber={}&episodeNumber={}",
            query_url, season, episode
        );
    }

    info!("Seedpool API Query URL: {}", query_url);

    let search_response = client
        .get(&query_url)
        .send()
        .map_err(|e| format!("Failed to query Seedpool for '{}': {}", name, e))?;

    if !search_response.status().is_success() {
        return Err(format!(
            "Failed to query Seedpool for '{}': HTTP {}",
            name,
            search_response.status()
        ));
    }

    let raw_response = search_response.text().unwrap_or_else(|_| "Failed to read response body".to_string());
    info!("Seedpool API Response: {}", raw_response);

    let search_results: serde_json::Value = serde_json::from_str(&raw_response)
        .map_err(|e| format!("Failed to parse Seedpool response for '{}': {}", name, e))?;

    let empty_vec = vec![];
    let data = search_results["data"].as_array().unwrap_or(&empty_vec);

    for result in data {
        if let Some(attributes) = result["attributes"].as_object() {
            if let Some(title) = attributes.get("name").and_then(|t| t.as_str()) {
                info!("Checking result title: {}", title);

                if let Some((season, episode)) = &season_episode {
                    if !title.contains(&format!("S{:02}E{:02}", season, episode)) {
                        info!("Skipping result due to mismatched season/episode: {}", title);
                        continue;
                    }
                }

                if let Some(download_link) = attributes.get("download_link").and_then(|d| d.as_str()) {
                    info!("Duplicate found for '{}'. Download link: {}", name, download_link);
                    return Ok(Some(download_link.to_string()));
                }
            }
        }
    }

    info!("No duplicate found for '{}'.", name);
    Ok(None)
}

fn sync_qbittorrent(configs: &[QbittorrentConfig], seedpool_api_key: &str) -> Result<(), String> {
    for config in configs {
        let client = Client::new();

        info!("Logging in to qBittorrent at {}...", config.webui_url);
        let login_response = client
            .post(format!("{}/api/v2/auth/login", config.webui_url))
            .form(&[
                ("username", config.username.as_str()),
                ("password", config.password.as_str()),
            ])
            .send()
            .map_err(|e| format!("Failed to log in to qBittorrent: {}", e))?;

        if !login_response.status().is_success() {
            error!("Failed to log in to qBittorrent at {}: {}", config.webui_url, login_response.status());
            continue;
        }
        info!("Logged in to qBittorrent at {} successfully.", config.webui_url);

        let torrents_response = client
            .get(format!("{}/api/v2/torrents/info", config.webui_url))
            .send()
            .map_err(|e| format!("Failed to fetch torrents info: {}", e))?;

        if !torrents_response.status().is_success() {
            return Err(format!(
                "Failed to fetch torrents info: {}",
                torrents_response.status()
            ));
        }

        let torrents: Vec<serde_json::Value> = torrents_response
            .json()
            .map_err(|e| format!("Failed to parse torrents info: {}", e))?;

        let completed_torrents: Vec<&serde_json::Value> = torrents
            .iter()
            .filter(|torrent| torrent["progress"].as_f64().unwrap_or(0.0) == 1.0)
            .collect();

        info!("Completed Torrents:");
        for torrent in &completed_torrents {
            let name = torrent["name"].as_str().unwrap_or("Unknown");
            let save_path = torrent["save_path"].as_str().unwrap_or("");
            let _is_folder = Path::new(save_path).is_dir();
            let _torrent_file = format!("/tmp/{}.torrent", name);

            info!("Checking for duplicate on Seedpool for '{}'", name);
            match check_seedpool(name, seedpool_api_key) {
                Ok(Some(_download_link)) => {
                    info!("Found duplicate for '{}'. Skipping upload.", name);
                }
                Ok(None) => {
                    info!("No duplicate found for '{}'.", name);
                }
                Err(e) => {
                    error!("Error checking for duplicate for '{}': {}", name, e);
                }
            }

            thread::sleep(Duration::from_secs(1));
        }
    }

    Ok(())
}