use log::{error, info, warn};
use regex::Regex;
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::core::{MediaFile, MediaType, VideoCategory, VideoFile, VideoSourceType, VideoType};
use crate::core::{PathsConfig, VideoSettings};
use crate::processing::description::{DescriptionBuilder, DescriptionConfig};
use crate::processing::extraction::process_and_extract_archives;
use crate::processing::naming::generate_release_name;

/// Metadata extracted from video filename
#[derive(Debug, Clone)]
pub struct VideoMetadata {
    pub title: String,        // Cleaned title for metadata lookups
    pub release_name: String, // Original filename with dots preserved
    pub year: Option<u32>,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    pub category: VideoCategory,
    pub source_type: VideoSourceType,
    pub is_boxset: bool,
    pub is_dated_tv: bool,
    pub resolution: Option<String>,
    pub codec: Option<String>,
}

impl Default for VideoMetadata {
    fn default() -> Self {
        Self {
            title: String::new(),
            release_name: String::new(),
            year: None,
            season: None,
            episode: None,
            category: VideoCategory::Unknown,
            source_type: VideoSourceType::Unknown,
            is_boxset: false,
            is_dated_tv: false,
            resolution: None,
            codec: None,
        }
    }
}

/// Processed data ready for upload (media-agnostic)
/// This struct accumulates all the data needed for the upload process
#[derive(Debug, Clone)]
pub struct UploadData {
    pub nfo_data: Option<(String, Vec<u8>)>, // (path, content)
    pub mediainfo: Option<String>,
    pub screenshots: Vec<String>,
    pub thumbnails: Vec<String>,
    pub sample_url: Option<String>,
    pub torrent_path: Option<String>,
    pub release_name: Option<String>,
    pub description: Option<String>,
    pub tmdb_id: Option<u32>,
    pub imdb_id: Option<String>,
    pub tvdb_id: Option<u32>,
    pub igdb_id: Option<u64>,
    pub cover_url: Option<String>,
    // TV show specific fields
    pub season: Option<u32>,
    pub episode: Option<u32>,
    pub resolution: Option<String>,
    /// Actual input path (may be different from original if file was renamed)
    pub actual_input_path: Option<String>,
}

impl UploadData {
    pub fn new() -> Self {
        Self {
            nfo_data: None,
            mediainfo: None,
            screenshots: Vec::new(),
            thumbnails: Vec::new(),
            sample_url: None,
            torrent_path: None,
            release_name: None,
            description: None,
            tmdb_id: None,
            imdb_id: None,
            tvdb_id: None,
            igdb_id: None,
            cover_url: None,
            season: None,
            episode: None,
            resolution: None,
            actual_input_path: None,
        }
    }
}

