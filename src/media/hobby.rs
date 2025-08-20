use crate::core::types::{HobbyCategory, HobbyFile, HobbyType, MediaFile, MediaType};
use crate::processing::extraction::process_and_extract_archives;
use log::{info, warn};
use regex::Regex;
use std::path::Path;

/// Hobby metadata extracted from filename and content
#[derive(Debug, Clone)]
pub struct HobbyMetadata {
    pub title: String,
    pub category: HobbyCategory,
    pub year: Option<u32>,
    pub version: Option<String>,
    pub author: Option<String>,
    pub language: Option<String>,
    pub description: Option<String>,
    pub file_count: usize,
    pub total_size: u64,
}

impl Default for HobbyMetadata {
    fn default() -> Self {
        Self {
            title: String::new(),
            category: HobbyCategory::Unknown,
            year: None,
            version: None,
            author: None,
            language: None,
            description: None,
            file_count: 0,
            total_size: 0,
        }
    }
}

/// Generate hobby description with template support
pub fn generate_description_with_template(
    metadata: &serde_json::Value,
    enriched_metadata: Option<&std::collections::HashMap<String, String>>,
    template_name: Option<&str>,
) -> Result<String, String> {
    use crate::templates::TemplateProcessor;

    let template_processor = TemplateProcessor::with_defaults()
        .map_err(|e| format!("Failed to initialize template processor: {}", e))?;

    let template_to_use = template_name.unwrap_or("default");

    if let Some(template) = template_processor.get_template("hobby", template_to_use) {
        template_processor.apply_template(template, metadata, enriched_metadata)
    } else {
        // Fallback to traditional description generation
        Ok(generate_description(metadata))
    }
}

