use std::path::{Path, PathBuf};
use std::fs::{self, File};
use std::io::copy;
use std::process::Command;
use std::collections::HashMap;
use log::{info, warn, error};
use zip::ZipArchive;
use crate::types::ArchiveType;

/// Options for archive extraction
#[derive(Debug, Clone)]
pub struct ExtractionOptions {
    /// Whether to create subdirectory for direct archive inputs
    pub create_subdirectory: bool,
    /// Whether to preserve archive after extraction
    pub preserve_archive: bool,
    /// Whether to overwrite existing files
    pub overwrite: bool,
}

impl Default for ExtractionOptions {
    fn default() -> Self {
        Self {
            create_subdirectory: true,
            preserve_archive: true,
            overwrite: true,
        }
    }
}

/// Result of archive extraction
#[derive(Debug)]
pub struct ExtractionResult {
    /// Path where files were extracted
    pub extraction_path: PathBuf,
    /// Number of files extracted
    pub files_extracted: usize,
    /// Archive type that was extracted
    pub archive_type: ArchiveType,
}

/// Process and extract archives from input path before media classification
/// 
/// This function handles archive extraction for all media types:
/// - If input is a direct archive file, extracts to a subdirectory
/// - If input is a directory with archives, extracts them in place
/// - Supports nested archives (archives within archives)
/// - Supports multi-part archives (RAR parts)
/// - Returns the path where files should be processed from:
///   - For direct archive files: returns the extraction subdirectory path
///   - For directories or non-archive files: returns the original input path
pub fn process_and_extract_archives(input_path: &str) -> Result<String, String> {
    let path = Path::new(input_path);
    
    if !path.exists() {
        return Err(format!("Path not found: {}", input_path));
    }
    
    // Maximum extraction depth to prevent infinite loops
    const MAX_EXTRACTION_DEPTH: u32 = 5;
    
    if path.is_file() {
        // Check if it's an archive
        if let Some(archive_type) = ArchiveType::from_path(path) {
            info!("Detected archive file: {} (type: {:?})", input_path, archive_type);
            
            // Extract archive to a subdirectory
            let extraction_options = ExtractionOptions {
                create_subdirectory: true,
                preserve_archive: true,
                overwrite: true,
            };
            
            match extract_archive(input_path, extraction_options) {
                Ok(result) => {
                    info!("Successfully extracted {} files from archive to: {}", 
                          result.files_extracted, 
                          result.extraction_path.display());
                    
                    // Recursively extract any archives in the extraction directory
                    extract_nested_archives(&result.extraction_path, 1, MAX_EXTRACTION_DEPTH)?;
                    
                    // Return the extraction path where files were extracted
                    Ok(result.extraction_path.to_str()
                        .ok_or_else(|| "Failed to convert extraction path to string".to_string())?
                        .to_string())
                }
                Err(e) => {
                    error!("Failed to extract archive: {}", e);
                    Err(format!("Failed to extract archive '{}': {}", input_path, e))
                }
            }
        } else {
            // Not an archive, return original path
            Ok(input_path.to_string())
        }
    } else if path.is_dir() {
        // Extract any archives in the directory (including nested ones)
        extract_all_archives_recursively(input_path, MAX_EXTRACTION_DEPTH)?;
        // Return original directory path since archives were extracted in place
        Ok(input_path.to_string())
    } else {
        Err(format!("Path '{}' is neither a file nor a directory", input_path))
    }
}

