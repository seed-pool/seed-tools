use epub::doc::EpubDoc;
use log::{info, warn};
use regex::Regex;
use std::fs;
use std::path::Path;
use std::process::Command;
use walkdir::WalkDir;

use crate::core::{Config, SeedpoolConfig};
use crate::core::{EbookCategory, EbookFile, EbookType, MediaFile, MediaType};
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
        return Err(format!(
            "No supported ebook files (.epub, .pdf, .cbz, .cbr) found in directory '{}'",
            working_dir
        ));
    }

    // Check if we have any comic files (CBR/CBZ)
    let comic_files: Vec<EbookFile> = found_files
        .iter()
        .filter(|f| f.ebook_type.is_comic())
        .cloned()
        .collect();

    if !comic_files.is_empty() {
        // If we have comic files, return ALL of them
        log::info!(
            "Found {} comic file(s) in directory: {}",
            comic_files.len(),
            working_dir
        );
        Ok(comic_files)
    } else {
        // If no comic files, use the original priority system to return one file
        found_files.sort_by_key(|f| match f.ebook_type {
            EbookType::Epub => 0,
            EbookType::Cbz => 1,
            EbookType::Cbr => 2,
            EbookType::Pdf => 3,
            EbookType::Mobi
            | EbookType::Azw
            | EbookType::Azw3
            | EbookType::Lit
            | EbookType::Pdb => 4,
        });
        Ok(vec![found_files.into_iter().next().unwrap()])
    }
}

/// Extract metadata (title, author) from an ebook file
fn extract_ebook_metadata(
    ebook_file: &EbookFile,
) -> Result<(Option<String>, Option<String>), String> {
    match ebook_file.ebook_type {
        EbookType::Pdf => extract_metadata_from_pdf(ebook_file.path.to_str().unwrap()),
        EbookType::Epub => extract_metadata_from_epub(ebook_file.path.to_str().unwrap()),
        EbookType::Cbz | EbookType::Cbr => extract_metadata_from_comic(&ebook_file.path),
        EbookType::Mobi | EbookType::Azw | EbookType::Azw3 | EbookType::Lit | EbookType::Pdb => {
            // For now, return generic metadata for these formats
            Ok((
                Some("Unknown Title".to_string()),
                Some("Unknown Author".to_string()),
            ))
        }
    }
}

/// Extract metadata from comic files (based on filename)
fn extract_metadata_from_comic(
    comic_path: &Path,
) -> Result<(Option<String>, Option<String>), String> {
    // Extract title from filename, remove common comic suffixes
    let filename = comic_path
        .file_stem()
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
    let comic_name = comic_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("comic");
    let extract_dir = Path::new(working_dir).join(comic_name);

    // Create the extraction directory if it doesn't exist
    std::fs::create_dir_all(&extract_dir).map_err(|e| {
        format!(
            "Failed to create extraction directory '{}': {}",
            extract_dir.display(),
            e
        )
    })?;

    match comic_file.ebook_type {
        EbookType::Cbz => {
            // Extract CBZ (ZIP) file to subfolder
            log::info!(
                "Extracting CBZ file: {} to {}",
                comic_path.display(),
                extract_dir.display()
            );
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
        }
        EbookType::Cbr => {
            // Extract CBR (RAR) file to subfolder
            log::info!(
                "Extracting CBR file: {} to {}",
                comic_path.display(),
                extract_dir.display()
            );
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
        }
        _ => {
            return Err(format!(
                "File '{}' is not a comic archive",
                comic_path.display()
            ))
        }
    }

    log::info!(
        "Successfully extracted comic images to: {}",
        extract_dir.display()
    );
    Ok(extract_dir.to_string_lossy().to_string())
}

/// Extract metadata from PDF files
pub fn extract_metadata_from_pdf(pdf_path: &str) -> Result<(Option<String>, Option<String>), String> {
    use lopdf::{Document, Object};

    let doc = Document::load(pdf_path).map_err(|e| format!("Failed to open PDF: {}", e))?;
    let info_obj = match doc.trailer.get(b"Info") {
        Ok(obj) => obj,
        Err(_) => return Ok((None, None)),
    };
    let info_ref = info_obj
        .as_reference()
        .map_err(|e| format!("Failed to get Info reference: {}", e))?;
    let dict = doc
        .get_dictionary(info_ref)
        .map_err(|e| format!("Failed to get PDF info dictionary: {}", e))?;

    fn get_pdf_string(dict: &lopdf::Dictionary, key: &[u8]) -> Option<String> {
        match dict.get(key) {
            Ok(Object::String(s, _)) => Some(String::from_utf8_lossy(s).to_string()),
            Ok(obj) => obj
                .as_str()
                .ok()
                .map(|s| String::from_utf8_lossy(s).to_string()),
            _ => None,
        }
    }

    let title = get_pdf_string(&dict, b"Title");
    let author = get_pdf_string(&dict, b"Author");
    Ok((title, author))
}

/// Extract metadata from EPUB files
pub fn extract_metadata_from_epub(epub_path: &str) -> Result<(Option<String>, Option<String>), String> {
    let epub = EpubDoc::new(epub_path)
        .map_err(|e| format!("Failed to open EPUB file '{}': {}", epub_path, e))?;

    // Extract title from metadata
    let title = epub
        .metadata
        .get("title")
        .and_then(|titles| titles.get(0).cloned());

    // Extract author from metadata
    let author = epub
        .metadata
        .get("creator")
        .and_then(|creators| creators.get(0).cloned());

    Ok((title, author))
}

