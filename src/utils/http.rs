// HTTP client utilities

use crate::core::error::{Result, SeedError};
use log::{debug, info};
use reqwest::blocking::{multipart::Form, Client};
use serde_json::Value;
use std::time::Duration;

/// Download a file from a URL
pub fn download_file(url: &str, timeout_secs: u64) -> Result<Vec<u8>> {
    let client = Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .build()
        .map_err(|e| SeedError::ApiError(format!("Failed to create HTTP client: {}", e)))?;

    let response = client
        .get(url)
        .send()
        .map_err(|e| SeedError::ApiError(format!("Failed to download from {}: {}", url, e)))?;

    if !response.status().is_success() {
        return Err(SeedError::ApiError(format!(
            "HTTP {} from {}",
            response.status(),
            url
        )));
    }

    let bytes = response
        .bytes()
        .map_err(|e| SeedError::ApiError(format!("Failed to read response body: {}", e)))?;

    Ok(bytes.to_vec())
}

/// Create a default HTTP client with sensible timeouts
pub fn create_default_client() -> Result<Client> {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| SeedError::ApiError(format!("Failed to create HTTP client: {}", e)))
}

/// Check if a URL is reachable
pub fn check_url_reachable(url: &str) -> bool {
    if let Ok(client) = create_default_client() {
        if let Ok(response) = client.head(url).send() {
            return response.status().is_success();
        }
    }
    false
}

/// Upload image to ImgBB
pub fn upload_to_imgbb(
    image_path: &str,
    imgbb_api_key: &str,
    dry_run: bool,
) -> Result<(String, String)> {
    let client = Client::new();

    // Log the image path and API key for debugging
    debug!(
        "Uploading image to ImgBB: path={}, api_key={}",
        image_path, imgbb_api_key
    );

    let form = Form::new()
        .file("image", image_path)
        .map_err(|e| SeedError::ApiError(format!("Failed to attach image file: {}", e)))?;

    let url = format!("https://api.imgbb.com/1/upload?key={}", imgbb_api_key);
    debug!("ImgBB API URL: {}", url);

    if dry_run {
        info!("[DRY RUN] Would upload image to ImgBB: {}", url);
        info!("[DRY RUN] Would generate ImgBB URLs: https://i.ibb.co/fake-url and https://i.ibb.co/fake-thumb");
        return Ok((
            "https://i.ibb.co/fake-url".to_string(),
            "https://i.ibb.co/fake-thumb".to_string(),
        ));
    }

    let response = client
        .post(&url)
        .multipart(form)
        .send()
        .map_err(|e| SeedError::ApiError(format!("Failed to upload image to ImgBB: {}", e)))?;

    if !response.status().is_success() {
        let status = response.status();
        let response_body = response
            .text()
            .unwrap_or_else(|_| "Failed to read response body".to_string());
        log::error!(
            "ImgBB API Error: HTTP Status: {}, Response: {}",
            status,
            response_body
        );
        return Err(SeedError::ApiError(format!(
            "Failed to upload image to ImgBB. HTTP Status: {}. Response: {}",
            status, response_body
        )));
    }

    let json: Value = response
        .json()
        .map_err(|e| SeedError::ApiError(format!("Failed to parse ImgBB response: {}", e)))?;

    let full_image_url = json["data"]["image"]["url"]
        .as_str()
        .ok_or(SeedError::ApiError(
            "Failed to extract full image URL from ImgBB response".to_string(),
        ))?
        .to_string();
    let thumb_url = json["data"]["thumb"]["url"]
        .as_str()
        .ok_or(SeedError::ApiError(
            "Failed to extract thumbnail URL from ImgBB response".to_string(),
        ))?
        .to_string();

    info!(
        "ImgBB Upload Successful: full_image_url={}, thumb_url={}",
        full_image_url, thumb_url
    );

    Ok((full_image_url, thumb_url))
}

/// Upload file to CDN via SCP
pub fn upload_to_cdn(file_path: &str, remote_path: &str) -> Result<()> {
    use std::process::Command;

    info!("Uploading file to CDN: {}", file_path);

    let status = Command::new("scp")
        .arg(file_path)
        .arg(remote_path)
        .status()
        .map_err(|e| SeedError::Other(format!("Failed to execute scp: {}", e)))?;

    if !status.success() {
        return Err(SeedError::Other(format!(
            "Failed to upload file to CDN: {}",
            file_path
        )));
    }

    Ok(())
}

/// Extract torrent ID from upload response
pub fn extract_torrent_id(response_text: &str) -> Result<String> {
    // Unescape any escaped slashes
    let response_text = response_text.replace(r"\/", "/");

    // Updated regex to match the numeric ID followed by a dot and a 32-character hash
    let re = regex::Regex::new(r#"/download/(\d+)\.[a-fA-F0-9]{32}"#)
        .map_err(|e| SeedError::Validation(format!("Failed to compile regex: {}", e)))?;

    if let Some(captures) = re.captures(&response_text) {
        if let Some(torrent_id) = captures.get(1) {
            return Ok(torrent_id.as_str().to_string());
        }
    }
    Err(SeedError::Parse(
        "Failed to extract torrent ID from response.".to_string(),
    ))
}
