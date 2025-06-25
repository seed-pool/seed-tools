use std::path::Path;
use crate::types::{Config, MediaType, ContentCategory, ContentType};
use crate::definitions::TorrentInfo;
use regex::Regex;

/// Process upload with optional 4-digit category code from -c argument
pub fn process_upload(
    input_path: &str,
    category_code: Option<&str>,
    config: &Config,
    dry_run: bool,
) -> Result<(), String> {
    let _path = Path::new(input_path);
    
    match category_code {
        Some("0000") | None => {
            // Auto-detect based on content and filename
            log::info!("Auto-detecting content type for: {}", input_path);
            auto_detect_and_process(input_path, config, dry_run)
        },
        Some(code) => {
            // Use provided 4-digit category/type code
            log::info!("Processing upload with provided category/type code: {}", code);
            
            // Note: This will be handled by the caller in main.rs using tracker-specific parsing
            // For now, we'll route to auto-detection as a fallback
            log::warn!("4-digit code {} should be parsed by caller before calling process_upload", code);
            auto_detect_and_process(input_path, config, dry_run)
        }
    }
}

/// Process upload when explicit torrent info is provided (called from main.rs)
pub fn process_upload_with_info<T: TorrentInfo>(
    input_path: &str,
    torrent_info: &T,
    config: &Config,
    dry_run: bool,
) -> Result<(), String> {
    // Use provided torrent classification
    log::info!("Processing upload with provided classification: category {} ({}) and type {} ({})",
               torrent_info.category_code(),
               torrent_info.category_name(),
               torrent_info.type_code(),
               torrent_info.type_name());
    
    route_by_torrent_info(input_path, torrent_info, config, dry_run)
}

/// Auto-detect content type and process accordingly
fn auto_detect_and_process(
    input_path: &str,
    config: &Config,
    dry_run: bool,
) -> Result<(), String> {
    let path = Path::new(input_path);
    
    // Step 1: Check if input is folder or single file
    if path.is_dir() {
        log::info!("Input is a folder, analyzing folder contents for auto-detection");
        let detection_result = auto_detect_from_folder_contents(input_path)?;
        log::info!("Folder analysis result: {}", detection_result.description);
        route_by_detection_result(input_path, &detection_result, config, dry_run)
    } else if path.is_file() {
        log::info!("Input is a single file, analyzing file extension and name");
        let detection_result = auto_detect_from_single_file(input_path)?;
        log::info!("File analysis result: {}", detection_result.description);
        route_by_detection_result(input_path, &detection_result, config, dry_run)
    } else {
        Err(format!("Input path does not exist or is not accessible: {}", input_path))
    }
}

