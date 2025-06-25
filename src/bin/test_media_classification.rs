use seed_tools::media::video::classify_video_content;
use seed_tools::media::audio::classify_audio_content;
use seed_tools::media::ebook::classify_ebook_content;
use seed_tools::types::{AudioType, VideoCategory, AudioCategory};
use clap::{Parser, ValueEnum};
use std::fs;
use std::path::Path;
use rand::Rng;
use std::io::Write;

#[derive(Parser, Debug)]
#[command(author, version, about = "Test media classification", long_about = None)]
struct Args {
    /// Media type to test (video, audio, ebook)
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
    #[arg(short = 'n', long, default_value = "50")]
    count: usize,
}

#[derive(Debug, Clone, ValueEnum)]
enum MediaType {
    Video,
    Audio,
    Ebook,
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
    let args = Args::parse();
    
    // Initialize logger
    let mut logger = Logger::new();
    match logger.init("test-media-classification.log") {
        Ok(_) => {
            log_println!(logger, "=== Test Media Classification Log ===");
            log_println!(logger, "Media type: {:?}", args.media_type);
            log_println!(logger, "");
        }
        Err(e) => {
            eprintln!("Warning: Could not create log file: {}", e);
        }
    }
    
    // Collect all filenames to test
    let mut filenames = Vec::new();
    
    // From file
    if let Some(ref file_path) = args.file {
        match fs::read_to_string(file_path) {
            Ok(content) => {
                filenames.extend(content.lines().map(String::from));
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
            Ok(files) => filenames.extend(files),
            Err(e) => {
                eprintln!("Error scanning directory {}: {}", dir_path, e);
                std::process::exit(1);
            }
        }
    }
    
    // From direct input
    let had_direct_input = !args.input.is_empty();
    filenames.extend(args.input);
    
    // Generate random files if requested (even if we have some input)
    if filenames.is_empty() || (args.count > 0 && args.file.is_none() && args.directory.is_none() && !had_direct_input) {
        filenames.extend(generate_random_filenames(&args.media_type, args.count));
    } else if args.count > 0 && had_direct_input {
        // If we have direct input AND a count specified, add random files too
        filenames.extend(generate_random_filenames(&args.media_type, args.count));
    }
    
    // Run classification tests
    match args.media_type {
        MediaType::Video => test_video_classification(&filenames, &mut logger),
        MediaType::Audio => test_audio_classification(&filenames, &mut logger),
        MediaType::Ebook => test_ebook_classification(&filenames, &mut logger),
    }
    
    // Close log file
    log_println!(logger, "\n=== Test completed ===");
}

fn scan_directory(dir_path: &str, media_type: &MediaType) -> Result<Vec<String>, String> {
    let mut files = Vec::new();
    let extensions = match media_type {
        MediaType::Video => vec!["mkv", "mp4", "avi", "mov", "wmv", "flv", "webm", "m4v", "ts", "mpg", "mpeg", "iso"],
        MediaType::Audio => vec!["mp3", "flac", "wav", "aac", "ogg", "m4a", "wma", "aiff", "ape", "opus"],
        MediaType::Ebook => vec!["epub", "pdf", "mobi", "azw", "azw3", "cbr", "cbz", "lit", "pdb"],
    };
    
    for entry in fs::read_dir(dir_path).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        
        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if extensions.contains(&ext.to_lowercase().as_str()) {
                    if let Some(filename) = path.file_name().and_then(|f| f.to_str()) {
                        files.push(filename.to_string());
                    }
                }
            }
        }
    }
    
    Ok(files)
}

