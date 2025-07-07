use crate::core::error::{Result, SeedError};
use log::info;
use std::fs;
use std::path::Path;
use std::process::Command;

/// Process and extract archives in a directory, returns the path to process
pub fn process_and_extract_archives(working_path: &str) -> Result<String> {
    // Extract any archives in the working path first
    if let Err(e) = extract_archives_in_directory(working_path) {
        // Log error but don't fail - we can still process without extraction
        info!("Archive extraction failed: {}", e);
    }

    // Return the working path for further processing
    Ok(working_path.to_string())
}

/// Extract RAR archives in a given directory
pub fn extract_rar_archives(folder_path: &str) -> Result<Option<String>> {
    info!("Checking for RAR archives in folder: {}", folder_path);

    let path = Path::new(folder_path);
    if !path.is_dir() {
        return Err(SeedError::Validation(format!(
            "Provided path is not a directory: {}",
            folder_path
        )));
    }

    // Collect all .rar, .r00, and .r01 files
    let mut rar_files = Vec::new();
    let mut r00_files = Vec::new();
    let mut r01_files = Vec::new();

    for entry in fs::read_dir(path).map_err(|e| SeedError::Io(e))? {
        let entry = entry.map_err(|e| SeedError::Io(e))?;
        let file_path = entry.path();
        if let Some(ext) = file_path.extension().and_then(|e| e.to_str()) {
            if ext.eq_ignore_ascii_case("rar") {
                rar_files.push(file_path.clone());
            } else if ext.eq_ignore_ascii_case("r00") {
                r00_files.push(file_path.clone());
            } else if ext.eq_ignore_ascii_case("r01") {
                r01_files.push(file_path.clone());
            }
        }
    }

    // Prefer .rar, then .r00, then .r01
    let to_extract = if !rar_files.is_empty() {
        rar_files
    } else if !r00_files.is_empty() {
        r00_files
    } else {
        r01_files
    };

    if to_extract.is_empty() {
        info!(
            "No RAR, R00, or R01 archives found in folder: {}",
            folder_path
        );
        return Ok(None); // No extraction occurred
    }

    info!("Found RAR/R00/R01 archives: {:?}", to_extract);

    for archive_file in to_extract {
        info!("Extracting archive: {}", archive_file.display());

        let output = Command::new("unrar")
            .args(&["x", "-o+", archive_file.to_str().unwrap(), folder_path])
            .output()
            .map_err(|e| SeedError::Other(format!("Failed to execute unrar command: {}", e)))?;

        if !output.status.success() {
            return Err(SeedError::Other(format!(
                "Failed to extract archive: {}. Error: {}",
                archive_file.display(),
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        info!("Successfully extracted archive: {}", archive_file.display());
    }

    info!(
        "Extraction completed. Extracted files are in: {}",
        folder_path
    );
    Ok(Some(folder_path.to_string()))
}

/// Extract archives (ZIP, TAR, and RAR) in the given directory
pub fn extract_archives_in_directory(working_dir: &str) -> Result<()> {
    // Process all archive types
    extract_zip_archives(working_dir)?;
    extract_tar_archives(working_dir)?;
    extract_rar_archives(working_dir)?;
    Ok(())
}

/// Extract ZIP archives in the given directory
pub fn extract_zip_archives(working_dir: &str) -> Result<()> {
    let zip_files: Vec<_> = fs::read_dir(working_dir)
        .map_err(|e| SeedError::Io(e))?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path
                .extension()
                .map_or(false, |ext| ext.eq_ignore_ascii_case("zip"))
            {
                Some(path)
            } else {
                None
            }
        })
        .collect();

    for zip_file in &zip_files {
        info!("Extracting ZIP archive: {}", zip_file.display());
        let output = Command::new("unzip")
            .arg("-o")
            .arg(zip_file)
            .arg("-d")
            .arg(working_dir)
            .output()
            .map_err(|e| SeedError::Other(format!("Failed to execute unzip: {}", e)))?;
        if !output.status.success() {
            return Err(SeedError::Other(format!(
                "Failed to extract ZIP archive: {}. Error: {}",
                zip_file.display(),
                String::from_utf8_lossy(&output.stderr)
            )));
        }
    }
    Ok(())
}

/// Extract TAR archives in the given directory
pub fn extract_tar_archives(working_dir: &str) -> Result<()> {
    let tar_files: Vec<_> = fs::read_dir(working_dir)
        .map_err(|e| SeedError::Io(e))?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            let file_name = path.to_string_lossy().to_lowercase();
            if file_name.ends_with(".tar.xz")
                || file_name.ends_with(".tar.gz")
                || file_name.ends_with(".tar.bz2")
                || file_name.ends_with(".tgz")
            {
                Some(path)
            } else {
                None
            }
        })
        .collect();

    for tar_file in &tar_files {
        info!("Extracting TAR archive: {}", tar_file.display());

        // Determine compression type
        let file_name = tar_file.to_string_lossy().to_lowercase();
        let compression_flag = if file_name.ends_with(".tar.xz") {
            "J"
        } else if file_name.ends_with(".tar.gz") || file_name.ends_with(".tgz") {
            "z"
        } else if file_name.ends_with(".tar.bz2") {
            "j"
        } else {
            ""
        };

        let output = Command::new("tar")
            .arg(&format!("-x{}f", compression_flag))
            .arg(tar_file)
            .arg("-C")
            .arg(working_dir)
            .output()
            .map_err(|e| SeedError::Other(format!("Failed to execute tar: {}", e)))?;

        if !output.status.success() {
            return Err(SeedError::Other(format!(
                "Failed to extract TAR archive: {}. Error: {}",
                tar_file.display(),
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        info!("Successfully extracted TAR archive: {}", tar_file.display());
    }
    Ok(())
}

/// Extract a single archive file to a specific destination directory
pub fn extract_single_archive(archive_path: &Path, dest_dir: &Path) -> Result<()> {
    if !dest_dir.exists() {
        fs::create_dir_all(dest_dir).map_err(|e| SeedError::Io(e))?;
    }

    let file_name = archive_path.to_string_lossy().to_lowercase();

    if file_name.ends_with(".zip") {
        info!("Extracting ZIP archive: {}", archive_path.display());
        let output = Command::new("unzip")
            .arg("-o")
            .arg(archive_path)
            .arg("-d")
            .arg(dest_dir)
            .output()
            .map_err(|e| SeedError::Other(format!("Failed to execute unzip: {}", e)))?;
        if !output.status.success() {
            return Err(SeedError::Other(format!(
                "Failed to extract ZIP archive: {}. Error: {}",
                archive_path.display(),
                String::from_utf8_lossy(&output.stderr)
            )));
        }
    } else if file_name.ends_with(".tar.xz")
        || file_name.ends_with(".tar.gz")
        || file_name.ends_with(".tar.bz2")
        || file_name.ends_with(".tgz")
    {
        info!("Extracting TAR archive: {}", archive_path.display());

        // Determine compression type
        let compression_flag = if file_name.ends_with(".tar.xz") {
            "J"
        } else if file_name.ends_with(".tar.gz") || file_name.ends_with(".tgz") {
            "z"
        } else if file_name.ends_with(".tar.bz2") {
            "j"
        } else {
            ""
        };

        let output = Command::new("tar")
            .arg(&format!("-x{}f", compression_flag))
            .arg(archive_path)
            .arg("-C")
            .arg(dest_dir)
            .output()
            .map_err(|e| SeedError::Other(format!("Failed to execute tar: {}", e)))?;

        if !output.status.success() {
            return Err(SeedError::Other(format!(
                "Failed to extract TAR archive: {}. Error: {}",
                archive_path.display(),
                String::from_utf8_lossy(&output.stderr)
            )));
        }
    } else if file_name.ends_with(".deb") {
        info!("Extracting DEB package: {}", archive_path.display());

        // Extract DEB package using dpkg-deb or ar
        let output = Command::new("dpkg-deb")
            .arg("-x")
            .arg(archive_path)
            .arg(dest_dir)
            .output()
            .map_err(|e| SeedError::Other(format!("Failed to execute dpkg-deb: {}", e)))?;

        if !output.status.success() {
            return Err(SeedError::Other(format!(
                "Failed to extract DEB package: {}. Error: {}",
                archive_path.display(),
                String::from_utf8_lossy(&output.stderr)
            )));
        }
    } else {
        return Err(SeedError::Other(format!(
            "Unsupported archive format: {}",
            archive_path.display()
        )));
    }

    info!("Successfully extracted archive: {}", archive_path.display());
    Ok(())
}
