use std::{
    fs,
    path::{Path, PathBuf},
};
use log::{info, error, debug, LevelFilter};
use simplelog::{Config as SimpleLogConfig, CombinedLogger, WriteLogger};
use std::error::Error;
use seed_tools::utils::{generate_release_name, validate_file_path};
use seed_tools::types::{Config, SeedpoolConfig, TorrentLeechConfig, QbittorrentConfig, DelugeConfig};
use seed_tools::definitions::seedpool::{SeedpoolTorrentInfo, parse_seedpool_category_type, print_seedpool_categories_and_types};
use seed_tools::sync;
use seed_tools::irc::launch_irc_client;
use trackers::seedpool::preflight_check;
use seed_tools::ui;
use seed_tools::media::process::{process_upload, process_upload_with_info};
mod trackers {
    pub mod seedpool;
    pub mod torrentleech;
    pub mod common;
}
use std::fs::OpenOptions;
use clap::{Parser, CommandFactory};

fn load_yaml_config<T: serde::de::DeserializeOwned>(path: &str) -> Result<T, String> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read config file '{}': {}", path, e))?;
    serde_yaml::from_str(&content)
        .map_err(|e| format!("Failed to parse YAML config '{}': {}", path, e))
}

fn parse_category_type_argument(category_type_arg: &str) -> Result<SeedpoolTorrentInfo, String> {
    parse_seedpool_category_type(category_type_arg)
}

fn print_available_categories_and_types() {
    print_seedpool_categories_and_types();
}

fn extract_binary_paths(config_path: &str) -> Result<(PathBuf, PathBuf, PathBuf, PathBuf), String> {
    let config: serde_yaml::Value = serde_yaml::from_str(
        &fs::read_to_string(config_path).map_err(|e| format!("Failed to read config file: {}", e))?,
    )
    .map_err(|e| format!("Failed to parse config file: {}", e))?;
    let paths = config["paths"]
        .as_mapping()
        .ok_or("Missing or invalid 'paths' field in config")?;
    
    let required_binaries = ["ffmpeg", "ffprobe", "mkbrr", "mediainfo"];
    let mut binary_paths = Vec::new();
    
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
        binary_paths.push(PathBuf::from(binary_path));
    }
    
    Ok((
        binary_paths[0].clone(), // ffmpeg
        binary_paths[1].clone(), // ffprobe  
        binary_paths[2].clone(), // mkbrr
        binary_paths[3].clone(), // mediainfo
    ))
}

#[derive(Parser, Debug)]
#[command(author, version, about = "Automated tool for processing and uploading releases to trackers.", long_about = None)]
struct Cli {
    #[arg(long, conflicts_with_all = ["sp", "tl", "custom_cat_type", "irc"])]
    sync: bool,

    #[arg(long = "SP", requires = "input_path")]
    sp: bool,

    #[arg(long = "TL", requires = "input_path")]
    tl: bool,

    #[arg(short = 'c', long, value_name = "CAT_TYPE", requires = "input_path")]
    custom_cat_type: Option<String>,

    #[arg(long, conflicts_with_all = ["sync", "sp", "tl", "custom_cat_type", "irc"])]
    ui: bool, // Add the `ui` argument

    #[arg(long, conflicts_with_all = ["sync", "sp", "tl", "custom_cat_type", "ui"])]
    irc: bool, // Add the `irc` argument

    #[arg(long, conflicts_with_all = ["sync", "sp", "tl", "custom_cat_type"])]
    pre: bool, // Add the `pre` argument

    #[arg(long, help = "Enable dry-run mode - simulate uploads without actually uploading")]
    dry_run: bool,

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
    
    // Log dry-run mode if enabled
    if cli.dry_run {
        info!("🚫 DRY RUN MODE ENABLED - No actual uploads or downloads will occur");
    }

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
    let (ffmpeg_path, ffprobe_path, mkbrr_path, mediainfo_path) = extract_binary_paths(main_config_path_str).map_err(|e| {
        error!("Failed to extract binary paths using config {:?}: {}", main_config_path, e);
        format!("Failed to extract binary paths: {}", e)
    })?;
    debug!(
        "Binary paths: ffmpeg={:?}, ffprobe={:?}, mkbrr={:?}, mediainfo={:?}",
        ffmpeg_path, ffprobe_path, mkbrr_path, mediainfo_path
    );

    let seedpool_config_path_str = seedpool_config_path.to_str()
        .ok_or_else(|| format!("Invalid non-UTF8 path for seedpool config: {:?}", seedpool_config_path))?;
    let torrentleech_config_path_str = torrentleech_config_path.to_str()
        .ok_or_else(|| format!("Invalid non-UTF8 path for torrentleech config: {:?}", torrentleech_config_path))?;

    let mut main_config: Config = load_yaml_config::<Config>(main_config_path_str)
        .map_err(|e| format!("Failed to load main config: {}", e))?;
    let seedpool_config: SeedpoolConfig = load_yaml_config(seedpool_config_path_str)
        .map_err(|e| format!("Failed to load seedpool config: {}", e))?;
    let torrentleech_config: TorrentLeechConfig = load_yaml_config(torrentleech_config_path_str)
        .map_err(|e| format!("Failed to load torrentleech config: {}", e))?;
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
        
        // Validate input path
        validate_file_path(input_path_str)
            .map_err(|e| format!("Input path validation failed: {}", e))?;
        
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

        // Validate tracker selection
        if !cli.sp && !cli.tl {
            error!("No tracker specified. Please use --SP for Seedpool or --TL for TorrentLeech.");
            return Ok(());
        }

        // --- Process Upload with New Media Detection System ---
        if let Some(category_type_arg) = cli.custom_cat_type {
            // User provided a 4-digit code with -c flag
            info!("Processing with provided category/type code: {}", category_type_arg);
            
            // Parse the 4-digit code (for now, we'll use Seedpool's parser since it's the most complete)
            let torrent_info = match parse_category_type_argument(&category_type_arg) {
                Ok(info) => {
                    info!("Parsed torrent classification: {}", info.description());
                    info
                }
                Err(e) => {
                    error!("Failed to parse category/type argument: {}", e);
                    println!("\n{}", e);
                    print_available_categories_and_types();
                    return Ok(());
                }
            };
            
            // Process with the explicit torrent info
            if let Err(e) = process_upload_with_info(
                input_path_str,
                &torrent_info,
                &main_config,
                cli.dry_run,
            ) {
                error!("Error processing upload: {}", e);
                return Err(e.into());
            }
        } else {
            // No -c flag provided, use auto-detection
            info!("No category/type code provided, using auto-detection for: {}", input_path_str);
            
            if let Err(e) = process_upload(
                input_path_str,
                None,
                &main_config,
                cli.dry_run,
            ) {
                error!("Error processing upload: {}", e);
                return Err(e.into());
            }
        }
        
        info!("Upload processing completed successfully.");
    } else {
        error!("Usage error: An input path is required unless using --sync.");
        Cli::command().print_help()?;
        return Ok(()); // Exit cleanly
    }

    info!("Seed Tools finished.");
    Ok(())
}