/// Auto-detect content type from folder contents
fn auto_detect_from_folder_contents(folder_path: &str) -> Result<DetectionResult, String> {
    log::info!("Analyzing folder contents: {}", folder_path);
    
    // First try media type detection on the folder contents
    if let Ok(media_type) = crate::media::detector::detect_primary_media_type(folder_path) {
        let folder_name = Path::new(folder_path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("")
            .to_lowercase();
        
        let result = media_type_to_detection_result(&media_type, &folder_name);
        log::info!("Detected from folder contents: {}", result.description);
        return Ok(result);
    }
    
    // Fallback to folder name pattern analysis
    let folder_name = Path::new(folder_path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("")
        .to_lowercase();
    
    log::info!("Analyzing folder name for patterns: {}", folder_name);
    auto_detect_from_title_patterns(&folder_name)
}

/// Auto-detect content type from single file
fn auto_detect_from_single_file(file_path: &str) -> Result<DetectionResult, String> {
    let path = Path::new(file_path);
    let filename = path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("")
        .to_lowercase();
    
    log::info!("Analyzing single file: {}", filename);
    
    // Step 1: Check file extension first
    if let Some(extension) = path.extension().and_then(|ext| ext.to_str()) {
        let ext_lower = extension.to_lowercase();
        log::info!("File extension detected: {}", ext_lower);
        
        // Check if it's an archive file
        if is_archive_extension(&ext_lower) {
            log::info!("Archive file detected: {}", filename);
            
            // First try filename pattern analysis
            if let Some(result) = detect_from_filename_patterns(&filename) {
                log::info!("Archive type detected from filename patterns: {}", result.description);
                return Ok(result);
            }
            
            // If filename analysis fails, inspect archive contents
            log::info!("Filename analysis inconclusive, inspecting archive contents");
            return auto_detect_from_archive_contents(file_path);
        }
        
        // Try to detect media type from extension
        if let Ok(media_type) = detect_media_type_from_extension(&ext_lower) {
            let result = media_type_to_detection_result(&media_type, &filename);
            log::info!("Detected from file extension: {}", result.description);
            return Ok(result);
        }
    }
    
    // Step 2: If extension doesn't help, analyze filename patterns
    log::info!("Extension analysis inconclusive, analyzing filename patterns");
    auto_detect_from_title_patterns(&filename)
}

/// Check if file extension indicates an archive
fn is_archive_extension(extension: &str) -> bool {
    matches!(extension, 
        "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | 
        "iso" | "img" | "dmg" | "cab" | "arj" | "ace" | "lha" | "lzh"
    )
}

/// Auto-detect content type by inspecting archive contents without extraction
fn auto_detect_from_archive_contents(archive_path: &str) -> Result<DetectionResult, String> {
    log::info!("Inspecting archive contents: {}", archive_path);
    
    let path = Path::new(archive_path);
    let extension = path.extension()
        .and_then(|ext| ext.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();
    
    // Get list of files inside the archive
    let file_list = match extension.as_str() {
        "zip" => list_zip_contents(archive_path)?,
        "rar" => list_rar_contents(archive_path)?,
        "7z" => list_7z_contents(archive_path)?,
        "tar" | "gz" | "bz2" | "xz" => list_tar_contents(archive_path)?,
        "iso" | "img" => {
            // For ISO files, try name-based detection only - no content inspection
            return detect_iso_from_filename(archive_path);
        },
        _ => {
            log::warn!("Unsupported archive format: {}", extension);
            return Err(format!("Unsupported archive format: {}", extension));
        }
    };
    
    log::info!("Found {} files in archive", file_list.len());
    
    // Analyze the file list to determine content type
    analyze_archive_file_list(&file_list, archive_path)
}

/// Analyze a list of files from an archive to determine content type
fn analyze_archive_file_list(file_list: &[ArchiveFileInfo], archive_path: &str) -> Result<DetectionResult, String> {
    log::info!("Analyzing {} files from archive", file_list.len());
    
    // Count different file types
    let mut video_count = 0;
    let mut audio_count = 0;
    let mut ebook_count = 0;
    let mut executable_count = 0;
    let mut image_count = 0;
    let mut data_count = 0;
    let mut large_files = 0;
    let mut total_size = 0u64;
    
    // Track specific game indicators
    let mut has_game_executables = false;
    let mut has_game_data_files = false;
    let mut has_installer_files = false;
    
    for file in file_list {
        total_size += file.size;
        
        if file.size > 100_000_000 { // Files larger than 100MB
            large_files += 1;
        }
        
        let filename_lower = file.name.to_lowercase();
        let extension = Path::new(&file.name)
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_default();
        
        // Count by file type
        match extension.as_str() {
            // Video files
            "mkv" | "mp4" | "avi" | "mov" | "wmv" | "flv" | "webm" | "m4v" | "ts" | "mpg" | "mpeg" => {
                video_count += 1;
            },
            // Audio files
            "mp3" | "flac" | "wav" | "aac" | "ogg" | "m4a" | "wma" | "aiff" | "ape" | "opus" => {
                audio_count += 1;
            },
            // E-book files
            "epub" | "pdf" | "cbz" | "cbr" | "mobi" | "azw" | "azw3" | "lit" | "pdb" => {
                ebook_count += 1;
            },
            // Executable files
            "exe" | "msi" | "deb" | "rpm" | "dmg" | "pkg" | "appimage" => {
                executable_count += 1;
                
                // Check for game-specific executables
                if filename_lower.contains("setup") || filename_lower.contains("install") || 
                   filename_lower.contains("launcher") || filename_lower.contains("game") {
                    has_game_executables = true;
                }
                
                if filename_lower.contains("setup") || filename_lower.contains("install") {
                    has_installer_files = true;
                }
            },
            // Image files
            "jpg" | "jpeg" | "png" | "gif" | "bmp" | "tiff" | "svg" => {
                image_count += 1;
            },
            // Game data files
            "pak" | "dat" | "bin" | "unity3d" | "assets" | "bundle" | "cache" | "save" => {
                data_count += 1;
                has_game_data_files = true;
            },
            _ => {
                // Check for game-specific file patterns
                if filename_lower.contains("unreal") || filename_lower.contains("unity") ||
                   filename_lower.contains("engine") || filename_lower.contains("steam_api") ||
                   filename_lower.contains("directx") || filename_lower.contains("redist") {
                    has_game_data_files = true;
                }
            }
        }
    }
    
    log::info!("Archive analysis: {} video, {} audio, {} ebook, {} exe, {} image, {} data files",
               video_count, audio_count, ebook_count, executable_count, image_count, data_count);
    log::info!("Large files: {}, Total size: {} MB", large_files, total_size / 1_000_000);
    
    // Determine content type based on analysis
    let archive_name = Path::new(archive_path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("")
        .to_lowercase();
    
    // Game detection (high priority to avoid extracting large game files)
    if has_game_executables || has_game_data_files || has_installer_files {
        let confidence = if has_game_executables && has_game_data_files { 0.9 } else { 0.7 };
        return Ok(DetectionResult {
            category_type: ContentCategory::Game,
            media_type: ContentType::PCGame,
            description: format!("PC Game detected from archive contents (confidence: {:.1}%)", confidence * 100.0),
            confidence,
        });
    }
    
    // Video content detection
    if video_count > 0 && video_count > audio_count && video_count > ebook_count {
        let content_type = if archive_name.contains("tv") || archive_name.contains("season") || 
                             archive_name.contains("episode") || archive_name.contains("s0") {
            ContentType::TvShow
        } else if archive_name.contains("anime") {
            ContentType::Anime
        } else {
            ContentType::Movie
        };
        
        return Ok(DetectionResult {
            category_type: ContentCategory::Video,
            media_type: content_type,
            description: format!("Video content detected from archive contents ({} video files)", video_count),
            confidence: 0.8,
        });
    }
    
    // Audio content detection
    if audio_count > 0 && audio_count > video_count && audio_count > ebook_count {
        let content_type = if archive_name.contains("flac") || 
                             file_list.iter().any(|f| f.name.to_lowercase().contains("flac")) {
            ContentType::MusicFlac
        } else {
            ContentType::MusicMp3
        };
        
        return Ok(DetectionResult {
            category_type: ContentCategory::Audio,
            media_type: content_type,
            description: format!("Audio content detected from archive contents ({} audio files)", audio_count),
            confidence: 0.8,
        });
    }
    
    // E-book content detection
    if ebook_count > 0 {
        let content_type = if archive_name.contains("comic") || 
                             file_list.iter().any(|f| f.name.to_lowercase().contains("comic") || 
                                                  f.name.ends_with(".cbz") || f.name.ends_with(".cbr")) {
            ContentType::Comic
        } else {
            ContentType::Ebook
        };
        
        return Ok(DetectionResult {
            category_type: ContentCategory::Ebook,
            media_type: content_type,
            description: format!("E-book content detected from archive contents ({} ebook files)", ebook_count),
            confidence: 0.8,
        });
    }
    
    // Application detection
    if executable_count > 0 && !has_game_executables {
        return Ok(DetectionResult {
            category_type: ContentCategory::Application,
            media_type: ContentType::WindowsApp, // Default to Windows for now
            description: format!("Application detected from archive contents ({} executables)", executable_count),
            confidence: 0.7,
        });
    }
    
    // If mostly images, could be a comic or hobby content
    if image_count > 10 {
        return Ok(DetectionResult {
            category_type: ContentCategory::Ebook,
            media_type: ContentType::Comic,
            description: format!("Comic/image collection detected from archive contents ({} images)", image_count),
            confidence: 0.6,
        });
    }
    
    // Fallback to filename analysis
    log::info!("Archive content analysis inconclusive, falling back to filename patterns");
    auto_detect_from_title_patterns(&archive_name)
}

/// Information about a file within an archive
#[derive(Debug, Clone)]
struct ArchiveFileInfo {
    name: String,
    size: u64,
    is_directory: bool,
}

/// List contents of a ZIP archive
fn list_zip_contents(zip_path: &str) -> Result<Vec<ArchiveFileInfo>, String> {
    use std::fs::File;
    use zip::ZipArchive;
    
    let file = File::open(zip_path)
        .map_err(|e| format!("Failed to open ZIP file {}: {}", zip_path, e))?;
    
    let mut archive = ZipArchive::new(file)
        .map_err(|e| format!("Failed to read ZIP archive {}: {}", zip_path, e))?;
    
    let mut files = Vec::new();
    
    for i in 0..archive.len() {
        let file = archive.by_index(i)
            .map_err(|e| format!("Failed to read file {} from ZIP: {}", i, e))?;
        
        files.push(ArchiveFileInfo {
            name: file.name().to_string(),
            size: file.size(),
            is_directory: file.name().ends_with('/'),
        });
    }
    
    log::info!("Listed {} files from ZIP archive", files.len());
    Ok(files)
}

/// List contents of a RAR archive (placeholder - requires external tool)
fn list_rar_contents(rar_path: &str) -> Result<Vec<ArchiveFileInfo>, String> {
    // For now, return error as RAR support requires external tools
    log::warn!("RAR archive inspection not yet implemented: {}", rar_path);
    Err("RAR archive inspection not yet implemented".to_string())
}

/// List contents of a 7z archive (placeholder - requires external tool) 
fn list_7z_contents(sevenZ_path: &str) -> Result<Vec<ArchiveFileInfo>, String> {
    // For now, return error as 7z support requires external tools
    log::warn!("7z archive inspection not yet implemented: {}", sevenZ_path);
    Err("7z archive inspection not yet implemented".to_string())
}

/// List contents of a TAR archive (placeholder)
fn list_tar_contents(tar_path: &str) -> Result<Vec<ArchiveFileInfo>, String> {
    // For now, return error as TAR support requires implementation
    log::warn!("TAR archive inspection not yet implemented: {}", tar_path);
    Err("TAR archive inspection not yet implemented".to_string())
}

/// Detect ISO content type from filename patterns
fn detect_iso_from_filename(iso_path: &str) -> Result<DetectionResult, String> {
    let filename = Path::new(iso_path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("")
        .to_lowercase();
    
    log::info!("Analyzing ISO filename: {}", filename);
    
    // Video disc patterns (movies, TV shows, etc.)
    let video_patterns = [
        // Movie patterns
        r"\b(19|20)\d{2}\b.*\b(bluray|brrip|dvd|disc)\b",
        r"\b(bluray|brrip|dvdrip|dvd|disc)\b.*\b(19|20)\d{2}\b",
        r"\b(full\.?disc|complete\.?disc|untouched)\b",
        
        // TV show patterns  
        r"\b[sS]\d{1,2}.*\b(disc|dvd|bluray)\b",
        r"\b(season|complete|series).*\b(disc|dvd|bluray)\b",
        
        // General video disc indicators
        r"\b(movie|film|cinema)\b.*\b(disc|iso)\b",
        r"\b(concert|live|performance)\b.*\b(disc|iso)\b",
    ];
    
    for pattern in &video_patterns {
        if regex::Regex::new(pattern).unwrap().is_match(&filename) {
            let content_type = if filename.contains("tv") || filename.contains("season") || 
                                 filename.contains("episode") || regex::Regex::new(r"\b[sS]\d{1,2}\b").unwrap().is_match(&filename) {
                ContentType::TvShow
            } else if filename.contains("concert") || filename.contains("live") || filename.contains("performance") {
                ContentType::Movie // Concert videos are still movies
            } else {
                ContentType::Movie
            };
            
            return Ok(DetectionResult {
                category_type: ContentCategory::Video,
                media_type: content_type,
                description: format!("Video disc image detected from filename patterns"),
                confidence: 0.8,
            });
        }
    }
    
    // Anime patterns
    if filename.contains("anime") || 
       regex::Regex::new(r"\b\d{1,3}\s*-\s*\d{1,3}\b").unwrap().is_match(&filename) ||
       (filename.contains("episode") && (filename.contains("sub") || filename.contains("dub"))) {
        return Ok(DetectionResult {
            category_type: ContentCategory::Video,
            media_type: ContentType::Anime,
            description: "Anime disc image detected from filename patterns".to_string(),
            confidence: 0.8,
        });
    }
    
    // Sports patterns
    if filename.contains("sports") || filename.contains("football") || filename.contains("soccer") ||
       filename.contains("basketball") || filename.contains("baseball") || filename.contains("hockey") ||
       filename.contains("championship") || filename.contains("tournament") {
        return Ok(DetectionResult {
            category_type: ContentCategory::Sports,
            media_type: ContentType::Sports,
            description: "Sports disc image detected from filename patterns".to_string(),
            confidence: 0.7,
        });
    }
    
    // If no video patterns match, default to game ISO
    log::info!("No video patterns matched for ISO, defaulting to game classification");
    Ok(DetectionResult {
        category_type: ContentCategory::Game,
        media_type: ContentType::PCGame,
        description: format!("Game disc image (ISO not matching video patterns)"),
        confidence: 0.6,
    })
}

/// List contents of an ISO archive (placeholder)
fn list_iso_contents(iso_path: &str) -> Result<Vec<ArchiveFileInfo>, String> {
    // For now, return error as ISO support requires implementation
    log::warn!("ISO archive content inspection not yet implemented: {}", iso_path);
    Err("ISO archive content inspection not yet implemented".to_string())
}

/// Detect media type from file extension
fn detect_media_type_from_extension(extension: &str) -> Result<MediaType, String> {
    use crate::types::MediaType;
    
    if let Some(media_type) = MediaType::from_extension(extension) {
        Ok(media_type)
    } else {
        Err(format!("Unknown file extension: {}", extension))
    }
}

/// Auto-detect content type from title/filename patterns (final fallback)
fn auto_detect_from_title_patterns(title: &str) -> Result<DetectionResult, String> {
    log::info!("Analyzing title patterns: {}", title);
    
    // First try specific filename pattern matching
    if let Some(result) = detect_from_filename_patterns(title) {
        log::info!("Detected from filename patterns: {}", result.description);
        return Ok(result);
    }
    
    // Then try general filename hints
    if let Some(result) = detect_from_filename_hints(title) {
        log::info!("Detected from filename hints: {}", result.description);
        return Ok(result);
    }
    
    // Ultimate fallback
    log::warn!("Could not auto-detect content type from title '{}', using hobby category as fallback", title);
    Ok(DetectionResult {
        category_type: ContentCategory::Hobby,
        media_type: ContentType::Mixed,
        description: format!("Unknown content type from title '{}' - using hobby processing", title),
        confidence: 0.1,
    })
}


/// Detect content type from common filename patterns
fn detect_from_filename_patterns(filename: &str) -> Option<DetectionResult> {
    // Video patterns
    if let Some(result) = detect_video_patterns(filename) {
        return Some(result);
    }
    
    // Audio patterns  
    if let Some(result) = detect_audio_patterns(filename) {
        return Some(result);
    }
    
    // E-book patterns
    if let Some(result) = detect_ebook_patterns(filename) {
        return Some(result);
    }
    
    // Game patterns
    if let Some(result) = detect_game_patterns(filename) {
        return Some(result);
    }
    
    // Application patterns
    if let Some(result) = detect_application_patterns(filename) {
        return Some(result);
    }
    
    None
}

/// Detect video content patterns
fn detect_video_patterns(filename: &str) -> Option<DetectionResult> {
    // Movie patterns
    let movie_patterns = [
        r"\b(19|20)\d{2}\b.*\b(1080p|720p|4k|2160p|bluray|brrip|webrip|webdl|dvdrip)\b",
        r"\b(bluray|brrip|webrip|webdl|dvdrip|hdtv|pdtv)\b",
        r"\b(remux|encode|x264|x265|h264|h265)\b",
    ];
    
    for pattern in &movie_patterns {
        if Regex::new(pattern).unwrap().is_match(filename) {
            let content_type = if filename.contains("4k") || filename.contains("2160p") {
                ContentType::Movie4K
            } else if filename.contains("remux") {
                ContentType::MovieRemux
            } else if filename.contains("webrip") || filename.contains("webdl") {
                ContentType::MovieWeb
            } else {
                ContentType::Movie
            };
            
            return Some(DetectionResult {
                category_type: ContentCategory::Video,
                media_type: content_type,
                description: format!("Movie detected from filename patterns"),
                confidence: 0.8,
            });
        }
    }
    
    // TV Show patterns
    let tv_patterns = [
        r"\b[sS]\d{1,2}[eE]\d{1,2}\b",
        r"\b\d{1,2}x\d{1,2}\b",
        r"\b(season|episode)\b",
        r"\b(complete|series)\b.*\b(season|series)\b",
    ];
    
    for pattern in &tv_patterns {
        if Regex::new(pattern).unwrap().is_match(filename) {
            return Some(DetectionResult {
                category_type: ContentCategory::Video,
                media_type: ContentType::TvShow,
                description: "TV Show detected from filename patterns".to_string(),
                confidence: 0.9,
            });
        }
    }
    
    // Anime patterns
    if filename.contains("anime") || 
       Regex::new(r"\b\d{1,3}\s*-\s*\d{1,3}\b").unwrap().is_match(filename) ||
       filename.contains("episode") && (filename.contains("sub") || filename.contains("dub")) {
        return Some(DetectionResult {
            category_type: ContentCategory::Video,
            media_type: ContentType::Anime,
            description: "Anime detected from filename patterns".to_string(),
            confidence: 0.8,
        });
    }
    
    None
}

/// Detect audio content patterns  
fn detect_audio_patterns(filename: &str) -> Option<DetectionResult> {
    // Music album patterns
    if filename.contains("album") || filename.contains("discography") ||
       Regex::new(r"\b(19|20)\d{2}\b.*\b(flac|mp3|320|lossless)\b").unwrap().is_match(filename) {
        
        let content_type = if filename.contains("flac") || filename.contains("lossless") {
            ContentType::MusicFlac
        } else {
            ContentType::MusicMp3
        };
        
        return Some(DetectionResult {
            category_type: ContentCategory::Audio,
            media_type: content_type,
            description: "Music album detected from filename patterns".to_string(),
            confidence: 0.8,
        });
    }
    
    // Audiobook patterns
    if filename.contains("audiobook") || filename.contains("audio book") ||
       (filename.contains("book") && (filename.contains("mp3") || filename.contains("m4b"))) {
        return Some(DetectionResult {
            category_type: ContentCategory::Audiobook,
            media_type: ContentType::Audiobook,
            description: "Audiobook detected from filename patterns".to_string(),
            confidence: 0.9,
        });
    }
    
    None
}

/// Detect e-book content patterns
fn detect_ebook_patterns(filename: &str) -> Option<DetectionResult> {
    // Comic book patterns
    if filename.contains("comic") || filename.contains("cbr") || filename.contains("cbz") ||
       Regex::new(r"\b(vol|volume|issue|#)\s*\d+\b").unwrap().is_match(filename) {
        return Some(DetectionResult {
            category_type: ContentCategory::Ebook,
            media_type: ContentType::Comic,
            description: "Comic book detected from filename patterns".to_string(),
            confidence: 0.9,
        });
    }
    
    // Magazine patterns
    if filename.contains("magazine") || 
       Regex::new(r"\b(january|february|march|april|may|june|july|august|september|october|november|december)\s*(19|20)\d{2}\b").unwrap().is_match(filename) {
        return Some(DetectionResult {
            category_type: ContentCategory::Ebook,
            media_type: ContentType::Magazine,
            description: "Magazine detected from filename patterns".to_string(),
            confidence: 0.8,
        });
    }
    
    // Standard e-book patterns
    if filename.contains("ebook") || filename.contains("epub") || filename.contains("mobi") ||
       (filename.contains("book") && !filename.contains("audio")) {
        return Some(DetectionResult {
            category_type: ContentCategory::Ebook,
            media_type: ContentType::Ebook,
            description: "E-book detected from filename patterns".to_string(),
            confidence: 0.7,
        });
    }
    
    None
}

/// Detect game content patterns
fn detect_game_patterns(filename: &str) -> Option<DetectionResult> {
    // PC game patterns
    if filename.contains("game") || filename.contains("crack") || filename.contains("repack") ||
       Regex::new(r"\b(gog|steam|origin|uplay|epic)\b").unwrap().is_match(filename) {
        return Some(DetectionResult {
            category_type: ContentCategory::Game,
            media_type: ContentType::PCGame,
            description: "PC Game detected from filename patterns".to_string(),
            confidence: 0.8,
        });
    }
    
    // Console game patterns
    if filename.contains("nsw") || filename.contains("switch") {
        return Some(DetectionResult {
            category_type: ContentCategory::Game,
            media_type: ContentType::NSWGame,
            description: "Nintendo Switch game detected from filename patterns".to_string(),
            confidence: 0.9,
        });
    }
    
    if filename.contains("ps4") || filename.contains("playstation") {
        return Some(DetectionResult {
            category_type: ContentCategory::Game,
            media_type: ContentType::PS4Game,
            description: "PS4 game detected from filename patterns".to_string(),
            confidence: 0.9,
        });
    }
    
    None
}

/// Detect application patterns
fn detect_application_patterns(filename: &str) -> Option<DetectionResult> {
    if filename.contains("windows") || filename.contains("win64") || filename.contains("win32") ||
       filename.contains(".exe") || filename.contains("installer") {
        return Some(DetectionResult {
            category_type: ContentCategory::Application,
            media_type: ContentType::WindowsApp,
            description: "Windows application detected from filename patterns".to_string(),
            confidence: 0.8,
        });
    }
    
    if filename.contains("linux") || filename.contains("ubuntu") || filename.contains("debian") ||
       filename.contains(".deb") || filename.contains(".rpm") {
        return Some(DetectionResult {
            category_type: ContentCategory::Application,
            media_type: ContentType::LinuxApp,
            description: "Linux application detected from filename patterns".to_string(),
            confidence: 0.8,
        });
    }
    
    if filename.contains("macos") || filename.contains("mac") || filename.contains(".dmg") {
        return Some(DetectionResult {
            category_type: ContentCategory::Application,
            media_type: ContentType::MacApp,
            description: "macOS application detected from filename patterns".to_string(),
            confidence: 0.8,
        });
    }
    
    None
}

/// Detect content type from general filename hints
fn detect_from_filename_hints(filename: &str) -> Option<DetectionResult> {
    // Education/Tutorial content
    if filename.contains("tutorial") || filename.contains("course") || filename.contains("lesson") ||
       filename.contains("training") || filename.contains("education") {
        return Some(DetectionResult {
            category_type: ContentCategory::Education,
            media_type: ContentType::Educational,
            description: "Educational content detected from filename hints".to_string(),
            confidence: 0.6,
        });
    }
    
    // Sports content
    if filename.contains("sports") || filename.contains("football") || filename.contains("soccer") ||
       filename.contains("basketball") || filename.contains("baseball") || filename.contains("hockey") {
        return Some(DetectionResult {
            category_type: ContentCategory::Sports,
            media_type: ContentType::Sports,
            description: "Sports content detected from filename hints".to_string(),
            confidence: 0.7,
        });
    }
    
    None
}

/// Public function to auto-detect content type from a file or folder path
pub fn auto_detect_content_type(input_path: &str) -> Result<DetectionResult, String> {
    let path = Path::new(input_path);
    
    if path.is_dir() {
        auto_detect_from_folder_contents(input_path)
    } else if path.is_file() {
        auto_detect_from_single_file(input_path)
    } else {
        Err(format!("Path does not exist or is not accessible: {}", input_path))
    }
}

/// Convert MediaType to DetectionResult
fn media_type_to_detection_result(media_type: &MediaType, filename: &str) -> DetectionResult {
    match media_type {
        MediaType::Video(_) => {
            // Try to determine specific video type from filename
            if filename.contains("anime") {
                DetectionResult {
                    category_type: ContentCategory::Video,
                    media_type: ContentType::Anime,
                    description: "Anime video detected".to_string(),
                    confidence: 0.7,
                }
            } else if Regex::new(r"\b[sS]\d{1,2}[eE]\d{1,2}\b").unwrap().is_match(filename) {
                DetectionResult {
                    category_type: ContentCategory::Video,
                    media_type: ContentType::TvShow,
                    description: "TV Show detected".to_string(),
                    confidence: 0.8,
                }
            } else {
                DetectionResult {
                    category_type: ContentCategory::Video,
                    media_type: ContentType::Movie,
                    description: "Movie detected".to_string(),
                    confidence: 0.6,
                }
            }
        },
        MediaType::Audio(_) => {
            if filename.contains("book") {
                DetectionResult {
                    category_type: ContentCategory::Audiobook,
                    media_type: ContentType::Audiobook,
                    description: "Audiobook detected".to_string(),
                    confidence: 0.8,
                }
            } else {
                DetectionResult {
                    category_type: ContentCategory::Audio,
                    media_type: ContentType::MusicMp3,
                    description: "Music detected".to_string(),
                    confidence: 0.6,
                }
            }
        },
        MediaType::Ebook(_) => {
            if filename.contains("comic") {
                DetectionResult {
                    category_type: ContentCategory::Ebook,
                    media_type: ContentType::Comic,
                    description: "Comic book detected".to_string(),
                    confidence: 0.8,
                }
            } else {
                DetectionResult {
                    category_type: ContentCategory::Ebook,
                    media_type: ContentType::Ebook,
                    description: "E-book detected".to_string(),
                    confidence: 0.7,
                }
            }
        },
        MediaType::Game(_) => DetectionResult {
            category_type: ContentCategory::Game,
            media_type: ContentType::PCGame,
            description: "Game detected".to_string(),
            confidence: 0.6,
        },
        MediaType::Hobby(_) => DetectionResult {
            category_type: ContentCategory::Hobby,
            media_type: ContentType::Hobby,
            description: "Hobby content detected".to_string(),
            confidence: 0.5,
        },
    }
}

/// Route processing based on torrent info
fn route_by_torrent_info<T: TorrentInfo>(
    input_path: &str,
    torrent_info: &T,
    config: &Config,
    dry_run: bool,
) -> Result<(), String> {
    // Route based on category type using trait methods
    if torrent_info.is_ebook_category() {
        process_ebook_category(input_path, torrent_info, config, dry_run)
    } else if torrent_info.is_game_category() {
        process_game_category(input_path, torrent_info, config, dry_run)
    } else if torrent_info.is_audio_category() {
        process_audio_category(input_path, torrent_info, config, dry_run)
    } else if torrent_info.is_video_category() {
        process_video_category(input_path, torrent_info, config, dry_run)
    } else if torrent_info.is_audiobook_category() {
        process_audiobook_category(input_path, torrent_info, config, dry_run)
    } else if torrent_info.is_hobby_category() {
        process_hobby_category(input_path, torrent_info, config, dry_run)
    } else {
        process_generic_category(input_path, torrent_info, config, dry_run)
    }
}

/// Route processing based on detection result  
fn route_by_detection_result(
    input_path: &str,
    detection: &DetectionResult,
    config: &Config,
    dry_run: bool,
) -> Result<(), String> {
    log::info!("Processing {} with confidence {:.1}%", 
               detection.description, detection.confidence * 100.0);
    
    match detection.category_type {
        ContentCategory::Video => {
            super::video::process_video(input_path, config, dry_run).map(|_| ())
        },
        ContentCategory::Audio => {
            super::audio::process_audio(input_path, config, dry_run).map(|_| ())
        },
        ContentCategory::Ebook => {
            super::ebook::process_ebook(input_path, config, dry_run).map(|_| ())
        },
        ContentCategory::Game | ContentCategory::Application => {
            super::game::process_game(input_path, config, dry_run).map(|_| ())
        },
        ContentCategory::Audiobook => {
            super::audio::process_audio(input_path, config, dry_run).map(|_| ())
        },
        ContentCategory::Sports => {
            super::video::process_video(input_path, config, dry_run).map(|_| ())
        },
        ContentCategory::Hobby | ContentCategory::Education => {
            super::hobby::process_hobby(input_path, config, dry_run).map(|_| ())
        },
    }
}

/// Content detection result
#[derive(Debug, Clone)]
pub struct DetectionResult {
    pub category_type: ContentCategory,
    pub media_type: ContentType,
    pub description: String,
    pub confidence: f32, // 0.0 to 1.0
}


/// Process e-book uploads with type-specific handling
fn process_ebook_category<T: TorrentInfo>(
    input_path: &str,
    torrent_info: &T,
    config: &Config,
    dry_run: bool,
) -> Result<(), String> {
    log::info!("Processing e-book upload with type: {}", torrent_info.type_name());
    
    // All e-book types use the same processing logic, but we log the specific type
    match torrent_info.type_name() {
        name if name.contains("Comic") => {
            log::info!("Detected comic book type - processing with comic-specific logic");
        },
        name if name.contains("Magazine") => {
            log::info!("Detected magazine type - processing with magazine-specific logic");
        },
        name if name.contains("Newspaper") => {
            log::info!("Detected newspaper type - processing with newspaper-specific logic");
        },
        name if name.contains("Pub") || name.contains("Book") => {
            log::info!("Detected standard e-book type - processing with e-book logic");
        },
        _ => {
            log::warn!("Unexpected type {} for e-book category, using default e-book processing", torrent_info.type_name());
        }
    }
    
    super::ebook::process_ebook(input_path, config, dry_run).map(|_| ())
}

/// Process game uploads with type-specific handling
fn process_game_category<T: TorrentInfo>(
    input_path: &str,
    torrent_info: &T,
    config: &Config,
    dry_run: bool,
) -> Result<(), String> {
    log::info!("Processing game upload with type: {}", torrent_info.type_name());
    
    // Log the specific platform but all use the same processing logic
    match torrent_info.type_name() {
        name if name.contains("PC") => {
            log::info!("Detected PC game - processing with PC-specific logic");
        },
        name if name.contains("NSW") || name.contains("Switch") => {
            log::info!("Detected Nintendo Switch game - processing with NSW-specific logic");
        },
        name if name.contains("Linux") => {
            log::info!("Detected Linux game - processing with Linux-specific logic");
        },
        name if name.contains("PS4") || name.contains("PlayStation") => {
            log::info!("Detected PS4 game - processing with PlayStation-specific logic");
        },
        name if name.contains("Xbox") => {
            log::info!("Detected Xbox game - processing with Xbox-specific logic");
        },
        name if name.contains("Wii") || name.contains("NES") || name.contains("retro") => {
            log::info!("Detected retro console game - processing with retro-specific logic");
        },
        _ => {
            log::warn!("Unexpected type {} for games category, using default game processing", torrent_info.type_name());
        }
    }
    
    super::game::process_game(input_path, config, dry_run).map(|_| ())
}

/// Process audio uploads with type-specific handling
fn process_audio_category<T: TorrentInfo>(
    input_path: &str,
    torrent_info: &T,
    config: &Config,
    dry_run: bool,
) -> Result<(), String> {
    log::info!("Processing audio upload with type: {}", torrent_info.type_name());
    
    match torrent_info.type_name() {
        name if name.contains("FLAC") => {
            log::info!("Detected FLAC audio - processing with lossless-specific logic");
        },
        name if name.contains("MP3") => {
            log::info!("Detected MP3 audio - processing with lossy-specific logic");
        },
        name if name.contains("Pack") => {
            log::info!("Detected music pack - processing with pack-specific logic");
        },
        name if name.contains("Karaoke") => {
            log::info!("Detected karaoke - processing with karaoke-specific logic");
        },
        _ => {
            log::warn!("Unexpected type {} for music category, using default audio processing", torrent_info.type_name());
        }
    }
    
    super::audio::process_audio(input_path, config, dry_run).map(|_| ())
}

/// Process video uploads with type-specific handling
fn process_video_category<T: TorrentInfo>(
    input_path: &str,
    torrent_info: &T,
    config: &Config,
    dry_run: bool,
) -> Result<(), String> {
    log::info!("Processing video upload with type: {}", torrent_info.type_name());
    
    match torrent_info.type_name() {
        name if name.contains("Remux") || name.contains("Disc") => {
            log::info!("Detected high-quality video source - processing with remux/disc logic");
        },
        name if name.contains("WEB") || name.contains("Web") => {
            log::info!("Detected web source - processing with web-specific logic");
        },
        name if name.contains("Encode") => {
            log::info!("Detected encoded video - processing with encode-specific logic");
        },
        name if name.contains("HDTV") || name.contains("TV") => {
            log::info!("Detected HDTV source - processing with TV-specific logic");
        },
        name if name.contains("BluRay") || name.contains("Blu") => {
            log::info!("Detected Blu-ray source - processing with disc-specific logic");
        },
        name if name.contains("Upscale") => {
            log::info!("Detected upscaled video - processing with upscale-specific logic");
        },
        name if name.contains("Dubbed") || name.contains("Dub") => {
            log::info!("Detected dubbed content - processing with dub-specific logic");
        },
        name if name.contains("BoxSet") || name.contains("Box") => {
            log::info!("Detected box set - processing with collection-specific logic");
        },
        _ => {
            log::warn!("Unexpected type {} for video category, using default video processing", torrent_info.type_name());
        }
    }
    
    super::video::process_video(input_path, config, dry_run).map(|_| ())
}

/// Process audiobook uploads
fn process_audiobook_category<T: TorrentInfo>(
    input_path: &str,
    torrent_info: &T,
    config: &Config,
    dry_run: bool,
) -> Result<(), String> {
    log::info!("Processing audiobook upload with type: {}", torrent_info.type_name());
    
    // Audiobooks use audio processing but with audiobook-specific metadata handling
    super::audio::process_audio(input_path, config, dry_run).map(|_| ())
}

/// Process hobby uploads with type-specific handling
fn process_hobby_category<T: TorrentInfo>(
    input_path: &str,
    torrent_info: &T,
    config: &Config,
    dry_run: bool,
) -> Result<(), String> {
    log::info!("Processing hobby upload with type: {}", torrent_info.type_name());
    
    match torrent_info.type_name() {
        name if name.contains("3D") || name.contains("Print") => {
            log::info!("Detected 3D printing files - processing with 3D-specific logic");
        },
        name if name.contains("Education") => {
            log::info!("Detected educational content - processing with education-specific logic");
        },
        _ => {
            log::warn!("Unexpected type {} for hobby category, using default hobby processing", torrent_info.type_name());
        }
    }
    
    super::hobby::process_hobby(input_path, config, dry_run).map(|_| ())
}

/// Process uploads that don't fit standard media categories
fn process_generic_category<T: TorrentInfo>(
    input_path: &str,
    torrent_info: &T,
    config: &Config,
    dry_run: bool,
) -> Result<(), String> {
    log::info!("Processing generic upload for category: {}", torrent_info.category_name());
    
    if torrent_info.is_sports_category() {
        log::info!("Detected sports content - using video processing");
        super::video::process_video(input_path, config, dry_run).map(|_| ())
    } else if torrent_info.is_application_category() {
        log::info!("Detected application - using hobby processing for apps");
        super::hobby::process_hobby(input_path, config, dry_run).map(|_| ())
    } else if torrent_info.is_other_category() {
        log::info!("Detected other content - attempting auto-detection");
        
        // Try to auto-detect the media type for "Other" category
        if let Ok(media_type) = crate::media::detector::detect_primary_media_type(input_path) {
            match media_type {
                MediaType::Video(_) => super::video::process_video(input_path, config, dry_run).map(|_| ()),
                MediaType::Audio(_) => super::audio::process_audio(input_path, config, dry_run).map(|_| ()),
                MediaType::Ebook(_) => super::ebook::process_ebook(input_path, config, dry_run).map(|_| ()),
                MediaType::Game(_) => super::game::process_game(input_path, config, dry_run).map(|_| ()),
                MediaType::Hobby(_) => super::hobby::process_hobby(input_path, config, dry_run).map(|_| ()),
            }
        } else {
            log::warn!("Could not auto-detect media type for 'Other' category, using hobby processing as fallback");
            super::hobby::process_hobby(input_path, config, dry_run).map(|_| ())
        }
    } else {
        log::warn!("Unhandled category: {}, using hobby processing as fallback", torrent_info.category_name());
        super::hobby::process_hobby(input_path, config, dry_run).map(|_| ())
    }
}