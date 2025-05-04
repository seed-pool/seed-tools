use std::path::Path;
use std::process::Command;
use crate::{Config, TorrentLeechConfig};
use log::{info, error};
use std::collections::HashMap;
use seed_tools::utils::{generate_release_name, find_video_files, create_torrent, generate_mediainfo};
use regex::Regex;

pub fn determine_tl_category(meta: &HashMap<String, String>, categories: &HashMap<String, u32>) -> Result<u32, String> {
    if meta.get("anime").map_or(false, |v| v == "true") {
        return Ok(*categories.get("Anime").unwrap_or(&34));
    }
    match meta.get("category").map(|v| v.as_str()) {
        Some("MOVIE") => {
            if meta.get("original_language").map_or(false, |lang| lang != "en") {
                Ok(*categories.get("MovieForeign").unwrap_or(&36))
            } else if meta.get("genres").map_or(false, |genres| genres.contains("Documentary")) {
                Ok(*categories.get("MovieDocumentary").unwrap_or(&29))
            } else if meta.get("uhd").map_or(false, |v| v == "true") {
                Ok(*categories.get("Movie4K").unwrap_or(&47))
            } else if meta.get("is_disc").map_or(false, |v| v == "BDMV" || v == "HDDVD")
                || (meta.get("type").map_or(false, |v| v == "REMUX")
                    && meta.get("source").map_or(false, |v| v == "BluRay" || v == "HDDVD"))
            {
                Ok(*categories.get("MovieBluray").unwrap_or(&13))
            } else if meta.get("type").map_or(false, |v| v == "ENCODE")
                && meta.get("source").map_or(false, |v| v == "BluRay" || v == "HDDVD")
            {
                Ok(*categories.get("MovieBlurayRip").unwrap_or(&14))
            } else if meta.get("is_disc").map_or(false, |v| v == "DVD")
                || (meta.get("type").map_or(false, |v| v == "REMUX")
                    && meta.get("source").map_or(false, |v| v.contains("DVD")))
            {
                Ok(*categories.get("MovieDvd").unwrap_or(&12))
            } else if meta.get("type").map_or(false, |v| v == "ENCODE")
                && meta.get("source").map_or(false, |v| v.contains("DVD"))
            {
                Ok(*categories.get("MovieDvdRip").unwrap_or(&11))
            } else if meta.get("type").map_or(false, |v| v.contains("WEB")) {
                Ok(*categories.get("MovieWebrip").unwrap_or(&37))
            } else if meta.get("type").map_or(false, |v| v == "HDTV") {
                Ok(*categories.get("MovieHdRip").unwrap_or(&43))
            } else {
                Err("Failed to determine TorrentLeech movie category.".to_string())
            }
        }
        Some("TV") => {
            if meta.get("original_language").map_or(false, |lang| lang != "en") {
                Ok(*categories.get("TvForeign").unwrap_or(&44))
            } else if meta.get("tv_pack").map_or(false, |v| v == "true") {
                Ok(*categories.get("TvBoxsets").unwrap_or(&27))
            } else if meta.get("sd").map_or(false, |v| v == "true") {
                Ok(*categories.get("TvEpisodes").unwrap_or(&26))
            } else {
                Ok(*categories.get("TvEpisodesHd").unwrap_or(&32))
            }
        }
        _ => Err("Failed to determine TorrentLeech category.".to_string()),
    }
}

fn determine_release_type_and_title(input_path: &str) -> (String, String) {
    let base_name = Path::new(input_path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let season_regex = Regex::new(r"(?i)S\d{2}").unwrap();
    let release_type = if season_regex.is_match(&base_name) {
        "boxset".to_string()
    } else {
        "movie".to_string()
    };

    let title = generate_release_name(&base_name);
    (release_type, title)
}

pub fn process_torrentleech_release(
    input_path: &str,
    sanitized_name: &str,
    config: &mut Config,
    torrentleech_config: &TorrentLeechConfig,
    mkbrr_path: &Path,
    mediainfo_path: &Path,
) -> Result<(), String> {
    let release_name = generate_release_name(sanitized_name);
    info!("Generated release name: {}", release_name);

    let (release_type, title) = determine_release_type_and_title(input_path);
    info!("Determined release type: {}, title: {}", release_type, title);

    let (video_files, _) = find_video_files(input_path, &config.paths, &torrentleech_config.settings)?;
    if video_files.is_empty() {
        return Err("No valid video files detected.".to_string());
    }

    let torrent_file = create_torrent(
        &video_files[0], // Use the first video file as a &str
        &config.paths.torrent_dir,
        &torrentleech_config.general.announce_url_1,
        &mkbrr_path.to_string_lossy(),
        false, // Disable filtering for non-Standard Upload Mode
    )?;

    let nfo_path = format!("{}/{}.nfo", config.paths.torrent_dir, release_name);
    let mediainfo_output = generate_mediainfo(&video_files[0], &mediainfo_path.to_string_lossy())?;
    std::fs::write(&nfo_path, mediainfo_output).map_err(|e| format!("Failed to write NFO file: {}", e))?;

    // Determine metadata
    let meta = HashMap::from([
        ("category".to_string(), if release_type == "boxset" { "TV".to_string() } else { "MOVIE".to_string() }),
        ("original_language".to_string(), "en".to_string()),
        ("type".to_string(), "WEB".to_string()),
    ]);

    // Determine category_id
    let category_id = if release_type == "boxset" {
        27 // Boxset category
    } else if release_type == "tv" && video_files.len() == 1 {
        32 // Single episode category
    } else {
        determine_tl_category(&meta, &torrentleech_config.categories)?
    };

    info!("Selected category_id: {}", category_id);

    // Upload torrent
    let output = Command::new("curl")
        .args(&[
            "-X", "POST",
            "-F", &format!("announcekey={}", torrentleech_config.settings.tl_key),
            "-F", &format!("category={}", category_id),
            "-F", &format!("nfo=@{}", nfo_path),
            "-F", &format!("torrent=@{}", torrent_file),
            &torrentleech_config.settings.upload_url,
        ])
        .output()
        .map_err(|e| format!("Failed to execute curl: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    info!("Curl stdout: {}", stdout);
    if !stderr.is_empty() {
        error!("Curl stderr: {}", stderr);
    }

    if stdout.contains("Duplicate torrent") {
        return Err("Duplicate torrent detected. Upload aborted.".to_string());
    }

    if !output.status.success() {
        return Err(format!(
            "Failed to upload to TorrentLeech. HTTP Status: {}. Error: {}",
            output.status,
            stderr
        ));
    }

    info!("Successfully uploaded torrent to TorrentLeech.");
    Ok(())
}