/// Extract any supported archive type
pub fn extract_archive(
    archive_path: &str,
    options: ExtractionOptions,
) -> Result<ExtractionResult, String> {
    let path = Path::new(archive_path);
    
    if !path.exists() {
        return Err(format!("Archive file not found: {}", archive_path));
    }
    
    if !path.is_file() {
        return Err(format!("Path is not a file: {}", archive_path));
    }
    
    // Determine archive type
    let archive_type = ArchiveType::from_path(path)
        .ok_or_else(|| format!("Unsupported archive type: {}", archive_path))?;
    
    // Determine extraction directory
    let extraction_dir = if options.create_subdirectory {
        // Create subdirectory based on archive name without extension
        let parent_dir = path.parent()
            .ok_or_else(|| "Cannot determine parent directory".to_string())?;
        let stem = path.file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| "Cannot determine archive name".to_string())?;
        parent_dir.join(stem)
    } else {
        // Extract in place (same directory as archive)
        path.parent()
            .ok_or_else(|| "Cannot determine parent directory".to_string())?
            .to_path_buf()
    };
    
    // Create extraction directory if it doesn't exist
    if !extraction_dir.exists() {
        fs::create_dir_all(&extraction_dir)
            .map_err(|e| format!("Failed to create extraction directory: {}", e))?;
    }
    
    info!("Extracting {} archive: {} to {}", 
          archive_type.as_str(), 
          archive_path, 
          extraction_dir.display());
    
    // Extract based on archive type
    let files_extracted = match archive_type {
        ArchiveType::Zip => extract_zip_archive(archive_path, &extraction_dir, options.overwrite)?,
        ArchiveType::Rar => extract_rar_archive(archive_path, &extraction_dir, options.overwrite)?,
        ArchiveType::SevenZ => extract_7z_archive(archive_path, &extraction_dir, options.overwrite)?,
        ArchiveType::Tar => extract_tar_archive(archive_path, &extraction_dir, options.overwrite)?,
        ArchiveType::TarGz => extract_tar_gz_archive(archive_path, &extraction_dir, options.overwrite)?,
        ArchiveType::TarBz2 => extract_tar_bz2_archive(archive_path, &extraction_dir, options.overwrite)?,
        ArchiveType::TarXz => extract_tar_xz_archive(archive_path, &extraction_dir, options.overwrite)?,
        ArchiveType::Gz => extract_gz_archive(archive_path, &extraction_dir, options.overwrite)?,
        ArchiveType::Bz2 => extract_bz2_archive(archive_path, &extraction_dir, options.overwrite)?,
        ArchiveType::Xz => extract_xz_archive(archive_path, &extraction_dir, options.overwrite)?,
    };
    
    // Remove archive if requested
    if !options.preserve_archive {
        fs::remove_file(path)
            .map_err(|e| format!("Failed to remove archive after extraction: {}", e))?;
        info!("Removed archive after extraction: {}", archive_path);
    }
    
    Ok(ExtractionResult {
        extraction_path: extraction_dir,
        files_extracted,
        archive_type,
    })
}

/// Extract all archives in a directory (with support for nested archives)
pub fn extract_all_archives_in_directory(
    directory_path: &str,
    _options: ExtractionOptions,
) -> Result<Vec<ExtractionResult>, String> {
    // This now uses the recursive extraction system
    process_and_extract_archives(directory_path)?;
    
    // Return empty vec for backward compatibility
    // The actual extraction is handled by process_and_extract_archives
    Ok(Vec::new())
}

/// Recursively extract all archives in a directory, including nested archives
fn extract_all_archives_recursively(directory_path: &str, max_depth: u32) -> Result<(), String> {
    extract_nested_archives(Path::new(directory_path), 0, max_depth)
}

/// Extract nested archives recursively
fn extract_nested_archives(dir_path: &Path, current_depth: u32, max_depth: u32) -> Result<(), String> {
    if current_depth >= max_depth {
        warn!("Maximum extraction depth ({}) reached, stopping extraction", max_depth);
        return Ok(());
    }
    
    // Keep extracting until no more archives are found
    loop {
        let extraction_options = ExtractionOptions {
            create_subdirectory: false, // Extract in place for nested archives
            preserve_archive: false, // Remove archives after extraction to avoid re-processing
            overwrite: true,
        };
        
        // Find and extract all archives in the current directory
        let archives_found = extract_all_archives_in_single_pass(dir_path, &extraction_options)?;
        
        if archives_found == 0 {
            // No more archives found at this level
            break;
        }
        
        info!("Extracted {} archives at depth {}", archives_found, current_depth + 1);
    }
    
    // Now recursively process subdirectories
    for entry in fs::read_dir(dir_path)
        .map_err(|e| format!("Failed to read directory {:?}: {}", dir_path, e))? 
    {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
        let entry_path = entry.path();
        
        if entry_path.is_dir() {
            extract_nested_archives(&entry_path, current_depth + 1, max_depth)?;
        }
    }
    
    Ok(())
}

