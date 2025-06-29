use seedbrr::processing::process_builder;
use seedbrr::core::Config;
use clap::{Parser, ValueEnum};
use std::fs;
use rand::Rng;
use std::io::Write;
use simplelog::LevelFilter;

#[derive(Parser, Debug)]
#[command(author, version, about = "Test full media processing in dry run mode", long_about = None)]
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
    #[arg(short = 'n', long, default_value = "10")]
    count: usize,
    
    /// Force category/type with 4-digit code (e.g., 0102)
    #[arg(short = 'c', long)]
    category_code: Option<String>,
    
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
        self.file = Some(fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)?);
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
    let _ = simplelog::SimpleLogger::init(
        LevelFilter::Info,
        simplelog::Config::default()
    );

    let args = Args::parse();
    
    // Load configuration
    let config_str = match fs::read_to_string(&args.config) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to read config file: {}", e);
            std::process::exit(1);
        }
    };
    
    let config: Config = match serde_yaml::from_str(&config_str) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Failed to parse config: {}", e);
            std::process::exit(1);
        }
    };
    
    // Initialize logger
    let mut logger = Logger::new();
    match logger.init("test-full-process.log") {
        Ok(_) => {
            log_println!(logger, "=== Test Full Process Log ===");
            log_println!(logger, "Media type: {:?}", args.media_type);
            log_println!(logger, "Category code: {:?}", args.category_code);
            log_println!(logger, "Dry run mode: ENABLED");
            log_println!(logger, "");
        }
        Err(e) => {
            eprintln!("Warning: Could not create log file: {}", e);
        }
    }
    
    // Collect all filenames to test
    let mut test_files = Vec::new();
    
    // From file
    if let Some(ref file_path) = args.file {
        match fs::read_to_string(file_path) {
            Ok(content) => {
                test_files.extend(content.lines().map(String::from));
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
            Ok(files) => test_files.extend(files),
            Err(e) => {
                eprintln!("Error scanning directory {}: {}", dir_path, e);
                std::process::exit(1);
            }
        }
    }
    
    // From direct input
    let had_direct_input = !args.input.is_empty();
    test_files.extend(args.input);
    
    // Generate random files if requested
    if test_files.is_empty() || (args.count > 0 && args.file.is_none() && args.directory.is_none() && !had_direct_input) {
        // Create temporary directory for test files
        let temp_dir = create_temp_test_directory(&args.media_type);
        let generated = generate_test_files(&temp_dir, &args.media_type, args.count);
        test_files.extend(generated);
    }
    
    // Process each test file
    log_println!(logger, "=== Processing {} files ===\n", test_files.len());
    
    let mut success_count = 0;
    let mut error_count = 0;
    
    for (i, file_path) in test_files.iter().enumerate() {
        log_println!(logger, "{}. Processing: {}", i + 1, file_path);
        
        let result = if let Some(ref code) = args.category_code {
            // Process with forced category/type
            log_println!(logger, "   Using forced category/type code: {}", code);
            
            match parse_category_type_code(code) {
                Ok(_info) => {
                    // Use ProcessBuilder with category code (simplified for testing)
                    match process_builder::upload_builder(file_path, std::sync::Arc::new(config.clone()))
                        .dry_run(true)
                        .build() {
                        Ok(_) => Ok(()),
                        Err(e) => Err(e),
                    }
                },
                Err(e) => {
                    log_println!(logger, "   ERROR: Invalid category code: {}", e);
                    error_count += 1;
                    continue;
                }
            }
        } else {
            // Process with auto-detection using ProcessBuilder
            log_println!(logger, "   Using auto-detection");
            match process_builder::upload_builder(file_path, std::sync::Arc::new(config.clone()))
                .dry_run(true)
                .build() {
                Ok(_) => Ok(()),
                Err(e) => Err(e),
            }
        };
        
        match result {
            Ok(_) => {
                log_println!(logger, "   SUCCESS: Processing completed");
                success_count += 1;
            }
            Err(e) => {
                log_println!(logger, "   ERROR: {}", e);
                error_count += 1;
            }
        }
        
        log_println!(logger, "");
    }
    
    // Print summary
    log_println!(logger, "\n=== Test Summary ===");
    log_println!(logger, "Total files tested: {}", test_files.len());
    log_println!(logger, "Successful: {} ({:.1}%)", 
        success_count, 
        (success_count as f64 / test_files.len() as f64) * 100.0
    );
    log_println!(logger, "Errors: {} ({:.1}%)", 
        error_count,
        (error_count as f64 / test_files.len() as f64) * 100.0
    );
    
    // Clean up temporary files
    cleanup_temp_directory();
}