/// Generate video description with template support
pub fn generate_description_with_template(
    metadata: &serde_json::Value,
    enriched_metadata: Option<&std::collections::HashMap<String, String>>,
    template_name: Option<&str>,
) -> Result<String, String> {
    use crate::templates::TemplateProcessor;
    use log::info;

    info!("🎬 Video: generate_description_with_template called");
    info!("  Template name: {:?}", template_name);

    if let Some(enriched) = enriched_metadata {
        info!("  📊 Enriched metadata passed: {} fields", enriched.len());
        for (key, value) in enriched.iter() {
            if key.starts_with("tmdb_") {
                info!("    📌 {} = {}", key, value);
            }
        }
    } else {
        info!("  ⚠️ No enriched metadata passed");
    }

    let template_processor = TemplateProcessor::with_defaults()
        .map_err(|e| format!("Failed to initialize template processor: {}", e))?;

    let template_to_use = template_name.unwrap_or("default");
    info!("  Using template: {}", template_to_use);

    if let Some(template) = template_processor.get_template("video", template_to_use) {
        info!("  ✅ Template found, applying...");
        template_processor.apply_template(template, metadata, enriched_metadata)
    } else {
        info!("  ⚠️ Template not found, using fallback");
        // Fallback to traditional description generation
        let screenshots = metadata
            .get("screenshots")
            .and_then(|s| s.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(String::from)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let sample_url = metadata.get("sample_url").and_then(|s| s.as_str());

        Ok(generate_description_with_metadata(
            metadata,
            &screenshots,
            sample_url,
            enriched_metadata,
        ))
    }
}

pub fn find_video_files<T>(
    input_path: &str,
    _paths: &PathsConfig,
    settings: &T,
) -> Result<(Vec<String>, Option<String>), String>
where
    T: VideoSettings,
{
    let supported_extensions = [
        "mkv", "mp4", "avi", "mov", "wmv", "flv", "webm", "m4v", "ts", "mpg", "mpeg",
    ];
    let path = Path::new(input_path);

    let mut video_files = Vec::new();
    let mut nfo_file: Option<String> = None;

    let exclusions_enabled = settings.stripshit_from_videos();
    info!("Exclusions enabled: {}", exclusions_enabled);

    fn process_path(
        file_path: &Path,
        video_files: &mut Vec<String>,
        nfo_file: &mut Option<String>,
        supported_extensions: &[&str],
        exclusions_enabled: bool,
    ) -> Result<(), String> {
        if file_path.is_dir() {
            for entry in
                fs::read_dir(file_path).map_err(|e| format!("Failed to read directory: {}", e))?
            {
                let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
                let entry_path = entry.path();
                process_path(
                    &entry_path,
                    video_files,
                    nfo_file,
                    supported_extensions,
                    exclusions_enabled,
                )?;
            }
        } else {
            info!("Processing file: {}", file_path.display());
            process_file(
                file_path,
                video_files,
                nfo_file,
                supported_extensions,
                exclusions_enabled,
            )?;
        }
        Ok(())
    }

    process_path(
        path,
        &mut video_files,
        &mut nfo_file,
        &supported_extensions,
        exclusions_enabled,
    )?;

    if video_files.is_empty() {
        error!("No valid video files detected after exclusions.");
        return Err("No valid video files detected.".to_string());
    }

    info!("Final NFO file: {:?}", nfo_file);

    Ok((video_files, nfo_file))
}

pub fn process_file(
    file_path: &Path,
    video_files: &mut Vec<String>,
    nfo_file: &mut Option<String>,
    _supported_extensions: &[&str], // Legacy parameter, now unused
    exclusions_enabled: bool,
) -> Result<(), String> {
    let file_name = file_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    if let Some(ext) = file_path.extension() {
        let ext = ext.to_string_lossy().to_lowercase();
        if VideoType::from_extension(&ext).is_some() {
            video_files.push(file_path.to_string_lossy().to_string());
        } else if ext == "nfo" && nfo_file.is_none() {
            *nfo_file = Some(file_path.to_string_lossy().to_string());
        }
    } else if exclusions_enabled && contains_excluded_keywords(&file_name) {
        info!("Excluding file due to keywords: {}", file_name);
    }

    Ok(())
}

pub fn contains_excluded_keywords(name: &str) -> bool {
    let keywords = ["sample", "screens", "screenshots", "proof"];
    let lowercase_name = name.to_lowercase();
    let result = keywords
        .iter()
        .any(|keyword| lowercase_name.contains(keyword));
    info!(
        "Checking if '{}' contains excluded keywords: {}",
        name, result
    );
    result
}

pub fn generate_sample(
    video_file: &str,
    screenshots_dir: &str,
    remote_path: &str,
    image_path: &str,
    ffmpeg_path: &str,
    input_name: &str,
    dry_run: bool,
) -> Result<String, String> {
    let sanitized_input_name = generate_release_name(input_name);
    let sample_file = format!("{}/{}.sample.mkv", screenshots_dir, sanitized_input_name);

    // Generate the sample file
    let ffmpeg_command = format!(
        "{} -i \"{}\" -ss 00:05:00 -t 00:00:20 -map 0 -c copy \"{}\"",
        ffmpeg_path, video_file, sample_file
    );
    let output = Command::new("sh")
        .arg("-c")
        .arg(ffmpeg_command)
        .output()
        .map_err(|e| format!("Failed to execute ffmpeg: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "Failed to generate sample file. ffmpeg output: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    // Set permissions to 777
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&sample_file, fs::Permissions::from_mode(0o777)).map_err(|e| {
            format!(
                "Failed to set permissions for sample file '{}': {}",
                sample_file, e
            )
        })?;
    }

    // Upload the sample file
    if !dry_run {
        crate::utils::upload_to_cdn(
            &sample_file,
            &format!("{}/previews/", remote_path.trim_end_matches('/')),
        )
        .map_err(|e| format!("{:?}", e))?;
        info!("Sample file uploaded to CDN.");
    } else {
        info!(
            "[DRY RUN] Would upload sample to CDN: {}{}",
            &format!("{}/previews/", remote_path.trim_end_matches('/')),
            sanitized_input_name
        );
    }

    // Return the public-facing URL for the sample
    // If image_path is empty, just return the filename
    if image_path.is_empty() {
        Ok(format!("{}.sample.mkv", sanitized_input_name))
    } else {
        // The CDN serves files from the root, not from subdirectories
        Ok(format!(
            "{}/{}.sample.mkv",
            image_path.trim_end_matches('/'),
            sanitized_input_name
        ))
    }
}

pub fn get_video_duration(video_file: &str, ffprobe_path: &str) -> Result<f64, String> {
    let ffprobe_output = Command::new(ffprobe_path)
        .args(&[
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
            video_file,
        ])
        .output()
        .map_err(|e| format!("Failed to run ffprobe: {}", e))?;

    if !ffprobe_output.status.success() {
        return Err(format!(
            "ffprobe failed with status: {}. Stderr: {}",
            ffprobe_output.status,
            String::from_utf8_lossy(&ffprobe_output.stderr)
        ));
    }

    let duration_str = String::from_utf8_lossy(&ffprobe_output.stdout)
        .trim()
        .to_string();
    duration_str
        .parse::<f64>()
        .map_err(|_| "Failed to parse video duration.".to_string())
}

pub fn default_non_video_description() -> String {
    format!(
        "[b][size=12][color=#757575]Created with mkbrr, ffmpeg, and mediainfo. Posted to this fine tracker with seedbrr.[/color][/size][/b]

        [url=https://github.com/seed-pool/seed-tools][img]https://cdn.seedpool.org/sp.png[/img][/url]  \
        [url=https://github.com/autobrr/mkbrr][img]https://cdn.seedpool.org/mkbrr.png[/img][/url]  \
        [url=https://www.rust-lang.org][img]https://cdn.seedpool.org/rust.png[/img][/url]"
    )
}

pub fn generate_description(
    screenshots: &[String],
    _thumbnails: &[String],
    sample_url: &str,
    _datestamp: &str,
    custom_description: Option<&str>,
    youtube_trailer_url: Option<&str>,
    _base_url: &str,
    _release_name: &str,
) -> String {
    // Description builder imports already at top of file
    use crate::core::ImageLayout;

    // Create config for video screenshots
    let mut config = DescriptionConfig::default();
    config.image_layout = ImageLayout::Grid2x2;

    let mut builder = DescriptionBuilder::with_config(MediaType::Video(VideoType::Mkv), config);

    // Add screenshots
    if !screenshots.is_empty() {
        builder = builder.images(screenshots.to_vec());
    }

    // Add sample
    if !sample_url.is_empty() {
        if let Some(filename) = Path::new(sample_url).file_name().and_then(|f| f.to_str()) {
            builder = builder.sample(sample_url, filename);
        }
    }

    // Add trailer
    if let Some(trailer_url) = youtube_trailer_url {
        builder = builder.trailer(trailer_url, "YouTube");
    }

    // Add custom description
    if let Some(custom_desc) = custom_description {
        builder = builder.raw(custom_desc);
    }

    builder.build()
}

/// Generate enhanced video description with TMDB metadata
pub fn generate_description_with_metadata(
    metadata: &serde_json::Value,
    screenshots: &[String],
    sample_url: Option<&str>,
    tmdb_enrichment: Option<&std::collections::HashMap<String, String>>,
) -> String {
    use crate::core::{DescriptionComponent, ImageLayout, MediaType, SectionFormat, VideoType};
    use crate::processing::description::{DescriptionBuilder, DescriptionConfig};

    // Helper function to get value from either enriched metadata or base metadata
    let get_value = |key: &str| -> Option<&str> {
        tmdb_enrichment
            .and_then(|enriched| enriched.get(key))
            .map(|s| s.as_str())
            .or_else(|| metadata.get(key).and_then(|v| v.as_str()))
    };

    // Create config for video screenshots
    let mut config = DescriptionConfig::default();
    config.image_layout = ImageLayout::Grid2x2;

    let mut builder = DescriptionBuilder::with_config(MediaType::Video(VideoType::Mkv), config);

    // Add title
    if let Some(title) = metadata.get("title").and_then(|t| t.as_str()) {
        builder = builder.title(title);
    }

    // Add TMDB overview/synopsis if available
    if let Some(tmdb_overview) = get_value("tmdb_overview") {
        builder = builder.synopsis(tmdb_overview);
    } else if let Some(description) = get_value("description") {
        builder = builder.synopsis(description);
    }

    // Add screenshots
    if !screenshots.is_empty() {
        builder = builder.images(screenshots.to_vec());
    }

    // Add sample
    if let Some(sample_url) = sample_url {
        if !sample_url.is_empty() {
            if let Some(filename) = Path::new(sample_url).file_name().and_then(|f| f.to_str()) {
                builder = builder.sample(sample_url, filename);
            }
        }
    }

    // Add TMDB trailer if available
    if let Some(trailer_url) = get_value("tmdb_trailer_url") {
        builder = builder.trailer(trailer_url, "YouTube");
    }

    // Add custom description if available
    if let Some(custom_desc) = get_value("custom_description") {
        if !custom_desc.is_empty() {
            builder = builder.raw(custom_desc);
        }
    }

    // Create video information table
    let mut info_rows = Vec::new();

    // Year
    if let Some(year) = get_value("year") {
        info_rows.push(vec!["Year".to_string(), year.to_string()]);
    }

    // Genres (prefer TMDB data)
    if let Some(genres) = get_value("tmdb_genres") {
        info_rows.push(vec!["Genres".to_string(), genres.to_string()]);
    }

    // Runtime
    if let Some(runtime) = get_value("tmdb_runtime") {
        info_rows.push(vec!["Runtime".to_string(), runtime.to_string()]);
    }

    // Rating
    if let Some(rating) = get_value("tmdb_rating") {
        info_rows.push(vec!["TMDB Rating".to_string(), format!("{}/10", rating)]);
    }

    // Directors
    if let Some(directors) = get_value("tmdb_directors") {
        info_rows.push(vec!["Directors".to_string(), directors.to_string()]);
    }

    // Cast
    if let Some(cast) = get_value("tmdb_cast") {
        info_rows.push(vec!["Cast".to_string(), cast.to_string()]);
    }

    // Networks (for TV shows)
    if let Some(networks) = get_value("tmdb_networks") {
        info_rows.push(vec!["Networks".to_string(), networks.to_string()]);
    }

    // Production companies
    if let Some(companies) = get_value("tmdb_production_companies") {
        info_rows.push(vec!["Production".to_string(), companies.to_string()]);
    }

    // Add video information table
    if !info_rows.is_empty() {
        builder = builder.add_component(DescriptionComponent::Table { rows: info_rows });
    }

    // Add budget/revenue for movies
    if let Some(budget) = get_value("tmdb_budget") {
        builder = builder.custom_section("Budget", budget, SectionFormat::Plain);
    }

    if let Some(revenue) = get_value("tmdb_revenue") {
        builder = builder.custom_section("Box Office", revenue, SectionFormat::Plain);
    }

    // Add keywords as a spoiler section
    if let Some(keywords) = get_value("tmdb_keywords") {
        builder = builder.custom_section("Keywords", keywords, SectionFormat::Spoiler);
    }

    builder.build()
}

/// Recursively process a directory for video files
fn process_directory_recursive(
    dir: &Path,
    root_dir: &Path,
    results: &mut Vec<(VideoFile, VideoMetadata)>,
    rejected_files: &mut Vec<String>,
) -> Result<(), String> {
    for entry in
        fs::read_dir(dir).map_err(|e| format!("Failed to read directory {:?}: {}", dir, e))?
    {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
        let entry_path = entry.path();

        if entry_path.is_dir() {
            // Skip proof directories and other unwanted directories
            if let Some(dir_name) = entry_path.file_name().and_then(|n| n.to_str()) {
                let dir_name_lower = dir_name.to_lowercase();
                if dir_name_lower == "proof" || dir_name_lower == "sample" || dir_name_lower == "screens" || dir_name_lower == "screenshots" {
                    info!("Skipping directory: {}", dir_name);
                    continue;
                }
            }
            // Recursively process subdirectories
            process_directory_recursive(&entry_path, root_dir, results, rejected_files)?;
        } else if entry_path.is_file() {
            if let Some(extension) = entry_path.extension().and_then(|ext| ext.to_str()) {
                let ext_lower = extension.to_lowercase();
                
                // Handle special disc formats
                if ext_lower == "iso" || ext_lower == "m2ts" {
                    let filename = entry_path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("");
                    let parent_dir_name = root_dir.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("");
                    
                    // Check if this looks like a video disc
                    if looks_like_video_release(filename) || 
                       looks_like_video_release(parent_dir_name) ||
                       is_full_disc_release(filename) ||
                       is_full_disc_release(parent_dir_name) {
                        
                        info!("Found disc file indicating full disc release: {} ({})", filename, ext_lower);
                        
                        let video_file = VideoFile {
                            path: entry_path.clone(),
                            video_type: VideoType::Directory, // Treat as disc content
                        };

                        // Use the root directory path for classification to get proper release info
                        let metadata = classify_video_content(root_dir.to_str().unwrap_or(filename));

                        if metadata.category != VideoCategory::Unknown {
                            info!(
                                "Processed disc file: {} -> Category: {:?}, Source: {:?}",
                                filename, metadata.category, metadata.source_type
                            );

                            results.push((video_file, metadata));
                            return Ok(()); // Found disc indicator, no need to process more files
                        }
                    }
                } else if let Some(video_type) = VideoType::from_extension(extension) {
                    let filename = entry_path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("");
                    
                    // Skip sample files and other unwanted files
                    let filename_lower = filename.to_lowercase();
                    if filename_lower.contains("sample") || filename_lower.contains("proof") {
                        info!("Skipping sample/proof file: {}", filename);
                        continue;
                    }

                    let video_file = VideoFile {
                        path: entry_path.clone(),
                        video_type,
                    };

                    // Use the root directory path for classification to get proper release info
                    let metadata = classify_video_content(root_dir.to_str().unwrap_or(filename));

                    if metadata.category == VideoCategory::Unknown {
                        rejected_files.push(filename.to_string());
                        warn!("Rejected video file with unknown category: {}", filename);
                        continue;
                    }

                    info!(
                        "Processed video: {} -> Category: {:?}, Source: {:?}",
                        filename, metadata.category, metadata.source_type
                    );

                    results.push((video_file, metadata));
                }
            }
        }
    }

    Ok(())
}

/// Process video file(s) from a path (file or directory) and classify content
pub fn process_video(
    input_path: &str,
    _config: &crate::core::Config,
    _dry_run: bool,
) -> Result<Vec<(VideoFile, VideoMetadata)>, String> {
    let path = Path::new(input_path);

    if !path.exists() {
        // For preflight mode with non-existent paths, return empty results
        info!("Path '{}' does not exist, returning empty video results for preflight mode", input_path);
        return Ok(Vec::new());
    }

    // Extract any archives first and get the path to process
    let processing_path =
        process_and_extract_archives(input_path).map_err(|e| format!("{:?}", e))?;

    // Now process the path (which may contain extracted files)
    let mut results = Vec::new();
    let mut rejected_files = Vec::new();

    // Update path to use the processing path
    let path = Path::new(&processing_path);

    if path.is_file() {
        // Single file case (non-archive video file or movie ISO)
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .ok_or_else(|| "Could not determine file extension".to_string())?;

        let filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");

        // Handle ISO files specially for movie discs
        let video_type = if extension.to_lowercase() == "iso" {
            // Check if this ISO looks like a movie release using parent directory context
            let parent_dir_name = path.parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("");
                
            if looks_like_video_release(filename) || 
               looks_like_video_release(parent_dir_name) ||
               is_full_disc_release(filename) ||
               is_full_disc_release(parent_dir_name) {
                VideoType::Directory // Treat movie ISO as a disc image
            } else {
                return Err(format!("ISO file '{}' doesn't appear to be a movie disc based on naming patterns", filename));
            }
        } else {
            VideoType::from_extension(extension)
                .ok_or_else(|| format!("Unsupported video file type: {}", extension))?
        };

        let video_file = VideoFile {
            path: path.to_path_buf(),
            video_type,
        };

        // Pass the full path for classification
        let metadata = classify_video_content(path.to_str().unwrap_or(filename));

        if metadata.category == VideoCategory::Unknown {
            return Err(format!(
                "Unable to determine video category for '{}'. File must have recognizable TV show (S##E##), movie (year), anime, sports, documentary, or concert patterns in the filename.",
                filename
            ));
        }

        results.push((video_file, metadata));
    } else if path.is_dir() {
        // Handle directory - recursively process all video files
        process_directory_recursive(path, path, &mut results, &mut rejected_files)?;

        if results.is_empty() {
            // Check if this is a full disc/complete release before failing
            if let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) {
                if is_full_disc_release(dir_name) {
                    info!("Detected full disc release: {}", dir_name);
                    
                    // Create a virtual video file entry for the full disc
                    let video_file = VideoFile {
                        path: path.to_path_buf(),
                        video_type: VideoType::Directory,
                    };
                    
                    // Classify using the directory name
                    let metadata = classify_video_content(path.to_str().unwrap_or(dir_name));
                    
                    if metadata.category == VideoCategory::Unknown {
                        return Err(format!(
                            "Unable to determine video category for full disc release '{}'. Directory must have recognizable movie or TV show patterns.",
                            dir_name
                        ));
                    }
                    
                    info!(
                        "Processed full disc: {} -> Category: {:?}, Source: {:?}",
                        dir_name, metadata.category, metadata.source_type
                    );
                    
                    results.push((video_file, metadata));
                } else {
                    // Original error handling for non-full-disc directories
                    if !rejected_files.is_empty() {
                        return Err(format!(
                            "No valid video files found. {} file(s) rejected due to unrecognizable naming patterns: {}",
                            rejected_files.len(),
                            rejected_files.join(", ")
                        ));
                    } else {
                        return Err("No video files found in directory".to_string());
                    }
                }
            } else {
                return Err("No video files found in directory".to_string());
            }
        }

        if !rejected_files.is_empty() {
            warn!(
                "Processed {} valid video files, rejected {} files with unknown categories",
                results.len(),
                rejected_files.len()
            );
        }
    } else {
        return Err("Path is neither a file nor a directory".to_string());
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_looks_like_video_release_with_patterns() {
        let test_cases = vec![
            // Generic movie patterns - year + technical specs
            ("Movie.Title.1960.2160p.UHD.Blu-ray.SDR.HEVC.DTS-HD.MA.2.0-GROUP", true),
            ("Film.Name.1969.2160p.COMPLETE.UHD.BLURAY-TEAM", true),
            ("Title.Here.2021.UHD.BluRay.2160p.HEVC.Atmos.TrueHD7.1-RELEASE", true),
            ("Movie.2025.ITA.COMPLETE.BLURAY-GROUP", true),
            ("Film.1987.2160p.COMPLETE.UHD.BLURAY-TEAM", true),
            ("Title 1973 2160p USA UHD Blu-ray DV HDR HEVC DTS-HD MA 1.0-GROUP", true),
            ("Long.Movie.Title.2003.2160p.USA.UHD.Blu-ray.DV.HDR.HEVC.TrueHD.7.1.Atmos-TEAM", true),
            ("Film.Title.1969.1080p.Blu-ray.AVC.DTS-HD.MA.2.0-GROUP", true),
            ("Movie.Name.1991.2160p.MULTI.COMPLETE.UHD.BLURAY-TEAM", true),
            ("Title.1997.Blu-ray.1080i.AVC.DTS-HD.5.1", true),
            // Test some non-video examples
            ("random_folder", false),
            ("documents_2023", false),
            ("vacation_photos", false),
        ];

        for (test_case, expected) in test_cases {
            let result = looks_like_video_release(test_case);
            assert_eq!(
                result, expected,
                "Test failed for '{}': expected {}, got {}",
                test_case, expected, result
            );
        }
    }

    #[test]
    fn test_is_full_disc_release() {
        let test_cases = vec![
            // Full disc patterns - GENERIC PATTERNS ONLY
            ("Movie.Title.1993.Bluray.1080p.AVC.DTS-HDMA5.1-GROUP", true),
            ("Film.Name.1969.2160p.COMPLETE.UHD.BLURAY-TEAM", true),
            ("Title.2025.ITA.COMPLETE.BLURAY-GROUP", true),
            ("Movie.1991.2160p.MULTI.COMPLETE.UHD.BLURAY-TEAM", true),
            ("Film.2020.UHD.Blu-ray.COMPLETE.DISC", true),
            ("Movie.2021.BDMV.COMPLETE", true),
            
            // Test generic patterns for various formats
            ("Title.2023.UHD.BluRay.2160p.HEVC.DTS-HD.MA.7.1-GROUP", true),
            ("Film.1995.COMPLETE.BLURAY-TEAM", true),
            ("Movie.2020.Blu.ray.1080p.AVC.TrueHD.5.1-GROUP", true),
            ("Title.2024.UHD.Blu-ray.COMPLETE.DISC.DV.HDR-TEAM", true),
            ("Film.2022.BDMV.COMPLETE-GROUP", true),
            ("Movie.2021.Full.Disc.1080p.AVC-TEAM", true),
            
            // Non-full-disc releases (encoded videos)
            ("Movie.2020.1080p.x264-GROUP", false),
            ("Show.S01E01.720p.HDTV.x264-GROUP", false),
            ("Documentary.2021.WEB-DL.1080p", false),
            ("random_folder", false),
        ];

        for (test_case, expected) in test_cases {
            let result = is_full_disc_release(test_case);
            assert_eq!(
                result, expected,
                "Full disc test failed for '{}': expected {}, got {}",
                test_case, expected, result
            );
        }
    }

    #[test]
    fn test_full_disc_classification() {
        // Test generic full disc example
        let metadata = classify_video_content("Movie.Title.1993.Bluray.1080p.AVC.DTS-HDMA5.1-GROUP");
        
        assert_eq!(metadata.category, VideoCategory::Movie, "Should be classified as Movie");
        assert_eq!(metadata.source_type, VideoSourceType::FullDisc, "Should be classified as FullDisc");
        assert_eq!(metadata.year, Some(1993), "Should extract year 1993");
        assert!(metadata.title.contains("Movie"), "Should extract title");
        
        // Test another full disc example
        let metadata2 = classify_video_content("Film.Name.1969.2160p.COMPLETE.UHD.BLURAY-TEAM");
        assert_eq!(metadata2.category, VideoCategory::Movie, "Should be classified as Movie");
        assert_eq!(metadata2.source_type, VideoSourceType::FullDisc, "Should be classified as FullDisc");
        assert_eq!(metadata2.year, Some(1969), "Should extract year 1969");
    }

    #[test]
    fn test_iso_movie_detection() {
        // Test that movie ISOs are detected as video files
        let temp_dir = std::env::temp_dir();
        let test_dir = temp_dir.join("Movie Title 2011 Blu-ray AVC 1080p DTS-HD 7.1");
        std::fs::create_dir_all(&test_dir).unwrap();
        
        let iso_file = test_dir.join("MOVIE_TITLE.iso");
        std::fs::write(&iso_file, b"fake iso content").unwrap();
        
        // Test detection on the ISO file directly
        let result = detect_video_files(iso_file.to_str().unwrap());
        
        // Clean up
        std::fs::remove_dir_all(&test_dir).ok();
        
        assert!(result.is_ok(), "Should detect ISO as video file");
        let video_files = result.unwrap();
        assert!(!video_files.is_empty(), "Should find at least one video file");
        assert_eq!(video_files[0].video_type, VideoType::Directory, "ISO should be treated as Directory type");
        
        // Test classification - use the directory path which has the year and blu-ray pattern
        let metadata = classify_video_content("Movie Title 2011 Blu-ray AVC 1080p DTS-HD 7.1");
        assert_eq!(metadata.category, VideoCategory::Movie, "Directory should be classified as Movie");
        assert_eq!(metadata.source_type, VideoSourceType::FullDisc, "Directory should be classified as FullDisc");
        
        // Test ISO file with parent directory context (the real scenario)
        let iso_with_context = classify_video_content("Movie Title 2011 Blu-ray AVC 1080p DTS-HD 7.1/MOVIE_TITLE.iso");
        assert_eq!(iso_with_context.category, VideoCategory::Movie, "ISO with context should be classified as Movie");
        assert_eq!(iso_with_context.source_type, VideoSourceType::FullDisc, "ISO with context should be classified as FullDisc");
        
        // Test generic ISO without context - should fall back gracefully
        println!("Testing generic ISO classification...");
        let iso_metadata = classify_video_content("MOVIE_TITLE.iso");
        println!("Result: category={:?}, source_type={:?}", iso_metadata.category, iso_metadata.source_type);
    }
}

/// Detect video files in a path (without classification)
pub fn detect_video_files(path: &str) -> Result<Vec<VideoFile>, String> {
    let mut video_files = Vec::new();
    detect_video_files_recursive(Path::new(path), &mut video_files)?;
    
    // If no actual video files found, check if directory name looks like video content
    if video_files.is_empty() {
        let path_obj = Path::new(path);
        if path_obj.is_dir() {
            if let Some(dir_name) = path_obj.file_name().and_then(|n| n.to_str()) {
                if looks_like_video_release(dir_name) {
                    // Create a virtual video file entry for the directory
                    video_files.push(VideoFile {
                        path: path_obj.to_path_buf(),
                        video_type: VideoType::Directory, // We'll need to add this variant
                    });
                }
            }
        }
    }
    
    Ok(video_files)
}

/// Recursively search for video files in a directory tree
fn detect_video_files_recursive(
    path: &Path,
    video_files: &mut Vec<VideoFile>,
) -> Result<(), String> {
    if path.is_file() {
        if let Some(extension) = path.extension().and_then(|ext| ext.to_str()) {
            if let Some(video_type) = VideoType::from_extension(extension) {
                video_files.push(VideoFile {
                    path: path.to_path_buf(),
                    video_type,
                });
            } else if extension.to_lowercase() == "iso" || extension.to_lowercase() == "m2ts" {
                // Check if this disc file (ISO/M2TS) looks like a movie release
                if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                    // Check both the filename and parent directory for movie patterns
                    let parent_dir_name = path.parent()
                        .and_then(|p| p.file_name())
                        .and_then(|n| n.to_str())
                        .unwrap_or("");
                    
                    if looks_like_video_release(filename) || 
                       looks_like_video_release(parent_dir_name) ||
                       is_full_disc_release(filename) ||
                       is_full_disc_release(parent_dir_name) {
                        video_files.push(VideoFile {
                            path: path.to_path_buf(),
                            video_type: VideoType::Directory, // Treat disc files as disc content
                        });
                    }
                }
            }
        }
    } else if path.is_dir() {
        for entry in
            fs::read_dir(path).map_err(|e| format!("Failed to read directory {:?}: {}", path, e))?
        {
            let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
            let entry_path = entry.path();

            // Recursively process subdirectories and files
            detect_video_files_recursive(&entry_path, video_files)?;
        }
    }

    Ok(())
}

/// Check if a directory name indicates a full disc/complete release
pub fn is_full_disc_release(dir_name: &str) -> bool {
    use regex::Regex;
    
    // Patterns that indicate full disc releases
    let full_disc_regex = Regex::new(r"(?i)\b(complete|full\.?disc|complete\.?disc|bdmv|disc\.?image)\b").unwrap();
    let bluray_regex = Regex::new(r"(?i)\b(blu.?ray|bluray|uhdbd|uhd\.?blu.?ray)\b").unwrap();
    let year_regex = Regex::new(r"\b(19|20)\d{2}\b").unwrap();
    
    // Strong indicators of full disc releases
    if full_disc_regex.is_match(dir_name) {
        return true;
    }
    
    // Movie with bluray + year is likely a full disc if it contains "bluray" or "complete"
    if year_regex.is_match(dir_name) && bluray_regex.is_match(dir_name) {
        return true;
    }
    
    false
}

/// Check if a directory name looks like a video release based on common patterns
pub fn looks_like_video_release(dir_name: &str) -> bool {
    use regex::Regex;
    
    // Initialize regex patterns for video content detection
    let season_episode_regex = Regex::new(r"(?i)S(\d{1,2})E(\d{1,3})").unwrap();
    let season_only_regex = Regex::new(r"(?i)S(\d{1,2})").unwrap();
    let episode_only_regex = Regex::new(r"(?i)\bE(\d{1,4})\b").unwrap();
    let year_regex = Regex::new(r"\b(19|20)\d{2}\b").unwrap();
    let resolution_regex = Regex::new(r"(?i)\b(2160p|1080p|720p|480p|4K|UHD|HD)\b").unwrap();
    let source_regex = Regex::new(r"(?i)\b(BluRay|Blu-ray|WebDL|WEB-DL|WebRip|WEB-Rip|HDTV|DVDRip|Remux|UHDBluRay)\b").unwrap();
    let codec_regex = Regex::new(r"(?i)\b(HEVC|x265|x264|AVC|H\.264|H\.265)\b").unwrap();
    let audio_regex = Regex::new(r"(?i)\b(DTS|DTS-HD|TrueHD|DD|AC3|AAC|FLAC|Atmos)\b").unwrap();
    let quality_regex = Regex::new(r"(?i)\b(COMPLETE|PROPER|REPACK|INTERNAL|LIMITED|UNCUT|EXTENDED|DIRECTORS?\.CUT)\b").unwrap();
    
    // Check for TV show patterns (strong indicators)
    if season_episode_regex.is_match(dir_name) 
        || episode_only_regex.is_match(dir_name)
        || season_only_regex.is_match(dir_name) {
        return true;
    }
    
    // Check for movie patterns (year + technical specs)
    let has_year = year_regex.is_match(dir_name);
    let has_resolution = resolution_regex.is_match(dir_name);
    let has_source = source_regex.is_match(dir_name);
    let has_codec = codec_regex.is_match(dir_name);
    let has_audio = audio_regex.is_match(dir_name);
    let has_quality = quality_regex.is_match(dir_name);
    
    // Strong movie indicators: year + at least 2 technical specs
    if has_year && [has_resolution, has_source, has_codec, has_audio, has_quality].iter().filter(|&&x| x).count() >= 2 {
        return true;
    }
    
    // Medium confidence: year + resolution (common for movies)
    if has_year && has_resolution {
        return true;
    }
    
    // Check for common scene group suffixes
    let group_regex = Regex::new(r"(?i)-([A-Z0-9]+)$").unwrap();
    if has_year && group_regex.is_match(dir_name) {
        return true;
    }
    
    false
}

/// Convert VideoFile to MediaFile
pub fn to_media_file(video_file: &VideoFile) -> MediaFile {
    MediaFile {
        path: video_file.path.clone(),
        media_type: MediaType::Video(video_file.video_type.clone()),
    }
}

/// Classify video content based on filename patterns
/// Enhanced version of determine_release_type_and_title from seedpool.rs
pub fn classify_video_content(path: &str) -> VideoMetadata {
    let mut metadata = VideoMetadata::default();

    // Determine what to classify - prioritize directory name for directories
    let path_obj = Path::new(path);
    let filename_str = if path_obj.is_dir() {
        // Use directory name for classification
        path_obj
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    } else {
        // For files, check if it's an ISO and use parent directory if available
        let filename = path_obj
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
            
        // If this is an ISO file, try to use parent directory for better classification
        if filename.to_lowercase().ends_with(".iso") {
            if let Some(parent_dir) = path_obj.parent().and_then(|p| p.file_name()) {
                let parent_name = parent_dir.to_string_lossy().to_string();
                // Use parent directory if it looks like a video release
                if looks_like_video_release(&parent_name) || is_full_disc_release(&parent_name) {
                    parent_name
                } else {
                    filename
                }
            } else {
                filename
            }
        } else {
            filename
        }
    };
    let filename = filename_str.as_str();

    // Store the original release name with dots preserved
    metadata.release_name = generate_release_name(filename);

    // Initialize regex patterns (enhanced from seedpool.rs)
    let season_episode_regex = Regex::new(r"(?i)S(\d{1,2})E(\d{1,3})").unwrap();
    let season_only_regex = Regex::new(r"(?i)S(\d{1,2})").unwrap();
    let episode_only_regex = Regex::new(r"(?i)\bE(\d{1,4})\b").unwrap(); // Support E1-E9999 for anime
    let boxset_regex =
        Regex::new(r"(?i)\b(boxset|complete|collection|season\s*\d+.*complete)\b").unwrap();
    let year_regex = Regex::new(r"\b(19|20)\d{2}\b").unwrap();
    let full_date_regex =
        Regex::new(r"\b((19|20)\d{2})[.\-](0[1-9]|1[0-2])[.\-](0[1-9]|[12][0-9]|3[01])\b").unwrap();

    // Enhanced pattern matching for anime, sports, documentaries
    // Common anime titles and keywords
    let anime_regex = Regex::new(r"(?i)\b(anime|dubbed|subbed|jpn|japanese|[Ss]ub|[Dd]ub|naruto|one\.piece|attack\.on\.titan|bleach|dragon\.ball|demon\.slayer|jujutsu\.kaisen|my\.hero\.academia|boku\.no\.hero|death\.note|hunter\.x\.hunter|fullmetal\.alchemist|sword\.art\.online|tokyo\.ghoul|steins\.gate|evangelion|cowboy\.bebop|one\.punch\.man|mob\.psycho|chainsaw\.man|spy\.x\.family|vinland\.saga|haikyuu|fairy\.tail|black\.clover|boruto|shippuden|kimetsu\.no\.yaiba)\b").unwrap();

    // Sports patterns - more specific to avoid false positives
    let sports_regex = Regex::new(r"(?i)\b(nba|nfl|nhl|mlb|uefa|fifa|premier\.league|bundesliga|la\.liga|serie\.a|ligue\.1|championship|tournament|vs\.|boxing|mma|ufc|wwe|aew|f1|formula\.1|formula\.one|olympics?|world\.cup|super\.bowl|wrestlemania|summerslam|grand\.prix|tennis|wimbledon|golf|pga|cricket|rugby)\b").unwrap();

    let documentary_regex = Regex::new(r"(?i)\b(documentary|docu|national\.geographic|discovery|history|nature|wildlife|science|biography|bio)\b").unwrap();
    let concert_regex =
        Regex::new(r"(?i)\b(concert|live\.at|tour|festival|acoustic|unplugged|live\.from)\b")
            .unwrap();

    // Source type patterns - order matters for proper detection
    let uhd_bluray_regex = Regex::new(r"(?i)\b(uhd\.?blu.?ray|4k\.?blu.?ray)\b").unwrap();
    let bluray_regex = Regex::new(r"(?i)\b(blu.?ray|bd)\b").unwrap(); // Removed m2ts from here
    let dvd_regex = Regex::new(r"(?i)\b(dvd|dvdrip)\b").unwrap();
    let remux_regex = Regex::new(r"(?i)\b(remux|remiux)\b").unwrap();
    let full_disc_regex =
        Regex::new(r"(?i)\b(full\.?disc|complete\.?disc|bdmv|disc\.?image)\b").unwrap();
    let iso_regex = Regex::new(r"(?i)\.iso$").unwrap();
    let web_dl_regex =
        Regex::new(r"(?i)\b(web[\.\-]?dl|webdl|amzn|nf|hmax|dsnp|atvp|hulu|pcok|pmtp)\b").unwrap();
    let web_rip_regex = Regex::new(r"(?i)\b(web[\.\-]?rip|webrip)\b").unwrap();
    let hdtv_regex = Regex::new(r"(?i)\b(hdtv)\b").unwrap();
    let pdtv_regex = Regex::new(r"(?i)\b(pdtv)\b").unwrap();
    let sdtv_regex = Regex::new(r"(?i)\b(sdtv)\b").unwrap();
    let encode_regex = Regex::new(r"(?i)\b(encode|x264|x265|h264|h265|hevc|xvid|divx)\b").unwrap();
    let upscale_regex = Regex::new(r"(?i)\b(upscale|upscaled|ai.?upscale)\b").unwrap();

    // Resolution patterns
    let resolution_regex = Regex::new(r"\b(2160p|1080p|720p|480p|360p|4K|UHD)\b").unwrap();

    // Codec patterns
    let codec_regex = Regex::new(r"\b(x264|x265|h264|h265|hevc|avc|xvid|divx|av1)\b").unwrap();

    info!("Classifying video content for: {}", filename);

    // 1. First check for TV show patterns (S##E## takes priority)
    if let Some(captures) = season_episode_regex.captures(filename) {
        info!("Matched SxxEyy pattern: {:?}", captures);
        metadata.category = VideoCategory::TvShow;
        metadata.season = captures.get(1).and_then(|m| m.as_str().parse::<u32>().ok());
        metadata.episode = captures.get(2).and_then(|m| m.as_str().parse::<u32>().ok());
        metadata.title = extract_title_before_pattern(filename, &season_episode_regex);
    } else if let Some(captures) = episode_only_regex.captures(filename) {
        info!("Matched Eyy pattern: {:?}", captures);
        metadata.category = VideoCategory::TvShow;
        metadata.season = Some(1);
        metadata.episode = captures.get(1).and_then(|m| m.as_str().parse::<u32>().ok());
        metadata.title = extract_title_before_pattern(filename, &episode_only_regex);
    } else if let Some(captures) = season_only_regex.captures(filename) {
        info!("Matched Sxx pattern: {:?}", captures);
        metadata.category = VideoCategory::TvShow;
        metadata.season = captures.get(1).and_then(|m| m.as_str().parse::<u32>().ok());
        metadata.episode = Some(0); // Season pack has no specific episode
        metadata.is_boxset = true; // Season-only pattern indicates a boxset/season pack
        metadata.title = extract_title_before_pattern(filename, &season_only_regex);
    } else if boxset_regex.is_match(filename) && !is_full_disc_release(filename) {
        info!("Matched boxset keywords in filename: {}", filename);
        metadata.category = VideoCategory::TvShow;
        metadata.is_boxset = true;
        metadata.season = Some(1);
        metadata.episode = Some(0);
        metadata.title = extract_title_before_pattern(filename, &boxset_regex);
    } else if let Some(date_caps) = full_date_regex.captures(filename) {
        info!("Matched full date pattern in filename: {}", filename);
        metadata.category = VideoCategory::TvShow;
        metadata.is_dated_tv = true;
        // Use the full year (group 1)
        if let Some(year_str) = date_caps.get(1).map(|m| m.as_str()) {
            if let Ok(year) = year_str.parse::<u32>() {
                metadata.year = Some(year);
                metadata.season = Some(year);
                metadata.episode = Some(0);
            }
        }
        metadata.title = extract_title_before_pattern(filename, &full_date_regex);
    } else if year_regex.is_match(filename) {
        info!("Matched year pattern in filename: {}", filename);
        metadata.category = VideoCategory::Movie;

        // Find all year matches and pick the most likely release year
        let year_matches: Vec<u32> = year_regex
            .find_iter(filename)
            .filter_map(|m| m.as_str().parse::<u32>().ok())
            .collect();

        if !year_matches.is_empty() {
            // Prefer years between 1960 and current year + 1
            let current_year = 2024; // Or use chrono to get actual current year
            let valid_year = year_matches
                .iter()
                .find(|&&y| y >= 1960 && y <= current_year + 1)
                .or_else(|| year_matches.first());

            if let Some(&year) = valid_year {
                metadata.year = Some(year);
            }
        }

        metadata.title = extract_title_before_pattern(filename, &year_regex);
    } else {
        // No clear pattern, extract full title
        metadata.title = clean_title(filename);

        // For ISO files without clear patterns, check for movie-like titles
        if iso_regex.is_match(filename)
            && (filename.to_lowercase().contains("trilogy")
                || filename.to_lowercase().contains("collection")
                || filename.to_lowercase().contains("saga")
                || bluray_regex.is_match(filename)
                || dvd_regex.is_match(filename))
        {
            metadata.category = VideoCategory::Movie;
        }
    }

    // 2. Refine category based on content-specific patterns
    // Check anime first as it often has episode patterns
    if anime_regex.is_match(filename) {
        info!("Detected anime patterns in filename");
        metadata.category = VideoCategory::Anime;
    } else if documentary_regex.is_match(filename) {
        info!("Detected documentary patterns in filename");
        metadata.category = VideoCategory::Documentary;
    } else if concert_regex.is_match(filename) {
        info!("Detected concert patterns in filename");
        metadata.category = VideoCategory::Concert;
    } else if sports_regex.is_match(filename) {
        info!("Detected sports patterns in filename");
        metadata.category = VideoCategory::Sports;
    }

    // 3. Determine source type (priority order matters)
    // Check for actual source types first, then fallback to SeasonPack for boxsets
    if iso_regex.is_match(filename) || full_disc_regex.is_match(filename) || is_full_disc_release(filename) {
        metadata.source_type = VideoSourceType::FullDisc;
    } else if uhd_bluray_regex.is_match(filename) {
        metadata.source_type = VideoSourceType::UHDBluRay;
    } else if remux_regex.is_match(filename) {
        metadata.source_type = VideoSourceType::Remux;
    } else if bluray_regex.is_match(filename) {
        metadata.source_type = VideoSourceType::BluRay;
    } else if dvd_regex.is_match(filename) {
        metadata.source_type = VideoSourceType::DVD;
    } else if web_dl_regex.is_match(filename) {
        metadata.source_type = VideoSourceType::WebDL;
    } else if web_rip_regex.is_match(filename) {
        metadata.source_type = VideoSourceType::WebRip;
    } else if hdtv_regex.is_match(filename) {
        metadata.source_type = VideoSourceType::HDTV;
    } else if pdtv_regex.is_match(filename) {
        metadata.source_type = VideoSourceType::PDTV;
    } else if sdtv_regex.is_match(filename) {
        metadata.source_type = VideoSourceType::SDTV;
    } else if upscale_regex.is_match(filename) {
        metadata.source_type = VideoSourceType::Upscale;
    } else if encode_regex.is_match(filename) {
        // Only set to Encode if no other source type was detected
        metadata.source_type = VideoSourceType::Encode;
    } else if metadata.is_boxset {
        // Fallback to SeasonPack for boxsets with no detected source type
        metadata.source_type = VideoSourceType::SeasonPack;
    } else {
        // Default to Unknown if nothing matches
        metadata.source_type = VideoSourceType::Unknown;
    }

    // Validate movie source types using the new validation methods
    // This automatically converts disallowed types to Encode for movies
    metadata.source_type = metadata.category.validate_source_type(&metadata.source_type);

    // 4. Extract resolution
    if let Some(res_match) = resolution_regex.find(filename) {
        metadata.resolution = Some(res_match.as_str().to_uppercase());
    }

    // 5. Extract codec
    if let Some(codec_match) = codec_regex.find(filename) {
        metadata.codec = Some(codec_match.as_str().to_uppercase());
    }

    // Additional check for directories - see if it's a season pack
    if path_obj.is_dir() && metadata.category == VideoCategory::TvShow {
        // Check if directory contains multiple episodes from same season
        if let Ok(entries) = fs::read_dir(path_obj) {
            let mut seasons = std::collections::HashSet::new();
            let mut episodes = std::collections::HashSet::new();
            let mut video_count = 0;

            for entry in entries.flatten().take(20) {
                // Check first 20 files
                if let Some(ext) = entry.path().extension().and_then(|e| e.to_str()) {
                    if VideoType::from_extension(ext).is_some() {
                        video_count += 1;
                        let file_path = entry.path();
                        if let Some(file_path_str) = file_path.to_str() {
                            let file_metadata = classify_video_content(file_path_str);
                            if let Some(season) = file_metadata.season {
                                seasons.insert(season);
                            }
                            if let Some(episode) = file_metadata.episode {
                                if episode > 0 {
                                    episodes.insert(episode);
                                }
                            }
                        }
                    }
                }
            }

            // If we have multiple episodes from the same season, it's a boxset
            if seasons.len() == 1 && episodes.len() > 1 && video_count > 1 {
                metadata.is_boxset = true;
                if metadata.episode.is_none() || metadata.episode == Some(0) {
                    // Keep episode as 0 for season packs
                    metadata.episode = Some(0);
                }
            }
        }
    }

    info!("Video classification result: {:?}", metadata);
    metadata
}

/// Extract title before a regex pattern match
fn extract_title_before_pattern(filename: &str, pattern: &Regex) -> String {
    if let Some(pattern_match) = pattern.find(filename) {
        clean_title(&filename[..pattern_match.start()])
    } else {
        clean_title(filename)
    }
}

/// Clean up title by only replacing separators, preserving technical info
fn clean_title(title: &str) -> String {
    let cleaned = title
        .trim()
        .replace('.', " ")
        .replace('_', " ")
        .replace('-', " ");

    // Only clean up extra whitespace, preserve all technical indicators
    let whitespace_regex = Regex::new(r"\s+").unwrap();
    whitespace_regex
        .replace_all(&cleaned, " ")
        .trim()
        .to_string()
}

/// Classify video content for upload pipeline
pub fn classify_for_upload(
    input_path: &str,
    metadata: &serde_json::Value,
) -> Result<(Option<String>, Option<String>, serde_json::Value), String> {
    // If we already have classification data in metadata, use it
    if metadata.get("category").is_some() {
        let category = metadata
            .get("category")
            .and_then(|c| c.as_str())
            .map(|c| format!("VideoCategory::{}", c.replace("VideoCategory::", "")));

        let source_type = metadata
            .get("source_type")
            .and_then(|s| s.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| format!("VideoSourceType::{}", s.replace("VideoSourceType::", "")));

        return Ok((category, source_type, metadata.clone()));
    }

    // Otherwise, run classification
    let video_metadata = classify_video_content(input_path);

    let category = Some(format!("VideoCategory::{:?}", video_metadata.category));
    let source_type = Some(format!("VideoSourceType::{:?}", video_metadata.source_type));

    // Manually create JSON metadata
    let mut json_metadata = serde_json::json!({
        "title": video_metadata.title,
        "year": video_metadata.year,
        "season": video_metadata.season,
        "episode": video_metadata.episode,
        "category": format!("{:?}", video_metadata.category),
        "source_type": format!("{:?}", video_metadata.source_type),
        "is_boxset": video_metadata.is_boxset,
        "is_dated_tv": video_metadata.is_dated_tv,
        "resolution": video_metadata.resolution,
        "codec": video_metadata.codec,
    });

    // Merge with existing metadata
    if let (Some(json_obj), Some(existing_obj)) =
        (json_metadata.as_object_mut(), metadata.as_object())
    {
        for (key, value) in existing_obj {
            json_obj.entry(key.clone()).or_insert(value.clone());
        }
    }

    Ok((category, source_type, json_metadata))
}
