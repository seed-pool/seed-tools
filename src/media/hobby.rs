use crate::types::{HobbyFile, HobbyType, MediaFile, MediaType, HobbyCategory};
use std::path::Path;
use log::{info, warn};
use regex::Regex;
use crate::extraction::process_and_extract_archives;

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

/// Process hobby file(s) from a path (file or directory) and classify content
pub fn process_hobby(
    input_path: &str,
    _config: &crate::types::Config,
    _dry_run: bool,
) -> Result<Vec<(HobbyFile, HobbyMetadata)>, String> {
    let path = Path::new(input_path);
    
    if !path.exists() {
        return Err(format!("Path not found: {}", input_path));
    }
    
    // Extract any archives first and get the path to process
    let processing_path = process_and_extract_archives(input_path)?;
    
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
        
        let filename = path.file_name()
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
        for entry in std::fs::read_dir(path)
            .map_err(|e| format!("Failed to read directory: {}", e))? 
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
                        
                        let filename = file_path.file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or("");
                        
                        let metadata = classify_hobby_content(filename, &hobby_type);
                        
                        // Track file types for collection analysis
                        let type_name = format!("{:?}", hobby_type);
                        *files_by_type.entry(type_name).or_insert(0) += 1;
                        
                        info!("Processed hobby file: {} -> Category: {:?}", 
                              filename, metadata.category);
                        
                        results.push((hobby_file, metadata));
                    }
                }
            }
        }
        
        // If we processed files, create a collection metadata
        if !results.is_empty() {
            let collection_name = path.file_name()
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
        use crate::upload::UploadBuilder;
        use std::sync::Arc;
        
        let (_hobby_file, metadata) = &results[0];
        
        // Build upload data directly using UploadBuilder
        use crate::description::DescriptionConfig;
        use crate::types::ImageLayout;
        
        // Configure description based on hobby type
        let mut desc_config = DescriptionConfig::default();
        
        match metadata.category {
            HobbyCategory::Images => {
                desc_config.image_layout = ImageLayout::Gallery;
                desc_config.max_images = 12;
                desc_config.image_width = 400;
            }
            HobbyCategory::CAD3D => {
                desc_config.image_layout = ImageLayout::TwoColumn;
                desc_config.max_images = 6;
                desc_config.image_width = 500;
            }
            _ => {
                desc_config.image_layout = ImageLayout::SingleColumn;
                desc_config.max_images = 2;
                desc_config.image_width = 600;
            }
        }
        
        // Create the upload builder with hobby-specific components
        let mut builder = UploadBuilder::new(
            &processing_path,
            MediaType::Hobby(HobbyType::Directory),
            Arc::new((*_config).clone())
        )
        .with_extensions(HobbyType::all_extensions())
        .with_description_config(desc_config)
        .dry_run(_dry_run);
        
        // Add title info
        builder = builder.with_title_info(
            &metadata.title, 
            metadata.year.map(|y| y.to_string()).as_deref()
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
            hobby_metadata.insert("total_size_mb".to_string(), (metadata.total_size / 1_000_000).to_string());
        }
        
        builder = builder
            .with_nfo()
            .with_duplicate_check()
            .with_custom_component("hobby_metadata", crate::types::UploadComponent::Metadata(hobby_metadata));
        
        // Add screenshots for image collections
        if metadata.category == HobbyCategory::Images {
            builder = builder.with_screenshots(6);
        }
        
        let _upload_data = builder.build()?;
        
        info!("Built upload data for hobby processing");
        
        // Create the upload processor - it will auto-detect the active tracker
        let mut processor = crate::upload::UploadProcessor::new(
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
        HobbyType::Doc | HobbyType::Docx | HobbyType::Txt | HobbyType::Rtf => HobbyCategory::Documents,
        HobbyType::Jpg | HobbyType::Png | HobbyType::Gif | HobbyType::Bmp | 
        HobbyType::Tiff | HobbyType::Svg => HobbyCategory::Images,
        HobbyType::Dwg | HobbyType::Dxf | HobbyType::Stl | HobbyType::Obj | HobbyType::Ply => HobbyCategory::CAD3D,
        HobbyType::Zip | HobbyType::Rar | HobbyType::SevenZ => HobbyCategory::Archives,
        HobbyType::Csv | HobbyType::Json | HobbyType::Xml | HobbyType::Sql => HobbyCategory::DataFiles,
        HobbyType::Ttf | HobbyType::Otf | HobbyType::Woff => HobbyCategory::Fonts,
        HobbyType::Directory => HobbyCategory::Collection,
    };
    
    // Initialize regex patterns
    let year_regex = Regex::new(r"\b(19|20)\d{2}\b").unwrap();
    let version_regex = Regex::new(r"(?i)\b(?:v|ver|version)\.?\s*(\d+(?:\.\d+)*)\b").unwrap();
    let tutorial_regex = Regex::new(r"(?i)\b(tutorial|guide|howto|manual|instructions)\b").unwrap();
    let template_regex = Regex::new(r"(?i)\b(template|templates|mockup|preset|presets)\b").unwrap();
    let resource_regex = Regex::new(r"(?i)\b(resource|resources|asset|assets|pack|bundle|collection)\b").unwrap();
    let language_regex = Regex::new(r"(?i)\b(english|spanish|french|german|italian|russian|japanese|chinese)\b").unwrap();
    
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
fn determine_collection_category(files_by_type: &std::collections::HashMap<String, usize>) -> HobbyCategory {
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
        s if s.contains("Doc") || s.contains("Txt") || s.contains("Rtf") => HobbyCategory::Documents,
        s if s.contains("Jpg") || s.contains("Png") || s.contains("Gif") => HobbyCategory::Images,
        s if s.contains("Dwg") || s.contains("Dxf") || s.contains("Stl") => HobbyCategory::CAD3D,
        s if s.contains("Zip") || s.contains("Rar") => HobbyCategory::Archives,
        s if s.contains("Csv") || s.contains("Json") || s.contains("Xml") => HobbyCategory::DataFiles,
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
    let search_path = Path::new(path);
    
    if search_path.is_file() {
        let extension = search_path
            .extension()
            .and_then(|ext| ext.to_str())
            .ok_or_else(|| "Could not determine file extension".to_string())?;

        if let Some(hobby_type) = HobbyType::from_extension(extension) {
            hobby_files.push(HobbyFile {
                path: search_path.to_path_buf(),
                hobby_type,
            });
        }
    } else if search_path.is_dir() {
        // Check if directory itself should be treated as a hobby collection
        if looks_like_hobby_collection(search_path) {
            hobby_files.push(HobbyFile {
                path: search_path.to_path_buf(),
                hobby_type: HobbyType::Directory,
            });
        } else {
            // Search for hobby files in directory
            for entry in std::fs::read_dir(search_path)
                .map_err(|e| format!("Failed to read directory: {}", e))? 
            {
                let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
                let file_path = entry.path();
                
                if file_path.is_file() {
                    if let Some(extension) = file_path.extension().and_then(|ext| ext.to_str()) {
                        if let Some(hobby_type) = HobbyType::from_extension(extension) {
                            hobby_files.push(HobbyFile {
                                path: file_path,
                                hobby_type,
                            });
                        }
                    }
                }
            }
        }
    }
    
    Ok(hobby_files)
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

/// Check if this is an image file
pub fn is_image_file(hobby_file: &HobbyFile) -> bool {
    hobby_file.hobby_type.is_image()
}

/// Check if this is a document file
pub fn is_document_file(hobby_file: &HobbyFile) -> bool {
    hobby_file.hobby_type.is_document()
}

/// Group hobby files by type for better organization
pub fn group_by_type(hobby_files: &[HobbyFile]) -> std::collections::HashMap<String, Vec<&HobbyFile>> {
    let mut groups = std::collections::HashMap::new();
    
    for file in hobby_files {
        let category = match &file.hobby_type {
            HobbyType::Doc | HobbyType::Docx | HobbyType::Txt | HobbyType::Rtf => "Documents",
            HobbyType::Jpg | HobbyType::Png | HobbyType::Gif | HobbyType::Bmp | 
            HobbyType::Tiff | HobbyType::Svg => "Images",
            HobbyType::Dwg | HobbyType::Dxf | HobbyType::Stl | HobbyType::Obj | HobbyType::Ply => "CAD/3D",
            HobbyType::Zip | HobbyType::Rar | HobbyType::SevenZ => "Archives",
            HobbyType::Csv | HobbyType::Json | HobbyType::Xml | HobbyType::Sql => "Data",
            HobbyType::Ttf | HobbyType::Otf | HobbyType::Woff => "Fonts",
            HobbyType::Directory => "Collections",
        };
        
        groups.entry(category.to_string()).or_insert_with(Vec::new).push(file);
    }
    
    groups
}

/// Convert HobbyFile to MediaFile
pub fn to_media_file(hobby_file: &HobbyFile) -> MediaFile {
    MediaFile {
        path: hobby_file.path.clone(),
        media_type: MediaType::Hobby(hobby_file.hobby_type.clone()),
    }
}