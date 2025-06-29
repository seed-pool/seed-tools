use std::fs;
use std::path::Path;
use std::process::Command;
use walkdir::WalkDir;
use reqwest::blocking::multipart::Form;
use log::{info, warn, debug};
use epub::doc::EpubDoc;
use regex::Regex;

use crate::core::{Config, SeedpoolConfig};
use crate::core::{EbookType, EbookFile, MediaFile, MediaType, EbookCategory};
use crate::processing::torrent::create_torrent;
use crate::processing::extraction::process_and_extract_archives;
use crate::processing::naming::generate_release_name;
use urlencoding;



/// Metadata extracted from ebook filename and content
#[derive(Debug, Clone)]
pub struct EbookMetadata {
    pub title: String,
    pub author: Option<String>,
    pub year: Option<u32>,
    pub edition: Option<String>,
    pub volume: Option<String>,
    pub issue: Option<String>,
    pub category: EbookCategory,
    pub format_type: Option<EbookType>,
    pub series: Option<String>,
    pub publisher: Option<String>,
    pub isbn: Option<String>,
    pub language: Option<String>,
}

impl Default for EbookMetadata {
    fn default() -> Self {
        Self {
            title: String::new(),
            author: None,
            year: None,
            edition: None,
            volume: None,
            issue: None,
            category: EbookCategory::Unknown,
            format_type: None,
            series: None,
            publisher: None,
            isbn: None,
            language: None,
        }
    }
}



/// Find all ebook files in a directory
fn find_all_ebook_files(working_dir: &str) -> Result<Vec<EbookFile>, String> {
    let mut found_files = Vec::new();
    
    for entry in WalkDir::new(working_dir).max_depth(1) {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
        let path = entry.path();
        
        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if let Some(ebook_type) = EbookType::from_extension(ext) {
                    found_files.push(EbookFile {
                        path: path.to_path_buf(),
                        ebook_type,
                    });
                }
            }
        }
    }

    if found_files.is_empty() {
        return Err(format!("No supported ebook files (.epub, .pdf, .cbz, .cbr) found in directory '{}'", working_dir));
    }

    // Check if we have any comic files (CBR/CBZ)
    let comic_files: Vec<EbookFile> = found_files.iter()
        .filter(|f| f.ebook_type.is_comic())
        .cloned()
        .collect();

    if !comic_files.is_empty() {
        // If we have comic files, return ALL of them
        log::info!("Found {} comic file(s) in directory: {}", comic_files.len(), working_dir);
        Ok(comic_files)
    } else {
        // If no comic files, use the original priority system to return one file
        found_files.sort_by_key(|f| match f.ebook_type {
            EbookType::Epub => 0,
            EbookType::Cbz => 1,
            EbookType::Cbr => 2,
            EbookType::Pdf => 3,
            EbookType::Mobi | EbookType::Azw | EbookType::Azw3 | EbookType::Lit | EbookType::Pdb => 4,
        });
        Ok(vec![found_files.into_iter().next().unwrap()])
    }
}

/// Extract metadata (title, author) from an ebook file
fn extract_ebook_metadata(ebook_file: &EbookFile) -> Result<(Option<String>, Option<String>), String> {
    match ebook_file.ebook_type {
        EbookType::Pdf => extract_metadata_from_pdf(ebook_file.path.to_str().unwrap()),
        EbookType::Epub => extract_metadata_from_epub(ebook_file.path.to_str().unwrap()),
        EbookType::Cbz | EbookType::Cbr => extract_metadata_from_comic(&ebook_file.path),
        EbookType::Mobi | EbookType::Azw | EbookType::Azw3 | EbookType::Lit | EbookType::Pdb => {
            // For now, return generic metadata for these formats
            Ok((Some("Unknown Title".to_string()), Some("Unknown Author".to_string())))
        },
    }
}

/// Extract metadata from comic files (based on filename)
fn extract_metadata_from_comic(comic_path: &Path) -> Result<(Option<String>, Option<String>), String> {
    // Extract title from filename, remove common comic suffixes
    let filename = comic_path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown Comic")
        .to_string();
    
    // Clean up common comic filename patterns
    let title = filename
        .replace("_", " ")
        .replace("-", " ")
        .trim()
        .to_string();
    
    // For comics, author is typically not available from filename
    Ok((Some(title), None))
}