fn generate_random_filenames(media_type: &MediaType, count: usize) -> Vec<String> {
    let mut rng = rand::thread_rng();
    let mut filenames = Vec::new();
    
    match media_type {
        MediaType::Video => {
            let titles = [
                // TV Shows - Drama
                "Breaking.Bad", "The.Wire", "Game.of.Thrones", "Stranger.Things", "The.Crown",
                "Better.Call.Saul", "The.Sopranos", "Mad.Men", "True.Detective", "Westworld",
                "The.Handmaids.Tale", "Succession", "House.of.Cards", "Ozark", "Yellowstone",
                
                // TV Shows - Comedy
                "The.Office", "Parks.and.Recreation", "Brooklyn.Nine-Nine", "Community", "Arrested.Development",
                "Its.Always.Sunny.in.Philadelphia", "Seinfeld", "Friends", "The.Big.Bang.Theory", "Modern.Family",
                
                // TV Shows - Reality/Documentary
                "Planet.Earth", "Blue.Planet", "Cosmos", "Making.a.Murderer", "Tiger.King",
                "The.Last.Dance", "Formula.1.Drive.to.Survive", "Chefs.Table", "Our.Planet", "Free.Solo",
                
                // TV Shows - Anime
                "Attack.on.Titan", "Death.Note", "Naruto", "One.Piece", "Demon.Slayer",
                "My.Hero.Academia", "Jujutsu.Kaisen", "Tokyo.Ghoul", "Steins.Gate", "Fullmetal.Alchemist",
                
                // Movies - Action/Sci-Fi
                "Inception", "The.Matrix", "Interstellar", "The.Dark.Knight", "Pulp.Fiction",
                "Avengers.Endgame", "Dune", "Blade.Runner.2049", "Tenet", "John.Wick",
                
                // Movies - Drama
                "The.Shawshank.Redemption", "The.Godfather", "Schindlers.List", "Forrest.Gump", "The.Green.Mile",
                "12.Years.a.Slave", "Parasite", "The.Departed", "No.Country.for.Old.Men", "There.Will.Be.Blood",
                
                // Sports
                "NFL.2024", "NBA.Finals", "UEFA.Champions.League", "F1.Grand.Prix", "WWE.Royal.Rumble",
                "UFC.295", "World.Cup.2022", "Olympics.2024", "Super.Bowl.LVIII", "Wimbledon.2024",
                
                // Adult Content
                "JAV.Uncensored", "XXX.Parody", "OnlyFans.Leak", "Amateur.Homemade", "MILF.Adventures"
            ];
            let years = ["1999", "2005", "2010", "2015", "2019", "2020", "2021", "2022", "2023", "2024"];
            let resolutions = ["480p", "720p", "1080p", "2160p", "4K", "576p", "1080i"];
            let sources = ["BluRay", "WEB-DL", "HDTV", "DVDRip", "WEBRip", "BRRip", "HDRip", "AMZN", "NF", "HMAX"];
            let codecs = ["x264", "x265", "H264", "HEVC", "XviD", "DivX", "VP9", "AV1"];
            let groups = ["SPARKS", "DEMAND", "NTb", "RARBG", "YTS", "FGT", "EZTV", "LOL", "FLEET", "SYNCOPY", "MeGusta", "ION10", "REiGN", "AFG"];
            
            for _ in 0..count {
                let title = titles[rng.gen_range(0..titles.len())];
                
                let filename = if rng.gen_bool(0.6) {
                    // TV Show
                    let season = rng.gen_range(1..10);
                    let episode = rng.gen_range(1..25);
                    format!("{}.S{:02}E{:02}.{}.{}.{}-{}",
                        title,
                        season,
                        episode,
                        resolutions[rng.gen_range(0..resolutions.len())],
                        sources[rng.gen_range(0..sources.len())],
                        codecs[rng.gen_range(0..codecs.len())],
                        groups[rng.gen_range(0..groups.len())]
                    )
                } else {
                    // Movie
                    format!("{}.{}.{}.{}.{}-{}",
                        title,
                        years[rng.gen_range(0..years.len())],
                        resolutions[rng.gen_range(0..resolutions.len())],
                        sources[rng.gen_range(0..sources.len())],
                        codecs[rng.gen_range(0..codecs.len())],
                        groups[rng.gen_range(0..groups.len())]
                    )
                };
                
                let ext = ["mkv", "mp4", "avi"][rng.gen_range(0..3)];
                
                // 50% chance of being in a folder
                if rng.gen_bool(0.5) {
                    let folder_name = if rng.gen_bool(0.7) {
                        // Same as filename base
                        filename.clone()
                    } else {
                        // Simpler folder name
                        if filename.contains("S0") {
                            // TV show season folder
                            format!("{}.Season.{}", title, rng.gen_range(1..10))
                        } else {
                            // Movie folder
                            format!("{}.{}", title, years[rng.gen_range(0..years.len())])
                        }
                    };
                    filenames.push(format!("{}/{}.{}", folder_name, filename, ext));
                } else {
                    filenames.push(format!("{}.{}", filename, ext));
                }
            }
            
            // Add some edge cases
            filenames.push("test.mp4".to_string());
            filenames.push("random.mkv".to_string());
            filenames.push("Mare.Nostrum.Collection.1080p.BluRay.HEVC.iso".to_string());
            filenames.push("Movies/The.Godfather.1972.1080p.BluRay.x264-SPARKS/The.Godfather.1972.1080p.BluRay.x264-SPARKS.mkv".to_string());
            filenames.push("TV Shows/The Wire/Season 1/The.Wire.S01E01.The.Target.1080p.BluRay.x264-DEMAND.mkv".to_string());
            filenames.push("Anime/[HorribleSubs] One Piece - 1000 [1080p].mkv".to_string());
            filenames.push("Documentary.Planet.Earth.II.S01E01.Islands.2160p.UHD.BluRay.x265-DEPTH/planet.earth.ii.s01e01.2160p.bluray.x265-depth.mkv".to_string());
            filenames.push("Sports/NFL.2024.Week.01.Cowboys.vs.Giants.1080p.WEB.h264-SPORTSNET.mkv".to_string());
            filenames.push("Complete.Series/Friends.Complete.Series.1080p.BluRay.x264/Friends.S01E01.mkv".to_string());
            filenames.push("The.Office.US.S09E23-E24.Finale.1080p.WEB-DL.DD5.1.H.264-BS.mkv".to_string());
        }
        
        MediaType::Audio => {
            let artists = [
                // Rock/Classic Rock
                "Pink Floyd", "Led Zeppelin", "The Beatles", "Queen", "Nirvana",
                "Radiohead", "Metallica", "AC DC", "The Rolling Stones", "Bob Dylan",
                "Guns N Roses", "Black Sabbath", "Deep Purple", "Aerosmith", "Kiss",
                
                // Electronic/EDM
                "Daft Punk", "Deadmau5", "Skrillex", "Calvin Harris", "David Guetta",
                "Swedish House Mafia", "Avicii", "Tiesto", "Armin van Buuren", "Above & Beyond",
                
                // Hip Hop/Rap
                "Eminem", "Kanye West", "Drake", "Kendrick Lamar", "J. Cole",
                "Jay-Z", "Nas", "2Pac", "The Notorious B.I.G.", "Travis Scott",
                
                // Pop
                "Taylor Swift", "Ed Sheeran", "Ariana Grande", "Billie Eilish", "The Weeknd",
                "Bruno Mars", "Lady Gaga", "Justin Bieber", "Dua Lipa", "Post Malone",
                
                // Jazz/Classical
                "Miles Davis", "John Coltrane", "Ludwig van Beethoven", "Wolfgang Amadeus Mozart", "Johann Sebastian Bach",
                
                // Country
                "Johnny Cash", "Willie Nelson", "Dolly Parton", "Luke Combs", "Morgan Wallen",
                
                // Soundtracks/Compilations
                "Hans Zimmer", "John Williams", "Various Artists", "VA", "OST"
            ];
            let albums = [
                "Greatest Hits", "Live at Wembley", "Unplugged", "The Collection",
                "Studio Sessions", "B-Sides", "Rarities", "The Essential", "Anthology",
                "Deluxe Edition", "Remastered", "Anniversary Edition", "Complete Recordings",
                "Live in Concert", "BBC Sessions", "Bootleg Series", "Lost Tapes",
                "EP", "Single", "Remix Album", "Best Of", "Acoustic Sessions"
            ];
            let years = ["1965", "1970", "1975", "1980", "1985", "1990", "1995", "2000", "2005", "2010", "2015", "2020", "2021", "2022", "2023", "2024"];
            let formats = ["FLAC", "MP3 320", "MP3 V0", "AAC", "24-96", "DSD", "24-192", "16-44", "MP3 192", "M4A", "OPUS"];
            let sources = ["CD", "Vinyl Rip", "WEB", "SACD", "Cassette", "Spotify", "Tidal", "Qobuz", "iTunes", "Amazon", "Bandcamp", "SoundCloud"];
            
            for _ in 0..count {
                let artist = artists[rng.gen_range(0..artists.len())];
                let album = albums[rng.gen_range(0..albums.len())];
                let year = years[rng.gen_range(0..years.len())];
                let format = formats[rng.gen_range(0..formats.len())];
                let _source = sources[rng.gen_range(0..sources.len())];
                
                let folder = format!("{} - {} ({}) [{}]", artist, album, year, format);
                let track_num = rng.gen_range(1..20);
                let track_name = format!("{:02} - Track {}.flac", track_num, track_num);
                
                filenames.push(format!("{}/{}", folder, track_name));
            }
            
            // Add special cases
            filenames.push("VA - Now That's What I Call Music! 100 (2024) [MP3]/01 - Various Artists - Hit Song.mp3".to_string());
            filenames.push("Joe Rogan Experience #1999 - Guest Name [MP3]/jre-1999.mp3".to_string());
            filenames.push("test.mp3".to_string());
            filenames.push("Podcasts/The Tim Ferriss Show/Episode 500 - Special Guest.mp3".to_string());
            filenames.push("[2024] Taylor Swift - The Tortured Poets Department (24bit-96kHz FLAC)/01. Fortnight (feat. Post Malone).flac".to_string());
            filenames.push("Classical/Berlin Philharmonic - Beethoven Complete Symphonies (DSD)/Symphony No. 9 in D minor, Op. 125.dsf".to_string());
            filenames.push("Soundtracks/Hans Zimmer - Interstellar OST (2014) [FLAC]/01 - Dreaming of the Crash.flac".to_string());
            filenames.push("DJ Sets/Carl Cox - Live at Space Ibiza (2016-09-20) [320kbps]/carl_cox_space_ibiza_final.mp3".to_string());
            filenames.push("Audiobooks/Stephen King - The Stand (Unabridged) [MP3]/Part 01 - Captain Trips.mp3".to_string());
            filenames.push("Various Artists - Grammy Nominees 2024 [MP3 V0]/CD1/01 - Artist Name - Song Title.mp3".to_string());
        }
        
        MediaType::Ebook => {
            let authors = [
                // Fiction Authors
                "Stephen King", "J.K. Rowling", "George R.R. Martin", "Agatha Christie",
                "Isaac Asimov", "Neil Gaiman", "Margaret Atwood", "Dan Brown", "John Grisham",
                "Brandon Sanderson", "Patrick Rothfuss", "Terry Pratchett", "Douglas Adams",
                
                // Non-Fiction Authors
                "Malcolm Gladwell", "Yuval Noah Harari", "Bill Bryson", "Mary Roach",
                "Michael Lewis", "Walter Isaacson", "Michelle Obama", "Trevor Noah",
                
                // Technical Authors
                "Robert C. Martin", "Martin Fowler", "Donald Knuth", "Brian Kernighan",
                
                // Comic Artists/Writers
                "Stan Lee", "Alan Moore", "Frank Miller", "Neil Gaiman", "Brian K. Vaughan",
                "Robert Kirkman", "Grant Morrison", "Warren Ellis", "Garth Ennis"
            ];
            let titles = [
                // Fiction Titles
                "The Mystery", "Adventures in Space", "The Last Kingdom", "Dark Secrets",
                "The Final Chapter", "Beyond the Stars", "The Hidden Truth", "Lost Worlds",
                "Chronicles of Time", "The Forgotten Realm", "Shadow Walker", "The Prophecy",
                
                // Non-Fiction Titles
                "The Complete Guide", "A History of Everything", "The Science Behind",
                "Understanding the World", "The Art of Living", "Mastering Skills",
                
                // Technical Titles
                "Programming in Python", "Clean Code", "Design Patterns", "Algorithms",
                "Machine Learning Basics", "The Pragmatic Programmer",
                
                // Comic Titles
                "Spider-Man", "Batman", "The Walking Dead", "Saga", "Watchmen",
                "The Sandman", "X-Men", "Superman", "Wonder Woman", "Iron Man",
                "Captain America", "The Avengers", "Justice League", "Hellboy"
            ];
            let years = ["1985", "1990", "1995", "2000", "2005", "2010", "2015", "2020", "2021", "2022", "2023", "2024"];
            let publishers = [
                "Penguin", "Random House", "HarperCollins", "Simon & Schuster",
                "Tor Books", "Del Rey", "Orbit", "Ace Books", "Baen Books",
                "O'Reilly Media", "Manning Publications", "Packt Publishing",
                "Marvel Comics", "DC Comics", "Image Comics", "Dark Horse Comics",
                "IDW Publishing", "Boom Studios", "Vertigo", "Valiant Comics"
            ];
            
            for _ in 0..count {
                let author = authors[rng.gen_range(0..authors.len())];
                let title = titles[rng.gen_range(0..titles.len())];
                
                // Determine if it's a comic book
                let is_comic = title.contains("Spider-Man") || title.contains("Batman") || 
                              title.contains("X-Men") || title.contains("Walking Dead") ||
                              title.contains("Superman") || title.contains("Wonder Woman") ||
                              rng.gen_bool(0.2); // 20% chance for any book to be a comic
                
                let filename = if is_comic {
                    // Comic book formatting
                    if rng.gen_bool(0.6) {
                        // Issue number format
                        let issue = rng.gen_range(1..500);
                        let year = years[rng.gen_range(0..years.len())];
                        format!("{} #{:03} ({})", title, issue, year)
                    } else {
                        // Volume/TPB format
                        let volume = rng.gen_range(1..20);
                        format!("{} Vol {} - {} ({})", title, volume, author, years[rng.gen_range(0..years.len())])
                    }
                } else {
                    // Regular book formatting
                    if rng.gen_bool(0.7) {
                        // With year
                        let year = years[rng.gen_range(0..years.len())];
                        format!("{} - {} ({})", author, title, year)
                    } else if rng.gen_bool(0.5) {
                        // With publisher
                        let publisher = publishers[rng.gen_range(0..publishers.len())];
                        format!("{} - {} [{}]", author, title, publisher)
                    } else {
                        // Simple
                        format!("{} - {}", author, title)
                    }
                };
                
                let ext = if is_comic {
                    ["cbz", "cbr", "pdf"][rng.gen_range(0..3)]
                } else {
                    ["epub", "pdf", "mobi", "azw3", "lit"][rng.gen_range(0..5)]
                };
                
                // 40% chance of being in a folder
                if rng.gen_bool(0.4) {
                    let folder_name = if is_comic {
                        // Comic folder structures
                        if rng.gen_bool(0.5) {
                            format!("Comics/{}/{}", title, years[rng.gen_range(0..years.len())])
                        } else {
                            format!("{} Collection", title)
                        }
                    } else {
                        // Book folder structures
                        if rng.gen_bool(0.6) {
                            format!("{} Collection", author)
                        } else {
                            format!("Books/{}", author.replace(" ", "_"))
                        }
                    };
                    filenames.push(format!("{}/{}.{}", folder_name, filename, ext));
                } else {
                    filenames.push(format!("{}.{}", filename, ext));
                }
            }
            
            // Add special types
            filenames.push("Marvel Comics - Spider-Man #001 (2024).cbz".to_string());
            filenames.push("National Geographic - December 2023.pdf".to_string());
            filenames.push("O'Reilly - Learning Python, 5th Edition.pdf".to_string());
            filenames.push("test.epub".to_string());
            
            // More CBZ/CBR examples
            filenames.push("DC Comics/Batman/Batman #700 (2010).cbr".to_string());
            filenames.push("The Walking Dead Compendium Vol 1.cbz".to_string());
            filenames.push("Manga/One Piece/One Piece - Chapter 1000.cbz".to_string());
            filenames.push("[GetComics] X-Men Gold 001-036 (2017-2018) Complete.cbr".to_string());
            filenames.push("Comics/Marvel/Avengers (2018)/Avengers 001 (2018) (Digital).cbz".to_string());
            filenames.push("Image Comics - Saga 001-054 (2012-2018)/Saga 001 (2012).cbr".to_string());
            
            // Technical books with folders
            filenames.push("Programming/Robert C Martin - Clean Code (2008) [PDF]/Clean Code.pdf".to_string());
            filenames.push("IT Books/The Pragmatic Programmer - From Journeyman to Master.epub".to_string());
            
            // Fiction with series info
            filenames.push("Fantasy/Brandon Sanderson/Mistborn 01 - The Final Empire (2006).epub".to_string());
            filenames.push("George R.R. Martin - A Song of Ice and Fire/Book 1 - A Game of Thrones.mobi".to_string());
        }
    }
    
    filenames
}

