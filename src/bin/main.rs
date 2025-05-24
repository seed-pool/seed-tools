use std::{
    fs,
    env,
    path::{Path, PathBuf},
    collections::HashMap,
};
use serde::Deserialize;
use log::{info, error, debug, LevelFilter};
use simplelog::{Config as SimpleLogConfig, CombinedLogger, WriteLogger};
use std::fs::File;
use std::error::Error;
use reqwest::blocking::Client;
use seed_tools::utils;
use seed_tools::utils::generate_release_name;
use seed_tools::types::{Config, SeedpoolConfig, TorrentLeechConfig, QbittorrentConfig, DelugeConfig};
use seed_tools::sync;
use seed_tools::irc::launch_irc_client;
use seed_tools::types::PreflightCheckResult;
use trackers::seedpool::preflight_check;
use seed_tools::ui;
use tokio::main;
mod trackers {
    pub mod seedpool;
    pub mod torrentleech;
    pub mod common;
}
use std::fs::OpenOptions;
use trackers::common::{process_custom_upload, sanitize_game_title, process_game_upload, Tracker};
use clap::{Parser, CommandFactory};
#[derive(Deserialize)]
struct GeneralConfig {
    pub tmdb_api_key: String,
}

fn load_yaml_config<T: serde::de::DeserializeOwned>(path: &str) -> T {
    serde_yaml::from_str(&fs::read_to_string(path).expect("Failed to read config file"))
        .expect("Failed to parse YAML config")
}