/// Process hobby file(s) from a path (file or directory) and classify content
pub fn process_hobby(
    input_path: &str,
    _config: &crate::core::Config,
    _dry_run: bool,
) -> Result<Vec<(HobbyFile, HobbyMetadata)>, String> {
    let path = Path::new(input_path);

    if !path.exists() {
        // For preflight mode with non-existent paths, return empty results
        info!("Path '{}' does not exist, returning empty hobby results for preflight mode", input_path);
        return Ok(Vec::new());
    }

    // Extract any archives first and get the path to process
    let processing_path =
        process_and_extract_archives(input_path).map_err(|e| format!("{:?}", e))?;

    let mut results = Vec::new();

    // Update path to use the processing path
    let path = Path::new(&processing_path);

    if path.is_file() {
        // Single file case
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .ok_or_else(|| "Could not determine file extension".to_string())?;

        let hobby_type = HobbyType::from_extension(extension)
            .ok_or_else(|| format!("Unsupported hobby file type: {}", extension))?;

        let hobby_file = HobbyFile {
            path: path.to_path_buf(),
            hobby_type: hobby_type.clone(),
        };

        let filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");

        let metadata = classify_hobby_content(filename, &hobby_type);

        results.push((hobby_file, metadata));
    } else if path.is_dir() {
        // Directory case - check if it's a collection
        let mut total_size = 0u64;
        let mut file_count = 0;
        let mut files_by_type = std::collections::HashMap::new();

        // Analyze directory contents
        for entry in
            std::fs::read_dir(path).map_err(|e| format!("Failed to read directory: {}", e))?
        {
            let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
            let file_path = entry.path();

            if file_path.is_file() {
                if let Ok(metadata) = entry.metadata() {
                    total_size += metadata.len();
                    file_count += 1;
                }

                if let Some(extension) = file_path.extension().and_then(|ext| ext.to_str()) {
                    if let Some(hobby_type) = HobbyType::from_extension(extension) {
                        let hobby_file = HobbyFile {
                            path: file_path.clone(),
                            hobby_type: hobby_type.clone(),
                        };

                        let filename = file_path
                            .file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or("");

                        let metadata = classify_hobby_content(filename, &hobby_type);

                        // Track file types for collection analysis
                        let type_name = format!("{:?}", hobby_type);
                        *files_by_type.entry(type_name).or_insert(0) += 1;

                        info!(
                            "Processed hobby file: {} -> Category: {:?}",
                            filename, metadata.category
                        );

                        results.push((hobby_file, metadata));
                    }
                }
            }
        }

        // If we processed files, create a collection metadata
        if !results.is_empty() {
            let collection_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Hobby Collection");

            // Determine dominant category
            let dominant_category = determine_collection_category(&files_by_type);

            // Create a directory entry for the collection
            let collection_file = HobbyFile {
                path: path.to_path_buf(),
                hobby_type: HobbyType::Directory,
            };

            let mut collection_metadata = HobbyMetadata {
                title: collection_name.to_string(),
                category: dominant_category,
                file_count,
                total_size,
                ..Default::default()
            };

            // Extract metadata from collection name
            if let Some(year) = extract_year_from_name(collection_name) {
                collection_metadata.year = Some(year);
            }

            results.insert(0, (collection_file, collection_metadata));
        }
    }

    if results.is_empty() {
        return Err("No hobby files found in the specified path".to_string());
    }

    // After we have the results, build the upload data if we have hobby files
    if !results.is_empty() {
        use crate::processing::upload::UploadBuilder;
        use std::sync::Arc;

        let (_hobby_file, metadata) = &results[0];

        // Build upload data directly using UploadBuilder
        // use crate::description::DescriptionConfig;
        // use crate::core::ImageLayout;

        // Configure description based on hobby type
        // let mut desc_config = DescriptionConfig::default();

        // match metadata.category {
        //     HobbyCategory::Images => {
        //         desc_config.image_layout = ImageLayout::Gallery;
        //         desc_config.max_images = 12;
        //         desc_config.image_width = 400;
        //     }
        //     HobbyCategory::CAD3D => {
        //         desc_config.image_layout = ImageLayout::TwoColumn;
        //         desc_config.max_images = 6;
        //         desc_config.image_width = 500;
        //     }
        //     _ => {
        //         desc_config.image_layout = ImageLayout::SingleColumn;
        //         desc_config.max_images = 2;
        //         desc_config.image_width = 600;
        //     }
        // }

        // Create the upload builder with hobby-specific components
        let mut builder = UploadBuilder::new(
            &processing_path,
            MediaType::Hobby(HobbyType::Directory),
            Arc::new((*_config).clone()),
        )
        .with_extensions(HobbyType::all_extensions())
        // .with_description_config(desc_config)
        .dry_run(_dry_run);

        // Add title info
        builder = builder.with_title_info(
            &metadata.title,
            metadata.year.map(|y| y.to_string()).as_deref(),
        );

        // Add hobby-specific metadata
        let mut hobby_metadata = std::collections::HashMap::new();
        hobby_metadata.insert("title".to_string(), metadata.title.clone());
        hobby_metadata.insert("category".to_string(), format!("{:?}", metadata.category));
        if let Some(author) = &metadata.author {
            hobby_metadata.insert("author".to_string(), author.clone());
        }
        if let Some(version) = &metadata.version {
            hobby_metadata.insert("version".to_string(), version.clone());
        }
        if let Some(language) = &metadata.language {
            hobby_metadata.insert("language".to_string(), language.clone());
        }
        if metadata.file_count > 0 {
            hobby_metadata.insert("file_count".to_string(), metadata.file_count.to_string());
            hobby_metadata.insert(
                "total_size_mb".to_string(),
                (metadata.total_size / 1_000_000).to_string(),
            );
        }

        builder = builder
            .with_nfo()
            .with_duplicate_check()
            .with_custom_component(
                "hobby_metadata",
                crate::core::UploadComponent::Metadata(hobby_metadata),
            );

        // Add screenshots for image collections
        if metadata.category == HobbyCategory::Images {
            builder = builder.with_screenshots(6);
        }

        let _upload_data = builder.build()?;

        info!("Built upload data for hobby processing");

        // Create the upload processor - it will auto-detect the active tracker
        let mut processor = crate::processing::upload::UploadProcessor::new(
            _upload_data,
            std::sync::Arc::new(_config.clone()),
        )
        .dry_run(_dry_run);

        // Get media classification for mapping
        if !results.is_empty() {
            let (_, metadata) = &results[0];
            let category_str = format!("HobbyCategory::{:?}", metadata.category);

            processor = processor.with_media_classification(
                Some(category_str),
                None, // Hobby files don't have source types
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

/// Classify hobby content based on filename and type
fn classify_hobby_content(filename: &str, hobby_type: &HobbyType) -> HobbyMetadata {
    let mut metadata = HobbyMetadata::default();

    // Set category based on file type
    metadata.category = match hobby_type {
        HobbyType::Doc | HobbyType::Docx | HobbyType::Txt | HobbyType::Rtf => {
            HobbyCategory::Documents
        }
        HobbyType::Jpg
        | HobbyType::Png
        | HobbyType::Gif
        | HobbyType::Bmp
        | HobbyType::Tiff
        | HobbyType::Svg => HobbyCategory::Images,
        HobbyType::Dwg | HobbyType::Dxf | HobbyType::Stl | HobbyType::Obj | HobbyType::Ply => {
            HobbyCategory::CAD3D
        }
        HobbyType::Zip | HobbyType::Rar | HobbyType::SevenZ => HobbyCategory::Archives,
        HobbyType::Csv | HobbyType::Json | HobbyType::Xml | HobbyType::Sql => {
            HobbyCategory::DataFiles
        }
        HobbyType::Ttf | HobbyType::Otf | HobbyType::Woff => HobbyCategory::Fonts,
        HobbyType::Directory => HobbyCategory::Collection,
    };

    // Initialize regex patterns
    let year_regex = Regex::new(r"\b(19|20)\d{2}\b").unwrap();
    let version_regex = Regex::new(r"(?i)\b(?:v|ver|version)\.?\s*(\d+(?:\.\d+)*)\b").unwrap();
    let tutorial_regex = Regex::new(r"(?i)\b(tutorial|guide|howto|manual|instructions)\b").unwrap();
    let template_regex = Regex::new(r"(?i)\b(template|templates|mockup|preset|presets)\b").unwrap();
    let resource_regex =
        Regex::new(r"(?i)\b(resource|resources|asset|assets|pack|bundle|collection)\b").unwrap();
    let language_regex =
        Regex::new(r"(?i)\b(english|spanish|french|german|italian|russian|japanese|chinese)\b")
            .unwrap();

    // Clean filename for processing
    let clean_name = filename
        .trim()
        .trim_end_matches(|c: char| c == '.' || c.is_numeric());

    // Extract title
    metadata.title = clean_name
        .replace('_', " ")
        .replace('.', " ")
        .replace('-', " ")
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ");

    // Extract year
    if let Some(year_match) = year_regex.find(clean_name) {
        metadata.year = year_match.as_str().parse::<u32>().ok();
    }

    // Extract version
    if let Some(version_match) = version_regex.captures(clean_name) {
        if let Some(ver) = version_match.get(1) {
            metadata.version = Some(ver.as_str().to_string());
        }
    }

    // Refine category based on content patterns
    if tutorial_regex.is_match(clean_name) {
        metadata.category = HobbyCategory::Tutorial;
    } else if template_regex.is_match(clean_name) {
        metadata.category = HobbyCategory::Template;
    } else if resource_regex.is_match(clean_name) {
        metadata.category = HobbyCategory::Resource;
    }

    // Extract language
    if let Some(lang_match) = language_regex.find(clean_name) {
        metadata.language = Some(lang_match.as_str().to_string());
    }

    metadata
}

/// Determine the dominant category for a collection
fn determine_collection_category(
    files_by_type: &std::collections::HashMap<String, usize>,
) -> HobbyCategory {
    let mut max_count = 0;
    let mut dominant_type = "Unknown";

    for (type_name, count) in files_by_type {
        if *count > max_count {
            max_count = *count;
            dominant_type = type_name;
        }
    }

    // Map the dominant file type to a category
    match dominant_type {
        s if s.contains("Doc") || s.contains("Txt") || s.contains("Rtf") => {
            HobbyCategory::Documents
        }
        s if s.contains("Jpg") || s.contains("Png") || s.contains("Gif") => HobbyCategory::Images,
        s if s.contains("Dwg") || s.contains("Dxf") || s.contains("Stl") => HobbyCategory::CAD3D,
        s if s.contains("Zip") || s.contains("Rar") => HobbyCategory::Archives,
        s if s.contains("Csv") || s.contains("Json") || s.contains("Xml") => {
            HobbyCategory::DataFiles
        }
        s if s.contains("Ttf") || s.contains("Otf") => HobbyCategory::Fonts,
        _ => HobbyCategory::Collection,
    }
}

/// Extract year from filename
fn extract_year_from_name(name: &str) -> Option<u32> {
    let year_regex = Regex::new(r"\b(19|20)\d{2}\b").unwrap();
    if let Some(year_match) = year_regex.find(name) {
        year_match.as_str().parse::<u32>().ok()
    } else {
        None
    }
}

/// Detect hobby files in a path
pub fn detect_hobby_files(path: &str) -> Result<Vec<HobbyFile>, String> {
    let mut hobby_files = Vec::new();
    detect_hobby_files_recursive(Path::new(path), &mut hobby_files)?;
    Ok(hobby_files)
}

/// Recursively search for hobby files in a directory tree
fn detect_hobby_files_recursive(
    path: &Path,
    hobby_files: &mut Vec<HobbyFile>,
) -> Result<(), String> {
    if path.is_file() {
        if let Some(extension) = path.extension().and_then(|ext| ext.to_str()) {
            if let Some(hobby_type) = HobbyType::from_extension(extension) {
                hobby_files.push(HobbyFile {
                    path: path.to_path_buf(),
                    hobby_type,
                });
            }
        }
    } else if path.is_dir() {
        // Check if directory itself should be treated as a hobby collection
        if looks_like_hobby_collection(path) {
            hobby_files.push(HobbyFile {
                path: path.to_path_buf(),
                hobby_type: HobbyType::Directory,
            });
            // Don't recurse into hobby collection directories
            return Ok(());
        }

        // Recursively search subdirectories
        for entry in std::fs::read_dir(path)
            .map_err(|e| format!("Failed to read directory {:?}: {}", path, e))?
        {
            let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
            let entry_path = entry.path();

            // Recursively process subdirectories and files
            detect_hobby_files_recursive(&entry_path, hobby_files)?;
        }
    }

    Ok(())
}

/// Check if directory looks like a hobby collection
fn looks_like_hobby_collection(path: &Path) -> bool {
    if !path.is_dir() {
        return false;
    }

    let mut file_count = 0;
    let mut hobby_file_count = 0;

    for entry in std::fs::read_dir(path).unwrap_or_else(|_| std::fs::read_dir(".").unwrap()) {
        if let Ok(entry) = entry {
            let file_path = entry.path();
            file_count += 1;

            if file_path.is_file() {
                if let Some(extension) = file_path.extension().and_then(|ext| ext.to_str()) {
                    if HobbyType::from_extension(extension).is_some() {
                        hobby_file_count += 1;
                    }
                }
            }
        }
    }

    // Consider it a hobby collection if majority of files are hobby files
    file_count > 0 && hobby_file_count as f32 / file_count as f32 > 0.5
}

/// Convert HobbyFile to MediaFile
pub fn to_media_file(hobby_file: &HobbyFile) -> MediaFile {
    MediaFile {
        path: hobby_file.path.clone(),
        media_type: MediaType::Hobby(hobby_file.hobby_type.clone()),
    }
}

/// Classify hobby content for upload pipeline
pub fn classify_for_upload(
    input_path: &str,
    metadata: &serde_json::Value,
) -> Result<(Option<String>, Option<String>, serde_json::Value), String> {
    // Check if we have any specific category hints in metadata
    if let Some(category) = metadata.get("category").and_then(|c| c.as_str()) {
        // Ensure we have comprehensive metadata structure
        let mut enriched_metadata = metadata.clone();

        // Add placeholder fields that description system expects if they don't exist
        let fields_to_check = [
            "title",
            "author",
            "version",
            "language",
            "images",
            "instructions",
            "requirements",
            "usage_notes",
        ];
        for field in fields_to_check.iter() {
            if !enriched_metadata.get(field).is_some() {
                match *field {
                    "images" => enriched_metadata[field] = serde_json::Value::Array(vec![]),
                    _ => enriched_metadata[field] = serde_json::Value::Null,
                }
            }
        }

        return Ok((
            Some(format!("HobbyCategory::{}", category)),
            None,
            enriched_metadata,
        ));
    }

    // Otherwise, detect and classify from input path
    if let Ok(hobby_files) = detect_hobby_files(input_path) {
        if let Some(hobby_file) = hobby_files.first() {
            let filename = hobby_file
                .path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            let hobby_metadata = classify_hobby_content(filename, &hobby_file.hobby_type);

            let category = Some(format!("HobbyCategory::{:?}", hobby_metadata.category));

            // Create comprehensive JSON metadata
            let json_metadata = serde_json::json!({
                "title": hobby_metadata.title,
                "category": format!("{:?}", hobby_metadata.category),
                "author": hobby_metadata.author,
                "year": hobby_metadata.year,
                "version": hobby_metadata.version,
                "language": hobby_metadata.language,
                "description": hobby_metadata.description,
                "file_count": hobby_metadata.file_count,
                "total_size_mb": hobby_metadata.total_size / 1_000_000,
                // Placeholder fields for enrichment
                "images": serde_json::Value::Array(vec![]),
                "instructions": serde_json::Value::Null,
                "requirements": serde_json::Value::Null,
                "usage_notes": serde_json::Value::Null,
            });

            return Ok((category, None, json_metadata));
        }
    }

    // Default to general hobby with basic metadata structure
    let default_metadata = serde_json::json!({
        "title": "Unknown Hobby Content",
        "category": "General",
        "images": serde_json::Value::Array(vec![]),
        "instructions": serde_json::Value::Null,
        "requirements": serde_json::Value::Null,
        "usage_notes": serde_json::Value::Null,
    });

    Ok((
        Some("HobbyCategory::General".to_string()),
        None,
        default_metadata,
    ))
}

/// Generate a description for hobby content
pub fn generate_description(metadata: &serde_json::Value) -> String {
    use crate::core::{DescriptionComponent, HobbyType, ImageLayout, MediaType, SectionFormat};
    use crate::processing::description::{DescriptionBuilder, DescriptionConfig};

    // Configure description builder for hobby content
    let mut config = DescriptionConfig::default();
    config.image_layout = ImageLayout::TwoColumn;
    config.max_images = 6;

    let mut builder =
        DescriptionBuilder::with_config(MediaType::Hobby(HobbyType::Directory), config);

    // Add title
    if let Some(title) = metadata.get("title").and_then(|t| t.as_str()) {
        builder = builder.title(title);
    }

    // Add author if available
    if let Some(author) = metadata.get("author").and_then(|a| a.as_str()) {
        builder = builder.author(author);
    }

    // Add description/synopsis if available
    if let Some(description) = metadata.get("description").and_then(|d| d.as_str()) {
        builder = builder.synopsis(description);
    }

    // Add images if available
    let images: Vec<String> = metadata
        .get("images")
        .and_then(|i| i.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();

    if !images.is_empty() {
        builder = builder.images(images);
    }

    // Create hobby information table
    let mut info_rows = Vec::new();

    // Category
    if let Some(category) = metadata.get("category").and_then(|c| c.as_str()) {
        info_rows.push(vec!["Category".to_string(), category.to_string()]);
    }

    // Year
    if let Some(year) = metadata.get("year").and_then(|y| y.as_u64()) {
        info_rows.push(vec!["Year".to_string(), year.to_string()]);
    }

    // Version
    if let Some(version) = metadata.get("version").and_then(|v| v.as_str()) {
        info_rows.push(vec!["Version".to_string(), version.to_string()]);
    }

    // Language
    if let Some(language) = metadata.get("language").and_then(|l| l.as_str()) {
        info_rows.push(vec!["Language".to_string(), language.to_string()]);
    }

    // File count (for collections)
    if let Some(file_count) = metadata.get("file_count").and_then(|f| f.as_u64()) {
        info_rows.push(vec!["Files".to_string(), file_count.to_string()]);
    }

    // Total size (for collections)
    if let Some(total_size_mb) = metadata.get("total_size_mb").and_then(|s| s.as_u64()) {
        info_rows.push(vec![
            "Total Size".to_string(),
            format!("{} MB", total_size_mb),
        ]);
    }

    // Add hobby information table
    if !info_rows.is_empty() {
        builder = builder.add_component(DescriptionComponent::Table { rows: info_rows });
    }

    // Add installation instructions if available
    if let Some(instructions) = metadata.get("instructions").and_then(|i| i.as_str()) {
        builder = builder.custom_section(
            "Installation Instructions",
            instructions,
            SectionFormat::Quoted,
        );
    }

    // Add requirements if available
    if let Some(requirements) = metadata.get("requirements").and_then(|r| r.as_str()) {
        builder = builder.custom_section("Requirements", requirements, SectionFormat::Plain);
    }

    // Add usage notes if available
    if let Some(usage_notes) = metadata.get("usage_notes").and_then(|u| u.as_str()) {
        builder = builder.custom_section("Usage Notes", usage_notes, SectionFormat::Spoiler);
    }

    // Add custom description if available
    if let Some(custom_desc) = metadata.get("custom_description").and_then(|d| d.as_str()) {
        if !custom_desc.is_empty() {
            builder = builder.raw(custom_desc);
        }
    }

    builder.build()
}
