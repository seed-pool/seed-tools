// File system utilities

use std::path::{Path, PathBuf};
use std::fs;
use walkdir::WalkDir;
use log::info;
use crate::core::error::{SeedError, Result};

/// Filter files in a directory by accepted file extensions
/// 
/// Takes an input path (file or directory) and returns only files with accepted extensions.
/// If input is a file, returns it only if it has an accepted extension.
/// If input is a directory, recursively finds all files with accepted extensions.
/// 
/// # Arguments
/// * `input_path` - Path to file or directory to search
/// * `accepted_extensions` - Array of accepted file extensions (without dots, e.g., ["mp4", "mkv"])
/// 
/// # Returns
/// Vector of paths to files with accepted extensions
pub fn filter_files_by_extension(
    input_path: &str,
    accepted_extensions: &[&str],
) -> Result<Vec<PathBuf>> {
    let path = Path::new(input_path);
    
    if !path.exists() {
        return Err(SeedError::Validation(format!("Path not found: {}", input_path)));
    }
    
    let mut matching_files = Vec::new();
    
    // Convert extensions to lowercase for case-insensitive comparison
    let accepted_exts: Vec<String> = accepted_extensions
        .iter()
        .map(|ext| ext.to_lowercase())
        .collect();
    
    if path.is_file() {
        // Check if single file has accepted extension
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if accepted_exts.contains(&ext.to_lowercase()) {
                matching_files.push(path.to_path_buf());
            }
        }
    } else if path.is_dir() {
        // Keywords to exclude from paths
        let excluded_keywords = ["sample", "samples", "screen", "screens", "screenshot", "screenshots", 
                                "extra", "extras", "proof", "proofs"];
        
        // Recursively search directory for files with accepted extensions
        for entry in WalkDir::new(path)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let entry_path = entry.path();
            
            // Check if path contains any excluded keywords
            let path_str = entry_path.to_string_lossy().to_lowercase();
            let should_exclude = excluded_keywords.iter().any(|keyword| {
                path_str.contains(&format!("/{}/", keyword)) || 
                path_str.contains(&format!("\\\\{}\\\\", keyword)) ||
                path_str.ends_with(&format!("/{}", keyword)) ||
                path_str.ends_with(&format!("\\\\{}", keyword))
            });
            
            if should_exclude {
                continue;
            }
            
            if entry_path.is_file() {
                if let Some(ext) = entry_path.extension().and_then(|e| e.to_str()) {
                    if accepted_exts.contains(&ext.to_lowercase()) {
                        matching_files.push(entry_path.to_path_buf());
                    }
                }
            }
        }
    }
    
    Ok(matching_files)
}


/// Count files in a directory
pub fn count_files_in_directory(dir_path: &str) -> Result<usize> {
    let path = Path::new(dir_path);
    if !path.is_dir() {
        return Err(SeedError::Validation(format!("Not a directory: {}", dir_path)));
    }
    
    let count = WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .count();
    
    Ok(count)
}

/// Get file size in human-readable format
pub fn format_file_size(size: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = size as f64;
    let mut unit_index = 0;
    
    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }
    
    format!("{:.2} {}", size, UNITS[unit_index])
}

/// Find and read NFO file content from a directory
/// 
/// Searches for .nfo files in the specified directory and returns the content
/// of the first NFO file found. This can be used by any media processing function
/// to include NFO data in uploads.
/// 
/// # Arguments
/// * `working_path` - Path to directory to search for NFO files
/// 
/// # Returns
/// * `Ok(Some((path, content)))` - Path to NFO file and its content as bytes
/// * `Ok(None)` - No NFO file found
/// * `Err(String)` - Error reading NFO file
pub fn find_and_read_nfo(working_path: &str) -> Result<Option<(String, Vec<u8>)>> {
    let path = Path::new(working_path);
    
    if !path.exists() {
        return Err(SeedError::Validation(format!("Path not found: {}", working_path)));
    }
    
    // If it's a single file, check if it's an NFO
    if path.is_file() {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if ext.eq_ignore_ascii_case("nfo") {
                let content = fs::read(path)
                    .map_err(|e| SeedError::Other(format!("Failed to read NFO file \"{}\": {}", path.display(), e)))?;
                return Ok(Some((path.to_string_lossy().to_string(), content)));
            }
        }
        return Ok(None);
    }
    
    // Search directory for NFO files
    if path.is_dir() {
        for entry in fs::read_dir(path)
            .map_err(|e| SeedError::Other(format!("Failed to read directory \"{}\": {}", working_path, e)))?
        {
            let entry = entry.map_err(|e| SeedError::Other(format!("Failed to read directory entry: {}", e)))?;
            let entry_path = entry.path();
            
            if entry_path.is_file() {
                if let Some(ext) = entry_path.extension().and_then(|e| e.to_str()) {
                    if ext.eq_ignore_ascii_case("nfo") {
                        info!("Found NFO file: {}", entry_path.display());
                        let content = fs::read(&entry_path)
                            .map_err(|e| SeedError::Other(format!("Failed to read NFO file \"{}\": {}", entry_path.display(), e)))?;
                        return Ok(Some((entry_path.to_string_lossy().to_string(), content)));
                    }
                }
            }
        }
    }
    
    Ok(None)
}