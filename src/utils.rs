use reqwest::blocking::{multipart::Form, Client};
use std::path::Path;
use regex::Regex;
use log::info;
use serde_json::Value;
use zip::ZipArchive;
use std::fs::File;
use std::io::copy;
use walkdir::WalkDir;
use crate::types::{SeedpoolConfig, Config};

// Re-export validation functions for backward compatibility
pub use crate::validation::{validate_file_path, validate_api_key, validate_url};

// Re-export naming functions for backward compatibility
pub use crate::naming::generate_release_name;

// Re-export torrent functions for backward compatibility
pub use crate::torrent::{create_torrent, add_torrent_to_all_qbittorrent_instances, add_torrent_to_qbittorrent, add_torrent_to_deluge};

// Re-export archive functions for backward compatibility
pub use crate::archive::{extract_rar_archives, extract_archives_in_directory};

// Re-export ebook functions for backward compatibility
pub use crate::media::ebook::{process_ebook_upload, generate_ebook_description};

// Re-export video functions for backward compatibility1
pub use crate::media::video::{
    find_video_files, generate_sample, get_video_duration, default_non_video_description,
    generate_mediainfo, process_file, contains_excluded_keywords, generate_description,
    generate_screenshots, generate_screenshots_imgbb
};

pub fn fetch_tmdb_id(title: &str, year: Option<String>, tmdb_api_key: &str, release_type: &str) -> Result<u32, String> {
    let sanitized_title = if release_type == "tv" {
        // Extract everything before the SXX* pattern
        let season_regex = Regex::new(r"(?i)(S\d{2}.*)").unwrap();
        let cleaned_title = season_regex.replace(title, "").trim().to_string();

        // Remove the year if present
        let year_regex = Regex::new(r"\b(19|20)\d{2}\b").unwrap();
        year_regex.replace(&cleaned_title, "").trim().to_string()
    } else {
        // For movies, extract everything before the year
        let year_regex = Regex::new(r"\b(19|20)\d{2}\b").unwrap();
        year_regex.replace(title, "").trim().to_string()
    };

    let encoded_title = urlencoding::encode(&sanitized_title);

    let url = if release_type == "tv" {
        format!(
            "https://api.themoviedb.org/3/search/tv?query={}&first_air_date_year={}&api_key={}",
            encoded_title,
            year.unwrap_or_default(),
            tmdb_api_key
        )
    } else {
        format!(
            "https://api.themoviedb.org/3/search/movie?query={}&year={}&api_key={}",
            encoded_title,
            year.unwrap_or_default(),
            tmdb_api_key
        )
    };

    info!("TMDB API URL: {}", url);

    let client = Client::new();
    let response = client
        .get(&url)
        .send()
        .map_err(|e| format!("Failed to query TMDB for '{}': {}", title, e))?;

    if !response.status().is_success() {
        return Err(format!(
            "TMDB API request failed with status: {}",
            response.status()
        ));
    }

    let json: Value = response
        .json()
        .map_err(|e| format!("Failed to parse TMDB response for '{}': {}", title, e))?;

    let tmdb_id = json["results"]
        .as_array()
        .and_then(|results| results.get(0))
        .and_then(|result| result["id"].as_u64())
        .unwrap_or(0) as u32;

    if tmdb_id == 0 {
        info!("No TMDB ID found for '{}'.", title);
    }

    Ok(tmdb_id)
}

pub fn fetch_youtube_trailer(title: &str, year: Option<&str>, youtube_api_key: &str) -> Result<String, String> {
    let client = Client::new();

    // Construct the search query
    let query = if let Some(year) = year {
        format!("{} {} trailer", title, year)
    } else {
        format!("{} trailer", title)
    };

    // Construct the YouTube Data API URL
    let url = format!(
        "https://www.googleapis.com/youtube/v3/search?part=snippet&q={}&type=video&key={}&maxResults=1",
        urlencoding::encode(&query),
        youtube_api_key
    );

    // Send the API request
    let response = client
        .get(&url)
        .send()
        .map_err(|e| format!("Failed to send request to YouTube API: {}", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "YouTube API request failed with status: {}",
            response.status()
        ));
    }

    // Parse the JSON response
    let response_body = response.text().map_err(|e| format!("Failed to read YouTube API response: {}", e))?;
    let json: Value = serde_json::from_str(&response_body)
        .map_err(|e| format!("Failed to parse YouTube API response: {}", e))?;

    // Extract the video ID of the first result
    if let Some(video_id) = json["items"]
        .as_array()
        .and_then(|items| items.get(0))
        .and_then(|item| item["id"]["videoId"].as_str())
    {
        let video_url = format!("https://www.youtube.com/watch?v={}", video_id);
        Ok(video_url)
    } else {
        Err("No trailer found on YouTube.".to_string())
    }
}