fn test_video_classification(filenames: &[String], logger: &mut Logger) {
    log_println!(logger, "=== Video Classification Test Results ===\n");
    
    let mut stats = ClassificationStats::new();
    
    for (i, filename) in filenames.iter().enumerate() {
        let metadata = classify_video_content(filename);
        
        log_println!(logger, "{}. File: {}", i + 1, filename);
        log_println!(logger, "   Title: '{}'", metadata.title);
        log_println!(logger, "   Category: {:?}", metadata.category);
        log_println!(logger, "   Source: {:?}", metadata.source_type);
        
        if metadata.season.is_some() || metadata.episode.is_some() {
            log_println!(logger, "   Season: {:?}, Episode: {:?}", metadata.season, metadata.episode);
        }
        
        if let Some(year) = metadata.year {
            log_println!(logger, "   Year: {}", year);
        }
        
        if let Some(resolution) = metadata.resolution {
            log_println!(logger, "   Resolution: {}", resolution);
        }
        
        if let Some(codec) = metadata.codec {
            log_println!(logger, "   Codec: {}", codec);
        }
        
        if metadata.is_boxset || metadata.is_dated_tv {
            log_println!(logger, "   Special: Boxset={}, Dated TV={}", metadata.is_boxset, metadata.is_dated_tv);
        }
        
        log_println!(logger, "");
        
        // Update stats
        if metadata.category == VideoCategory::Unknown {
            stats.unknown += 1;
        } else {
            stats.classified += 1;
        }
        stats.total += 1;
    }
    
    print_stats(&stats, logger);
}

