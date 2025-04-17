use std::path::Path;
use log::{info, error};
use reqwest::blocking::multipart::Form;
use seed_tools::utils::{create_torrent, add_torrent_to_all_qbittorrent_instances};
use crate::{QbittorrentConfig, SeedpoolConfig, TorrentLeechConfig};

pub fn process_custom_upload(
    input_path: &str,
    category_id: u32,
    type_id: u32,
    qbittorrent_configs: &[QbittorrentConfig],
    tracker: &str, // "seedpool" or "torrentleech"
    seedpool_config: Option<&SeedpoolConfig>,
    torrentleech_config: Option<&TorrentLeechConfig>,
    mkbrr_path: &str, // Path to mkbrr binary
) -> Result<(), String> {
    let base_name = Path::new(input_path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    info!(
        "Processing custom upload with category_id={} and type_id={} for tracker={}",
        category_id, type_id, tracker
    );

    // Determine the announce and upload URLs based on the tracker
    let (announce_url, upload_url) = match tracker {
        "seedpool" => {
            let config = seedpool_config.ok_or("Seedpool configuration is missing")?;
            (config.settings.announce_url.clone(), config.settings.upload_url.clone())
        }
        "torrentleech" => {
            let config = torrentleech_config.ok_or("TorrentLeech configuration is missing")?;
            (config.general.announce_url_1.clone(), config.settings.upload_url.clone())
        }
        _ => return Err("Invalid tracker specified".to_string()),
    };

    // Create the torrent file using mkbrr
    let torrent_file = create_torrent(
        &[input_path.to_string()],
        "./torrents", // Output directory for torrents
        &announce_url,
        mkbrr_path, // Path to mkbrr binary
    )?;

    // Check for an .nfo file
    let nfo_file = Path::new(input_path)
        .with_extension("nfo")
        .exists()
        .then(|| input_path.to_string());

    // Prepare the upload form
    let client = reqwest::blocking::Client::new();
    let mut form = Form::new()
        .file("torrent", &torrent_file)
        .map_err(|e| format!("Failed to attach torrent file: {}", e))?
        .text("name", base_name)
        .text("category_id", category_id.to_string())
        .text("type_id", type_id.to_string())
        .text("tmdb", "0")
        .text("imdb", "0")
        .text("tvdb", "0")
        .text("anonymous", "0")
        .text("description", "Custom upload") // Add a default description
        .text("mal", "0") // Add default value for mal
        .text("igdb", "0") // Add default value for igdb
        .text("stream", "0") // Add default value for stream
        .text("sd", "0"); // Add default value for sd
    
    if let Some(nfo) = nfo_file {
        form = form.file("nfo", nfo).map_err(|e| format!("Failed to attach NFO file: {}", e))?;
    }
    
    // Send the upload request
    let response = client
        .post(&upload_url)
        .multipart(form)
        .send()
        .map_err(|e| format!("Failed to send upload request: {}", e))?;
    
    let status = response.status();
    let response_text = response.text().unwrap_or_else(|_| "Failed to read response body".to_string());
    info!("Upload response: HTTP {}: {}", status, response_text);
    
    if !status.is_success() {
        return Err(format!(
            "Failed to upload torrent. HTTP Status: {}. Response: {}",
            status, response_text
        ));
    }

    // Inject the torrent into qBittorrent
    add_torrent_to_all_qbittorrent_instances(
        &[torrent_file],
        qbittorrent_configs,
        input_path,
        Path::new(input_path).is_dir(),
    )?;

    Ok(())
}