fn scan_directory(dir_path: &str, media_type: &MediaType) -> Result<Vec<String>, String> {
    let mut files = Vec::new();
    let extensions = match media_type {
        MediaType::Video => vec!["mkv", "mp4", "avi", "mov", "wmv", "flv", "webm", "m4v", "ts", "mpg", "mpeg"],
        MediaType::Audio => vec!["mp3", "flac", "wav", "aac", "ogg", "m4a", "wma", "aiff", "ape", "opus"],
        MediaType::Ebook => vec!["epub", "pdf", "cbz", "cbr", "mobi", "azw", "azw3", "lit", "pdb"],
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
    let temp_dir = format!("/tmp/seed-tools-test-{}-{}", 
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

fn generate_test_files(temp_dir: &str, media_type: &MediaType, count: usize) -> Vec<String> {
    let mut rng = rand::thread_rng();
    let mut files = Vec::new();
    
    match media_type {
        MediaType::Video => {
            let titles = ["Breaking.Bad", "The.Wire", "Game.of.Thrones", "Inception", "The.Matrix"];
            let years = ["2010", "2015", "2020", "2023", "2024"];
            let qualities = ["720p", "1080p", "2160p"];
            let sources = ["BluRay", "WEB-DL", "HDTV"];
            
            for _ in 0..count {
                let title = titles[rng.gen_range(0..titles.len())];
                let is_tv = rng.gen_bool(0.6);
                
                let filename = if is_tv {
                    format!("{}.S{:02}E{:02}.{}.{}.x264-GROUP.mkv",
                        title,
                        rng.gen_range(1..5),
                        rng.gen_range(1..13),
                        qualities[rng.gen_range(0..qualities.len())],
                        sources[rng.gen_range(0..sources.len())]
                    )
                } else {
                    format!("{}.{}.{}.{}.x264-GROUP.mp4",
                        title,
                        years[rng.gen_range(0..years.len())],
                        qualities[rng.gen_range(0..qualities.len())],
                        sources[rng.gen_range(0..sources.len())]
                    )
                };
                
                let file_path = format!("{}/{}", temp_dir, filename);
                create_dummy_file(&file_path, 1024 * 1024); // 1MB dummy file
                files.push(file_path);
            }
        }
        MediaType::Audio => {
            let artists = ["Pink Floyd", "Led Zeppelin", "The Beatles", "Nirvana", "Radiohead"];
            let albums = ["Greatest Hits", "Live Album", "Studio Sessions", "Unplugged"];
            let years = ["1970", "1980", "1990", "2000", "2020"];
            
            for _ in 0..count {
                let artist = artists[rng.gen_range(0..artists.len())];
                let album = albums[rng.gen_range(0..albums.len())];
                let year = years[rng.gen_range(0..years.len())];
                
                let album_dir = format!("{}/{} - {} ({}) [FLAC]", temp_dir, artist, album, year);
                let _ = fs::create_dir_all(&album_dir);
                
                let file_path = format!("{}/01 - Track One.flac", album_dir);
                create_dummy_file(&file_path, 30 * 1024 * 1024); // 30MB dummy file
                files.push(file_path);
            }
        }
        MediaType::Ebook => {
            let authors = ["Stephen King", "J.K. Rowling", "George R.R. Martin", "Isaac Asimov"];
            let titles = ["The Mystery", "Adventures", "Complete Guide", "Volume 1"];
            let years = ["2010", "2015", "2020", "2023"];
            
            for _ in 0..count {
                let author = authors[rng.gen_range(0..authors.len())];
                let title = titles[rng.gen_range(0..titles.len())];
                let year = years[rng.gen_range(0..years.len())];
                let is_comic = rng.gen_bool(0.3);
                
                let filename = if is_comic {
                    format!("{} - {} #{:03} ({}).cbz", author, title, rng.gen_range(1..100), year)
                } else {
                    format!("{} - {} ({}).epub", author, title, year)
                };
                
                let file_path = format!("{}/{}", temp_dir, filename);
                create_dummy_file(&file_path, 5 * 1024 * 1024); // 5MB dummy file
                files.push(file_path);
            }
        }
        MediaType::Game => {
            let games = ["Cyberpunk.2077", "The.Witcher.3", "GTA.V", "Red.Dead.Redemption.2"];
            let platforms = ["PC", "PS5", "XBOX"];
            
            for _ in 0..count {
                let game = games[rng.gen_range(0..games.len())];
                let platform = platforms[rng.gen_range(0..platforms.len())];
                
                let filename = format!("{}-{}-CODEX.iso", game, platform);
                let file_path = format!("{}/{}", temp_dir, filename);
                create_dummy_file(&file_path, 50 * 1024 * 1024); // 50MB dummy file
                files.push(file_path);
            }
        }
        MediaType::Hobby => {
            let types = ["Tutorial", "Template", "Resource.Pack", "Collection"];
            let subjects = ["Photoshop", "3D.Modeling", "Photography", "Design"];
            
            for _ in 0..count {
                let type_name = types[rng.gen_range(0..types.len())];
                let subject = subjects[rng.gen_range(0..subjects.len())];
                
                let filename = format!("{}.{}.2024.zip", subject, type_name);
                let file_path = format!("{}/{}", temp_dir, filename);
                create_dummy_file(&file_path, 10 * 1024 * 1024); // 10MB dummy file
                files.push(file_path);
            }
        }
    }
    
    files
}

fn create_dummy_file(path: &str, size: usize) {
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
                    if name.starts_with("seed-tools-test-") {
                        let _ = fs::remove_dir_all(entry.path());
                    }
                }
            }
        }
    }
}