fn extract_binaries(config_path: &str) -> Result<String, String> {
    let config: serde_yaml::Value = serde_yaml::from_str(
        &fs::read_to_string(config_path).map_err(|e| format!("Failed to read config file: {}", e))?,
    )
    .map_err(|e| format!("Failed to parse config file: {}", e))?;
    let paths = config["paths"]
        .as_mapping()
        .ok_or("Missing or invalid 'paths' field in config")?;
    let required_binaries = ["ffmpeg", "ffprobe", "mkbrr", "mediainfo"];
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

#[derive(Parser, Debug)]
#[command(author, version, about = "Automated tool for processing and uploading releases to trackers.", long_about = None)]
struct Cli {
    #[arg(long, conflicts_with_all = ["sp", "tl", "custom_cat_type", "command", "irc"])]
    sync: bool,

    #[arg(long = "SP", requires = "input_path")]
    sp: bool,

    #[arg(long = "TL", requires = "input_path")]
    tl: bool,

    #[arg(short = 'c', long, value_name = "CAT_TYPE", requires = "input_path")]
    custom_cat_type: Option<String>,

    #[arg(long, conflicts_with_all = ["sync", "sp", "tl", "custom_cat_type", "command", "irc"])]
    ui: bool, // Add the `ui` argument

    #[arg(long, conflicts_with_all = ["sync", "sp", "tl", "custom_cat_type", "command", "ui"])]
    irc: bool, // Add the `irc` argument

    #[arg(long, conflicts_with_all = ["sync", "sp", "tl", "custom_cat_type", "command"])]
    pre: bool, // Add the `pre` argument

    #[command(subcommand)]
    command: Option<Commands>,

    #[arg(index = 1)]
    input_path: Option<PathBuf>,
}

#[derive(Parser, Debug)]
enum Commands {
    /// Check for duplicates in Seedpool
    Check {
        /// The name of the release to check for duplicates
        #[arg(index = 1)]
        name: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // --- Initialize Logging ---
    let log_path = Path::new("seed-tools.log");
    CombinedLogger::init(vec![WriteLogger::new(
        LevelFilter::Debug,
        SimpleLogConfig::default(),
        OpenOptions::new()
            .create(true) // Create the file if it doesn't exist
            .append(true) // Append to the file instead of truncating it
            .open(&log_path)?,
    )])?;
    info!("Logging initialized.");

    // Determine the executable directory
    let exe_dir = std::env::current_exe()
        .map_err(|e| format!("Failed to get executable path: {}", e))?
        .parent()
        .ok_or("Failed to determine executable directory")?
        .to_path_buf();
    info!("Executable directory determined as: {:?}", exe_dir);

    // Parse CLI arguments
    info!("Parsing arguments...");
    let cli = Cli::parse();
    debug!("Parsed arguments: {:?}", cli);

    // --- Handle IRC Mode ---
    if cli.irc {
        info!("Launching IRC mode...");
        return launch_irc_client().await;
    }    

    // --- Handle UI Mode (Default) ---
    if cli.ui || (cli.command.is_none() && cli.input_path.is_none() && !cli.sync && !cli.pre) {
        info!("Launching UI mode...");
        return ui::launch_ui();
    }

    // --- Build Configuration Paths ---
    info!("Building configuration paths...");
    let config_dir = exe_dir.join("config");
    let main_config_path = config_dir.join("config.yaml");
    let seedpool_config_path = config_dir.join("trackers/seedpool.yaml");
    let torrentleech_config_path = config_dir.join("trackers/torrentleech.yaml");
    info!("Configuration paths built.");

    // --- Load Configurations ---
    info!("Loading configurations...");
    let main_config_path_str = main_config_path.to_str()
        .ok_or_else(|| format!("Invalid non-UTF8 path for main config: {:?}", main_config_path))?;
    let binaries_dir = extract_binaries(main_config_path_str).unwrap_or_else(|e| {
        error!("Failed to extract binaries using config {:?}: {}", main_config_path, e);
        std::process::exit(1);
    });

    let ffmpeg_path = Path::new(&binaries_dir).join("ffmpeg");
    let ffprobe_path = Path::new(&binaries_dir).join("ffprobe");
    let mkbrr_path = Path::new(&binaries_dir).join("mkbrr");
    let mediainfo_path = Path::new(&binaries_dir).join("mediainfo");
    debug!(
        "Binary paths: ffmpeg={:?}, ffprobe={:?}, mkbrr={:?}, mediainfo={:?}",
        ffmpeg_path, ffprobe_path, mkbrr_path, mediainfo_path
    );

    let seedpool_config_path_str = seedpool_config_path.to_str()
        .ok_or_else(|| format!("Invalid non-UTF8 path for seedpool config: {:?}", seedpool_config_path))?;
    let torrentleech_config_path_str = torrentleech_config_path.to_str()
        .ok_or_else(|| format!("Invalid non-UTF8 path for torrentleech config: {:?}", torrentleech_config_path))?;

    let mut main_config: Config = load_yaml_config::<Config>(main_config_path_str);
    let seedpool_config: SeedpoolConfig = load_yaml_config(seedpool_config_path_str);
    let torrentleech_config: TorrentLeechConfig = load_yaml_config(torrentleech_config_path_str);
    info!("Configurations loaded.");

    if cli.pre {
        info!("Running pre-flight check...");
        if let Some(input_path) = cli.input_path {
            let input_path_str = input_path.to_str().ok_or("Invalid input path string")?;
            info!("Input path for pre-flight check: {}", input_path_str);
    
            match preflight_check(
                input_path_str,
                &main_config,
                &seedpool_config,
                &ffmpeg_path,
                &ffprobe_path,
                &mediainfo_path,
            ) {
                Ok(result) => {
                    println!("Pre-flight Check Results:");
                    println!("Title: {}", result.release_name);
                    println!("Release Name: {}", result.generated_release_name);
                    println!("Dupe Check: {}", result.dupe_check);
                    println!("Release Type: {}", result.release_type); // New line
                    println!(
                        "Season Number: {}",
                        result.season_number.map_or("N/A".to_string(), |s| s.to_string())
                    ); // New line
                    println!(
                        "Episode Number: {}",
                        result.episode_number.map_or("N/A".to_string(), |e| e.to_string())
                    ); // New line
                    println!("TMDB ID: {}", result.tmdb_id);
                    println!("IMDb ID: {}", result.imdb_id.unwrap_or_else(|| "N/A".to_string()));
                    println!("TVDB ID: {}", result.tvdb_id.map_or("N/A".to_string(), |id| id.to_string()));
                    println!("Excluded Files: {}", result.excluded_files);
                    println!("Audio Languages: {:?}", result.audio_languages);
                }
                Err(e) => {
                    error!("Pre-flight check failed: {}", e);
                    println!("Pre-flight check failed: {}", e);
                }
            }
        } else {
            error!("No input path provided for pre-flight check.");
            println!("Error: No input path provided for pre-flight check.");
        }
        return Ok(()); // Exit after running pre-flight check
    }

    // --- Handle Sync Mode ---
    if cli.sync {
        info!("Running in --sync mode.");
        if let Err(e) = sync::sync_qbittorrent(&main_config.qbittorrent, &seedpool_config.general.api_key) {
            error!("Error syncing qBittorrent: {}", e);
        } else {
            info!("Sync operation completed.");
        }
        return Ok(()); // Exit after sync
    }

    // --- Handle Commands ---
    if let Some(command) = cli.command {
        match command {
            Commands::Check { name } => {
                info!("Running check for duplicates with name: {}", name);

                // Call check_seedpool
                match sync::check_seedpool(&name, &seedpool_config.general.api_key) {
                    Ok(Some(download_link)) => {
                        println!("Duplicate found for '{}'. Download link: {}", name, download_link);
                        std::process::exit(1); // Exit with non-zero code if duplicate is found
                    }
                    Ok(None) => {
                        println!("No duplicate found for '{}'.", name);
                        std::process::exit(0); // Exit with zero code if no duplicate is found
                    }
                    Err(e) => {
                        error!("Error checking for duplicate: {}", e);
                        std::process::exit(2); // Exit with a different non-zero code for errors
                    }
                }
            }
        }
    }

    // --- Handle Input Path Dependent Modes ---
    if let Some(input_path) = cli.input_path {
        let input_path_str = input_path.to_str().ok_or("Invalid input path string")?;
        info!("Processing input path: {}", input_path_str);

        // Generate release name
        let sanitized_name = generate_release_name(
            &input_path
                .file_name()
                .ok_or("Could not get filename from input path")?
                .to_string_lossy()
                .to_string(),
        );
        info!("Generated sanitized release name: {}", sanitized_name);

        let mut errors = Vec::new();

        // --- Custom Upload Mode ---
        if let Some(category_type_arg) = cli.custom_cat_type {
            info!("Running in custom upload mode with category/type: {}", category_type_arg);

            // Validate and process custom upload
            if !cli.sp && !cli.tl {
                error!("Custom upload (-c/--custom-cat-type) requires either --SP or --TL to be specified.");
                return Ok(()); // Exit cleanly
            }

            if category_type_arg == "0720" || category_type_arg == "0740" || category_type_arg == "0741" {
                info!("Detected eBook upload mode with argument: {}", category_type_arg);
            
                // Assuming `config` and `seedpool_config` are already initialized
                if let Err(e) = utils::process_ebook_upload(input_path_str, &main_config, &seedpool_config) {
                    error!("Error processing eBook upload: {}", e);
                } else {
                    info!("Successfully processed eBook upload.");
                }
                return Ok(()); // Exit after eBook upload
            }

            if category_type_arg == "0742" {
                info!("Detected Newspaper upload mode with argument: {}", category_type_arg);

                if let Err(e) = utils::process_newspaper_upload(input_path_str, &main_config, &seedpool_config) {
                    error!("Error processing Newspaper upload: {}", e);
                } else {
                    info!("Successfully processed Newspaper upload.");
                }
                return Ok(()); // Exit after Newspaper upload
            }

            let category_id: u32 = category_type_arg[0..2].parse()?;
            let type_id: u32 = category_type_arg[2..4].parse()?;
            info!("Parsed Category ID: {}, Type ID: {}", category_id, type_id);

            let target_tracker = if cli.sp {
                "seedpool"
            } else {
                "torrentleech"
            };

            let base_name = input_path
                .file_name()
                .ok_or("Could not get filename from input path")?
                .to_string_lossy()
                .to_string();

            if category_type_arg == "1416" || category_type_arg == "1915" {
                let igdb_client_id = &main_config.general.igdb_client_id;
                let igdb_bearer_token = &main_config.general.igdb_bearer_token;
                let game_title = &sanitize_game_title(&base_name);

                if let Err(e) = process_game_upload(
                    input_path_str,
                    category_id,
                    type_id,
                    &main_config.qbittorrent,
                    &main_config.deluge,
                    target_tracker,
                    Some(&seedpool_config),
                    Some(&torrentleech_config),
                    mkbrr_path.to_str().ok_or("Invalid mkbrr_path")?,
                    &main_config.paths,
                    igdb_client_id,
                    igdb_bearer_token,
                ) {
                    error!("Error processing game upload for {}: {}", target_tracker, e);
                } else {
                    info!("Successfully processed game upload for {}.", target_tracker);
                }
                return Ok(());
            }            
            
            if category_type_arg.len() != 4 || !category_type_arg.chars().all(|c| c.is_digit(10)) {
                error!("Invalid format for custom upload specifier (-c/--custom-cat-type). Expected 4 digits (e.g., 0819), got: {}", category_type_arg);
                return Ok(()); // Exit cleanly
            }

            let category_id: u32 = category_type_arg[0..2].parse()?;
            let type_id: u32 = category_type_arg[2..4].parse()?;
            info!("Parsed Category ID: {}, Type ID: {}", category_id, type_id);

            let target_tracker = if cli.sp {
                "seedpool"
            } else {
                "torrentleech"
            };

            if let Err(e) = process_custom_upload(
                input_path_str,
                category_id,
                type_id,
                &main_config.qbittorrent,
                &main_config.deluge,
                target_tracker,
                Some(&seedpool_config),
                Some(&torrentleech_config),
                mkbrr_path.to_str().ok_or("Invalid mkbrr_path")?,
                &main_config.paths,
            ) {
                error!("Error processing custom upload for {}: {}", target_tracker, e);
            } else {
                info!("Successfully processed custom upload for {}.", target_tracker);
            }
            return Ok(()); // Exit after custom upload
        }

        // --- Standard Upload Mode ---
        info!("Running in standard upload mode.");
        let imgbb_api_key = main_config.imgbb.as_ref().map(|imgbb| imgbb.imgbb_api_key.clone());
        debug!("Loaded imgbb API key: {:?}", imgbb_api_key);
        
        // Pass the imgbb_api_key to the relevant functions
        if cli.sp {
            if let Err(e) = trackers::seedpool::process_seedpool_release(
                input_path_str,
                &sanitized_name,
                &mut main_config,
                &seedpool_config,
                &ffmpeg_path,
                &ffprobe_path,
                &mkbrr_path,
                &mediainfo_path,
                imgbb_api_key.as_deref(), // Pass the imgbb API key
            ) {
                error!("Error processing Seedpool release: {}", e);
                errors.push(format!("Seedpool: {}", e));
            } else {
                info!("Successfully processed Seedpool release for: {}", sanitized_name);
            }
        }

        if cli.tl {
            if let Err(e) = trackers::torrentleech::process_torrentleech_release(
                input_path_str,
                &sanitized_name,
                &mut main_config,
                &torrentleech_config,
                &mkbrr_path,
                &mediainfo_path,
            ) {
                error!("Error processing TorrentLeech release: {}", e);
                errors.push(format!("TorrentLeech: {}", e));
            } else {
                info!("Successfully processed TorrentLeech release for: {}", sanitized_name);
            }
        }

        if !cli.sp && !cli.tl {
            error!("No tracker specified for upload (--SP or --TL required for standard upload).");
        }

        if errors.is_empty() {
            info!("Upload completed successfully for all specified trackers.");
        } else {
            error!("Upload completed with errors: {:?}", errors);
        }
    } else {
        error!("Usage error: An input path is required unless using --sync.");
        Cli::command().print_help()?;
        return Ok(()); // Exit cleanly
    }

    info!("Seed Tools finished.");
    Ok(())
}