fn test_audio_classification(filenames: &[String], logger: &mut Logger) {
    log_println!(logger, "=== Audio Classification Test Results ===\n");
    
    let mut stats = ClassificationStats::new();
    
    for (i, filename) in filenames.iter().enumerate() {
        let path = Path::new(filename);
        let extension = path.extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");
        
        let audio_type = AudioType::from_extension(extension)
            .unwrap_or(AudioType::Mp3);
        
        let metadata = classify_audio_content(&path, &audio_type);
        
        log_println!(logger, "{}. File: {}", i + 1, filename);
        if let Some(artist) = &metadata.artist {
            log_println!(logger, "   Artist: {}", artist);
        }
        if let Some(album) = &metadata.album {
            log_println!(logger, "   Album: {}", album);
        }
        log_println!(logger, "   Category: {:?}", metadata.category);
        log_println!(logger, "   Source: {:?}", metadata.source_type);
        log_println!(logger, "   Format: {:?} (Lossless: {})", metadata.format, metadata.is_lossless);
        
        if let Some(year) = metadata.year {
            log_println!(logger, "   Year: {}", year);
        }
        
        if metadata.is_various_artists {
            log_println!(logger, "   Various Artists: true");
        }
        
        if metadata.is_24bit {
            log_println!(logger, "   24-bit: true");
        }
        
        if let Some(sample_rate) = &metadata.sample_rate {
            log_println!(logger, "   Sample Rate: {}", sample_rate);
        }
        
        log_println!(logger, "");
        
        // Update stats
        if metadata.category == AudioCategory::Unknown {
            stats.unknown += 1;
        } else {
            stats.classified += 1;
        }
        stats.total += 1;
    }
    
    print_stats(&stats, logger);
}

