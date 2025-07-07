use clap::{Parser, ValueEnum};
use rand::Rng;
use seedbrr::core::Config;
use seedbrr::media::detector::detect_media_type;
use simplelog::LevelFilter;
use std::fs;
use std::io::Write;
use std::path::Path;

#[derive(Parser, Debug)]
#[command(author, version, about = "Test process_upload classification and tracker mapping", long_about = None)]
struct Args {
    /// Media type to test (video, audio, ebook, game, hobby)
    #[arg(short, long, value_enum)]
    media_type: MediaType,

    /// Input file containing filenames (one per line)
    #[arg(short = 'f', long)]
    file: Option<String>,

    /// Input directory to scan for files
    #[arg(short = 'd', long)]
    directory: Option<String>,

    /// Direct filename(s) to test
    #[arg(short = 'i', long)]
    input: Vec<String>,

    /// Number of random files to generate if no input provided
    #[arg(short = 'n', long, default_value = "20")]
    count: usize,

    /// Config file path
    #[arg(short = 'C', long, default_value = "config/config.yaml")]
    config: String,
}

#[derive(Debug, Clone, ValueEnum)]
enum MediaType {
    Video,
    Audio,
    Ebook,
    Game,
    Hobby,
}

// Logger struct to handle dual output
struct Logger {
    file: Option<fs::File>,
}

impl Logger {
    fn new() -> Self {
        Logger { file: None }
    }

    fn init(&mut self, path: &str) -> std::io::Result<()> {
        self.file = Some(
            fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(path)?,
        );
        Ok(())
    }

    fn println(&mut self, msg: &str) {
        println!("{}", msg);
        if let Some(ref mut file) = self.file {
            let _ = writeln!(file, "{}", msg);
            let _ = file.flush();
        }
    }
}

// Macro to print to both console and log file
macro_rules! log_println {
    ($logger:expr, $($arg:tt)*) => {{
        let msg = format!($($arg)*);
        $logger.println(&msg);
    }};
}

fn main() {
    // Initialize simple logger
    let _ = simplelog::SimpleLogger::init(LevelFilter::Info, simplelog::Config::default());

    let args = Args::parse();

    // Load configuration
    let config_str = match fs::read_to_string(&args.config) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to read config file: {}", e);
            std::process::exit(1);
        }
    };

    let _config: Config = match serde_yaml::from_str(&config_str) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Failed to parse config: {}", e);
            std::process::exit(1);
        }
    };

    // Initialize logger
    let mut logger = Logger::new();
    match logger.init("test-process-classification.log") {
        Ok(_) => {
            log_println!(logger, "=== Test Process Classification Log ===");
            log_println!(logger, "Media type: {:?}", args.media_type);
            log_println!(logger, "");
        }
        Err(e) => {
            eprintln!("Warning: Could not create log file: {}", e);
        }
    }

    // Load tracker configurations to determine active tracker
    let seedpool_config =
        seedbrr::utils::load_tracker_config::<seedbrr::core::types::SeedpoolConfig>("seedpool")
            .ok();
    let torrentleech_config = seedbrr::utils::load_tracker_config::<
        seedbrr::core::types::TorrentLeechConfig,
    >("torrentleech")
    .ok();

    // Determine active tracker
    let active_tracker = if seedpool_config
        .as_ref()
        .map(|c| c.general.enabled)
        .unwrap_or(false)
    {
        Some("seedpool")
    } else if torrentleech_config
        .as_ref()
        .map(|c| c.general.enabled)
        .unwrap_or(false)
    {
        Some("torrentleech")
    } else {
        None
    };

    log_println!(logger, "Active tracker: {:?}", active_tracker);

    log_println!(logger, "");

    // Collect all filenames to test
    let mut test_paths = Vec::new();

    // From file
    if let Some(ref file_path) = args.file {
        match fs::read_to_string(file_path) {
            Ok(content) => {
                test_paths.extend(content.lines().map(String::from));
            }
            Err(e) => {
                eprintln!("Error reading file {}: {}", file_path, e);
                std::process::exit(1);
            }
        }
    }

    // From directory
    if let Some(ref dir_path) = args.directory {
        match scan_directory(dir_path, &args.media_type) {
            Ok(files) => test_paths.extend(files),
            Err(e) => {
                eprintln!("Error scanning directory {}: {}", dir_path, e);
                std::process::exit(1);
            }
        }
    }

    // From direct input
    let had_direct_input = !args.input.is_empty();
    test_paths.extend(args.input);

    // Generate random files if requested
    if test_paths.is_empty()
        || (args.count > 0 && args.file.is_none() && args.directory.is_none() && !had_direct_input)
    {
        // Create temporary files/directories for testing
        let temp_dir = create_temp_test_directory(&args.media_type);
        let generated = generate_test_paths(&temp_dir, &args.media_type, args.count);
        test_paths.extend(generated);
    }

    // Test classification
    log_println!(logger, "=== Testing {} paths ===\n", test_paths.len());

    let mut success_count = 0;
    let mut error_count = 0;

    for (i, test_path) in test_paths.iter().enumerate() {
        log_println!(logger, "{}. Path: {}", i + 1, test_path);

        // Use new media detection to get media files
        match detect_media_type(test_path) {
            Ok(media_files) => {
                success_count += 1;
                log_println!(logger, "   Detection: SUCCESS");
                if let Some(first_file) = media_files.first() {
                    log_println!(logger, "   Media Type: {:?}", first_file.media_type);
                    log_println!(logger, "   Path: {:?}", first_file.path);
                    log_println!(logger, "   Files detected: {}", media_files.len());
                } else {
                    log_println!(logger, "   No media files detected");
                }
            }
            Err(e) => {
                error_count += 1;
                log_println!(logger, "   Detection: ERROR");
                log_println!(logger, "   Error: {}", e);
            }
        }

        log_println!(logger, "");
    }

    // Print summary
    log_println!(logger, "\n=== Classification Summary ===");
    log_println!(logger, "Total paths tested: {}", test_paths.len());
    log_println!(
        logger,
        "Successful: {} ({:.1}%)",
        success_count,
        (success_count as f64 / test_paths.len() as f64) * 100.0
    );
    log_println!(
        logger,
        "Errors: {} ({:.1}%)",
        error_count,
        (error_count as f64 / test_paths.len() as f64) * 100.0
    );

    // Clean up temporary files
    cleanup_temp_directory();

    log_println!(logger, "\n=== Test completed ===");
}

