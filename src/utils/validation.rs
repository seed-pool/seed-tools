use std::path::Path;

/// Input validation functions for file paths, API keys, and URLs

pub fn validate_file_path(path: &str) -> Result<(), String> {
    if path.is_empty() {
        return Err("File path cannot be empty".to_string());
    }

    let path_obj = Path::new(path);
    if !path_obj.exists() {
        return Err(format!("File or directory does not exist: {}", path));
    }

    // Check for potential path traversal attacks
    if path.contains("../") || path.contains("..\\") {
        return Err("Path traversal patterns not allowed".to_string());
    }

    Ok(())
}

pub fn validate_api_key(api_key: &str, key_name: &str) -> Result<(), String> {
    if api_key.is_empty() {
        return Err(format!("{} cannot be empty", key_name));
    }

    if api_key.len() < 10 {
        return Err(format!("{} appears to be too short", key_name));
    }

    // Check for placeholder values
    if api_key.contains("xxxxx") || api_key == "your_api_key_here" {
        return Err(format!("{} appears to be a placeholder value", key_name));
    }

    Ok(())
}

pub fn validate_url(url: &str, url_name: &str) -> Result<(), String> {
    if url.is_empty() {
        return Err(format!("{} cannot be empty", url_name));
    }

    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(format!("{} must start with http:// or https://", url_name));
    }

    Ok(())
}