fn test_ebook_classification(filenames: &[String], logger: &mut Logger) {
    log_println!(logger, "=== Ebook Classification Test Results ===\n");
    
    let mut stats = ClassificationStats::new();
    
    for (i, filename) in filenames.iter().enumerate() {
        let extension = filename.split('.').last().unwrap_or("");
        let metadata = classify_ebook_content(filename, extension);
        
        log_println!(logger, "{}. File: {}", i + 1, filename);
        log_println!(logger, "   Title: '{}'", metadata.title);
        if let Some(author) = &metadata.author {
            log_println!(logger, "   Author: {}", author);
        }
        log_println!(logger, "   Category: {:?}", metadata.category);
        if let Some(format_type) = &metadata.format_type {
            log_println!(logger, "   Format: {:?}", format_type);
        }
        
        if let Some(year) = metadata.year {
            log_println!(logger, "   Year: {}", year);
        }
        
        if let Some(edition) = &metadata.edition {
            log_println!(logger, "   Edition: {}", edition);
        }
        
        if let Some(isbn) = &metadata.isbn {
            log_println!(logger, "   ISBN: {}", isbn);
        }
        
        log_println!(logger, "");
        
        // Update stats
        if metadata.category == seed_tools::types::EbookCategory::Unknown {
            stats.unknown += 1;
        } else {
            stats.classified += 1;
        }
        stats.total += 1;
    }
    
    print_stats(&stats, logger);
}

struct ClassificationStats {
    total: usize,
    classified: usize,
    unknown: usize,
}

impl ClassificationStats {
    fn new() -> Self {
        Self {
            total: 0,
            classified: 0,
            unknown: 0,
        }
    }
}

fn print_stats(stats: &ClassificationStats, logger: &mut Logger) {
    log_println!(logger, "\n=== Classification Statistics ===");
    log_println!(logger, "Total files tested: {}", stats.total);
    log_println!(logger, "Successfully classified: {} ({:.1}%)", 
        stats.classified, 
        (stats.classified as f64 / stats.total as f64) * 100.0
    );
    log_println!(logger, "Unknown/Rejected: {} ({:.1}%)", 
        stats.unknown,
        (stats.unknown as f64 / stats.total as f64) * 100.0
    );
}