/// Extract images from comic archives (CBR/CBZ)
fn extract_comic_images(comic_file: &EbookFile, working_dir: &str) -> Result<String, String> {
    let comic_path = &comic_file.path;
    
    // Create a subfolder based on the comic file name
    let comic_name = comic_path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("comic");
    let extract_dir = Path::new(working_dir).join(comic_name);
    
    // Create the extraction directory if it doesn't exist
    std::fs::create_dir_all(&extract_dir)
        .map_err(|e| format!("Failed to create extraction directory '{}': {}", extract_dir.display(), e))?;
    
    match comic_file.ebook_type {
        EbookType::Cbz => {
            // Extract CBZ (ZIP) file to subfolder
            log::info!("Extracting CBZ file: {} to {}", comic_path.display(), extract_dir.display());
            let output = Command::new("unzip")
                .arg("-o") // Overwrite files
                .arg("-j") // Junk paths (extract to flat directory)
                .arg(comic_path)
                .arg("-d")
                .arg(&extract_dir)
                .output()
                .map_err(|e| format!("Failed to execute unzip: {}", e))?;
            
            if !output.status.success() {
                return Err(format!(
                    "Failed to extract CBZ file '{}': {}",
                    comic_path.display(),
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
        },
        EbookType::Cbr => {
            // Extract CBR (RAR) file to subfolder
            log::info!("Extracting CBR file: {} to {}", comic_path.display(), extract_dir.display());
            let output = Command::new("unrar")
                .arg("x") // Extract with paths
                .arg("-o+") // Overwrite files
                .arg(comic_path)
                .arg(&extract_dir)
                .output()
                .map_err(|e| format!("Failed to execute unrar: {}", e))?;
            
            if !output.status.success() {
                return Err(format!(
                    "Failed to extract CBR file '{}': {}",
                    comic_path.display(),
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
        },
        _ => return Err(format!("File '{}' is not a comic archive", comic_path.display())),
    }
    
    log::info!("Successfully extracted comic images to: {}", extract_dir.display());
    Ok(extract_dir.to_string_lossy().to_string())
}

/// Extract metadata from PDF files
fn extract_metadata_from_pdf(pdf_path: &str) -> Result<(Option<String>, Option<String>), String> {
    use lopdf::{Document, Object};

    let doc = Document::load(pdf_path).map_err(|e| format!("Failed to open PDF: {}", e))?;
    let info_obj = match doc.trailer.get(b"Info") {
        Ok(obj) => obj,
        Err(_) => return Ok((None, None)),
    };
    let info_ref = info_obj.as_reference().map_err(|e| format!("Failed to get Info reference: {}", e))?;
    let dict = doc.get_dictionary(info_ref).map_err(|e| format!("Failed to get PDF info dictionary: {}", e))?;

    fn get_pdf_string(dict: &lopdf::Dictionary, key: &[u8]) -> Option<String> {
        match dict.get(key) {
            Ok(Object::String(s, _)) => Some(String::from_utf8_lossy(s).to_string()),
            Ok(obj) => obj.as_str().ok().map(|s| String::from_utf8_lossy(s).to_string()),
            _ => None,
        }
    }

    let title = get_pdf_string(&dict, b"Title");
    let author = get_pdf_string(&dict, b"Author");
    Ok((title, author))
}

/// Extract metadata from EPUB files
fn extract_metadata_from_epub(epub_path: &str) -> Result<(Option<String>, Option<String>), String> {
    let epub = EpubDoc::new(epub_path)
        .map_err(|e| format!("Failed to open EPUB file '{}': {}", epub_path, e))?;

    // Extract title from metadata
    let title = epub.metadata.get("title").and_then(|titles| titles.get(0).cloned());

    // Extract author from metadata
    let author = epub.metadata.get("creator").and_then(|creators| creators.get(0).cloned());

    Ok((title, author))
}

/// Extract images from EPUB files
fn extract_epub_images(epub_path: &str, temp_dir: &std::path::Path) -> Result<Vec<std::path::PathBuf>, String> {
    use std::fs::File;
    use zip::ZipArchive;
    use std::io::copy;

    let file = File::open(epub_path).map_err(|e| format!("Failed to open EPUB: {}", e))?;
    let mut archive = ZipArchive::new(file).map_err(|e| format!("Failed to read EPUB as zip: {}", e))?;

    fs::create_dir_all(temp_dir).map_err(|e| format!("Failed to create temp dir: {}", e))?;

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
    Ok(images)
}

/// Generate ebook description with template support
pub fn generate_description_with_template(
    metadata: &serde_json::Value,
    enriched_metadata: Option<&std::collections::HashMap<String, String>>,
    template_name: Option<&str>,
) -> Result<String, String> {
    use crate::templates::TemplateProcessor;
    
    let template_processor = TemplateProcessor::with_defaults()
        .map_err(|e| format!("Failed to initialize template processor: {}", e))?;
    
    let template_to_use = template_name.unwrap_or("default");
    
    if let Some(template) = template_processor.get_template("ebook", template_to_use) {
        template_processor.apply_template(template, metadata, enriched_metadata)
    } else {
        // Fallback to traditional description generation
        generate_ebook_description_from_metadata(metadata)
    }
}

/// Generate ebook description from metadata (fallback for template system)
fn generate_ebook_description_from_metadata(metadata: &serde_json::Value) -> Result<String, String> {
    use crate::processing::description::{DescriptionBuilder, DescriptionConfig};
    use crate::core::{MediaType, EbookType, ImageLayout, SectionFormat, DescriptionComponent};
    
    let mut config = DescriptionConfig::default();
    config.image_layout = ImageLayout::TwoColumn;
    
    let mut builder = DescriptionBuilder::with_config(
        MediaType::Ebook(EbookType::Epub),
        config
    );
    
    // Add title
    if let Some(title) = metadata.get("title").and_then(|t| t.as_str()) {
        builder = builder.title(title);
    }
    
    // Add author
    if let Some(author) = metadata.get("author").and_then(|a| a.as_str()) {
        builder = builder.author(author);
    }
    
    // Add metadata table
    let mut metadata_rows = Vec::new();
    
    if let Some(format) = metadata.get("format").and_then(|f| f.as_str()) {
        metadata_rows.push(vec!["Format".to_string(), format.to_string()]);
    }
    
    if let Some(file_size) = metadata.get("file_size").and_then(|s| s.as_str()) {
        metadata_rows.push(vec!["Size".to_string(), file_size.to_string()]);
    }
    
    if let Some(page_count) = metadata.get("page_count") {
        metadata_rows.push(vec!["Pages".to_string(), page_count.to_string()]);
    }
    
    if !metadata_rows.is_empty() {
        builder = builder.add_component(DescriptionComponent::Table { rows: metadata_rows });
    }
    
    // Add description if available
    if let Some(description) = metadata.get("description").and_then(|d| d.as_str()) {
        builder = builder.custom_section("Description", description, SectionFormat::Plain);
    }
    
    // Add images if available
    if let Some(images) = metadata.get("images").and_then(|i| i.as_array()) {
        let image_urls: Vec<String> = images.iter()
            .filter_map(|v| v.as_str())
            .map(String::from)
            .collect();
        
        if !image_urls.is_empty() {
            builder = builder.images(image_urls);
        }
    }
    
    Ok(builder.build())
}


/// Generate description with images for ebook/comic uploads
pub fn generate_ebook_description(
    ebook_path: &str,
    torrent_name: &str,
    remote_path: &str,
    public_image_path: &str,
    dry_run: bool,
) -> Result<String, String> {
    use std::fs;
    use std::path::Path;

    let mut image_urls = Vec::new();
    
    // Determine file type from extension
    let path = Path::new(ebook_path);
    
    let extension = path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_lowercase())
        .unwrap_or_default();
    match extension.as_str() {
        "cbr" | "cbz" => {
            // For CBR/CBZ files, find and use existing extracted images
            let parent_dir = path.parent()
                .ok_or_else(|| "Cannot determine parent directory".to_string())?;
            
            // Look for the extraction subfolder based on comic file name
            let comic_name = path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("comic");
            let extract_dir = parent_dir.join(comic_name);
            
            // Use the extraction directory if it exists, otherwise fall back to parent directory
            let search_dir = if extract_dir.exists() && extract_dir.is_dir() {
                &extract_dir
            } else {
                parent_dir
            };
            
            // Look for extracted image files in the appropriate directory
            let mut image_files = Vec::new();
            for entry in fs::read_dir(search_dir)
                .map_err(|e| format!("Failed to read directory: {}", e))? {
                let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
                let file_path = entry.path();
                if file_path.is_file() {
                    if let Some(ext) = file_path.extension().and_then(|e| e.to_str()) {
                        let ext_lower = ext.to_lowercase();
                        if ext_lower == "jpg" || ext_lower == "jpeg" || ext_lower == "png" 
                            || ext_lower == "gif" || ext_lower == "bmp" || ext_lower == "webp" {
                            image_files.push(file_path);
                        }
                    }
                }
            }
            
            // Sort files by name to ensure consistent ordering
            image_files.sort();
            
            // Use up to 8 images (similar to the 3-10 page range for PDFs)
            let images_to_use = image_files.iter().take(8);
            
            for (index, image_file) in images_to_use.enumerate() {
                let image_name = format!("{}-image{}.jpg", torrent_name, index + 1);
                let temp_image_path = format!("{}/{}", std::env::temp_dir().to_string_lossy(), image_name);
                
                // Copy the extracted image to temp directory with standardized name
                fs::copy(image_file, &temp_image_path)
                    .map_err(|e| format!("Failed to copy image '{}': {}", image_file.display(), e))?;
                
                // Set permissions to 777
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    fs::set_permissions(&temp_image_path, fs::Permissions::from_mode(0o777))
                        .map_err(|e| format!("Failed to set permissions for '{}': {}", temp_image_path, e))?;
                }

                // SCP to CDN (skip during dry run)
                if !dry_run {
                    let scp_status = std::process::Command::new("scp")
                        .arg(&temp_image_path)
                        .arg(format!("{}/screenshots/", remote_path.trim_end_matches('/')))
                        .status()
                        .map_err(|e| format!("Failed to scp '{}': {}", temp_image_path, e))?;
                    if !scp_status.success() {
                        return Err(format!("Failed to scp '{}'", temp_image_path));
                    }
                }

                // Build public URL
                let cdn_url = format!("{}/{}", public_image_path.trim_end_matches('/'), image_name);
                image_urls.push(cdn_url);
            }
        }
        "pdf" => {
            // For PDF files, use GhostScript to extract pages 3-10
            for page in 3..=10 {
                let image_name = format!("{}-page{}.jpg", torrent_name, page);
                let image_path = format!("{}/{}", std::env::temp_dir().to_string_lossy(), image_name);

                // Extract page as JPEG using GhostScript
                let output = std::process::Command::new("gs")
                    .args(&[
                        "-dBATCH", "-dNOPAUSE",
                        "-sDEVICE=jpeg",
                        &format!("-dFirstPage={}", page),
                        &format!("-dLastPage={}", page),
                        "-r300", "-dJPEGQ=95",
                        &format!("-sOutputFile={}", image_path),
                        ebook_path,
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

                // Set permissions to 777
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    fs::set_permissions(&image_path, fs::Permissions::from_mode(0o777))
                        .map_err(|e| format!("Failed to set permissions for '{}': {}", image_path, e))?;
                }

                // SCP to CDN (skip during dry run)
                if !dry_run {
                    let scp_status = std::process::Command::new("scp")
                        .arg(&image_path)
                        .arg(format!("{}/screenshots/", remote_path.trim_end_matches('/')))
                        .status()
                        .map_err(|e| format!("Failed to scp '{}': {}", image_path, e))?;
                    if !scp_status.success() {
                        return Err(format!("Failed to scp '{}'", image_path));
                    }
                }

                // Build public URL
                let cdn_url = format!("{}/{}", public_image_path.trim_end_matches('/'), image_name);
                image_urls.push(cdn_url);
            }
        }
        _ => {
            return Err(format!("Unsupported file type for description generation: {}", extension));
        }
    }

    // Build BBCode description using DescriptionBuilder
    use crate::processing::description::{DescriptionBuilder, DescriptionConfig};
    use crate::core::{ImageLayout, MediaType, EbookType};
    
    let mut config = DescriptionConfig::default();
    config.image_layout = ImageLayout::TwoColumn;
    config.title_color = "#2E86C1".to_string();
    
    let builder = DescriptionBuilder::with_config(
        MediaType::Ebook(EbookType::Pdf), // Using PDF as generic for image-based ebooks
        config
    )
    .title(torrent_name)
    .images(image_urls);
    
    Ok(builder.build())
}

/// Generate BBCode description for ebooks using Open Library API
pub fn generate_ebook_bbcode_description(
    title: &str,
    author: &str,
    open_library_work_key: &str,
    open_library_author_key: &str,
    client: &reqwest::blocking::Client,
) -> Result<(String, Vec<String>), String> {
    use serde_json::Value;
    use crate::processing::description::DescriptionBuilder;
    use crate::core::{SectionFormat, MediaType, EbookType};
    
    let mut subjects = Vec::new();

    // Fetch book details from Open Library
    let work_url = format!("https://openlibrary.org/works/{}.json", open_library_work_key);
    let work_response = client
        .get(&work_url)
        .send()
        .map_err(|e| format!("Failed to fetch book details: {}", e))?;
    let work_json: Value = work_response
        .json()
        .map_err(|e| format!("Failed to parse book details: {}", e))?;

    // Extract subjects (categories) but do not add them to the description
    if let Some(subjects_array) = work_json["subjects"].as_array() {
        subjects = subjects_array
            .iter()
            .filter_map(|s| s.as_str().map(|s| s.to_string()))
            .collect();
    }

    // Fetch author details from Open Library
    let author_url = format!("https://openlibrary.org/authors/{}.json", open_library_author_key);
    let author_response = client
        .get(&author_url)
        .send()
        .map_err(|e| format!("Failed to fetch author details: {}", e))?;
    let author_json: Value = author_response
        .json()
        .map_err(|e| format!("Failed to parse author details: {}", e))?;

    // Start building description using DescriptionBuilder
    let final_title = work_json["title"].as_str().unwrap_or(title);
    let final_author = author_json["name"].as_str().unwrap_or(author);
    
    let mut builder = DescriptionBuilder::new(MediaType::Ebook(EbookType::Epub))
        .title(final_title)
        .author(final_author);

    // Add book description
    if let Some(book_description) = work_json["description"]
        .as_str()
        .or_else(|| work_json["description"]["value"].as_str())
    {
        // Detect and extract links from the description
        let link_regex = regex::Regex::new(r#"https?://[^\s\]]+"#).unwrap();
        let mut extracted_links = Vec::new();

        for capture in link_regex.captures_iter(book_description) {
            if let Some(link) = capture.get(0) {
                extracted_links.push(link.as_str().to_string());
            }
        }

        // Remove links and lines containing "Contain" or brackets "[]" from the description
        let sanitized_description: String = link_regex
            .replace_all(book_description, "")
            .to_string()
            .lines()
            .filter(|line| !line.contains("Contain") && !line.contains('[') && !line.contains(']'))
            .collect::<Vec<_>>()
            .join("\n");

        // Add synopsis
        builder = builder.synopsis(sanitized_description.trim());

        // Add extracted links as a custom section
        if !extracted_links.is_empty() {
            let links_content = extracted_links.iter()
                .map(|link| format!("[url={}]{}[/url]", 
                    link.trim_end_matches(')'), 
                    link.trim_end_matches(')')))
                .collect::<Vec<_>>()
                .join("\n");
            
            builder = builder.custom_section("Additional Editions", &links_content, SectionFormat::Plain);
        }
    }

    // Add author bio
    if let Some(author_bio) = author_json["bio"]
        .as_str()
        .or_else(|| author_json["bio"]["value"].as_str())
    {
        // Remove the "([Source][1])" line and trim extra blank lines
        let source_regex = regex::Regex::new(r"\(\[Source\]\[\d+\]\)").unwrap();
        let sanitized_bio = source_regex
            .replace_all(author_bio, "")
            .to_string()
            .lines()
            .filter(|line| !line.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n");

        builder = builder.custom_section("About the Author", sanitized_bio.trim(), SectionFormat::Quoted);
    }

    Ok((builder.build(), subjects))
}

/// Main function to process ebook uploads
pub fn process_ebook_upload(input_path: &str, config: &Config, seedpool_config: &SeedpoolConfig, category_arg: Option<&str>, dry_run: bool) -> Result<(), String> {
    use reqwest::blocking::Client;
    use std::fs;
    use crate::processing::torrent::add_torrent_to_all_qbittorrent_instances;

    // Validate input path
    let path = Path::new(input_path);
    let is_file = path.is_file();
    
    // Set working directory: if it's a file, use parent dir; if it's a dir, use the dir itself
    let working_dir = if is_file {
        path.parent()
            .ok_or_else(|| format!("Cannot determine parent directory for file: {}", input_path))?
            .to_string_lossy()
            .to_string()
    } else {
        input_path.to_string()
    };
    if !is_file && !path.is_dir() {
        return Err(format!("Input path '{}' is neither a file nor a directory.", working_dir));
    }

    // Store archive files that will be extracted and deleted for later restoration
    let backup_extracted_archive_buffers = Vec::new();
    
    // Extract any archives first using centralized extraction and get the processing path
    let processing_path = process_and_extract_archives(input_path).map_err(|e| format!("{:?}", e))?;
    
    // Update working_dir to use the processing path if it was a file that got extracted
    let working_dir = if is_file && Path::new(&processing_path).is_dir() {
        processing_path.clone()
    } else {
        working_dir
    };
    
    // Now find ebook files - if we started with a single file, only process that file and its extracted content
    let ebook_files = if is_file {
        // Update path to use the processing path
        let path = Path::new(&processing_path);
        
        // If we started with a single file, look for that specific file or any files extracted from it
        let original_filename = Path::new(input_path).file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");
        
        // Check if the original file still exists (if it wasn't an archive)
        let mut files = Vec::new();
        if path.exists() && path.is_file() {
            let ebook_type = path.extension()
                .and_then(|ext| ext.to_str())
                .and_then(|ext| EbookType::from_extension(ext))
                .ok_or_else(|| format!("Unsupported file type: {}", path.display()))?;
            files.push(EbookFile {
                path: path.to_path_buf(),
                ebook_type,
            });
        } else {
            // Original file was extracted, look for ebook files that came from it
            // Look for any ebook files in the working directory that match the original filename
            for entry in fs::read_dir(&working_dir).map_err(|e| format!("Failed to read directory '{}': {}", working_dir, e))? {
                let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
                let file_path = entry.path();
                
                if file_path.is_file() {
                    if let Some(ext) = file_path.extension().and_then(|e| e.to_str()) {
                        if let Some(ebook_type) = EbookType::from_extension(ext) {
                            // Check if this file relates to our original file by name
                            if let Some(filename) = file_path.file_stem().and_then(|s| s.to_str()) {
                                if filename.contains(original_filename) || original_filename.contains(filename) {
                                    files.push(EbookFile {
                                        path: file_path,
                                        ebook_type,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
        
        if files.is_empty() {
            return Err(format!("No ebook files found for input file: {}", path.display()));
        }
        files
    } else {
        // If we started with a directory, find all ebook files as before
        find_all_ebook_files(&working_dir)?
    };

    // Use the first file for metadata extraction and main processing
    let main_ebook_file = &ebook_files[0];

    // Extract metadata and cover
    let (title_opt, author_opt) = extract_ebook_metadata(&main_ebook_file)?;
    let mut title = title_opt.unwrap_or_else(|| "Unknown Title".to_string());
    let mut author = author_opt.unwrap_or_else(|| "Unknown Author".to_string());

    // Extract comic images if we have CBR/CBZ files
    let mut extracted_comic_dir = None;
    if main_ebook_file.ebook_type.is_comic() {
        // Extract images from all comic files and use the first extraction directory
        for ebook_file in &ebook_files {
            if ebook_file.ebook_type.is_comic() {
                let extracted_dir = extract_comic_images(&ebook_file, &working_dir)?;
                log::info!("Comic images extracted from {} to: {}", ebook_file.path.display(), extracted_dir);
                // Use the first extracted directory as the content path for torrent creation
                if extracted_comic_dir.is_none() {
                    extracted_comic_dir = Some(extracted_dir);
                }
            }
        }
    }

    // Sanitize the file name and rename the ebook file if needed
    let new_ebook_path = if main_ebook_file.ebook_type.needs_renaming() {
        let sanitized_author = {
            let parts: Vec<&str> = author.split_whitespace().collect();
            if parts.len() > 1 {
                format!("{}, {}", parts.last().unwrap(), parts[..parts.len() - 1].join(" "))
            } else {
                author.to_string()
            }
        };
        let sanitized_title = title
            .replace(".", " ")
            .replace(":", " ")
            .replace("'", "")
            .replace("/", " ")
            .replace("\\", " ")
            .replace("&", "and")
            .replace("?", "")
            .replace("*", "");
        let new_ext = "epub";
        let new_ebook_name = format!("{} - {}.{}", sanitized_author, sanitized_title, new_ext);
        let new_ebook_path = main_ebook_file.path.with_file_name(new_ebook_name);
        fs::rename(&main_ebook_file.path, &new_ebook_path)
            .map_err(|e| format!("Failed to rename ebook file: {}", e))?;
        new_ebook_path
    } else {
        main_ebook_file.path.clone() // Don't rename PDF, CBR, CBZ files
    };

    // Determine the content path for torrent creation (after any file renaming)
    let actual_content_path = if let Some(comic_dir) = extracted_comic_dir {
        // Use the extracted comic directory
        comic_dir
    } else if is_file {
        // If processing a single non-comic file, use the (possibly renamed) file path
        new_ebook_path.to_string_lossy().to_string()
    } else {
        // If processing a directory, use the directory path
        working_dir.clone()
    };

    // Clean up other ebook files except the selected one
    for entry in fs::read_dir(&working_dir).map_err(|e| format!("Failed to read directory '{}': {}", working_dir, e))? {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
        let path = entry.path();

        if matches!(main_ebook_file.ebook_type, EbookType::Pdf | EbookType::Cbr | EbookType::Cbz) {
            // Remove all .epub and .zip files, but keep the selected file
            if (path.extension().map(|ext| ext.eq_ignore_ascii_case("epub")).unwrap_or(false)
                || path.extension().map(|ext| ext.eq_ignore_ascii_case("zip")).unwrap_or(false))
                && path != new_ebook_path
            {
                fs::remove_file(&path)
                    .map_err(|e| format!("Failed to remove file '{}': {}", path.display(), e))?;
            }
            // Do NOT remove the PDF file at main_ebook_file.path (or new_ebook_path)
        } else {
            // For EPUBs: keep only the renamed epub, remove all other epubs
            if path.extension().map(|ext| ext.eq_ignore_ascii_case("epub")).unwrap_or(false)
                && path != new_ebook_path
            {
                fs::remove_file(&path)
                    .map_err(|e| format!("Failed to remove extra epub file '{}': {}", path.display(), e))?;
            }
            // Keep all ZIPs for EPUBs
        }
    }

    // Generate comic description before removing CBR/CBZ files (or for PDF magazines/comics)
    let base_name = Path::new(&actual_content_path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    
    let lower_base = base_name.to_lowercase();
    let type_id = if let Some("0700") = category_arg {
        // 0700 = automatic detection based on file type and filename
        info!("Using automatic type detection (0700) for file: {}", main_ebook_file.path.display());
        let detected_type = match main_ebook_file.ebook_type {
            EbookType::Cbr | EbookType::Cbz => {
                // CBR/CBZ files are comics - check filename for magazine vs comic
                if lower_base.contains("magazine") {
                    info!("Detected type: Magazine (41) - CBR/CBZ file with 'magazine' in filename");
                    "41" // Magazine
                } else {
                    info!("Detected type: Comic (40) - CBR/CBZ file");
                    "40" // Comic
                }
            },
            EbookType::Pdf => {
                // PDF files - check filename content to determine type
                if lower_base.contains("magazine") {
                    info!("Detected type: Magazine (41) - PDF with 'magazine' in filename");
                    "41" // Magazine
                } else if lower_base.contains("newspaper") || lower_base.contains("news") {
                    info!("Detected type: Newspaper (42) - PDF with 'newspaper'/'news' in filename");
                    "42" // Newspaper  
                } else if lower_base.contains("comic") {
                    info!("Detected type: Comic (40) - PDF with 'comic' in filename");
                    "40" // Comic
                } else {
                    info!("Detected type: Regular ebook (20) - PDF file");
                    "20" // Regular ebook/PDF
                }
            },
            EbookType::Epub => {
                // EPUB files - check filename content to determine if it's a magazine
                if lower_base.contains("magazine") {
                    info!("Detected type: Magazine (41) - EPUB with 'magazine' in filename");
                    "41" // Magazine
                } else if lower_base.contains("newspaper") || lower_base.contains("news") {
                    info!("Detected type: Newspaper (42) - EPUB with 'newspaper'/'news' in filename");
                    "42" // Newspaper
                } else {
                    info!("Detected type: Regular ebook (20) - EPUB file");
                    "20" // Regular ebook
                }
            },
            EbookType::Mobi | EbookType::Azw | EbookType::Azw3 | EbookType::Lit | EbookType::Pdb => {
                // Other ebook formats default to regular ebook
                info!("Detected type: Regular ebook (20) - {:?} file", main_ebook_file.ebook_type);
                "20" // Regular ebook
            }
        };
        detected_type
    } else if let Some(cat_arg) = category_arg {
        // Extract type from specific codes like 0720, 0740, 0741
        let forced_type = match cat_arg {
            "0720" => {
                info!("Forced type: Regular ebook (20) - Category code 0720");
                "20" // Regular ebook
            },
            "0740" => {
                info!("Forced type: Comic (40) - Category code 0740");
                "40" // Comic
            },
            "0741" => {
                info!("Forced type: Magazine (41) - Category code 0741");
                "41" // Magazine
            },
            _ => {
                info!("Unknown category code '{}', defaulting to Regular ebook (20)", cat_arg);
                "20" // Default to regular ebook
            }
        };
        forced_type
    } else {
        // Fallback to automatic detection (same as 0700)
        info!("No category specified, using automatic type detection for file: {}", main_ebook_file.path.display());
        let detected_type = match main_ebook_file.ebook_type {
            EbookType::Cbr | EbookType::Cbz => {
                if lower_base.contains("magazine") {
                    info!("Detected type: Magazine (41) - CBR/CBZ file with 'magazine' in filename");
                    "41"
                } else {
                    info!("Detected type: Comic (40) - CBR/CBZ file");
                    "40"
                }
            },
            EbookType::Pdf => {
                if lower_base.contains("magazine") {
                    info!("Detected type: Magazine (41) - PDF with 'magazine' in filename");
                    "41"
                } else if lower_base.contains("newspaper") || lower_base.contains("news") {
                    info!("Detected type: Newspaper (42) - PDF with 'newspaper'/'news' in filename");
                    "42"
                } else if lower_base.contains("comic") {
                    info!("Detected type: Comic (40) - PDF with 'comic' in filename");
                    "40"
                } else {
                    info!("Detected type: Regular ebook (20) - PDF file");
                    "20"
                }
            },
            EbookType::Epub => {
                if lower_base.contains("magazine") {
                    info!("Detected type: Magazine (41) - EPUB with 'magazine' in filename");
                    "41"
                } else if lower_base.contains("newspaper") || lower_base.contains("news") {
                    info!("Detected type: Newspaper (42) - EPUB with 'newspaper'/'news' in filename");
                    "42"
                } else {
                    info!("Detected type: Regular ebook (20) - EPUB file");
                    "20"
                }
            },
            EbookType::Mobi | EbookType::Azw | EbookType::Azw3 | EbookType::Lit | EbookType::Pdb => {
                info!("Detected type: Regular ebook (20) - {:?} file", main_ebook_file.ebook_type);
                "20"
            }
        };
        detected_type
    };

    let (mut description, mut keywords, mut cover_id) = if main_ebook_file.ebook_type.is_comic() || (matches!(main_ebook_file.ebook_type, EbookType::Pdf) && (type_id == "40" || type_id == "41")) {
        let torrent_name = generate_release_name(&base_name);
        let desc = generate_ebook_description(
            new_ebook_path.to_str().unwrap(),
            &torrent_name,
            &seedpool_config.screenshots.remote_path,
            &seedpool_config.screenshots.image_path,
            dry_run,
        )?;
        let keywords = if type_id == "41" { "magazine".to_string() } else { "comic".to_string() };
        (desc, keywords, None)
    } else {
        (String::new(), String::new(), None)
    };

    // Store original CBR/CBZ files in memory buffers, then remove from disk for torrent creation
    let mut backup_archive_buffers = Vec::new();
    for ebook_file in &ebook_files {
        if ebook_file.ebook_type.is_comic() {
            log::info!("Reading comic archive into memory buffer: {}", ebook_file.path.display());
            let file_content = fs::read(&ebook_file.path)
                .map_err(|e| format!("Failed to read comic file '{}' into memory: {}", ebook_file.path.display(), e))?;
            backup_archive_buffers.push((ebook_file.path.clone(), file_content));
            
            log::info!("Temporarily removing comic archive from disk: {}", ebook_file.path.display());
            fs::remove_file(&ebook_file.path)
                .map_err(|e| format!("Failed to remove original comic file '{}': {}", ebook_file.path.display(), e))?;
        }
    }

    // Create a restore guard to ensure files are always restored, even on error
    struct RestoreGuard {
        comic_backup_buffers: Vec<(std::path::PathBuf, Vec<u8>)>,
        archive_backup_buffers: Vec<(std::path::PathBuf, Vec<u8>)>,
    }
    
    impl Drop for RestoreGuard {
        fn drop(&mut self) {
            // Restore comic files
            for (original_path, file_content) in &self.comic_backup_buffers {
                if let Err(e) = fs::write(original_path, file_content) {
                    log::error!("Failed to restore comic file '{}' during cleanup: {}", original_path.display(), e);
                } else {
                    log::info!("Restored comic file from memory during cleanup: {}", original_path.display());
                }
            }
            // Restore extracted archive files  
            for (original_path, file_content) in &self.archive_backup_buffers {
                if let Err(e) = fs::write(original_path, file_content) {
                    log::error!("Failed to restore archive file '{}' during cleanup: {}", original_path.display(), e);
                } else {
                    log::info!("Restored archive file from memory during cleanup: {}", original_path.display());
                }
            }
        }
    }
    
    let _restore_guard = RestoreGuard {
        comic_backup_buffers: backup_archive_buffers.clone(),
        archive_backup_buffers: backup_extracted_archive_buffers.clone(),
    };

    let torrent_input = &actual_content_path;
    let torrent_file = create_torrent(
        torrent_input,
        &config.paths.torrent_dir,
        &seedpool_config.settings.announce_url,
        &config.paths.mkbrr,
        false, // Don't exclude image files for ebook/comic processing
    )?;


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

    // --- SKIP OPEN LIBRARY FOR COMICS & MAGAZINES ---
    if !main_ebook_file.ebook_type.is_comic() && !(matches!(main_ebook_file.ebook_type, EbookType::Pdf) && (type_id == "40" || type_id == "41")) {
        // --- ORIGINAL OPEN LIBRARY LOOKUP AND DESCRIPTION LOGIC ---
        let mut subjects = Vec::new();
        let mut desc = format!(
            "[center][b][size=32][color=#2E86C1]{}[/color][/size][/b]\n\
            [b][size=16][color=#117A65]By:[/color][/size][/b] [i]{}[/i][/center]\n\n\
            [b][size=15][color=#6C3483]Synopsis:[/color][/size][/b]\n\
            [quote]No metadata available.[/quote]\n\n\
            [center]{}[/center]",
            title,
            author,
            default_non_video_description()
        );

        // Only try Open Library if we have at least a title or author
        if title != "Unknown Title" || author != "Unknown Author" {
            let query = format!(
                "https://openlibrary.org/search.json?title={}&author={}",
                urlencoding::encode(&title),
                urlencoding::encode(&author)
            );

            info!("Querying Open Library API: {}", query);

            let client = Client::new();
            let response = client
                .get(&query)
                .send()
                .map_err(|e| format!("Failed to query Open Library API: {}", e))?;

            if response.status().is_success() {
                let json: serde_json::Value = response
                    .json()
                    .map_err(|e| format!("Failed to parse Open Library API response: {}", e))?;

                if let Some(first_result) = json["docs"].as_array().and_then(|docs| docs.get(0)) {
                    // Use Open Library's title and author if available
                    let ol_title = first_result["title"]
                        .as_str()
                        .unwrap_or(&title)
                        .to_string();
                    let ol_author = first_result["author_name"]
                        .as_array()
                        .and_then(|authors| authors.get(0))
                        .and_then(|author| author.as_str())
                        .unwrap_or(&author)
                        .to_string();

                    info!("Using title: '{}' and author: '{}'", ol_title, ol_author);

                    // Update title and author with Open Library values
                    title = ol_title;
                    author = ol_author;

                    // Extract Open Library work and author keys
                    let open_library_work_key = first_result["key"]
                        .as_str()
                        .unwrap_or("")
                        .trim_start_matches("/works/")
                        .to_string();
                    let open_library_author_key = first_result["author_key"]
                        .as_array()
                        .and_then(|keys| keys.get(0))
                        .and_then(|key| key.as_str())
                        .unwrap_or("")
                        .to_string();

                    // Extract cover ID
                    cover_id = first_result["cover_i"].as_u64();

                    // Generate the BBCode description and fetch subjects
                    let (desc2, subj) = generate_ebook_bbcode_description(
                        &title,
                        &author,
                        &open_library_work_key,
                        &open_library_author_key,
                        &client,
                    )?;
                    desc = desc2;
                    subjects = subj;
                }
            }
        }
        description = desc;
        keywords = subjects.join(", ");
    }

    info!("Processing eBook upload for title: '{}' and author: '{}'", title, author);

    // If PDF, extract cover image from first page using Ghostscript
    let mut pdf_cover_image_path = None;
    if matches!(main_ebook_file.ebook_type, EbookType::Pdf) {
        let cover_path = format!("{}.cover.jpg", new_ebook_path.to_str().unwrap());
        let output = std::process::Command::new("gs")
            .args(&[
                "-dBATCH", "-dNOPAUSE",
                "-sDEVICE=jpeg",
                "-dFirstPage=1", "-dLastPage=1",
                "-r150", "-dJPEGQ=95",
                &format!("-sOutputFile={}", cover_path),
                new_ebook_path.to_str().unwrap(),
            ])
            .output()
            .map_err(|e| format!("Failed to run gs: {}", e))?;
        if !output.status.success() {
            return Err(format!(
                "Failed to extract cover from PDF with gs: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        pdf_cover_image_path = Some(cover_path);
    }

    let mut form = Form::new()
        .file("torrent", &torrent_file)
        .map_err(|e| format!("Failed to attach torrent file: {}", e))?
        .text("name", base_name.clone())
        .text("category_id", "7") // eBooks category
        .text("type_id", type_id)
        .text("tmdb", "0")
        .text("imdb", "0")
        .text("tvdb", "0")
        .text("anonymous", "0")
        .text("description", description)
        .text("keywords", keywords)
        .text("mal", "0")
        .text("igdb", "0")
        .text("stream", "0")
        .text("sd", "0");

    if let Some(nfo) = nfo_file {
        form = form.file("nfo", nfo).map_err(|e| format!("Failed to attach NFO file: {}", e))?;
    }

    // Send the upload request
    let client = Client::new();
    
    if dry_run {
        info!("[DRY RUN] Would upload eBook to Seedpool: {}", seedpool_config.settings.upload_url);
        info!("[DRY RUN] Form data would include: torrent file, description, category, type, etc.");
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
    let torrent_id = crate::utils::extract_torrent_id(&response_text).map_err(|e| format!("{:?}", e))?;

    // --- COVER HANDLING ---

    // For EPUBs: Fetch the cover image using the cover ID from Open Library (existing logic)
    if !matches!(main_ebook_file.ebook_type, EbookType::Pdf){
        let mut cover_handled = false;
        if let Some(cover_id) = cover_id {
            let cover_url = format!("https://covers.openlibrary.org/b/id/{}-L.jpg", cover_id);
            info!("Fetching cover image from: {}", cover_url);

            let cover_response = client
                .get(&cover_url)
                .send()
                .map_err(|e| format!("Failed to fetch cover image: {}", e))?;

            if cover_response.status().is_success() {
                // Save the cover image locally
                let cover_path = new_ebook_path.with_extension("jpg");
                std::fs::write(&cover_path, cover_response.bytes().map_err(|e| format!("Failed to read cover image bytes: {}", e))?)
                    .map_err(|e| format!("Failed to save cover image: {}", e))?;

                info!("Saved cover image to: {}", cover_path.display());

                // Rename the cover image to include the torrent ID
                let renamed_cover_path = cover_path.with_file_name(format!("torrent-cover_{}.jpg", torrent_id));
                std::fs::rename(&cover_path, &renamed_cover_path)
                    .map_err(|e| format!("Failed to rename cover image: {}", e))?;

                // Set permissions to 777 for the renamed cover image
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;

                    info!("Setting permissions to 777 for cover image: {}", renamed_cover_path.display());
                    fs::set_permissions(&renamed_cover_path, fs::Permissions::from_mode(0o777))
                        .map_err(|e| format!("Failed to set permissions for cover image '{}': {}", renamed_cover_path.display(), e))?;
                    info!("Successfully set permissions to 777 for cover image: {}", renamed_cover_path.display());
                }

                // Upload the cover image to the CDN using SCP (skip during dry run)
                let remote_covers_path = format!(
                    "{}/covers",
                    seedpool_config.screenshots.remote_path.trim_end_matches('/')
                );
                if !dry_run {
                    let scp_command = std::process::Command::new("scp")
                        .arg(&renamed_cover_path)
                        .arg(&remote_covers_path)
                        .output()
                        .map_err(|e| format!("Failed to upload cover image via SCP: {}", e))?;

                    if !scp_command.status.success() {
                        return Err(format!(
                            "Failed to upload cover image via SCP. Error: {}",
                            String::from_utf8_lossy(&scp_command.stderr)
                        ));
                    }
                }

                info!("Successfully uploaded cover image to CDN: {}", remote_covers_path);
                cover_handled = true;
            } else {
                warn!("Failed to fetch cover image with status: {}. Skipping cover image fetch.", cover_response.status());
            }
        }
        // If no cover was handled, extract first image from EPUB as cover using Rust
        if !cover_handled {
            info!("No Open Library cover found, extracting first image from EPUB as cover.");
            let temp_dir = std::env::temp_dir().join(format!("{}_cover_extract", base_name));
            let page_images = extract_epub_images(new_ebook_path.to_str().unwrap(), &temp_dir)?;
            if let Some(cover_img) = page_images.get(0) {
                let renamed_cover_path = temp_dir.join(format!("torrent-cover_{}.jpg", torrent_id));
                fs::copy(&cover_img, &renamed_cover_path)
                    .map_err(|e| format!("Failed to copy extracted cover image: {}", e))?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    fs::set_permissions(&renamed_cover_path, fs::Permissions::from_mode(0o777))
                        .map_err(|e| format!("Failed to set permissions for cover image '{}': {}", renamed_cover_path.display(), e))?;
                }
                let remote_covers_path = format!(
                    "{}/covers",
                    seedpool_config.screenshots.remote_path.trim_end_matches('/')
                );
                // SCP to CDN (skip during dry run)
                if !dry_run {
                    let scp_command = std::process::Command::new("scp")
                        .arg(&renamed_cover_path)
                        .arg(&remote_covers_path)
                        .output()
                        .map_err(|e| format!("Failed to upload extracted cover image via SCP: {}", e))?;
                    if !scp_command.status.success() {
                        return Err(format!(
                            "Failed to upload extracted cover image via SCP. Error: {}",
                            String::from_utf8_lossy(&scp_command.stderr)
                        ));
                    }
                }
                info!("Successfully uploaded extracted EPUB cover image to CDN: {}", remote_covers_path);
            } else {
                warn!("No images found to use as cover from EPUB.");
            }
        }
    }

    // For PDFs: Upload the extracted cover image (if any)
    if matches!(main_ebook_file.ebook_type, EbookType::Pdf) {
        if let Some(cover_path) = pdf_cover_image_path {
            // Rename the cover image to include the torrent ID
            let renamed_cover_path = Path::new(&cover_path)
                .with_file_name(format!("torrent-cover_{}.jpg", torrent_id));
            std::fs::rename(&cover_path, &renamed_cover_path)
                .map_err(|e| format!("Failed to rename PDF cover image: {}", e))?;

            // Set permissions to 777 for the renamed cover image
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                info!("Setting permissions to 777 for cover image: {}", renamed_cover_path.display());
                std::fs::set_permissions(&renamed_cover_path, std::fs::Permissions::from_mode(0o777))
                    .map_err(|e| format!("Failed to set permissions for cover image '{}': {}", renamed_cover_path.display(), e))?;
                info!("Successfully set permissions to 777 for cover image: {}", renamed_cover_path.display());
            }

            info!("Preparing to upload extracted PDF cover image: {}", renamed_cover_path.display());
            let remote_covers_path = format!(
                "{}/covers",
                seedpool_config.screenshots.remote_path.trim_end_matches('/')
            );
            // SCP to CDN (skip during dry run)
            if !dry_run {
                let scp_command = std::process::Command::new("scp")
                    .arg(&renamed_cover_path)
                    .arg(&remote_covers_path)
                    .output()
                    .map_err(|e| format!("Failed to upload cover image via SCP: {}", e))?;

                if !scp_command.status.success() {
                    return Err(format!(
                        "Failed to upload cover image via SCP. Error: {}",
                        String::from_utf8_lossy(&scp_command.stderr)
                    ));
                }
            }
            info!("Successfully uploaded cover image to CDN: {}", remote_covers_path);
        }
    }

    // Add torrent to all qBittorrent instances
    add_torrent_to_all_qbittorrent_instances(
        &[torrent_file.clone()],
        &config.qbittorrent,
        &config.deluge,
        new_ebook_path.to_str().unwrap(),
        &config.paths,
    )?;

    // Files will be automatically restored by the RestoreGuard when function exits

    Ok(())
}

/// Process ebook file(s) from a path (file or directory) and classify content
pub fn process_ebook(
    input_path: &str,
    _config: &crate::core::Config,
    _dry_run: bool,
) -> Result<Vec<(EbookFile, EbookMetadata)>, String> {
    let path = Path::new(input_path);
    
    if !path.exists() {
        return Err(format!("Path not found: {}", input_path));
    }
    
    // Extract any archives first and get the path to process
    let processing_path = process_and_extract_archives(input_path).map_err(|e| format!("{:?}", e))?;
    
    let mut results = Vec::new();
    let mut rejected_files = Vec::new();
    
    // Update path to use the processing path
    let path = Path::new(&processing_path);
    
    if path.is_file() {
        // Single file case (non-archive ebook file)
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .ok_or_else(|| "Could not determine file extension".to_string())?;

        let ebook_type = EbookType::from_extension(extension)
            .ok_or_else(|| format!("Unsupported ebook file type: {}", extension))?;
        
        let ebook_file = EbookFile {
            path: path.to_path_buf(),
            ebook_type,
        };
        
        let filename = path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        
        let metadata = classify_ebook_content(filename, extension);
        
        if metadata.category == EbookCategory::Unknown {
            return Err(format!(
                "Unable to determine ebook category for '{}'. File must have recognizable novel, comic, magazine, technical, educational, or manga patterns in the filename.", 
                filename
            ));
        }
        
        results.push((ebook_file, metadata));
        
    } else if path.is_dir() {
        // Handle directory - process all ebook files (including extracted ones)
        for entry in fs::read_dir(path).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let entry_path = entry.path();
            
            if entry_path.is_file() {
                if let Some(extension) = entry_path.extension().and_then(|ext| ext.to_str()) {
                    if let Some(ebook_type) = EbookType::from_extension(extension) {
                        let ebook_file = EbookFile {
                            path: entry_path.clone(),
                            ebook_type,
                        };
                        
                        let filename = entry_path.file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or("");
                        
                        let metadata = classify_ebook_content(filename, extension);
                        
                        if metadata.category == EbookCategory::Unknown {
                            rejected_files.push(filename.to_string());
                            continue;
                        }
                        
                        results.push((ebook_file, metadata));
                    }
                }
            }
        }
        
        if results.is_empty() && !rejected_files.is_empty() {
            return Err(format!(
                "No valid ebook files found. {} file(s) rejected due to unknown category: {}",
                rejected_files.len(),
                rejected_files.join(", ")
            ));
        }
    }
    
    if results.is_empty() {
        return Err("No ebook files found in the specified path".to_string());
    }
    
    // After we have the results, build the upload data if we have ebook files
    if !results.is_empty() {
        use crate::processing::upload::UploadBuilder;
        use std::sync::Arc;
        
        let (ebook_file, metadata) = &results[0];
        
        // Build upload data directly using UploadBuilder
        // TODO: Uncomment when description module is available
        // use crate::description::DescriptionConfig;
        use crate::core::ImageLayout;
        
        // Configure description based on ebook type
        // TODO: Uncomment when description module is available
        // let mut desc_config = DescriptionConfig::default();
        
        // // Different layouts for different ebook types
        // match metadata.category {
        //     EbookCategory::Comic => {
        //         desc_config.image_layout = ImageLayout::TwoColumn; // Comics use 2 column for preview pages
        //         desc_config.max_images = 10; // Show more preview pages
        //         desc_config.image_width = 350; // Smaller width for comic pages
        //     }
        //     EbookCategory::Magazine | EbookCategory::Newspaper => {
        //         desc_config.image_layout = ImageLayout::TwoColumn; // Magazines/newspapers use 2 column
        //         desc_config.max_images = 6; // Show several pages
        //         desc_config.image_width = 400;
        //     }
        //     _ => {
        //         desc_config.image_layout = ImageLayout::SingleColumn; // Regular books use single column for cover
        //         desc_config.max_images = 2; // Front and back cover
        //         desc_config.image_width = 500;
        //     }
        // }
        
        // Create the upload builder with ebook-specific components
        let mut builder = UploadBuilder::new(
            &processing_path,
            MediaType::Ebook(ebook_file.ebook_type.clone()),
            Arc::new((*_config).clone())
        )
        .with_extensions(EbookType::all_extensions())
        // TODO: Uncomment when description module is available
        // .with_description_config(desc_config)
        .dry_run(_dry_run);
        
        // Add title info
        builder = builder.with_title_info(
            &metadata.title, 
            metadata.year.map(|y| y.to_string()).as_deref()
        );
        
        // Add ebook-specific metadata
        let mut ebook_metadata = std::collections::HashMap::new();
        if let Some(author) = &metadata.author {
            ebook_metadata.insert("author".to_string(), author.clone());
        }
        if let Some(publisher) = &metadata.publisher {
            ebook_metadata.insert("publisher".to_string(), publisher.clone());
        }
        if let Some(isbn) = &metadata.isbn {
            ebook_metadata.insert("isbn".to_string(), isbn.clone());
        }
        if let Some(edition) = &metadata.edition {
            ebook_metadata.insert("edition".to_string(), edition.clone());
        }
        if let Some(volume) = &metadata.volume {
            ebook_metadata.insert("volume".to_string(), volume.clone());
        }
        if let Some(issue) = &metadata.issue {
            ebook_metadata.insert("issue".to_string(), issue.clone());
        }
        if let Some(series) = &metadata.series {
            ebook_metadata.insert("series".to_string(), series.clone());
        }
        if let Some(language) = &metadata.language {
            ebook_metadata.insert("language".to_string(), language.clone());
        }
        ebook_metadata.insert("category".to_string(), format!("{:?}", metadata.category));
        ebook_metadata.insert("format".to_string(), format!("{:?}", ebook_file.ebook_type));
        
        builder = builder
            .with_nfo()
            .with_mediainfo()
            .with_duplicate_check()
            .with_custom_component("ebook_metadata", crate::core::UploadComponent::Metadata(ebook_metadata));
        
        // Add screenshots for comics/magazines (extract preview pages)
        if matches!(metadata.category, EbookCategory::Comic | EbookCategory::Magazine | EbookCategory::Newspaper) {
            builder = builder.with_screenshots(6); // Extract 6 preview pages
        }
        
        // For comics, also handle comic image extraction if needed
        if ebook_file.ebook_type.is_comic() {
            // TODO: Extract comic images for preview
            // This would be handled by the upload builder's screenshot component
        }
        
        let _upload_data = builder.build()?;
        
        info!("Built upload data for ebook processing");
        
        // Create the upload processor - it will auto-detect the active tracker
        let mut processor = crate::processing::upload::UploadProcessor::new(
            _upload_data,
            std::sync::Arc::new(_config.clone()),
        )
        .dry_run(_dry_run);
        
        // Get media classification for mapping
        if !results.is_empty() {
            let (_, metadata) = &results[0];
            let category_str = format!("EbookCategory::{:?}", metadata.category);
            
            processor = processor.with_media_classification(
                Some(category_str),
                None, // Ebooks don't have source types
            );
        }
        
        // Process the upload - it handles tracker detection and mapping internally
        let upload_result = processor.process()?;
        
        if upload_result.success {
            info!("Upload completed successfully to {}", upload_result.tracker);
            if let Some(torrent_id) = upload_result.torrent_id {
                info!("Torrent ID: {}", torrent_id);
            }
        } else {
            warn!("Upload failed: {}", upload_result.message);
        }
    }
    
    Ok(results)
}

/// Detect ebook files in a path (without metadata classification)
pub fn detect_ebook_files(path: &str) -> Result<Vec<EbookFile>, String> {
    let mut ebook_files = Vec::new();
    detect_ebook_files_recursive(Path::new(path), &mut ebook_files)?;
    Ok(ebook_files)
}

/// Recursively search for ebook files in a directory tree
fn detect_ebook_files_recursive(path: &Path, ebook_files: &mut Vec<EbookFile>) -> Result<(), String> {
    if path.is_file() {
        if let Some(extension) = path.extension().and_then(|ext| ext.to_str()) {
            if let Some(ebook_type) = EbookType::from_extension(extension) {
                ebook_files.push(EbookFile {
                    path: path.to_path_buf(),
                    ebook_type,
                });
            }
        }
    } else if path.is_dir() {
        for entry in fs::read_dir(path)
            .map_err(|e| format!("Failed to read directory {:?}: {}", path, e))? 
        {
            let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
            let entry_path = entry.path();
            
            // Recursively process subdirectories and files
            detect_ebook_files_recursive(&entry_path, ebook_files)?;
        }
    }
    
    Ok(())
}

/// Convert EbookFile to MediaFile
pub fn to_media_file(ebook_file: &EbookFile) -> MediaFile {
    MediaFile {
        path: ebook_file.path.clone(),
        media_type: MediaType::Ebook(ebook_file.ebook_type.clone()),
    }
}

/// Classify ebook content based on filename patterns
pub fn classify_ebook_content(filename: &str, extension: &str) -> EbookMetadata {
    let mut metadata = EbookMetadata::default();
    
    // Set format type based on extension
    metadata.format_type = EbookType::from_extension(extension);
    
    // Initialize regex patterns
    let author_title_regex = Regex::new(r"^([^-]+?)\s*-\s*(.+?)(?:\s*\((\d{4})\))?(?:\s*\[(.+?)\])?$").unwrap();
    let year_regex = Regex::new(r"\b(19|20)\d{2}\b").unwrap();
    let edition_regex = Regex::new(r"(?i)\b(\d+)(?:st|nd|rd|th)?\s*(?:edition|ed\.?)\b").unwrap();
    let volume_regex = Regex::new(r"(?i)\b(?:vol|volume)\.?\s*(\d+)\b").unwrap();
    let issue_regex = Regex::new(r"(?i)\b(?:issue|#)\s*(\d+)\b").unwrap();
    let isbn_regex = Regex::new(r"(?i)\b(?:isbn[-\s]?(?:10|13)?:?\s*)([\d-]+)\b").unwrap();
    
    // Category patterns
    let comic_regex = Regex::new(r"(?i)\b(comic|comics|manga|graphic\.novel|cbr|cbz)\b").unwrap();
    let magazine_regex = Regex::new(r"(?i)\b(magazine|mag|periodical|journal|monthly|weekly|quarterly|review|economist|geographic|nature|psychology|car\s+and\s+driver)\b").unwrap();
    let newspaper_regex = Regex::new(r"(?i)\b(newspaper|news|daily|times|post|gazette|herald|tribune)\b").unwrap();
    let technical_regex = Regex::new(r"(?i)\b(programming|coding|software|computer|technology|technical|engineering|mathematics|physics|chemistry|algorithm|python|java|javascript|mit\s+press|introduction\s+to\s+algorithms)\b").unwrap();
    let educational_regex = Regex::new(r"(?i)\b(textbook|course|tutorial|guide|manual|handbook|education|learning|study|exam|test\.prep)\b").unwrap();
    let biography_regex = Regex::new(r"(?i)\b(biography|autobiography|memoir|life\.of|story\.of)\b").unwrap();
    let history_regex = Regex::new(r"(?i)\b(history|historical|ancient|medieval|war|battle|civilization)\b").unwrap();
    let science_regex = Regex::new(r"(?i)\b(science|scientific|biology|astronomy|geology|research|medical|medicine|health|clinic|anatomy|physics|einstein)\b").unwrap();
    let religion_regex = Regex::new(r"(?i)\b(bible|quran|torah|religion|religious|spiritual|theology|buddhism|christianity|islam|hindu)\b").unwrap();
    let cookbook_regex = Regex::new(r"(?i)\b(cookbook|cooking|recipe|recipes|cuisine|culinary|baking|food|crocker|ramsay|oliver)\b").unwrap();
    let travel_regex = Regex::new(r"(?i)\b(travel|guide|lonely\.planet|frommer|tourism|vacation|michelin|rick\s+steves|through\s+the\s+back\s+door)\b").unwrap();
    let children_regex = Regex::new(r"(?i)\b(children|kids|juvenile|young\.adult|ya|picture\.book|seuss)\b").unwrap();
    
    // Series patterns
    let series_regex = Regex::new(r"(?i)\b(?:book|part|series)\s*(\d+)\b").unwrap();
    
    debug!("Classifying ebook content for: {}", filename);
    
    // Clean filename for processing
    let clean_name = filename
        .trim()
        .trim_end_matches(&format!(".{}", extension));
    
    // 1. Try to extract author and title pattern (Author - Title)
    if let Some(captures) = author_title_regex.captures(clean_name) {
        if let Some(author) = captures.get(1) {
            metadata.author = Some(author.as_str().trim().to_string());
        }
        if let Some(title) = captures.get(2) {
            metadata.title = title.as_str().trim().to_string();
        }
        if let Some(year) = captures.get(3) {
            metadata.year = year.as_str().parse::<u32>().ok();
        }
        if let Some(extra) = captures.get(4) {
            // Extra info in brackets might be publisher, edition, etc.
            let extra_str = extra.as_str();
            if extra_str.chars().all(|c| c.is_alphanumeric() || c == '-') && extra_str.len() == 13 {
                metadata.isbn = Some(extra_str.to_string());
            } else {
                metadata.publisher = Some(extra_str.to_string());
            }
        }
    } else {
        // Fallback: use the whole filename as title
        metadata.title = clean_name.replace('_', " ").replace('.', " ").trim().to_string();
    }
    
    // 2. Extract additional metadata
    if metadata.year.is_none() {
        if let Some(year_match) = year_regex.find(clean_name) {
            metadata.year = year_match.as_str().parse::<u32>().ok();
        }
    }
    
    if let Some(edition_match) = edition_regex.captures(clean_name) {
        if let Some(edition_num) = edition_match.get(1) {
            metadata.edition = Some(format!("{} edition", edition_num.as_str()));
        }
    }
    
    if let Some(volume_match) = volume_regex.captures(clean_name) {
        if let Some(vol_num) = volume_match.get(1) {
            metadata.volume = Some(vol_num.as_str().to_string());
        }
    }
    
    if let Some(issue_match) = issue_regex.captures(clean_name) {
        if let Some(issue_num) = issue_match.get(1) {
            metadata.issue = Some(issue_num.as_str().to_string());
        }
    }
    
    if let Some(isbn_match) = isbn_regex.captures(clean_name) {
        if let Some(isbn) = isbn_match.get(1) {
            metadata.isbn = Some(isbn.as_str().replace("-", ""));
        }
    }
    
    if let Some(series_match) = series_regex.captures(clean_name) {
        if let Some(series_num) = series_match.get(1) {
            metadata.series = Some(format!("Book {}", series_num.as_str()));
        }
    }
    
    // 3. Determine category based on content patterns and format
    // Comics have highest priority
    if metadata.format_type.as_ref().map(|t| t.is_comic()).unwrap_or(false) || comic_regex.is_match(filename) {
        metadata.category = EbookCategory::Comic;
    } else if newspaper_regex.is_match(filename) {
        metadata.category = EbookCategory::Newspaper;
    } else if cookbook_regex.is_match(filename) {
        metadata.category = EbookCategory::Cookbook;
    } else if travel_regex.is_match(filename) {
        metadata.category = EbookCategory::Travel;
    } else if children_regex.is_match(filename) {
        metadata.category = EbookCategory::Children;
    } else if technical_regex.is_match(filename) {
        metadata.category = EbookCategory::Technical;
    } else if magazine_regex.is_match(filename) {
        metadata.category = EbookCategory::Magazine;
    } else if biography_regex.is_match(filename) {
        metadata.category = EbookCategory::Biography;
    } else if history_regex.is_match(filename) {
        metadata.category = EbookCategory::History;
    } else if science_regex.is_match(filename) {
        metadata.category = EbookCategory::Science;
    } else if religion_regex.is_match(filename) {
        metadata.category = EbookCategory::Religion;
    } else if educational_regex.is_match(filename) {
        metadata.category = EbookCategory::Educational;
    } else if metadata.author.is_some() && !metadata.title.is_empty() {
        // If we have author and title, assume it's a novel
        metadata.category = EbookCategory::Novel;
    }
    
    debug!("Ebook classification result: {:?}", metadata);
    metadata
}

/// Classify ebook content for upload pipeline
pub fn classify_for_upload(input_path: &str, metadata: &serde_json::Value) -> Result<(Option<String>, Option<String>, serde_json::Value), String> {
    // Check if we already have classification in metadata
    if let Some(format_str) = metadata.get("format").and_then(|f| f.as_str()) {
        let category = if format_str.contains("Comic") || format_str.contains("Cbr") || format_str.contains("Cbz") {
            Some("EbookCategory::Comic".to_string())
        } else {
            Some("EbookCategory::General".to_string())
        };
        
        return Ok((category, None, metadata.clone()));
    }
    
    // Otherwise, detect and classify
    if let Ok(ebook_files) = detect_ebook_files(input_path) {
        if let Some(ebook_file) = ebook_files.first() {
            let filename = ebook_file.path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            
            let extension = ebook_file.path.extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");
            
            let ebook_metadata = classify_ebook_content(filename, extension);
            
            let category = match ebook_metadata.category {
                EbookCategory::Comic => Some("EbookCategory::Comic".to_string()),
                EbookCategory::Magazine => Some("EbookCategory::Magazine".to_string()),
                EbookCategory::Educational | EbookCategory::Technical | EbookCategory::Science => {
                    Some("EbookCategory::Educational".to_string())
                }
                _ => Some("EbookCategory::General".to_string()),
            };
            
            // Manually create JSON metadata
            let json_metadata = serde_json::json!({
                "title": ebook_metadata.title,
                "author": ebook_metadata.author,
                "publisher": ebook_metadata.publisher,
                "year": ebook_metadata.year,
                "isbn": ebook_metadata.isbn,
                "series": ebook_metadata.series,
                "edition": ebook_metadata.edition,
                "volume": ebook_metadata.volume,
                "issue": ebook_metadata.issue,
                "language": ebook_metadata.language,
                "category": format!("{:?}", ebook_metadata.category),
                "format_type": ebook_metadata.format_type.as_ref().map(|t| format!("{:?}", t)),
                "format": extension,
            });
            
            return Ok((category, None, json_metadata));
        }
    }
    
    // Default to general ebook
    Ok((Some("EbookCategory::General".to_string()), None, metadata.clone()))
}

/// Generate default non-video description footer
fn default_non_video_description() -> String {
    format!(
        "[center][b][color=#E74C3C]Uploaded with seedbrr[/color][/b][/center]"
    )
}