pub fn fetch_external_ids(tmdb_id: u32, release_type: &str, tmdb_api_key: &str) -> Result<(Option<String>, Option<u32>), String> {
    if tmdb_id == 0 {
        return Ok((None, None));
    }

    let tmdb_type = if release_type == "boxset" { "tv" } else { release_type };
    let url = format!(
        "https://api.themoviedb.org/3/{}/{}/external_ids?api_key={}",
        tmdb_type, tmdb_id, tmdb_api_key
    );

    log::info!("TMDB External IDs API URL: {}", url);

    let client = reqwest::blocking::Client::new();
    let response = client.get(&url).send().map_err(|e| format!("Failed to fetch external IDs: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("Failed to fetch external IDs: HTTP {}", response.status()));
    }

    let json: serde_json::Value = response.json().map_err(|e| format!("Failed to parse external IDs response: {}", e))?;
    let imdb_id = json["imdb_id"].as_str().map(|s| s.trim_start_matches("tt").to_string());
    let tvdb_id = json["tvdb_id"].as_u64().map(|id| id as u32);

    log::info!("Fetched IMDb ID: {:?}", imdb_id);
    log::info!("Fetched TVDB ID: {:?}", tvdb_id);

    Ok((imdb_id, tvdb_id))
}


pub fn extract_torrent_id(response_text: &str) -> Result<String, String> {
    // Unescape any escaped slashes
    let response_text = response_text.replace(r"\/", "/");

    // Updated regex to match the numeric ID followed by a dot and a 32-character hash
    let re = regex::Regex::new(r#"/download/(\d+)\.[a-fA-F0-9]{32}"#).map_err(|e| format!("Failed to compile regex: {}", e))?;
    if let Some(captures) = re.captures(&response_text) {
        if let Some(torrent_id) = captures.get(1) {
            return Ok(torrent_id.as_str().to_string());
        }
    }
    Err("Failed to extract torrent ID from response.".to_string())
}

pub fn process_newspaper_upload(
    input_path: &str,
    config: &Config,
    seedpool_config: &SeedpoolConfig,
    dry_run: bool,
) -> Result<(), String> {
    use reqwest::blocking::Client;
    use std::fs;

    let _working_dir = input_path.to_string();

    // If input is a file, get its parent directory for extraction
    let path = Path::new(input_path);
    let is_file = path.is_file();
    let working_dir = if is_file {
        // For single file, just use the file path
        input_path.to_string()
    } else {
        // For directory, use as-is
        input_path.to_string()
    };

    // 1. Extract all ZIP files in the directory
    if !is_file {
        let zip_files: Vec<_> = fs::read_dir(&working_dir)
            .map_err(|e| format!("Failed to read directory '{}': {}", working_dir, e))?
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let path = entry.path();
                if path.extension().map_or(false, |ext| ext.eq_ignore_ascii_case("zip")) {
                    Some(path)
                } else {
                    None
                }
            })
            .collect();

        for zip_file in &zip_files {
            log::info!("Extracting ZIP archive: {}", zip_file.display());
            let output = std::process::Command::new("unzip")
                .arg("-o")
                .arg(zip_file)
                .arg("-d")
                .arg(&working_dir)
                .output()
                .map_err(|e| format!("Failed to execute unzip: {}", e))?;
            if !output.status.success() {
                return Err(format!(
                    "Failed to extract ZIP archive: {}. Error: {}",
                    zip_file.display(),
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
        }

        // 2. Extract all RAR files in the directory
        extract_rar_archives(&working_dir)?;
    }

    // 3. Find the main .epub or .pdf file
    let mut found_pdf: Option<String> = None;
    let mut found_epub: Option<String> = None;
    for entry in WalkDir::new(&working_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if ext.eq_ignore_ascii_case("epub") {
                    found_epub = Some(path.to_string_lossy().to_string());
                    break;
                } else if ext.eq_ignore_ascii_case("pdf") {
                    found_pdf = Some(path.to_string_lossy().to_string());
                }
            }
        }
    }
    let (newspaper_path, is_pdf) = if let Some(epub) = found_epub {
        (epub, false)
    } else if let Some(pdf) = found_pdf {
        (pdf, true)
    } else {
        return Err(format!("No .epub or .pdf file found in directory '{}'", working_dir));
    };

    // 4. Extract images for description and cover
    let mut desc_image_urls = Vec::new();
    let mut cover_image_path: Option<String> = None;
    let base_name = Path::new(&newspaper_path)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    if is_pdf {
        // --- PDF: Use Ghostscript for cover and description images ---
        let temp_dir = std::env::temp_dir().join(format!("{}_pdf_images", base_name));
        fs::create_dir_all(&temp_dir)
            .map_err(|e| format!("Failed to create temp dir for images: {}", e))?;

        // Extract cover (page 1)
        let cover_path = temp_dir.join("page-1.jpg");
        let output = std::process::Command::new("gs")
            .args(&[
                "-dBATCH", "-dNOPAUSE",
                "-sDEVICE=jpeg",
                "-dFirstPage=1", "-dLastPage=1",
                "-r150", "-dJPEGQ=95",
                &format!("-sOutputFile={}", cover_path.display()),
                &newspaper_path,
            ])
            .output()
            .map_err(|e| format!("Failed to run gs for cover: {}", e))?;
        if !output.status.success() {
            return Err(format!(
                "Failed to extract cover from PDF: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        cover_image_path = Some(cover_path.to_string_lossy().to_string());

        // Extract pages 2-11 for description
        for page in 2..=11 {
            let img_name = format!("{}-page{}.jpg", base_name, page);
            let img_path = temp_dir.join(&img_name);
            let output = std::process::Command::new("gs")
                .args(&[
                    "-dBATCH", "-dNOPAUSE",
                    "-sDEVICE=jpeg",
                    &format!("-dFirstPage={}", page),
                    &format!("-dLastPage={}", page),
                    "-r300", "-dJPEGQ=95",
                    &format!("-sOutputFile={}", img_path.display()),
                    &newspaper_path,
                ])
                .output()
                .map_err(|e| format!("Failed to run gs for page {}: {}", page, e))?;
            if !output.status.success() {
                return Err(format!(
                    "Failed to extract page {}: {}",
                    page,
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&img_path, fs::Permissions::from_mode(0o777))
                    .map_err(|e| format!("Failed to set permissions for '{}': {}", img_path.display(), e))?;
            }
            // SCP to CDN (skip during dry run)
            if !dry_run {
                let scp = std::process::Command::new("scp")
                    .arg(&img_path)
                    .arg(format!("{}/screenshots/", seedpool_config.screenshots.remote_path.trim_end_matches('/')))
                    .output()
                    .map_err(|e| format!("Failed to upload description image via SCP: {}", e))?;
                if !scp.status.success() {
                    return Err(format!(
                        "Failed to upload description image via SCP. Error: {}",
                        String::from_utf8_lossy(&scp.stderr)
                    ));
                }
            }
            let url = format!("{}/{}", seedpool_config.screenshots.image_path.trim_end_matches('/'), img_name);
            desc_image_urls.push(url);
        }
    } else {
        // --- EPUB: Use Rust to extract images for cover and description ---
        let temp_dir = std::env::temp_dir().join(format!("{}_epub_images", base_name));
        fs::create_dir_all(&temp_dir)
            .map_err(|e| format!("Failed to create temp dir for images: {}", e))?;

        let page_images = {
            let file = File::open(&newspaper_path).map_err(|e| format!("Failed to open EPUB: {}", e))?;
            let mut archive = ZipArchive::new(file).map_err(|e| format!("Failed to read EPUB as zip: {}", e))?;

            let mut images = Vec::new();

            for i in 0..archive.len() {
                let mut file = archive.by_index(i).map_err(|e| format!("Failed to access EPUB entry: {}", e))?;
                let name = file.name().to_lowercase();
                if name.ends_with(".jpg") || name.ends_with(".jpeg") || name.ends_with(".png") || name.ends_with(".gif") {
                    let out_path = temp_dir.join(std::path::Path::new(&name).file_name().unwrap());
                    let mut out_file = File::create(&out_path).map_err(|e| format!("Failed to create image file: {}", e))?;
                    copy(&mut file, &mut out_file).map_err(|e| format!("Failed to extract image: {}", e))?;
                    images.push(out_path);
                }
            }

            images.sort();
            images
        };

        if page_images.len() < 2 {
            return Err("Not enough images extracted from EPUB.".to_string());
        }

        // Pages 2-11 for description
        for (i, img) in page_images.iter().enumerate().skip(1).take(10) {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(img, fs::Permissions::from_mode(0o777))
                    .map_err(|e| format!("Failed to set permissions for image '{}': {}", img.display(), e))?;
            }
            let img_name = format!("{}-page{}.jpg", base_name, i + 1);
            // SCP to CDN (skip during dry run)
            if !dry_run {
                let scp = std::process::Command::new("scp")
                    .arg(img)
                    .arg(format!("{}/screenshots/", seedpool_config.screenshots.remote_path.trim_end_matches('/')))
                    .output()
                    .map_err(|e| format!("Failed to upload description image via SCP: {}", e))?;
                if !scp.status.success() {
                    return Err(format!(
                        "Failed to upload description image via SCP. Error: {}",
                        String::from_utf8_lossy(&scp.stderr)
                    ));
                }
            }
            let url = format!("{}/{}", seedpool_config.screenshots.image_path.trim_end_matches('/'), img_name);
            desc_image_urls.push(url);
        }
        // Cover image is page 1
        if let Some(cover_img) = page_images.get(0) {
            cover_image_path = Some(cover_img.to_string_lossy().to_string());
        }
    }

    // 5. Build BBCode description
    let mut description = format!(
        "[center][b][size=18][color=#2E86C1]{}[/color][/size][/b]\n\n[table]\n",
        base_name
    );
    for (i, url) in desc_image_urls.iter().enumerate() {
        if i % 2 == 0 {
            description.push_str("  [tr]\n");
        }
        description.push_str(&format!("    [td][img width=720]{}[/img][/td]\n", url));
        if i % 2 == 1 {
            description.push_str("  [/tr]\n");
        }
    }
    if desc_image_urls.len() % 2 != 0 {
        description.push_str("    [td][/td]\n  [/tr]\n");
    }
    description.push_str("[/table][/center]\n\n");
    description.push_str(&format!("[center]{}[/center]", default_non_video_description()));

    if !is_file {
        for entry in fs::read_dir(&working_dir).map_err(|e| format!("Failed to read directory '{}': {}", working_dir, e))? {
            let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
            let path = entry.path();
            if path.extension().map(|ext| ext.eq_ignore_ascii_case("zip")).unwrap_or(false) {
                fs::remove_file(&path)
                    .map_err(|e| format!("Failed to remove zip file '{}': {}", path.display(), e))?;
            }
        }
    }

    // 6. Create torrent
    let torrent_input = &working_dir;
    let torrent_file = create_torrent(
        torrent_input,
        &config.paths.torrent_dir,
        &seedpool_config.settings.announce_url,
        &config.paths.mkbrr,
        true,
    )?;

    // 7. Prepare upload form and upload to Seedpool
    let nfo_file = fs::read_dir(&working_dir)
        .ok()
        .and_then(|mut entries| {
            entries.find_map(|entry| {
                let entry = entry.ok()?;
                let path = entry.path();
                if path.extension().map(|ext| ext.eq_ignore_ascii_case("nfo")).unwrap_or(false) {
                    Some(path.to_string_lossy().to_string())
                } else {
                    None
                }
            })
        });

    let mut form = Form::new()
        .file("torrent", &torrent_file)
        .map_err(|e| format!("Failed to attach torrent file: {}", e))?
        .text("name", Path::new(input_path).file_name().unwrap_or_default().to_string_lossy().to_string())
        .text("category_id", "7") // eBooks category
        .text("type_id", "42")    // Newspaper type
        .text("tmdb", "0")
        .text("imdb", "0")
        .text("tvdb", "0")
        .text("anonymous", "0")
        .text("description", description)
        .text("keywords", "newspaper")
        .text("mal", "0")
        .text("igdb", "0")
        .text("stream", "0")
        .text("sd", "0");

    if let Some(nfo) = nfo_file {
        form = form.file("nfo", nfo).map_err(|e| format!("Failed to attach NFO file: {}", e))?;
    }

    let client = Client::new();
    
    if dry_run {
        info!("[DRY RUN] Would upload newspaper to Seedpool: {}", seedpool_config.settings.upload_url);
        info!("[DRY RUN] Form data would include: torrent file, description, category 7, type 42, etc.");
        return Ok(());
    }
    
    let response = client
        .post(&seedpool_config.settings.upload_url)
        .header("Authorization", format!("Bearer {}", seedpool_config.general.api_key))
        .multipart(form)
        .send()
        .map_err(|e| format!("Failed to send request to Seedpool: {}", e))?;

    let status = response.status();
    let response_text = response.text().unwrap_or_else(|_| "Failed to read response body".to_string());
    info!("Seedpool API Response: {}", response_text);

    if !status.is_success() {
        return Err(format!(
            "Failed to upload to Seedpool. HTTP Status: {}. Response: {}",
            status, response_text
        ));
    }

    // Extract the torrent ID from the response
    let torrent_id = extract_torrent_id(&response_text)?;

    // 8. Upload cover image to CDN, named with torrent id
    if let Some(cover_img_path) = cover_image_path {
        let cover_name = format!("torrent-cover_{}.jpg", torrent_id);
        let temp_cover_path = std::env::temp_dir().join(&cover_name);

        // Rename or copy the cover image to the correct name in temp
        fs::copy(&cover_img_path, &temp_cover_path)
            .map_err(|e| format!("Failed to copy cover image for CDN upload: {}", e))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&temp_cover_path, fs::Permissions::from_mode(0o777))
                .map_err(|e| format!("Failed to set permissions for cover image '{}': {}", temp_cover_path.display(), e))?;
        }

        let cover_remote_path = format!("{}/covers", seedpool_config.screenshots.remote_path.trim_end_matches('/'));
        // SCP to CDN (skip during dry run)
        if !dry_run {
            let cover_scp = std::process::Command::new("scp")
                .arg(&temp_cover_path)
                .arg(&cover_remote_path)
                .output()
                .map_err(|e| format!("Failed to upload cover image via SCP: {}", e))?;
            if !cover_scp.status.success() {
                return Err(format!(
                    "Failed to upload cover image via SCP. Error: {}",
                    String::from_utf8_lossy(&cover_scp.stderr)
                ));
            }
        }

        // Optionally clean up the temp file
        let _ = fs::remove_file(&temp_cover_path);
    }

    // 9. Add torrent to all qBittorrent instances
    add_torrent_to_all_qbittorrent_instances(
        &[torrent_file.clone()],
        &config.qbittorrent,
        &config.deluge,
        newspaper_path.as_str(),
        &config.paths,
    )?;

    Ok(())
}

pub fn upload_to_cdn(file_path: &str, remote_path: &str) -> Result<(), String> {
    use std::process::Command;

    info!("Uploading file to CDN: {}", file_path);

    let status = Command::new("scp")
        .arg(file_path)
        .arg(remote_path)
        .status()
        .map_err(|e| format!("Failed to execute scp: {}", e))?;

    if !status.success() {
        return Err(format!("Failed to upload file to CDN: {}", file_path));
    }

    Ok(())
}

pub fn upload_to_imgbb(image_path: &str, imgbb_api_key: &str, dry_run: bool) -> Result<(String, String), String> {
    let client = Client::new();

    // Log the image path and API key for debugging
    log::debug!("Uploading image to ImgBB: path={}, api_key={}", image_path, imgbb_api_key);

    let form = Form::new()
        .file("image", image_path)
        .map_err(|e| format!("Failed to attach image file: {}", e))?;

    let url = format!("https://api.imgbb.com/1/upload?key={}", imgbb_api_key);
    log::debug!("ImgBB API URL: {}", url);

    if dry_run {
        info!("[DRY RUN] Would upload image to ImgBB: {}", url);
        info!("[DRY RUN] Would generate ImgBB URLs: https://i.ibb.co/fake-url and https://i.ibb.co/fake-thumb");
        return Ok(("https://i.ibb.co/fake-url".to_string(), "https://i.ibb.co/fake-thumb".to_string()));
    }

    let response = client
        .post(&url)
        .multipart(form)
        .send()
        .map_err(|e| format!("Failed to upload image to ImgBB: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let response_body = response.text().unwrap_or_else(|_| "Failed to read response body".to_string());
        log::error!("ImgBB API Error: HTTP Status: {}, Response: {}", status, response_body);
        return Err(format!(
            "Failed to upload image to ImgBB. HTTP Status: {}. Response: {}",
            status, response_body
        ));
    }

    let json: serde_json::Value = response
        .json()
        .map_err(|e| format!("Failed to parse ImgBB response: {}", e))?;

    let full_image_url = json["data"]["image"]["url"]
        .as_str()
        .ok_or("Failed to extract full image URL from ImgBB response")?
        .to_string();
    let thumb_url = json["data"]["thumb"]["url"]
        .as_str()
        .ok_or("Failed to extract thumbnail URL from ImgBB response")?
        .to_string();

    log::info!("ImgBB Upload Successful: full_image_url={}, thumb_url={}", full_image_url, thumb_url);

    Ok((full_image_url, thumb_url))
}