fn parse_category_type_code(code: &str) -> Result<TorrentInfo, String> {
    if code.len() != 4 {
        return Err("Category code must be exactly 4 digits".to_string());
    }
    
    let category = code[..2].parse::<u8>()
        .map_err(|_| "Invalid category code")?;
    let type_id = code[2..].parse::<u8>()
        .map_err(|_| "Invalid type code")?;
    
    Ok(TorrentInfo {
        category,
        type_id,
    })
}

// Simple struct to hold category/type info
struct TorrentInfo {
    category: u8,
    type_id: u8,
}

impl seedbrr::trackers::TorrentInfo for TorrentInfo {
    fn category_code(&self) -> u8 {
        self.category
    }
    
    fn type_code(&self) -> u8 {
        self.type_id
    }
    
    fn is_ebook_category(&self) -> bool {
        self.category == 5
    }
    
    fn is_game_category(&self) -> bool {
        self.category == 6
    }
    
    fn is_audio_category(&self) -> bool {
        self.category == 2
    }
    
    fn is_video_category(&self) -> bool {
        self.category == 1
    }
    
    fn is_audiobook_category(&self) -> bool {
        self.category == 4
    }
    
    fn is_hobby_category(&self) -> bool {
        self.category == 11
    }
    
    fn is_sports_category(&self) -> bool {
        self.category == 8
    }
    
    fn is_application_category(&self) -> bool {
        self.category == 9
    }
    
    fn is_other_category(&self) -> bool {
        self.category >= 100
    }
    
    fn type_name(&self) -> &'static str {
        "Generic Type"
    }
    
    fn category_name(&self) -> &'static str {
        "Generic Category"
    }
    
    fn description(&self) -> String {
        format!("Category {} Type {}", self.category, self.type_id)
    }
}