use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    error::Error,
};
use log::{info, error, debug, LevelFilter};
use simplelog::{Config as SimpleLogConfig, CombinedLogger, WriteLogger};
use clap::{Parser, CommandFactory};
use std::fs::OpenOptions;

use seedbrr::core::{Config, types::{SeedpoolConfig, TorrentLeechConfig}};
use seedbrr::processing::naming::generate_release_name;
use seedbrr::utils::{validate_file_path, load_tracker_config};
use seedbrr::trackers::{TorrentInfo, seedpool::{SeedpoolTorrentInfo, parse_seedpool_category_type, print_seedpool_categories_and_types}};
use seedbrr::clients::{sync, irc::launch_irc_client};
use seedbrr::ui::tui::launch_ui;
use seedbrr::processing::{process_builder, preflight::{preflight_check, print_preflight_results}};

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

    #[arg(long, conflicts_with_all = ["sync", "sp", "tl", "custom_cat_type"], requires = "input_path")]
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
    /// Check for duplicates across trackers using input path
    Check {
        /// Path to the media file or directory to check for duplicates
        #[arg(index = 1)]
        input_path: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // --- Initialize Logging ---
    let log_path = Path::new("seedbrr.log");
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
        return launch_ui();
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

    let main_config: Config = load_yaml_config::<Config>(main_config_path_str)
        .map_err(|e| format!("Failed to load main config: {}", e))?;
    let seedpool_config: SeedpoolConfig = load_yaml_config(seedpool_config_path_str)
        .map_err(|e| format!("Failed to load seedpool config: {}", e))?;
    let _torrentleech_config: TorrentLeechConfig = load_yaml_config(torrentleech_config_path_str)
        .map_err(|e| format!("Failed to load torrentleech config: {}", e))?;
    info!("Configurations loaded.");

    if cli.pre {
        info!("Running preflight check mode...");
        
        // Require input path for preflight check
        if cli.input_path.is_none() {
            error!("Input path is required for preflight check.");
            println!("Error: Please provide an input path for the preflight check.");
            return Ok(());
        }
        
        let input_path = cli.input_path.unwrap();
        let input_path_str = input_path.to_str().ok_or("Invalid input path string")?;
        
        // Run the preflight check
        match preflight_check(input_path_str, &main_config, cli.dry_run) {
            Ok(result) => {
                // Print the results
                print_preflight_results(&result);
                return Ok(());
            }
            Err(e) => {
                error!("Preflight check failed: {}", e);
                println!("Error during preflight check: {}", e);
                return Err(e.into());
            }
        }
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
            Commands::Check { input_path } => {
                let input_path_str = input_path.to_str().ok_or("Invalid input path string")?;
                info!("Running duplicate check for input path: {}", input_path_str);

                // Validate input path
                validate_file_path(input_path_str)
                    .map_err(|e| format!("Input path validation failed: {}", e))?;

                // Use the process builder for duplicate checking
                match process_builder::duplicate_check_builder(input_path_str, Arc::new(main_config.clone())).build() {
                    Ok(result) => {
                        // Check if we found duplicates
                        if let Some(preflight_data) = result.preflight_data {
                            if preflight_data.dupe_check.contains("FAIL") {
                                println!("🚫 Duplicate found for '{}'", result.title);
                                println!("Duplicate check result: {}", preflight_data.dupe_check);
                                std::process::exit(1);
                            } else {
                                println!("✅ No duplicates found for '{}'", result.title);
                                println!("Duplicate check result: {}", preflight_data.dupe_check);
                                std::process::exit(0);
                            }
                        } else {
                            println!("⚠️  Could not perform duplicate check for '{}'", result.title);
                            std::process::exit(2);
                        }
                    }
                    Err(e) => {
                        error!("Error during duplicate check: {}", e);
                        println!("Error during duplicate check: {}", e);
                        std::process::exit(2);
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
            
            // Process with the explicit torrent info using process builder
            match process_builder::upload_builder(input_path_str, Arc::new(main_config.clone()))
                .force_category(format!("{}Category::{}", 
                    if torrent_info.is_video_category() { "Video" }
                    else if torrent_info.is_audio_category() { "Audio" }
                    else if torrent_info.is_ebook_category() { "Ebook" }
                    else if torrent_info.is_game_category() { "Game" }
                    else { "Hobby" },
                    torrent_info.category_name()
                ))
                .dry_run(cli.dry_run)
                .build() {
                Ok(result) => {
                    info!("Upload processing completed successfully for: {}", result.title);
                }
                Err(e) => {
                    error!("Error processing upload: {}", e);
                    return Err(e.into());
                }
            }
        } else {
            // No -c flag provided, use auto-detection with process builder
            info!("No category/type code provided, using auto-detection for: {}", input_path_str);
            
            match process_builder::upload_builder(input_path_str, Arc::new(main_config.clone()))
                .dry_run(cli.dry_run)
                .build() {
                Ok(result) => {
                    info!("Upload processing completed successfully for: {}", result.title);
                }
                Err(e) => {
                    error!("Error processing upload: {}", e);
                    return Err(e.into());
                }
            }
        }
        
        info!("Upload processing completed successfully.");
    } else {
        error!("Usage error: An input path is required unless using --sync.");
        Cli::command().print_help()?;
        return Ok(()); // Exit cleanly
    }

    info!("seedbrr finished.");
    Ok(())
}