/// Extract all archives in a directory in a single pass, returning the count of archives extracted
fn extract_all_archives_in_single_pass(dir_path: &Path, options: &ExtractionOptions) -> Result<usize, String> {
    let mut archives_extracted = 0;
    let mut multi_part_rars = HashMap::new();
    
    // First, collect all files and group multi-part RARs
    for entry in fs::read_dir(dir_path)
        .map_err(|e| format!("Failed to read directory {:?}: {}", dir_path, e))? 
    {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
        let file_path = entry.path();
        
        if file_path.is_file() {
            if let Some(file_name) = file_path.file_name().and_then(|n| n.to_str()) {
                // Check for multi-part RAR patterns
                if let Some(base_name) = extract_rar_base_name(file_name) {
                    multi_part_rars.entry(base_name.to_string())
                        .or_insert_with(Vec::new)
                        .push(file_path);
                }
            }
        }
    }
    
    // Extract multi-part RARs (only the first part)
    for (base_name, parts) in multi_part_rars {
        if parts.len() > 1 {
            // Sort to ensure we get the first part
            let mut sorted_parts = parts;
            sorted_parts.sort();
            
            if let Some(first_part) = sorted_parts.first() {
                if is_first_rar_part(first_part) {
                    info!("Extracting multi-part RAR: {} ({} parts)", base_name, sorted_parts.len());
                    
                    match extract_archive(
                        first_part.to_str().unwrap_or(""),
                        options.clone()
                    ) {
                        Ok(_) => {
                            info!("Successfully extracted multi-part RAR: {}", base_name);
                            archives_extracted += 1;
                            
                            // Remove all parts if requested
                            if !options.preserve_archive {
                                for part in &sorted_parts {
                                    if let Err(e) = fs::remove_file(part) {
                                        warn!("Failed to remove RAR part {:?}: {}", part, e);
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Failed to extract multi-part RAR {}: {}", base_name, e);
                        }
                    }
                }
            }
        }
    }
    
    // Now extract all other archives
    for entry in fs::read_dir(dir_path)
        .map_err(|e| format!("Failed to read directory {:?}: {}", dir_path, e))? 
    {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
        let file_path = entry.path();
        
        if file_path.is_file() {
            // Skip if it's a multi-part RAR (already handled)
            if let Some(file_name) = file_path.file_name().and_then(|n| n.to_str()) {
                if extract_rar_base_name(file_name).is_some() && !is_first_rar_part(&file_path) {
                    continue;
                }
            }
            
            if let Some(_archive_type) = ArchiveType::from_path(&file_path) {
                match extract_archive(
                    file_path.to_str().unwrap_or(""),
                    options.clone()
                ) {
                    Ok(_) => {
                        info!("Successfully extracted: {}", file_path.display());
                        archives_extracted += 1;
                    }
                    Err(e) => {
                        warn!("Failed to extract {:?}: {}", file_path, e);
                    }
                }
            }
        }
    }
    
    Ok(archives_extracted)
}

/// Extract base name from multi-part RAR filename
fn extract_rar_base_name(filename: &str) -> Option<&str> {
    let lower = filename.to_lowercase();
    
    // Check for .partXXX.rar pattern
    if let Some(pos) = lower.find(".part") {
        if lower[pos..].starts_with(".part") && lower.ends_with(".rar") {
            return Some(&filename[..pos]);
        }
    }
    
    // Check for .rXX or .rar pattern
    if lower.ends_with(".rar") || 
       (lower.len() > 4 && &lower[lower.len()-4..lower.len()-2] == ".r" && lower[lower.len()-2..].chars().all(|c| c.is_ascii_digit())) {
        // Find the base name before the extension
        if let Some(pos) = filename.rfind('.') {
            return Some(&filename[..pos]);
        }
    }
    
    None
}

/// Check if a file is the first part of a multi-part RAR
fn is_first_rar_part(path: &Path) -> bool {
    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        let lower = name.to_lowercase();
        
        // Check for .part001.rar or .part01.rar or .part1.rar
        if lower.contains(".part001.rar") || lower.contains(".part01.rar") || lower.contains(".part1.rar") {
            return true;
        }
        
        // Check for .rar (first part) vs .r00, .r01, etc.
        if lower.ends_with(".rar") && !lower.contains(".part") {
            return true;
        }
        
        // Check for .r00 (some archives start with .r00 instead of .rar)
        if lower.ends_with(".r00") {
            return true;
        }
    }
    
    false
}

// Individual extraction functions for each archive type

fn extract_zip_archive(zip_path: &str, extract_dir: &Path, overwrite: bool) -> Result<usize, String> {
    let file = File::open(zip_path)
        .map_err(|e| format!("Failed to open ZIP file: {}", e))?;
    
    let mut archive = ZipArchive::new(file)
        .map_err(|e| format!("Failed to read ZIP archive: {}", e))?;
    
    let file_count = archive.len();
    
    for i in 0..file_count {
        let mut file = archive.by_index(i)
            .map_err(|e| format!("Failed to read file from ZIP: {}", e))?;
        
        let outpath = extract_dir.join(file.name());
        
        if file.name().ends_with('/') {
            // Create directory
            fs::create_dir_all(&outpath)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
        } else {
            // Extract file
            if let Some(p) = outpath.parent() {
                fs::create_dir_all(p)
                    .map_err(|e| format!("Failed to create parent directory: {}", e))?;
            }
            
            if outpath.exists() && !overwrite {
                warn!("Skipping existing file: {}", outpath.display());
                continue;
            }
            
            let mut outfile = File::create(&outpath)
                .map_err(|e| format!("Failed to create file: {}", e))?;
            
            copy(&mut file, &mut outfile)
                .map_err(|e| format!("Failed to extract file: {}", e))?;
        }
    }
    
    Ok(file_count)
}

fn extract_rar_archive(rar_path: &str, extract_dir: &Path, overwrite: bool) -> Result<usize, String> {
    let overwrite_flag = if overwrite { "-o+" } else { "-o-" };
    
    let output = Command::new("unrar")
        .args(&["x", overwrite_flag, rar_path, extract_dir.to_str().unwrap()])
        .output()
        .map_err(|e| format!("Failed to execute unrar command: {}", e))?;
    
    if !output.status.success() {
        return Err(format!(
            "Failed to extract RAR archive: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    
    // Count extracted files (approximate)
    let file_count = fs::read_dir(extract_dir)
        .map(|entries| entries.count())
        .unwrap_or(0);
    
    Ok(file_count)
}

fn extract_7z_archive(seven_z_path: &str, extract_dir: &Path, overwrite: bool) -> Result<usize, String> {
    let overwrite_flag = if overwrite { "-y" } else { "-aos" };
    
    let output = Command::new("7z")
        .args(&["x", seven_z_path, &format!("-o{}", extract_dir.to_str().unwrap()), overwrite_flag])
        .output()
        .map_err(|e| format!("Failed to execute 7z command: {}", e))?;
    
    if !output.status.success() {
        return Err(format!(
            "Failed to extract 7Z archive: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    
    // Count extracted files (approximate)
    let file_count = fs::read_dir(extract_dir)
        .map(|entries| entries.count())
        .unwrap_or(0);
    
    Ok(file_count)
}

fn extract_tar_archive(tar_path: &str, extract_dir: &Path, _overwrite: bool) -> Result<usize, String> {
    let output = Command::new("tar")
        .args(&["-xf", tar_path, "-C", extract_dir.to_str().unwrap()])
        .output()
        .map_err(|e| format!("Failed to execute tar command: {}", e))?;
    
    if !output.status.success() {
        return Err(format!(
            "Failed to extract TAR archive: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    
    // Count extracted files (approximate)
    let file_count = fs::read_dir(extract_dir)
        .map(|entries| entries.count())
        .unwrap_or(0);
    
    Ok(file_count)
}

fn extract_tar_gz_archive(tar_gz_path: &str, extract_dir: &Path, _overwrite: bool) -> Result<usize, String> {
    let output = Command::new("tar")
        .args(&["-xzf", tar_gz_path, "-C", extract_dir.to_str().unwrap()])
        .output()
        .map_err(|e| format!("Failed to execute tar command: {}", e))?;
    
    if !output.status.success() {
        return Err(format!(
            "Failed to extract TAR.GZ archive: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    
    // Count extracted files (approximate)
    let file_count = fs::read_dir(extract_dir)
        .map(|entries| entries.count())
        .unwrap_or(0);
    
    Ok(file_count)
}

fn extract_tar_bz2_archive(tar_bz2_path: &str, extract_dir: &Path, _overwrite: bool) -> Result<usize, String> {
    let output = Command::new("tar")
        .args(&["-xjf", tar_bz2_path, "-C", extract_dir.to_str().unwrap()])
        .output()
        .map_err(|e| format!("Failed to execute tar command: {}", e))?;
    
    if !output.status.success() {
        return Err(format!(
            "Failed to extract TAR.BZ2 archive: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    
    // Count extracted files (approximate)
    let file_count = fs::read_dir(extract_dir)
        .map(|entries| entries.count())
        .unwrap_or(0);
    
    Ok(file_count)
}

fn extract_tar_xz_archive(tar_xz_path: &str, extract_dir: &Path, _overwrite: bool) -> Result<usize, String> {
    let output = Command::new("tar")
        .args(&["-xJf", tar_xz_path, "-C", extract_dir.to_str().unwrap()])
        .output()
        .map_err(|e| format!("Failed to execute tar command: {}", e))?;
    
    if !output.status.success() {
        return Err(format!(
            "Failed to extract TAR.XZ archive: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    
    // Count extracted files (approximate)
    let file_count = fs::read_dir(extract_dir)
        .map(|entries| entries.count())
        .unwrap_or(0);
    
    Ok(file_count)
}

fn extract_gz_archive(gz_path: &str, extract_dir: &Path, overwrite: bool) -> Result<usize, String> {
    let path = Path::new(gz_path);
    let file_name = path.file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| "Cannot determine output filename".to_string())?;
    
    let output_path = extract_dir.join(file_name);
    
    if output_path.exists() && !overwrite {
        warn!("Skipping existing file: {}", output_path.display());
        return Ok(0);
    }
    
    let output = Command::new("gunzip")
        .args(&["-c", gz_path])
        .output()
        .map_err(|e| format!("Failed to execute gunzip command: {}", e))?;
    
    if !output.status.success() {
        return Err(format!(
            "Failed to extract GZ archive: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    
    fs::write(&output_path, &output.stdout)
        .map_err(|e| format!("Failed to write extracted file: {}", e))?;
    
    Ok(1)
}

fn extract_bz2_archive(bz2_path: &str, extract_dir: &Path, overwrite: bool) -> Result<usize, String> {
    let path = Path::new(bz2_path);
    let file_name = path.file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| "Cannot determine output filename".to_string())?;
    
    let output_path = extract_dir.join(file_name);
    
    if output_path.exists() && !overwrite {
        warn!("Skipping existing file: {}", output_path.display());
        return Ok(0);
    }
    
    let output = Command::new("bunzip2")
        .args(&["-c", bz2_path])
        .output()
        .map_err(|e| format!("Failed to execute bunzip2 command: {}", e))?;
    
    if !output.status.success() {
        return Err(format!(
            "Failed to extract BZ2 archive: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    
    fs::write(&output_path, &output.stdout)
        .map_err(|e| format!("Failed to write extracted file: {}", e))?;
    
    Ok(1)
}

fn extract_xz_archive(xz_path: &str, extract_dir: &Path, overwrite: bool) -> Result<usize, String> {
    let path = Path::new(xz_path);
    let file_name = path.file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| "Cannot determine output filename".to_string())?;
    
    let output_path = extract_dir.join(file_name);
    
    if output_path.exists() && !overwrite {
        warn!("Skipping existing file: {}", output_path.display());
        return Ok(0);
    }
    
    let output = Command::new("unxz")
        .args(&["-c", xz_path])
        .output()
        .map_err(|e| format!("Failed to execute unxz command: {}", e))?;
    
    if !output.status.success() {
        return Err(format!(
            "Failed to extract XZ archive: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    
    fs::write(&output_path, &output.stdout)
        .map_err(|e| format!("Failed to write extracted file: {}", e))?;
    
    Ok(1)
}