// Old function commented out - no longer needed with ProcessBuilder pattern
/*
fn map_detection_to_classification(detection: &DetectionResult) -> (String, Option<String>) {
    match detection.category_type {
        ContentCategory::Video => {
            let category = match detection.media_type {
                ContentType::Movie | ContentType::Movie4K | ContentType::MovieRemux | ContentType::MovieWeb => "VideoCategory::Movie",
                ContentType::TvShow => "VideoCategory::TvShow",
                ContentType::Anime => "VideoCategory::Anime",
                ContentType::Sports => "VideoCategory::Sports",
                _ => "VideoCategory::Unknown",
            };

            let source = match detection.media_type {
                ContentType::MovieRemux => Some("VideoSourceType::Remux".to_string()),
                ContentType::MovieWeb => Some("VideoSourceType::WebDL".to_string()),
                ContentType::Movie4K => Some("VideoSourceType::UHDBluRay".to_string()),
                _ => None,
            };

            (category.to_string(), source)
        }
        ContentCategory::Audio => {
            let category = match detection.media_type {
                ContentType::MusicFlac | ContentType::MusicMp3 => "AudioCategory::Album",
                ContentType::Audiobook => "AudioCategory::Audiobook",
                _ => "AudioCategory::Unknown",
            };

            let source = match detection.media_type {
                ContentType::MusicFlac => Some("AudioSourceType::CD".to_string()),
                ContentType::MusicMp3 => Some("AudioSourceType::Web".to_string()),
                _ => None,
            };

            (category.to_string(), source)
        }
        ContentCategory::Ebook => {
            let category = match detection.media_type {
                ContentType::Ebook => "EbookCategory::Novel",
                ContentType::Comic => "EbookCategory::Comic",
                ContentType::Magazine => "EbookCategory::Magazine",
                _ => "EbookCategory::Unknown",
            };

            (category.to_string(), None)
        }
        ContentCategory::Game => {
            let category = match detection.media_type {
                ContentType::PCGame => "GameCategory::PCGame",
                ContentType::PS4Game => "GameCategory::PS4Game",
                ContentType::NSWGame => "GameCategory::NintendoSwitch",
                _ => "GameCategory::Unknown",
            };

            (category.to_string(), None)
        }
        ContentCategory::Hobby => {
            ("HobbyCategory::Collection".to_string(), None)
        }
        ContentCategory::Sports => {
            ("VideoCategory::Sports".to_string(), None)
        }
        ContentCategory::Education => {
            ("HobbyCategory::Educational".to_string(), None)
        }
        _ => {
            ("Unknown::Unknown".to_string(), None)
        }
    }
}
*/