/// Extract images from EPUB files
fn extract_epub_images(
    epub_path: &str,
    temp_dir: &std::path::Path,
) -> Result<Vec<std::path::PathBuf>, String> {
    use std::fs::File;
    use std::io::copy;
    use zip::ZipArchive;

    let file = File::open(epub_path).map_err(|e| format!("Failed to open EPUB: {}", e))?;
    let mut archive =
        ZipArchive::new(file).map_err(|e| format!("Failed to read EPUB as zip: {}", e))?;

    fs::create_dir_all(temp_dir).map_err(|e| format!("Failed to create temp dir: {}", e))?;

    let mut images = Vec::new();

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Failed to access EPUB entry: {}", e))?;
        let name = file.name().to_lowercase();
        if name.ends_with(".jpg")
            || name.ends_with(".jpeg")
            || name.ends_with(".png")
            || name.ends_with(".gif")
        {
            let out_path = temp_dir.join(std::path::Path::new(&name).file_name().unwrap());
            let mut out_file = File::create(&out_path)
                .map_err(|e| format!("Failed to create image file: {}", e))?;
            copy(&mut file, &mut out_file)
                .map_err(|e| format!("Failed to extract image: {}", e))?;
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
fn generate_ebook_description_from_metadata(
    metadata: &serde_json::Value,
) -> Result<String, String> {
    use crate::core::{DescriptionComponent, EbookType, ImageLayout, MediaType, SectionFormat};
    use crate::processing::description::{DescriptionBuilder, DescriptionConfig};

    let mut config = DescriptionConfig::default();
    config.image_layout = ImageLayout::TwoColumn;  // Use 2-column table for ebooks

    let mut builder = DescriptionBuilder::with_config(MediaType::Ebook(EbookType::Epub), config);

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
        builder = builder.add_component(DescriptionComponent::Table {
            rows: metadata_rows,
        });
    }

    // Add description if available
    if let Some(description) = metadata.get("description").and_then(|d| d.as_str()) {
        builder = builder.custom_section("Description", description, SectionFormat::Plain);
    }

    // Add images if available
    if let Some(images) = metadata.get("images").and_then(|i| i.as_array()) {
        let image_urls: Vec<String> = images
            .iter()
            .filter_map(|v| v.as_str())
            .map(String::from)
            .collect();

        if !image_urls.is_empty() {
            builder = builder.images(image_urls);
        }
    }

    // Add custom description if available
    if let Some(custom_desc) = metadata.get("custom_description").and_then(|d| d.as_str()) {
        if !custom_desc.is_empty() {
            builder = builder.raw(custom_desc);
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
    config: &crate::core::Config,
) -> Result<String, String> {
    use std::fs;
    use std::path::Path;

    let mut image_urls = Vec::new();

    // Determine file type from extension
    let path = Path::new(ebook_path);

    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_lowercase())
        .unwrap_or_default();
    match extension.as_str() {
        "cbr" | "cbz" => {
            // For CBR/CBZ files, find and use existing extracted images
            let parent_dir = path
                .parent()
                .ok_or_else(|| "Cannot determine parent directory".to_string())?;

            // Look for the extraction subfolder based on comic file name
            let comic_name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("comic");
            let extract_dir = parent_dir.join(comic_name);

            // Use the extraction directory if it exists, otherwise fall back to parent directory
            let search_dir = if extract_dir.exists() && extract_dir.is_dir() {
                &extract_dir
            } else {
                parent_dir
            };

            // Look for extracted image files in the appropriate directory
            let mut image_files = Vec::new();
            for entry in
                fs::read_dir(search_dir).map_err(|e| format!("Failed to read directory: {}", e))?
            {
                let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
                let file_path = entry.path();
                if file_path.is_file() {
                    if let Some(ext) = file_path.extension().and_then(|e| e.to_str()) {
                        let ext_lower = ext.to_lowercase();
                        if ext_lower == "jpg"
                            || ext_lower == "jpeg"
                            || ext_lower == "png"
                            || ext_lower == "gif"
                            || ext_lower == "bmp"
                            || ext_lower == "webp"
                        {
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
                let temp_image_path =
                    format!("{}/{}", std::env::temp_dir().to_string_lossy(), image_name);

                // Copy the extracted image to temp directory with standardized name
                fs::copy(image_file, &temp_image_path).map_err(|e| {
                    format!("Failed to copy image '{}': {}", image_file.display(), e)
                })?;

                // Set permissions to 777
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    fs::set_permissions(&temp_image_path, fs::Permissions::from_mode(0o777))
                        .map_err(|e| {
                            format!("Failed to set permissions for '{}': {}", temp_image_path, e)
                        })?;
                }

                // SCP to CDN (skip during dry run)
                if !dry_run {
                    let scp_status = std::process::Command::new("scp")
                        .arg(&temp_image_path)
                        .arg(format!(
                            "{}/screenshots/",
                            remote_path.trim_end_matches('/')
                        ))
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
        "epub" => {
            // For EPUB files, extract cover and generate rich description with Open Library
            use reqwest::blocking::Client;
            
            let path = Path::new(ebook_path);
            let parent_dir = path
                .parent()
                .ok_or_else(|| "Cannot determine parent directory".to_string())?;

            // Extract EPUB images using existing function
            match extract_epub_images(ebook_path, parent_dir) {
                Ok(image_paths) => {
                    if let Some(cover) = image_paths.first() {
                        let image_name = format!("{}-cover.jpg", torrent_name);
                        let temp_image_path =
                            format!("{}/{}", std::env::temp_dir().to_string_lossy(), image_name);

                        // Copy cover to temp with standardized name
                        fs::copy(&cover, &temp_image_path).map_err(|e| {
                            format!("Failed to copy EPUB cover '{}': {}", cover.display(), e)
                        })?;

                        // Set permissions to 777
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::PermissionsExt;
                            fs::set_permissions(&temp_image_path, fs::Permissions::from_mode(0o777))
                                .map_err(|e| {
                                    format!("Failed to set permissions for '{}': {}", temp_image_path, e)
                                })?;
                        }

                        // SCP to CDN (skip during dry run)
                        if !dry_run {
                            let scp_status = std::process::Command::new("scp")
                                .arg(&temp_image_path)
                                .arg(format!(
                                    "{}/screenshots/",
                                    remote_path.trim_end_matches('/')
                                ))
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
                Err(e) => {
                    info!("Warning: Could not extract EPUB cover: {}", e);
                    // Continue without cover - this is not a fatal error
                }
            }

            // Now generate rich description with Open Library lookup
            info!("📚 Starting Open Library lookup for EPUB description");
            
            // Extract metadata from EPUB
            let (epub_title, epub_author) = match extract_metadata_from_epub(ebook_path) {
                Ok((title_opt, author_opt)) => {
                    let title = title_opt.unwrap_or_else(|| torrent_name.to_string());
                    let author = author_opt.unwrap_or_else(|| "Unknown Author".to_string());
                    (title, author)
                }
                Err(e) => {
                    info!("Warning: Could not extract EPUB metadata: {}", e);
                    (torrent_name.to_string(), "Unknown Author".to_string())
                }
            };

            info!("📚 Extracted metadata - Title: '{}', Author: '{}'", epub_title, epub_author);

            // Try Open Library lookup
            let client = Client::new();
            
            // Try multiple Open Library search strategies
            if epub_title != "Unknown Title" || epub_author != "Unknown Author" {
                let mut search_queries = Vec::new();
                
                // Primary search: title + author
                if epub_title != "Unknown Title" && epub_author != "Unknown Author" {
                    search_queries.push(format!(
                        "https://openlibrary.org/search.json?title={}&author={}",
                        urlencoding::encode(&epub_title),
                        urlencoding::encode(&epub_author)
                    ));
                }
                
                // Fallback searches
                if epub_title != "Unknown Title" {
                    // Title only
                    search_queries.push(format!(
                        "https://openlibrary.org/search.json?title={}",
                        urlencoding::encode(&epub_title)
                    ));
                    
                    // Title without subtitle/series info
                    let clean_title = epub_title.split('(').next().unwrap_or(&epub_title).trim();
                    if clean_title != epub_title {
                        search_queries.push(format!(
                            "https://openlibrary.org/search.json?title={}",
                            urlencoding::encode(clean_title)
                        ));
                    }
                }
                
                if epub_author != "Unknown Author" {
                    // Author only  
                    search_queries.push(format!(
                        "https://openlibrary.org/search.json?author={}",
                        urlencoding::encode(&epub_author)
                    ));
                }

                let mut found_result = false;
                for (attempt, query) in search_queries.iter().enumerate() {
                    info!("📚 Open Library search attempt {} of {}: {}", attempt + 1, search_queries.len(), query);

                    match client.get(query).send() {
                        Ok(response) if response.status().is_success() => {
                            match response.json::<serde_json::Value>() {
                                Ok(json) => {
                                    if let Some(first_result) = json["docs"].as_array().and_then(|docs| docs.get(0)) {
                                        // Extract Open Library keys
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

                                        if !open_library_work_key.is_empty() && !open_library_author_key.is_empty() {
                                            // Verify author match before using result
                                            let ol_author = first_result["author_name"]
                                                .as_array()
                                                .and_then(|authors| authors.get(0))
                                                .and_then(|author| author.as_str())
                                                .unwrap_or("");
                                            
                                            let ol_title = first_result["title"].as_str().unwrap_or("");
                                            
                                            // Check if authors are similar (basic name matching)
                                            let epub_author_clean = epub_author.to_lowercase().replace(".", "").replace(" ", "");
                                            let ol_author_clean = ol_author.to_lowercase().replace(".", "").replace(" ", "");
                                            
                                            let author_similarity = if epub_author_clean.contains(&ol_author_clean) || 
                                                                      ol_author_clean.contains(&epub_author_clean) ||
                                                                      epub_author_clean == ol_author_clean {
                                                true
                                            } else {
                                                // Check for reversed name order (A.J. Donovan vs Donovan, A.J.)
                                                let epub_parts: Vec<&str> = epub_author.split_whitespace().collect();
                                                let ol_parts: Vec<&str> = ol_author.split_whitespace().collect();
                                                if epub_parts.len() >= 2 && ol_parts.len() >= 2 {
                                                    let epub_last = epub_parts.last().unwrap().to_lowercase();
                                                    let ol_last = ol_parts.last().unwrap().to_lowercase();
                                                    epub_last == ol_last
                                                } else {
                                                    false
                                                }
                                            };
                                            
                                            // Also check title similarity to avoid matching different books by same author
                                            let epub_title_clean = epub_title.to_lowercase().replace(".", "").replace(" ", "");
                                            let ol_title_clean = ol_title.to_lowercase().replace(".", "").replace(" ", "");
                                            
                                            let title_similarity = if epub_title_clean.contains(&ol_title_clean) || 
                                                                     ol_title_clean.contains(&epub_title_clean) ||
                                                                     epub_title_clean == ol_title_clean {
                                                true
                                            } else {
                                                // Check for partial title matches (first few words)
                                                let epub_words: Vec<&str> = epub_title.split_whitespace().collect();
                                                let ol_words: Vec<&str> = ol_title.split_whitespace().collect();
                                                if epub_words.len() >= 2 && ol_words.len() >= 2 {
                                                    // Check if first 2 words match
                                                    let epub_first_two = format!("{} {}", epub_words[0], epub_words[1]).to_lowercase();
                                                    let ol_first_two = format!("{} {}", ol_words[0], ol_words[1]).to_lowercase();
                                                    epub_first_two == ol_first_two
                                                } else {
                                                    false
                                                }
                                            };
                                            
                                            // More flexible matching: accept strong author match OR strong title+author match
                                            let accept_match = if author_similarity && title_similarity {
                                                info!("📚 Strong match found - both author and title similar");
                                                true
                                            } else if author_similarity && attempt == 0 {
                                                // On first search (title+author), accept author match if title has some overlap
                                                let title_overlap = epub_title_clean.len() > 3 && ol_title_clean.len() > 3 && 
                                                                   (epub_title_clean[..3] == ol_title_clean[..3] || 
                                                                    ol_title_clean.contains(&epub_title_clean[..epub_title_clean.len().min(5)]) ||
                                                                    epub_title_clean.contains(&ol_title_clean[..ol_title_clean.len().min(5)]));
                                                if title_overlap {
                                                    info!("📚 Good match found - strong author similarity with partial title overlap");
                                                    true
                                                } else {
                                                    false
                                                }
                                            } else if title_similarity && epub_author != "Unknown Author" && epub_author.len() > 3 {
                                                // Accept title match if author has some similarity
                                                let author_partial = epub_author_clean.len() > 2 && ol_author_clean.len() > 2 &&
                                                                    (epub_author_clean.chars().take(3).collect::<String>() == 
                                                                     ol_author_clean.chars().take(3).collect::<String>() ||
                                                                     epub_author.split_whitespace().any(|word| 
                                                                        ol_author.to_lowercase().contains(&word.to_lowercase())));
                                                if author_partial {
                                                    info!("📚 Reasonable match found - strong title similarity with partial author overlap");
                                                    true
                                                } else {
                                                    false
                                                }
                                            } else {
                                                false
                                            };

                                            if accept_match {
                                                info!("📚 Accepted Open Library match - Work: '{}', Title: '{}' -> '{}', Author: '{}' -> '{}'", 
                                                      open_library_work_key, epub_title, ol_title, epub_author, ol_author);
                                                
                                                // Generate rich description using existing function
                                                match generate_ebook_bbcode_description(
                                                    &epub_title,
                                                    &epub_author,
                                                    &open_library_work_key,
                                                    &open_library_author_key,
                                                    &client,
                                                ) {
                                                    Ok((rich_description, _subjects)) => {
                                                        info!("✅ Generated rich Open Library description");
                                                        // Store the rich description globally so it can be used later
                                                        std::env::set_var("SEEDBRR_RICH_DESCRIPTION", &rich_description);
                                                        std::env::set_var("SEEDBRR_OPEN_LIBRARY_ATTEMPTED", "true");
                                                        info!("📚 Stored rich description ({} chars) for later use", rich_description.len());
                                                        found_result = true;
                                                        break;
                                                    }
                                                    Err(e) => {
                                                        info!("⚠️ Failed to generate rich description: {}", e);
                                                    }
                                                }
                                            } else {
                                                info!("📚 Open Library result rejected - insufficient similarity: EPUB '{}' by '{}' vs Open Library '{}' by '{}'", 
                                                      epub_title, epub_author, ol_title, ol_author);
                                            }
                                        } else {
                                            info!("📚 Open Library result missing work or author keys");
                                        }
                                    } else {
                                        info!("📚 No results found in search attempt {}", attempt + 1);
                                    }
                                }
                                Err(e) => {
                                    info!("⚠️ Failed to parse Open Library response: {}", e);
                                }
                            }
                        }
                        Ok(response) => {
                            info!("⚠️ Open Library API returned status: {}", response.status());
                        }
                        Err(e) => {
                            info!("⚠️ Failed to query Open Library: {}", e);
                        }
                    }
                }
                
                if !found_result {
                    info!("📚 No results found in any Open Library search attempts");
                    
                    // Try fallback: search for author-only information to get rich author bio
                    if epub_author != "Unknown Author" && epub_author.len() > 3 {
                        info!("📚 Attempting author-only Open Library lookup for rich description");
                        match generate_author_fallback_description(&epub_title, &epub_author, &client) {
                            Ok(rich_description) => {
                                info!("✅ Generated fallback description with author details from Open Library");
                                // Store the fallback rich description globally 
                                std::env::set_var("SEEDBRR_RICH_DESCRIPTION", &rich_description);
                                info!("📚 Stored fallback rich description ({} chars) for later use", rich_description.len());
                                found_result = true;
                            }
                            Err(e) => {
                                info!("⚠️ Failed to generate author fallback description: {}", e);
                            }
                        }
                    }
                    
                    // Set flag to prevent post-upload processing duplication even if no rich description found
                    std::env::set_var("SEEDBRR_OPEN_LIBRARY_ATTEMPTED", "true");
                    info!("📚 Marked Open Library processing as attempted to prevent duplication");
                }
            } else {
                info!("📚 Skipping Open Library lookup - insufficient metadata");
            }

            // Note: Cover image is uploaded to CDN but not included in description per user request
            
            // Clean up extracted images from EPUB directory (keep only .epub and .nfo files)
            info!("📚 Cleaning up extracted EPUB images from directory");
            let epub_dir = Path::new(ebook_path).parent()
                .ok_or_else(|| "Cannot determine EPUB parent directory".to_string())?;
            
            for entry in fs::read_dir(epub_dir).map_err(|e| format!("Failed to read EPUB directory: {}", e))? {
                let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
                let entry_path = entry.path();
                
                if entry_path.is_file() {
                    if let Some(extension) = entry_path.extension().and_then(|ext| ext.to_str()) {
                        let extension_lower = extension.to_lowercase();
                        
                        // Remove image files that were extracted from EPUB
                        let should_remove = match extension_lower.as_str() {
                            "epub" | "nfo" => false, // Keep EPUB and NFO files
                            "jpg" | "jpeg" | "png" | "gif" => true, // Remove extracted images
                            _ => false, // Keep other files (they might be legitimate)
                        };
                        
                        if should_remove {
                            match fs::remove_file(&entry_path) {
                                Ok(()) => {
                                    info!("🗑️ Removed extracted image: {}", entry_path.display());
                                }
                                Err(e) => {
                                    info!("⚠️ Failed to remove extracted image {}: {}", entry_path.display(), e);
                                }
                            }
                        }
                    }
                }
            }
            
            // Continue to DescriptionBuilder for proper image formatting  
            // (Don't return early - let DescriptionBuilder handle images)
            
            // Check if a rich description was generated and return it
            if let Ok(stored_description) = std::env::var("SEEDBRR_RICH_DESCRIPTION") {
                info!("📚 Using rich description generated during EPUB processing ({} chars)", stored_description.len());
                info!("🔍 DEBUG: Rich description preview: {}...", &stored_description[..std::cmp::min(200, stored_description.len())]);
                // Don't clean up here - let UploadBuilder clean it up when it uses it
                return Ok(stored_description);
            }
            
            // Check if Open Library was attempted but no rich description was found
            // In this case, generate a basic description without cover images (per user request)
            if std::env::var("SEEDBRR_OPEN_LIBRARY_ATTEMPTED").is_ok() && std::env::var("SEEDBRR_RICH_DESCRIPTION").is_err() {
                info!("📚 Open Library was attempted but no rich description available, generating basic description without cover");
                let basic_description = format!(
                    "[center][b][size=32][color=#2E86C1]{}[/color][/size][/b][/center]\n\n[center][b][size=16][color=#117A65]By:[/color][/size][/b] [i]{}[/i][/center]\n\n[center][b][size=12][color=#757575]Created with mkbrr, ffmpeg, and mediainfo. Posted to this fine tracker with seedbrr.[/color][/size][/b]\n\n[url=https://github.com/seed-pool/seed-tools][img]https://cdn.seedpool.org/sp.png[/img][/url]  [url=https://github.com/autobrr/mkbrr][img]https://cdn.seedpool.org/mkbrr.png[/img][/url]  [url=https://www.rust-lang.org][img]https://cdn.seedpool.org/rust.png[/img][/url][/center]",
                    epub_title, epub_author
                );
                return Ok(basic_description);
            }
        }
        "pdf" => {
            // Get ghostscript path from config
            let (_, _, _, _, ghostscript_path) = crate::core::Config::get_binary_paths(config);
            let gs_path_str = ghostscript_path
                .to_str()
                .ok_or("Invalid ghostscript path")?;

            // For PDF files, use GhostScript to extract pages 3-10
            for page in 3..=10 {
                let image_name = format!("{}-page{}.jpg", torrent_name, page);
                let image_path =
                    format!("{}/{}", std::env::temp_dir().to_string_lossy(), image_name);

                // Extract page as JPEG using GhostScript
                let output = std::process::Command::new(gs_path_str)
                    .args(&[
                        "-dBATCH",
                        "-dNOPAUSE",
                        "-sDEVICE=jpeg",
                        &format!("-dFirstPage={}", page),
                        &format!("-dLastPage={}", page),
                        "-r300",
                        "-dJPEGQ=95",
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
                    fs::set_permissions(&image_path, fs::Permissions::from_mode(0o777)).map_err(
                        |e| format!("Failed to set permissions for '{}': {}", image_path, e),
                    )?;
                }

                // SCP to CDN (skip during dry run)
                if !dry_run {
                    let scp_status = std::process::Command::new("scp")
                        .arg(&image_path)
                        .arg(format!(
                            "{}/screenshots/",
                            remote_path.trim_end_matches('/')
                        ))
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
            return Err(format!(
                "Unsupported file type for description generation: {}",
                extension
            ));
        }
    }

    // Build BBCode description using DescriptionBuilder
    use crate::core::{EbookType, ImageLayout, MediaType};
    use crate::processing::description::{DescriptionBuilder, DescriptionConfig};

    let mut config = DescriptionConfig::default();
    config.image_layout = ImageLayout::Grid2x2;  // Use same layout as videos for consistency
    config.title_color = "#2E86C1".to_string();

    // Create proper description config for ebooks with Grid2x2 layout (same as videos)
    let mut desc_config = crate::processing::description::DescriptionConfig::default();
    desc_config.image_layout = crate::core::ImageLayout::Grid2x2;
    desc_config.max_images = 8; // Allow up to 8 images as configured in the function
    desc_config.image_width = 500; // Larger width to force 2-column layout

    // For EPUBs, extract title and author for richer description  
    let (final_title, final_author) = if ebook_path.to_lowercase().ends_with(".epub") {
        match extract_metadata_from_epub(ebook_path) {
            Ok((title_opt, author_opt)) => {
                let title = title_opt.unwrap_or_else(|| torrent_name.to_string());
                let author = author_opt.unwrap_or_else(|| "Unknown Author".to_string());
                (title, Some(author))
            }
            Err(_) => (torrent_name.to_string(), None),
        }
    } else {
        (torrent_name.to_string(), None)
    };

    let mut builder = DescriptionBuilder::with_config(
        MediaType::Ebook(EbookType::Pdf), // Using PDF as generic for image-based ebooks
        desc_config,
    )
    .title(&final_title);
    
    // Add author if available
    if let Some(author) = final_author {
        builder = builder.author(&author);
    }
    
    // Add images with proper centering
    if !image_urls.is_empty() {
        builder = builder.images(image_urls);
    }

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
    use crate::core::{EbookType, MediaType, SectionFormat};
    use crate::processing::description::DescriptionBuilder;
    use serde_json::Value;

    let mut subjects = Vec::new();

    // Fetch book details from Open Library
    let work_url = format!(
        "https://openlibrary.org/works/{}.json",
        open_library_work_key
    );
    let work_response = client
        .get(&work_url)
        .send()
        .map_err(|e| format!("Failed to fetch book details: {}", e))?;
    let work_json: Value = work_response
        .json()
        .map_err(|e| format!("Failed to parse book details: {}", e))?;

    // Extract subjects (categories) and add them to the description
    if let Some(subjects_array) = work_json["subjects"].as_array() {
        subjects = subjects_array
            .iter()
            .filter_map(|s| s.as_str().map(|s| s.to_string()))
            .collect();
    }

    // Fetch author details from Open Library
    let author_url = format!(
        "https://openlibrary.org/authors/{}.json",
        open_library_author_key
    );
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
            let links_content = extracted_links
                .iter()
                .map(|link| {
                    format!(
                        "[url={}]{}[/url]",
                        link.trim_end_matches(')'),
                        link.trim_end_matches(')')
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");

            builder =
                builder.custom_section("Additional Editions", &links_content, SectionFormat::Plain);
        }
    }

    // Store subjects/categories as metadata for use in upload keywords instead of description
    if !subjects.is_empty() {
        info!("📚 Storing {} Open Library subjects for upload keywords", subjects.len());
        std::env::set_var("SEEDBRR_OPEN_LIBRARY_SUBJECTS", subjects.join(","));
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

        builder = builder.custom_section(
            "About the Author",
            sanitized_bio.trim(),
            SectionFormat::Quoted,
        );
    }

    // Search for other works by the same author
    let author_search_url = format!(
        "https://openlibrary.org/search.json?author={}&limit=10",
        urlencoding::encode(final_author)
    );
    
    if let Ok(author_works_response) = client.get(&author_search_url).send() {
        if let Ok(author_works_json) = author_works_response.json::<Value>() {
            if let Some(docs) = author_works_json["docs"].as_array() {
                let mut other_works = Vec::new();
                
                for doc in docs.iter().take(8) { // Limit to 8 other works
                    if let (Some(work_title), Some(first_publish_year)) = (
                        doc["title"].as_str(),
                        doc["first_publish_year"].as_i64()
                    ) {
                        // Skip the current book
                        if work_title.to_lowercase() != final_title.to_lowercase() {
                            other_works.push(format!("• {} ({})", work_title, first_publish_year));
                        }
                    }
                }
                
                if !other_works.is_empty() {
                    let works_content = other_works.join("\n");
                    builder = builder.custom_section(
                        "Other Works by This Author",
                        &works_content,
                        SectionFormat::Plain,
                    );
                }
            }
        }
    }

    Ok((builder.build(), subjects))
}

/// Generate fallback BBCode description using author information from Open Library
pub fn generate_author_fallback_description(
    title: &str,
    author: &str, 
    client: &reqwest::blocking::Client,
) -> Result<String, String> {
    use crate::core::{EbookType, MediaType, SectionFormat};
    use crate::processing::description::DescriptionBuilder;
    use serde_json::Value;

    // Search for the author to get their key and details
    let author_search_url = format!(
        "https://openlibrary.org/search.json?author={}",
        urlencoding::encode(author)
    );
    
    info!("📚 Fallback author search: {}", author_search_url);
    
    let search_response = client
        .get(&author_search_url)
        .send()
        .map_err(|e| format!("Failed to search for author: {}", e))?;
    
    let search_json: Value = search_response
        .json()
        .map_err(|e| format!("Failed to parse author search: {}", e))?;
    
    // Find a good author match
    if let Some(docs) = search_json["docs"].as_array() {
        for doc in docs {
            if let Some(author_key) = doc["author_key"]
                .as_array()
                .and_then(|keys| keys.get(0))
                .and_then(|key| key.as_str()) 
            {
                if let Some(ol_author_name) = doc["author_name"]
                    .as_array()
                    .and_then(|names| names.get(0))
                    .and_then(|name| name.as_str())
                {
                    // Check if this author matches our target
                    let author_clean = author.to_lowercase().replace(".", "").replace(" ", "");
                    let ol_author_clean = ol_author_name.to_lowercase().replace(".", "").replace(" ", "");
                    
                    let author_match = author_clean.contains(&ol_author_clean) || 
                                       ol_author_clean.contains(&author_clean) ||
                                       author_clean == ol_author_clean;
                    
                    if author_match {
                        info!("📚 Found author match: '{}' -> '{}'", author, ol_author_name);
                        
                        // Fetch detailed author information
                        let author_url = format!("https://openlibrary.org/authors/{}.json", author_key);
                        let author_response = client
                            .get(&author_url)
                            .send()
                            .map_err(|e| format!("Failed to fetch author details: {}", e))?;
                        
                        let author_json: Value = author_response
                            .json()
                            .map_err(|e| format!("Failed to parse author details: {}", e))?;
                        
                        // Start building rich description
                        let mut builder = DescriptionBuilder::new(MediaType::Ebook(EbookType::Epub))
                            .title(title)
                            .author(ol_author_name);
                        
                        // Add author biography if available
                        if let Some(author_bio) = author_json["bio"]
                            .as_str()
                            .or_else(|| author_json["bio"]["value"].as_str())
                        {
                            // Clean up the bio
                            let source_regex = regex::Regex::new(r"\(\[Source\]\[\d+\]\)").unwrap();
                            let sanitized_bio = source_regex
                                .replace_all(author_bio, "")
                                .to_string()
                                .lines()
                                .filter(|line| !line.trim().is_empty())
                                .collect::<Vec<_>>()
                                .join("\n");
                            
                            builder = builder.custom_section(
                                "About the Author",
                                sanitized_bio.trim(),
                                SectionFormat::Quoted,
                            );
                        }
                        
                        // Add author's notable works if available
                        if let Some(subject_docs) = docs.get(0).and_then(|d| d["subject"].as_array()) {
                            if !subject_docs.is_empty() {
                                let subjects: Vec<String> = subject_docs
                                    .iter()
                                    .take(10) // Limit to avoid overwhelming
                                    .filter_map(|s| s.as_str())
                                    .map(|s| s.to_string())
                                    .collect();
                                
                                if !subjects.is_empty() {
                                    let subjects_text = subjects.join(", ");
                                    builder = builder.custom_section(
                                        "Subjects & Themes",
                                        &subjects_text,
                                        SectionFormat::Plain,
                                    );
                                }
                            }
                        }
                        
                        // Look for some notable works by this author
                        let works_search_url = format!(
                            "https://openlibrary.org/search.json?author={}&limit=5",
                            urlencoding::encode(ol_author_name)
                        );
                        
                        if let Ok(works_response) = client.get(&works_search_url).send() {
                            if let Ok(works_json) = works_response.json::<Value>() {
                                if let Some(works_docs) = works_json["docs"].as_array() {
                                    let notable_works: Vec<String> = works_docs
                                        .iter()
                                        .filter_map(|doc| doc["title"].as_str())
                                        .filter(|work_title| work_title != &title) // Don't include current book
                                        .take(5)
                                        .map(|work_title| format!("• {}", work_title))
                                        .collect();
                                    
                                    if !notable_works.is_empty() {
                                        let works_text = notable_works.join("\n");
                                        builder = builder.custom_section(
                                            "Other Works by This Author",
                                            &works_text,
                                            SectionFormat::Plain,
                                        );
                                    }
                                }
                            }
                        }
                        
                        return Ok(builder.build());
                    }
                }
            }
        }
    }
    
    Err("No matching author found in Open Library".to_string())
}

/// Main function to process ebook uploads
pub fn process_ebook_upload(
    input_path: &str,
    config: &Config,
    seedpool_config: &SeedpoolConfig,
    category_arg: Option<&str>,
    dry_run: bool,
) -> Result<(), String> {
    use reqwest::blocking::Client;
    use std::fs;

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
        return Err(format!(
            "Input path '{}' is neither a file nor a directory.",
            working_dir
        ));
    }

    // Store archive files that will be extracted and deleted for later restoration
    let backup_extracted_archive_buffers = Vec::new();

    // Extract any archives first using centralized extraction and get the processing path
    let processing_path =
        process_and_extract_archives(input_path).map_err(|e| format!("{:?}", e))?;

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
        let original_filename = Path::new(input_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");

        // Check if the original file still exists (if it wasn't an archive)
        let mut files = Vec::new();
        if path.exists() && path.is_file() {
            let ebook_type = path
                .extension()
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
            for entry in fs::read_dir(&working_dir)
                .map_err(|e| format!("Failed to read directory '{}': {}", working_dir, e))?
            {
                let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
                let file_path = entry.path();

                if file_path.is_file() {
                    if let Some(ext) = file_path.extension().and_then(|e| e.to_str()) {
                        if let Some(ebook_type) = EbookType::from_extension(ext) {
                            // Check if this file relates to our original file by name
                            if let Some(filename) = file_path.file_stem().and_then(|s| s.to_str()) {
                                if filename.contains(original_filename)
                                    || original_filename.contains(filename)
                                {
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
            return Err(format!(
                "No ebook files found for input file: {}",
                path.display()
            ));
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
                log::info!(
                    "Comic images extracted from {} to: {}",
                    ebook_file.path.display(),
                    extracted_dir
                );
                // Use the first extracted directory as the content path for torrent creation
                if extracted_comic_dir.is_none() {
                    extracted_comic_dir = Some(extracted_dir);
                }
            }
        }
    }

    // Note: EPUB renaming is now handled by ProcessBuilder during metadata extraction
    let new_ebook_path = main_ebook_file.path.clone();

    // Determine the content path for torrent creation (after any file renaming)
    let actual_content_path = if let Some(comic_dir) = extracted_comic_dir {
        // Use the extracted comic directory
        comic_dir
    } else if is_file {
        // For EPUB files, always preserve the folder structure by using the working directory
        // This ensures that if an EPUB is inside a folder, the folder structure is maintained
        if main_ebook_file.ebook_type == EbookType::Epub {
            working_dir.clone()
        } else {
            // For non-EPUB files, use the file path as before
            new_ebook_path.to_string_lossy().to_string()
        }
    } else {
        // If processing a directory, use the directory path
        working_dir.clone()
    };

    // Clean up other ebook files except the selected one
    for entry in fs::read_dir(&working_dir)
        .map_err(|e| format!("Failed to read directory '{}': {}", working_dir, e))?
    {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
        let path = entry.path();

        if matches!(
            main_ebook_file.ebook_type,
            EbookType::Pdf | EbookType::Cbr | EbookType::Cbz
        ) {
            // Remove all .epub and .zip files, but keep the selected file
            if (path
                .extension()
                .map(|ext| ext.eq_ignore_ascii_case("epub"))
                .unwrap_or(false)
                || path
                    .extension()
                    .map(|ext| ext.eq_ignore_ascii_case("zip"))
                    .unwrap_or(false))
                && path != new_ebook_path
            {
                fs::remove_file(&path)
                    .map_err(|e| format!("Failed to remove file '{}': {}", path.display(), e))?;
            }
            // Do NOT remove the PDF file at main_ebook_file.path (or new_ebook_path)
        } else {
            // For EPUBs: keep only the renamed epub, remove all other epubs
            if path
                .extension()
                .map(|ext| ext.eq_ignore_ascii_case("epub"))
                .unwrap_or(false)
                && path != new_ebook_path
            {
                fs::remove_file(&path).map_err(|e| {
                    format!(
                        "Failed to remove extra epub file '{}': {}",
                        path.display(),
                        e
                    )
                })?;
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
        info!(
            "Using automatic type detection (0700) for file: {}",
            main_ebook_file.path.display()
        );
        let detected_type = match main_ebook_file.ebook_type {
            EbookType::Cbr | EbookType::Cbz => {
                // CBR/CBZ files are comics - check filename for magazine vs comic
                if lower_base.contains("magazine") {
                    info!(
                        "Detected type: Magazine (41) - CBR/CBZ file with 'magazine' in filename"
                    );
                    "41" // Magazine
                } else {
                    info!("Detected type: Comic (40) - CBR/CBZ file");
                    "40" // Comic
                }
            }
            EbookType::Pdf => {
                // PDF files - check filename content to determine type
                if lower_base.contains("magazine") {
                    info!("Detected type: Magazine (41) - PDF with 'magazine' in filename");
                    "41" // Magazine
                } else if lower_base.contains("newspaper") || lower_base.contains("news") {
                    info!(
                        "Detected type: Newspaper (42) - PDF with 'newspaper'/'news' in filename"
                    );
                    "42" // Newspaper
                } else if lower_base.contains("comic") {
                    info!("Detected type: Comic (40) - PDF with 'comic' in filename");
                    "40" // Comic
                } else {
                    info!("Detected type: Regular ebook (20) - PDF file");
                    "20" // Regular ebook/PDF
                }
            }
            EbookType::Epub => {
                // EPUB files - check filename content to determine if it's a magazine
                if lower_base.contains("magazine") {
                    info!("Detected type: Magazine (41) - EPUB with 'magazine' in filename");
                    "41" // Magazine
                } else if lower_base.contains("newspaper") || lower_base.contains("news") {
                    info!(
                        "Detected type: Newspaper (42) - EPUB with 'newspaper'/'news' in filename"
                    );
                    "42" // Newspaper
                } else {
                    info!("Detected type: Regular ebook (20) - EPUB file");
                    "20" // Regular ebook
                }
            }
            EbookType::Mobi
            | EbookType::Azw
            | EbookType::Azw3
            | EbookType::Lit
            | EbookType::Pdb => {
                // Other ebook formats default to regular ebook
                info!(
                    "Detected type: Regular ebook (20) - {:?} file",
                    main_ebook_file.ebook_type
                );
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
            }
            "0740" => {
                info!("Forced type: Comic (40) - Category code 0740");
                "40" // Comic
            }
            "0741" => {
                info!("Forced type: Magazine (41) - Category code 0741");
                "41" // Magazine
            }
            _ => {
                info!(
                    "Unknown category code '{}', defaulting to Regular ebook (20)",
                    cat_arg
                );
                "20" // Default to regular ebook
            }
        };
        forced_type
    } else {
        // Fallback to automatic detection (same as 0700)
        info!(
            "No category specified, using automatic type detection for file: {}",
            main_ebook_file.path.display()
        );
        let detected_type = match main_ebook_file.ebook_type {
            EbookType::Cbr | EbookType::Cbz => {
                if lower_base.contains("magazine") {
                    info!(
                        "Detected type: Magazine (41) - CBR/CBZ file with 'magazine' in filename"
                    );
                    "41"
                } else {
                    info!("Detected type: Comic (40) - CBR/CBZ file");
                    "40"
                }
            }
            EbookType::Pdf => {
                if lower_base.contains("magazine") {
                    info!("Detected type: Magazine (41) - PDF with 'magazine' in filename");
                    "41"
                } else if lower_base.contains("newspaper") || lower_base.contains("news") {
                    info!(
                        "Detected type: Newspaper (42) - PDF with 'newspaper'/'news' in filename"
                    );
                    "42"
                } else if lower_base.contains("comic") {
                    info!("Detected type: Comic (40) - PDF with 'comic' in filename");
                    "40"
                } else {
                    info!("Detected type: Regular ebook (20) - PDF file");
                    "20"
                }
            }
            EbookType::Epub => {
                if lower_base.contains("magazine") {
                    info!("Detected type: Magazine (41) - EPUB with 'magazine' in filename");
                    "41"
                } else if lower_base.contains("newspaper") || lower_base.contains("news") {
                    info!(
                        "Detected type: Newspaper (42) - EPUB with 'newspaper'/'news' in filename"
                    );
                    "42"
                } else {
                    info!("Detected type: Regular ebook (20) - EPUB file");
                    "20"
                }
            }
            EbookType::Mobi
            | EbookType::Azw
            | EbookType::Azw3
            | EbookType::Lit
            | EbookType::Pdb => {
                info!(
                    "Detected type: Regular ebook (20) - {:?} file",
                    main_ebook_file.ebook_type
                );
                "20"
            }
        };
        detected_type
    };

    let (mut description, mut keywords, mut cover_id) = if main_ebook_file.ebook_type.is_comic()
        || (matches!(main_ebook_file.ebook_type, EbookType::Pdf)
            && (type_id == "40" || type_id == "41"))
    {
        let torrent_name = generate_release_name(&base_name);
        let desc = generate_ebook_description(
            new_ebook_path.to_str().unwrap(),
            &torrent_name,
            &seedpool_config.screenshots.remote_path,
            &seedpool_config.screenshots.image_path,
            dry_run,
            config,
        )?;
        let keywords = if type_id == "41" {
            "magazine".to_string()
        } else {
            "comic".to_string()
        };
        (desc, keywords, None::<u64>)
    } else {
        (String::new(), String::new(), None::<u64>)
    };

    // Store original CBR/CBZ files in memory buffers, then remove from disk for torrent creation
    let mut backup_archive_buffers = Vec::new();
    for ebook_file in &ebook_files {
        if ebook_file.ebook_type.is_comic() {
            log::info!(
                "Reading comic archive into memory buffer: {}",
                ebook_file.path.display()
            );
            let file_content = fs::read(&ebook_file.path).map_err(|e| {
                format!(
                    "Failed to read comic file '{}' into memory: {}",
                    ebook_file.path.display(),
                    e
                )
            })?;
            backup_archive_buffers.push((ebook_file.path.clone(), file_content));

            log::info!(
                "Temporarily removing comic archive from disk: {}",
                ebook_file.path.display()
            );
            fs::remove_file(&ebook_file.path).map_err(|e| {
                format!(
                    "Failed to remove original comic file '{}': {}",
                    ebook_file.path.display(),
                    e
                )
            })?;
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
                    log::error!(
                        "Failed to restore comic file '{}' during cleanup: {}",
                        original_path.display(),
                        e
                    );
                } else {
                    log::info!(
                        "Restored comic file from memory during cleanup: {}",
                        original_path.display()
                    );
                }
            }
            // Restore extracted archive files
            for (original_path, file_content) in &self.archive_backup_buffers {
                if let Err(e) = fs::write(original_path, file_content) {
                    log::error!(
                        "Failed to restore archive file '{}' during cleanup: {}",
                        original_path.display(),
                        e
                    );
                } else {
                    log::info!(
                        "Restored archive file from memory during cleanup: {}",
                        original_path.display()
                    );
                }
            }
        }
    }

    let _restore_guard = RestoreGuard {
        comic_backup_buffers: backup_archive_buffers.clone(),
        archive_backup_buffers: backup_extracted_archive_buffers.clone(),
    };

    // Use UploadBuilder for consistent torrent creation and upload processing
    use crate::core::{ImageLayout, UploadComponent};
    use crate::processing::description::DescriptionConfig;
    use crate::processing::upload::{UploadBuilder, UploadProcessor};
    use std::collections::HashMap;
    use std::sync::Arc;

    // Configure description layout based on ebook category
    let mut desc_config = DescriptionConfig::default();
    match main_ebook_file.ebook_type {
        EbookType::Cbr | EbookType::Cbz => {
            desc_config.image_layout = ImageLayout::TwoColumn;  // Use 2-column table for ebooks
            desc_config.max_images = 10;
            desc_config.image_width = 500; // Larger width to force 2-column layout
        }
        EbookType::Pdf => {
            desc_config.image_layout = ImageLayout::TwoColumn;  // Use 2-column table layout
            desc_config.max_images = 8;
            desc_config.image_width = 500; // Larger width to force 2-column layout
        }
        _ => {
            desc_config.image_layout = ImageLayout::SingleColumn;
            desc_config.max_images = 2;
            desc_config.image_width = 500;
        }
    }

    // Build upload data using UploadBuilder
    // Create extensions list that includes ebook files plus additional files that should be preserved
    let mut accepted_extensions = EbookType::all_extensions().iter().map(|s| s.to_string()).collect::<Vec<_>>();
    // Add additional extensions that should be preserved for ebooks
    accepted_extensions.extend_from_slice(&["diz".to_string(), "nfo".to_string()]);
    
    let mut builder = UploadBuilder::new(
        &actual_content_path,
        MediaType::Ebook(main_ebook_file.ebook_type.clone()),
        Arc::new(config.clone()),
    )
    .with_extensions(accepted_extensions.iter().map(|s| s.as_str()).collect::<Vec<_>>())
    .with_description_config(desc_config)
    .dry_run(dry_run);

    // Add title info (using enhanced metadata from Open Library if available)
    builder = builder.with_title_info(&title, None::<String>);

    // Add ebook-specific metadata
    let mut ebook_metadata = HashMap::new();
    ebook_metadata.insert("author".to_string(), author.clone());
    ebook_metadata.insert("description".to_string(), description.clone());
    ebook_metadata.insert("keywords".to_string(), keywords.clone());
    // Note: publisher, isbn, language would be extracted from ebook metadata if available
    // Store cover_id for post-upload processing
    if let Some(cid) = cover_id {
        ebook_metadata.insert("cover_id".to_string(), cid.to_string());
    }
    // Note: PDF cover extraction would be handled by component system

    builder = builder
        .with_nfo()
        .with_mediainfo()
        .with_duplicate_check()
        .with_custom_component(
            "ebook_metadata".to_string(),
            UploadComponent::Metadata(ebook_metadata),
        );

    // Add screenshots for comics/magazines (extract preview pages)
    if matches!(
        main_ebook_file.ebook_type,
        EbookType::Cbr | EbookType::Cbz | EbookType::Pdf
    ) {
        builder = builder.with_screenshots(6);
    }

    let upload_data = builder.build()?;

    // Create the upload processor
    let mut processor =
        UploadProcessor::new(upload_data, Arc::new(config.clone())).dry_run(dry_run);

    // Add classification for tracker mapping
    let category_str = if let Some(cat_arg) = category_arg {
        cat_arg.to_string()
    } else {
        format!("07{}", type_id)
    };
    processor = processor.with_media_classification(Some(category_str), None);

    // Process the upload
    let upload_result = processor.process()?;

    if !upload_result.success {
        return Err(format!("Upload failed: {}", upload_result.message));
    }

    info!("Upload completed successfully to {}", upload_result.tracker);
    let torrent_id = upload_result
        .torrent_id
        .ok_or("No torrent ID returned from upload")?;
    info!("Torrent ID: {}", torrent_id);

    // --- POST-UPLOAD COVER HANDLING ---
    // This preserves the existing Open Library and PDF cover upload logic

    // --- SKIP OPEN LIBRARY FOR COMICS & MAGAZINES ---
    if !main_ebook_file.ebook_type.is_comic()
        && !(matches!(main_ebook_file.ebook_type, EbookType::Pdf)
            && (type_id == "40" || type_id == "41"))
    {
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
                    let ol_title = first_result["title"].as_str().unwrap_or(&title).to_string();
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

    info!(
        "Processing eBook upload for title: '{}' and author: '{}'",
        title, author
    );

    // If PDF, extract cover image from first page using Ghostscript
    let mut pdf_cover_image_path = None;
    if matches!(main_ebook_file.ebook_type, EbookType::Pdf) {
        let cover_path = format!("{}.cover.jpg", new_ebook_path.to_str().unwrap());
        let output = std::process::Command::new("gs")
            .args(&[
                "-dBATCH",
                "-dNOPAUSE",
                "-sDEVICE=jpeg",
                "-dFirstPage=1",
                "-dLastPage=1",
                "-r150",
                "-dJPEGQ=95",
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

    // For EPUBs: Fetch the cover image using the cover ID from Open Library (existing logic)
    if !matches!(main_ebook_file.ebook_type, EbookType::Pdf) {
        let mut cover_handled = false;
        if let Some(cover_id) = cover_id {
            let cover_url = format!("https://covers.openlibrary.org/b/id/{}-L.jpg", cover_id);
            info!("Fetching cover image from: {}", cover_url);

            let client = Client::new();
            let cover_response = client
                .get(&cover_url)
                .send()
                .map_err(|e| format!("Failed to fetch cover image: {}", e))?;

            if cover_response.status().is_success() {
                // Save the cover image locally
                let cover_path = new_ebook_path.with_extension("jpg");
                std::fs::write(
                    &cover_path,
                    cover_response
                        .bytes()
                        .map_err(|e| format!("Failed to read cover image bytes: {}", e))?,
                )
                .map_err(|e| format!("Failed to save cover image: {}", e))?;

                info!("Saved cover image to: {}", cover_path.display());

                // Rename the cover image to include the torrent ID
                let renamed_cover_path =
                    cover_path.with_file_name(format!("torrent-cover_{}.jpg", torrent_id));
                std::fs::rename(&cover_path, &renamed_cover_path)
                    .map_err(|e| format!("Failed to rename cover image: {}", e))?;

                // Set permissions to 777 for the renamed cover image
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;

                    info!(
                        "Setting permissions to 777 for cover image: {}",
                        renamed_cover_path.display()
                    );
                    fs::set_permissions(&renamed_cover_path, fs::Permissions::from_mode(0o777))
                        .map_err(|e| {
                            format!(
                                "Failed to set permissions for cover image '{}': {}",
                                renamed_cover_path.display(),
                                e
                            )
                        })?;
                    info!(
                        "Successfully set permissions to 777 for cover image: {}",
                        renamed_cover_path.display()
                    );
                }

                // Upload the cover image to the CDN using SCP (skip during dry run)
                let remote_covers_path = format!(
                    "{}/covers",
                    seedpool_config
                        .screenshots
                        .remote_path
                        .trim_end_matches('/')
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

                info!(
                    "Successfully uploaded cover image to CDN: {}",
                    remote_covers_path
                );
                cover_handled = true;
            } else {
                warn!(
                    "Failed to fetch cover image with status: {}. Skipping cover image fetch.",
                    cover_response.status()
                );
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
                        .map_err(|e| {
                            format!(
                                "Failed to set permissions for cover image '{}': {}",
                                renamed_cover_path.display(),
                                e
                            )
                        })?;
                }
                let remote_covers_path = format!(
                    "{}/covers",
                    seedpool_config
                        .screenshots
                        .remote_path
                        .trim_end_matches('/')
                );
                // SCP to CDN (skip during dry run)
                if !dry_run {
                    let scp_command = std::process::Command::new("scp")
                        .arg(&renamed_cover_path)
                        .arg(&remote_covers_path)
                        .output()
                        .map_err(|e| {
                            format!("Failed to upload extracted cover image via SCP: {}", e)
                        })?;
                    if !scp_command.status.success() {
                        return Err(format!(
                            "Failed to upload extracted cover image via SCP. Error: {}",
                            String::from_utf8_lossy(&scp_command.stderr)
                        ));
                    }
                }
                info!(
                    "Successfully uploaded extracted EPUB cover image to CDN: {}",
                    remote_covers_path
                );
            } else {
                warn!("No images found to use as cover from EPUB.");
            }
        }
    }

    // For PDFs: Upload the extracted cover image (if any)
    if matches!(main_ebook_file.ebook_type, EbookType::Pdf) {
        if let Some(cover_path) = pdf_cover_image_path {
            // Rename the cover image to include the torrent ID
            let renamed_cover_path =
                Path::new(&cover_path).with_file_name(format!("torrent-cover_{}.jpg", torrent_id));
            std::fs::rename(&cover_path, &renamed_cover_path)
                .map_err(|e| format!("Failed to rename PDF cover image: {}", e))?;

            // Set permissions to 777 for the renamed cover image
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                info!(
                    "Setting permissions to 777 for cover image: {}",
                    renamed_cover_path.display()
                );
                std::fs::set_permissions(
                    &renamed_cover_path,
                    std::fs::Permissions::from_mode(0o777),
                )
                .map_err(|e| {
                    format!(
                        "Failed to set permissions for cover image '{}': {}",
                        renamed_cover_path.display(),
                        e
                    )
                })?;
                info!(
                    "Successfully set permissions to 777 for cover image: {}",
                    renamed_cover_path.display()
                );
            }

            info!(
                "Preparing to upload extracted PDF cover image: {}",
                renamed_cover_path.display()
            );
            let remote_covers_path = format!(
                "{}/covers",
                seedpool_config
                    .screenshots
                    .remote_path
                    .trim_end_matches('/')
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
            info!(
                "Successfully uploaded cover image to CDN: {}",
                remote_covers_path
            );
        }
    }

    // qBittorrent integration is handled by UploadProcessor

    // Files will be automatically restored by the RestoreGuard when function exits

    Ok(())
}

/// Generate a cover image for PDF ebooks using Ghostscript
pub fn generate_pdf_cover(
    pdf_path: &str,
    torrent_id: &str,
    config: &crate::core::Config,
    dry_run: bool,
) -> Result<String, String> {
    use std::fs;
    use std::path::Path;
    use std::process::Command;

    info!("📚 Generating PDF cover for torrent ID: {}", torrent_id);

    let pdf_path = Path::new(pdf_path);
    if !pdf_path.exists() {
        return Err(format!("PDF file not found: {}", pdf_path.display()));
    }

    // Create temp directory for cover generation
    let temp_dir = format!("{}/temp_covers", config.paths.screenshots_dir);
    fs::create_dir_all(&temp_dir)
        .map_err(|e| format!("Failed to create temp directory: {}", e))?;

    // Generate cover filename with torrent ID
    let cover_filename = format!("torrent-cover_{}.jpg", torrent_id);
    let cover_path = format!("{}/{}", temp_dir, cover_filename);

    if dry_run {
        info!("🔄 Dry run: Would generate PDF cover at {}", cover_path);
        return Ok(cover_path);
    }

    // Get Ghostscript path from config
    let (_, _, _, _, gs_path) = crate::core::Config::get_binary_paths(config);
    let gs_path_str = gs_path.to_string_lossy();

    // Use Ghostscript to extract first page as high-quality JPEG
    let gs_command = Command::new(&*gs_path_str)
        .args(&[
            "-dNOPAUSE",
            "-dBATCH",
            "-dSAFER",
            "-sDEVICE=jpeg",
            "-dJPEGQ=90",
            "-dGraphicsAlphaBits=4",
            "-dTextAlphaBits=4",
            "-dFirstPage=1",
            "-dLastPage=1",
            "-r300", // 300 DPI for high quality
            &format!("-sOutputFile={}", cover_path),
            pdf_path.to_str().unwrap(),
        ])
        .output()
        .map_err(|e| format!("Failed to run Ghostscript for cover generation: {}", e))?;

    if !gs_command.status.success() {
        return Err(format!(
            "Ghostscript failed to generate cover. stderr: {}",
            String::from_utf8_lossy(&gs_command.stderr)
        ));
    }

    // Verify the cover was created
    if !Path::new(&cover_path).exists() {
        return Err(format!("Cover image was not created at: {}", cover_path));
    }

    info!("✅ Successfully generated PDF cover: {}", cover_path);
    Ok(cover_path)
}

/// Process ebook file(s) from a path (file or directory) and classify content
pub fn process_ebook(
    input_path: &str,
    _config: &crate::core::Config,
    _dry_run: bool,
) -> Result<Vec<(EbookFile, EbookMetadata)>, String> {
    let path = Path::new(input_path);

    if !path.exists() {
        // For preflight mode with non-existent paths, return empty results
        info!("Path '{}' does not exist, returning empty ebook results for preflight mode", input_path);
        return Ok(Vec::new());
    }

    // Extract any archives first and get the path to process
    let processing_path =
        process_and_extract_archives(input_path).map_err(|e| format!("{:?}", e))?;

    let mut results = Vec::new();

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

        let filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");

        let mut metadata = classify_ebook_content(filename, extension);

        // Don't reject files if we have a forced category - this will be overridden later
        if metadata.category == EbookCategory::Unknown {
            info!("📚 Ebook file '{}' has unknown category, but may be overridden by forced category", filename);
            // Still include the file - the category will be overridden in ProcessBuilder if forced
            metadata.category = EbookCategory::Novel; // Default to Novel for processing
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

                        let filename = entry_path
                            .file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or("");

                        let mut metadata = classify_ebook_content(filename, extension);

                        // If classification failed but PDF is forced as comic/magazine, override it
                        if metadata.category == EbookCategory::Unknown && extension.to_lowercase() == "pdf" {
                            // Check if filename has comic/magazine indicators
                            let lower_filename = filename.to_lowercase();
                            if lower_filename.contains("comic") || lower_filename.contains("commando") || 
                               lower_filename.contains("magazine") || lower_filename.contains("mag") {
                                if lower_filename.contains("comic") || lower_filename.contains("commando") {
                                    metadata.category = EbookCategory::Comic;
                                } else {
                                    metadata.category = EbookCategory::Magazine;
                                }
                                info!("📚 Forced PDF category to {:?} based on filename patterns", metadata.category);
                            }
                        }

                        // Don't reject files if we have a forced category - this will be overridden later
                        if metadata.category == EbookCategory::Unknown {
                            info!("📚 Ebook file '{}' has unknown category, but may be overridden by forced category", filename);
                            // Still include the file - the category will be overridden in ProcessBuilder if forced
                            metadata.category = EbookCategory::Novel; // Default to Novel for processing
                        }

                        results.push((ebook_file, metadata));
                    }
                }
            }
        }

        // No longer rejecting files due to unknown category
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
        use crate::core::ImageLayout;
        use crate::processing::description::DescriptionConfig;

        // Configure description based on ebook type
        let mut desc_config = DescriptionConfig::default();

        // Different layouts for different ebook types
        match metadata.category {
            EbookCategory::Comic => {
                desc_config.image_layout = ImageLayout::TwoColumn; // Comics use 2-column table for preview pages
                desc_config.max_images = 10; // Show more preview pages
                desc_config.image_width = 500; // Larger width to force 2-column layout
            }
            EbookCategory::Magazine | EbookCategory::Newspaper => {
                desc_config.image_layout = ImageLayout::TwoColumn; // Magazines/newspapers use 2-column table
                desc_config.max_images = 6; // Show several pages
                desc_config.image_width = 500; // Larger width to force 2-column layout
            }
            _ => {
                desc_config.image_layout = ImageLayout::SingleColumn; // Regular books use single column for cover
                desc_config.max_images = 2; // Front and back cover
                desc_config.image_width = 500;
            }
        }

        // Create the upload builder with ebook-specific components
        // Create extensions list that includes ebook files plus additional files that should be preserved
        let mut accepted_extensions = EbookType::all_extensions().iter().map(|s| s.to_string()).collect::<Vec<_>>();
        // Add additional extensions that should be preserved for ebooks
        accepted_extensions.extend_from_slice(&["diz".to_string(), "nfo".to_string()]);
        
        let mut builder = UploadBuilder::new(
            &processing_path,
            MediaType::Ebook(ebook_file.ebook_type.clone()),
            Arc::new((*_config).clone()),
        )
        .with_extensions(accepted_extensions.iter().map(|s| s.as_str()).collect::<Vec<_>>())
        .with_description_config(desc_config)
        .dry_run(_dry_run);

        // Add title info
        builder = builder.with_title_info(
            &metadata.title,
            metadata.year.map(|y| y.to_string()).as_deref(),
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
            .with_custom_component(
                "ebook_metadata",
                crate::core::UploadComponent::Metadata(ebook_metadata),
            );

        // Add screenshots for comics/magazines (extract preview pages)
        if matches!(
            metadata.category,
            EbookCategory::Comic | EbookCategory::Magazine | EbookCategory::Newspaper
        ) {
            builder = builder.with_screenshots(6); // Extract 6 preview pages
        }

        // For comics, also handle comic image extraction if needed
        if ebook_file.ebook_type.is_comic() {
            // TODO: Extract comic images for preview
            // This would be handled by the upload builder's screenshot component
        }

        // NOTE: Upload processing is now handled by the main ProcessBuilder flow
        // This function only extracts metadata and returns it for use by ProcessBuilder
        // The ebook-specific upload builder has been removed to prevent duplicate uploads
        info!("Ebook metadata extraction completed without upload processing");
    }

    // Cleanup unnecessary files based on ebook type
    cleanup_ebook_files(&processing_path, &results)?;

    Ok(results)
}

/// Detect ebook files in a path (without metadata classification)
pub fn detect_ebook_files(path: &str) -> Result<Vec<EbookFile>, String> {
    let mut ebook_files = Vec::new();
    detect_ebook_files_recursive(Path::new(path), &mut ebook_files)?;
    Ok(ebook_files)
}

/// Clean up unnecessary files based on ebook type
pub fn cleanup_ebook_files(
    processing_path: &str,
    results: &[(EbookFile, EbookMetadata)],
) -> Result<(), String> {
    let path = Path::new(processing_path);
    
    if !path.is_dir() {
        return Ok(()); // Nothing to clean up for single files
    }
    
    // Check if we have any PDF ebooks (need different cleanup rules)
    let has_pdf_ebooks = results.iter().any(|(ebook_file, _)| {
        matches!(ebook_file.ebook_type, EbookType::Pdf)
    });
    
    // Debug: Log what types we actually found
    for (ebook_file, _) in results {
        info!("📚 Cleanup: Found ebook type: {:?} at path: {}", ebook_file.ebook_type, ebook_file.path.display());
    }
    info!("📚 Cleanup: has_pdf_ebooks = {}", has_pdf_ebooks);
    
    if has_pdf_ebooks {
        info!("📚 Cleaning up unnecessary files for PDF ebooks (keeping only PDF and .nfo files)");
    } else {
        info!("📚 Cleaning up unnecessary files for EPUB/CBR/CBZ ebooks (keeping ebook files and .nfo, removing archives)");
    }
    
    // Read directory and remove unwanted files
    for entry in fs::read_dir(path).map_err(|e| format!("Failed to read directory: {}", e))? {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
        let entry_path = entry.path();
        
        if entry_path.is_file() {
            if let Some(extension) = entry_path.extension().and_then(|ext| ext.to_str()) {
                let extension_lower = extension.to_lowercase();
                
                // Determine what to keep based on ebook type
                let should_remove = if has_pdf_ebooks {
                    // For PDF ebooks: Keep PDF files, NFO files, and .diz files, remove everything else
                    match extension_lower.as_str() {
                        "pdf" | "nfo" | "diz" => false, // Keep these
                        "rar" | "zip" => true, // Remove archives
                        ext if ext.starts_with("r") && ext.len() >= 3 => {
                            // Check if it's a RAR part file (r00, r01, r02, ..., r99, etc.)
                            ext[1..].chars().all(|c| c.is_ascii_digit())
                        }
                        _ => true, // Remove everything else by default for PDF ebooks
                    }
                } else {
                    // For EPUB/CBR/CBZ ebooks: Keep the ebook files, NFO, and .diz files, remove archives
                    match extension_lower.as_str() {
                        "diz" => false, // Always keep .diz files
                        "epub" | "cbr" | "cbz" => {
                            // For EPUB files, keep the main one and remove duplicates
                            if extension_lower == "epub" {
                                // Check if this is one of the main ebook files we detected
                                let is_main_ebook = results.iter().any(|(ebook_file, _)| {
                                    ebook_file.path == entry_path
                                });
                                
                                if is_main_ebook {
                                    false // Keep the main EPUB file
                                } else {
                                    // This is a duplicate EPUB, remove it
                                    true
                                }
                            } else {
                                false // Keep CBR/CBZ files
                            }
                        }
                        "rar" | "zip" => true, // Remove archives
                        ext if ext.starts_with("r") && ext.len() >= 3 => {
                            // Check if it's a RAR part file (r00, r01, r02, ..., r99, etc.)
                            ext[1..].chars().all(|c| c.is_ascii_digit())
                        }
                        _ => true, // Remove everything else by default
                    }
                };
                
                if should_remove {
                    match fs::remove_file(&entry_path) {
                        Ok(()) => {
                            info!("🗑️ Removed unnecessary file: {}", entry_path.display());
                        }
                        Err(e) => {
                            info!("⚠️ Failed to remove file {}: {}", entry_path.display(), e);
                        }
                    }
                }
            }
        }
    }
    
    Ok(())
}

/// Recursively search for ebook files in a directory tree
fn detect_ebook_files_recursive(
    path: &Path,
    ebook_files: &mut Vec<EbookFile>,
) -> Result<(), String> {
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
        for entry in
            fs::read_dir(path).map_err(|e| format!("Failed to read directory {:?}: {}", path, e))?
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
    let author_title_regex =
        Regex::new(r"^([^-]+?)\s*-\s*(.+?)(?:\s*\((\d{4})\))?(?:\s*\[(.+?)\])?$").unwrap();
    let year_regex = Regex::new(r"\b(19|20)\d{2}\b").unwrap();
    let edition_regex = Regex::new(r"(?i)\b(\d+)(?:st|nd|rd|th)?\s*(?:edition|ed\.?)\b").unwrap();
    let volume_regex = Regex::new(r"(?i)\b(?:vol|volume)\.?\s*(\d+)\b").unwrap();
    let issue_regex = Regex::new(r"(?i)\b(?:issue|#)\s*(\d+)\b").unwrap();
    let isbn_regex = Regex::new(r"(?i)\b(?:isbn[-\s]?(?:10|13)?:?\s*)([\d-]+)\b").unwrap();

    // Category patterns
    let comic_regex = Regex::new(r"(?i)\b(comic|comics|manga|graphic\.novel|cbr|cbz)\b").unwrap();
    let magazine_regex = Regex::new(r"(?i)\b(magazine|mag|periodical|journal|monthly|weekly|quarterly|review|economist|geographic|nature|psychology|car\s+and\s+driver)\b").unwrap();
    let newspaper_regex =
        Regex::new(r"(?i)\b(newspaper|news|daily|times|post|gazette|herald|tribune)\b").unwrap();
    let technical_regex = Regex::new(r"(?i)\b(programming|coding|software|computer|technology|technical|engineering|mathematics|physics|chemistry|algorithm|python|java|javascript|mit\s+press|introduction\s+to\s+algorithms)\b").unwrap();
    let educational_regex = Regex::new(r"(?i)\b(textbook|course|tutorial|guide|manual|handbook|education|learning|study|exam|test\.prep)\b").unwrap();
    let biography_regex =
        Regex::new(r"(?i)\b(biography|autobiography|memoir|life\.of|story\.of)\b").unwrap();
    let history_regex =
        Regex::new(r"(?i)\b(history|historical|ancient|medieval|war|battle|civilization)\b")
            .unwrap();
    let science_regex = Regex::new(r"(?i)\b(science|scientific|biology|astronomy|geology|research|medical|medicine|health|clinic|anatomy|physics|einstein)\b").unwrap();
    let religion_regex = Regex::new(r"(?i)\b(bible|quran|torah|religion|religious|spiritual|theology|buddhism|christianity|islam|hindu)\b").unwrap();
    let cookbook_regex = Regex::new(r"(?i)\b(cookbook|cooking|recipe|recipes|cuisine|culinary|baking|food|crocker|ramsay|oliver)\b").unwrap();
    let travel_regex = Regex::new(r"(?i)\b(travel|guide|lonely\.planet|frommer|tourism|vacation|michelin|rick\s+steves|through\s+the\s+back\s+door)\b").unwrap();
    let children_regex =
        Regex::new(r"(?i)\b(children|kids|juvenile|young\.adult|ya|picture\.book|seuss)\b")
            .unwrap();

    // Series patterns
    let series_regex = Regex::new(r"(?i)\b(?:book|part|series)\s*(\d+)\b").unwrap();

    info!("Classifying ebook content for: {}", filename);

    // Clean filename for processing
    let clean_name = filename.trim().trim_end_matches(&format!(".{}", extension));

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
        metadata.title = clean_name
            .replace('_', " ")
            .replace('.', " ")
            .trim()
            .to_string();
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
    if metadata
        .format_type
        .as_ref()
        .map(|t| t.is_comic())
        .unwrap_or(false)
        || comic_regex.is_match(filename)
    {
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

    info!("Ebook classification result: {:?}", metadata);
    metadata
}

/// Classify ebook content for upload pipeline
pub fn classify_for_upload(
    input_path: &str,
    metadata: &serde_json::Value,
) -> Result<(Option<String>, Option<String>, serde_json::Value), String> {
    // Check if we already have classification in metadata
    if let Some(format_str) = metadata.get("format").and_then(|f| f.as_str()) {
        let category = if format_str.contains("Comic")
            || format_str.contains("Cbr")
            || format_str.contains("Cbz")
        {
            Some("EbookCategory::Comic".to_string())
        } else {
            Some("EbookCategory::General".to_string())
        };

        return Ok((category, None, metadata.clone()));
    }

    // Otherwise, detect and classify
    if let Ok(ebook_files) = detect_ebook_files(input_path) {
        if let Some(ebook_file) = ebook_files.first() {
            let filename = ebook_file
                .path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            let extension = ebook_file
                .path
                .extension()
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
    Ok((
        Some("EbookCategory::General".to_string()),
        None,
        metadata.clone(),
    ))
}

/// Generate default non-video description footer
fn default_non_video_description() -> String {
    format!("[center][b][color=#E74C3C]Uploaded with seedbrr[/color][/b][/center]")
}