fn scan_directory(dir_path: &str, media_type: &MediaType) -> Result<Vec<String>, String> {
    let mut files = Vec::new();
    let extensions = match media_type {
        MediaType::Video => vec![
            "mkv", "mp4", "avi", "mov", "wmv", "flv", "webm", "m4v", "ts", "mpg", "mpeg", "iso",
        ],
        MediaType::Audio => vec![
            "mp3", "flac", "wav", "aac", "ogg", "m4a", "wma", "aiff", "ape", "opus",
        ],
        MediaType::Ebook => vec![
            "epub", "pdf", "cbz", "cbr", "mobi", "azw", "azw3", "lit", "pdb",
        ],
        MediaType::Game => vec!["zip", "rar", "7z", "exe", "msi", "iso", "pkg"],
        MediaType::Hobby => vec!["zip", "rar", "7z", "pdf", "jpg", "png", "doc", "docx"],
    };

    for entry in fs::read_dir(dir_path).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();

        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if extensions.contains(&ext.to_lowercase().as_str()) {
                    if let Some(path_str) = path.to_str() {
                        files.push(path_str.to_string());
                    }
                }
            }
        }
    }

    Ok(files)
}

fn create_temp_test_directory(media_type: &MediaType) -> String {
    let temp_dir = format!(
        "/tmp/seed-tools-test-classification-{}-{}",
        match media_type {
            MediaType::Video => "video",
            MediaType::Audio => "audio",
            MediaType::Ebook => "ebook",
            MediaType::Game => "game",
            MediaType::Hobby => "hobby",
        },
        std::process::id()
    );

    let _ = fs::create_dir_all(&temp_dir);
    temp_dir
}

fn generate_test_paths(temp_dir: &str, media_type: &MediaType, count: usize) -> Vec<String> {
    let mut rng = rand::thread_rng();
    let mut paths = Vec::new();

    match media_type {
        MediaType::Video => {
            let titles = [
                "Breaking.Bad",
                "The.Wire",
                "Game.of.Thrones",
                "Inception",
                "The.Matrix",
                "Stranger.Things",
                "The.Dark.Knight",
                "Interstellar",
                "Attack.on.Titan",
            ];
            let years = ["2010", "2015", "2020", "2023", "2024"];
            let qualities = ["720p", "1080p", "2160p", "4K"];
            let sources = ["BluRay", "WEB-DL", "HDTV", "WEBRip", "Remux"];

            for _ in 0..count {
                let title = titles[rng.gen_range(0..titles.len())];
                let is_tv = rng.gen_bool(0.6);

                let filename = if is_tv {
                    format!(
                        "{}.S{:02}E{:02}.{}.{}.x264-GROUP.mkv",
                        title,
                        rng.gen_range(1..5),
                        rng.gen_range(1..13),
                        qualities[rng.gen_range(0..qualities.len())],
                        sources[rng.gen_range(0..sources.len())]
                    )
                } else {
                    format!(
                        "{}.{}.{}.{}.x264-GROUP.mp4",
                        title,
                        years[rng.gen_range(0..years.len())],
                        qualities[rng.gen_range(0..qualities.len())],
                        sources[rng.gen_range(0..sources.len())]
                    )
                };

                // Create as directory 30% of the time
                if rng.gen_bool(0.3) {
                    let dir_path = format!(
                        "{}/{}",
                        temp_dir,
                        filename.replace(".mkv", "").replace(".mp4", "")
                    );
                    let _ = fs::create_dir_all(&dir_path);
                    let file_path = format!("{}/{}", dir_path, filename);
                    create_dummy_file(&file_path, 700 * 1024 * 1024); // 700MB dummy file
                    paths.push(dir_path);
                } else {
                    let file_path = format!("{}/{}", temp_dir, filename);
                    create_dummy_file(&file_path, 1024 * 1024 * 1024); // 1GB dummy file
                    paths.push(file_path);
                }
            }
        }
        MediaType::Audio => {
            let artists = [
                "Pink Floyd",
                "Led Zeppelin",
                "The Beatles",
                "Nirvana",
                "Radiohead",
                "Various Artists",
                "VA",
                "Podcast",
                "Audiobook Collection",
            ];
            let albums = [
                "Greatest Hits",
                "Live Album",
                "Studio Sessions",
                "Unplugged",
                "Discography",
            ];
            let years = ["1970", "1980", "1990", "2000", "2020", "2023", "2024"];

            for _ in 0..count {
                let artist = artists[rng.gen_range(0..artists.len())];
                let album = albums[rng.gen_range(0..albums.len())];
                let year = years[rng.gen_range(0..years.len())];

                let album_dir = format!("{}/{} - {} ({}) [FLAC]", temp_dir, artist, album, year);
                let _ = fs::create_dir_all(&album_dir);

                // Create multiple tracks
                for track in 1..=5 {
                    let file_path = format!("{}/{:02} - Track {}.flac", album_dir, track, track);
                    create_dummy_file(&file_path, 30 * 1024 * 1024); // 30MB per track
                }
                paths.push(album_dir);
            }
        }
        MediaType::Ebook => {
            let authors = [
                "Stephen King",
                "J.K. Rowling",
                "George R.R. Martin",
                "Marvel Comics",
                "DC Comics",
                "National Geographic",
                "O'Reilly",
            ];
            let titles = [
                "The Mystery",
                "Complete Guide",
                "Volume 1",
                "Issue 001",
                "Collection",
            ];
            let years = ["2010", "2015", "2020", "2023", "2024"];

            for _ in 0..count {
                let author = authors[rng.gen_range(0..authors.len())];
                let title = titles[rng.gen_range(0..titles.len())];
                let year = years[rng.gen_range(0..years.len())];
                let is_comic = author.contains("Comics") || rng.gen_bool(0.3);

                let filename = if is_comic {
                    format!(
                        "{} - {} #{:03} ({}).cbz",
                        author,
                        title,
                        rng.gen_range(1..100),
                        year
                    )
                } else {
                    format!("{} - {} ({}).epub", author, title, year)
                };

                let file_path = format!("{}/{}", temp_dir, filename);
                create_dummy_file(&file_path, 5 * 1024 * 1024); // 5MB
                paths.push(file_path);
            }
        }
        MediaType::Game => {
            let games = [
                "Cyberpunk.2077",
                "The.Witcher.3",
                "GTA.V",
                "Red.Dead.Redemption.2",
                "Call.of.Duty.Modern.Warfare",
                "FIFA.24",
                "NBA.2K24",
            ];
            let platforms = ["PC", "PS5", "PS4", "XBOX", "NSW"];
            let groups = ["CODEX", "PLAZA", "CPY", "RELOADED", "GOG", "ElAmigos"];

            for _ in 0..count {
                let game = games[rng.gen_range(0..games.len())];
                let platform = platforms[rng.gen_range(0..platforms.len())];
                let group = groups[rng.gen_range(0..groups.len())];

                // Create as ISO or archive
                if rng.gen_bool(0.5) {
                    let filename = format!("{}-{}-{}.iso", game, platform, group);
                    let file_path = format!("{}/{}", temp_dir, filename);
                    create_dummy_file(&file_path, 8 * 1024 * 1024 * 1024); // 8GB
                    paths.push(file_path);
                } else {
                    let dir_name = format!("{}-{}-{}", game, platform, group);
                    let dir_path = format!("{}/{}", temp_dir, dir_name);
                    let _ = fs::create_dir_all(&dir_path);

                    // Create setup.exe
                    let setup_path = format!("{}/setup.exe", dir_path);
                    create_dummy_file(&setup_path, 50 * 1024 * 1024); // 50MB

                    // Create game data files
                    for i in 1..=3 {
                        let data_path = format!("{}/data{}.bin", dir_path, i);
                        create_dummy_file(&data_path, 1024 * 1024 * 1024); // 1GB each
                    }

                    paths.push(dir_path);
                }
            }
        }
        MediaType::Hobby => {
            let types = [
                "Tutorial",
                "Template",
                "Resource.Pack",
                "Collection",
                "Assets",
                "Fonts",
            ];
            let subjects = [
                "Photoshop",
                "3D.Modeling",
                "Photography",
                "Design",
                "Crafts",
                "Premium",
            ];
            let years = ["2020", "2021", "2022", "2023", "2024"];

            for _ in 0..count {
                let type_name = types[rng.gen_range(0..types.len())];
                let subject = subjects[rng.gen_range(0..subjects.len())];
                let year = years[rng.gen_range(0..years.len())];

                if rng.gen_bool(0.5) {
                    // Create as archive
                    let filename = format!("{}.{}.{}.zip", subject, type_name, year);
                    let file_path = format!("{}/{}", temp_dir, filename);
                    create_dummy_file(&file_path, 100 * 1024 * 1024); // 100MB
                    paths.push(file_path);
                } else {
                    // Create as directory
                    let dir_name = format!("{}.{}.{}.Collection", subject, type_name, year);
                    let dir_path = format!("{}/{}", temp_dir, dir_name);
                    let _ = fs::create_dir_all(&dir_path);

                    // Create various files
                    for i in 1..=5 {
                        let file_path = format!("{}/file{}.psd", dir_path, i);
                        create_dummy_file(&file_path, 20 * 1024 * 1024); // 20MB each
                    }

                    paths.push(dir_path);
                }
            }
        }
    }

    // Add edge cases
    match media_type {
        MediaType::Video => {
            paths.push(format!("{}/test.mp4", temp_dir));
            paths.push(format!(
                "{}/[HorribleSubs] One Piece - 1000 [1080p].mkv",
                temp_dir
            ));
            paths.push(format!(
                "{}/NFL.2024.Week.01.Cowboys.vs.Giants.1080p.WEB.h264-SPORTSNET.mkv",
                temp_dir
            ));
        }
        MediaType::Audio => {
            paths.push(format!("{}/test.mp3", temp_dir));
            paths.push(format!(
                "{}/Joe Rogan Experience #2000 - Elon Musk.mp3",
                temp_dir
            ));
        }
        MediaType::Ebook => {
            paths.push(format!("{}/test.epub", temp_dir));
            paths.push(format!("{}/Time Magazine - December 2024.pdf", temp_dir));
        }
        MediaType::Game => {
            paths.push(format!("{}/test.exe", temp_dir));
            paths.push(format!("{}/Steam.Games.Collection.2024.rar", temp_dir));
        }
        MediaType::Hobby => {
            paths.push(format!("{}/test.zip", temp_dir));
            paths.push(format!(
                "{}/1000.Premium.Fonts.Collection.2024.rar",
                temp_dir
            ));
        }
    }

    // Create the edge case files
    for path in &paths {
        if !Path::new(path).exists() {
            if path.ends_with(".mp4")
                || path.ends_with(".mkv")
                || path.ends_with(".mp3")
                || path.ends_with(".epub")
                || path.ends_with(".pdf")
                || path.ends_with(".exe")
                || path.ends_with(".zip")
                || path.ends_with(".rar")
            {
                create_dummy_file(path, 10 * 1024 * 1024); // 10MB
            }
        }
    }

    paths
}

fn create_dummy_file(path: &str, size: usize) {
    if let Some(parent) = Path::new(path).parent() {
        let _ = fs::create_dir_all(parent);
    }

    if let Ok(mut file) = fs::File::create(path) {
        let buffer = vec![0u8; size.min(1024 * 1024)]; // Max 1MB chunks
        let chunks = size / buffer.len();
        let remainder = size % buffer.len();

        for _ in 0..chunks {
            let _ = file.write_all(&buffer);
        }
        if remainder > 0 {
            let _ = file.write_all(&buffer[..remainder]);
        }
    }
}

fn cleanup_temp_directory() {
    // Clean up any test directories
    if let Ok(entries) = fs::read_dir("/tmp") {
        for entry in entries {
            if let Ok(entry) = entry {
                if let Some(name) = entry.file_name().to_str() {
                    if name.starts_with("seed-tools-test-classification-") {
                        let _ = fs::remove_dir_all(entry.path());
                    }
                }
            }
        }
    }
}
