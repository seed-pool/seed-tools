// Template system for user-customizable descriptions

use crate::core::{ImageLayout, MediaType, SectionFormat};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Template configuration that can be loaded from YAML/JSON
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DescriptionTemplate {
    pub name: String,
    pub media_type: String, // "video", "audio", "game", "ebook", "hobby"
    pub version: String,
    pub description: String,

    // Layout settings
    pub layout: TemplateLayout,

    // Template sections
    pub sections: Vec<TemplateSection>,

    // Conditional rules
    #[serde(default)]
    pub conditionals: Vec<ConditionalSection>,

    // Variable definitions
    #[serde(default)]
    pub variables: HashMap<String, String>,
}

/// Layout configuration for the template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateLayout {
    pub title_color: String,
    pub author_color: String,
    pub section_colors: HashMap<String, String>,
    pub image_layout: String, // "grid2x2", "two_column", "single_column", "gallery"
    pub image_width: u32,
    pub max_images: usize,
    pub include_footer: bool,
    #[serde(default)]
    pub custom_footer: Option<String>,
}

/// Individual section in the template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateSection {
    pub name: String,
    pub section_type: String, // "title", "author", "synopsis", "table", "images", "custom", "raw"

    #[serde(default)]
    pub content: String, // Template content with variables like {{title}}, {{tmdb_overview}}

    #[serde(default)]
    pub format: String, // "plain", "quoted", "spoiler", "colored"

    #[serde(default)]
    pub color: Option<String>,

    #[serde(default)]
    pub required_fields: Vec<String>, // Only show if these fields exist

    #[serde(default)]
    pub table_fields: Vec<TableField>, // For table sections

    #[serde(default)]
    pub order: i32, // Section order
}

/// Table field configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableField {
    pub label: String,
    pub field: String,            // metadata field name
    pub format: Option<String>,   // Optional formatting like "{{value}}/10" for ratings
    pub fallback: Option<String>, // Fallback field if primary field is empty
}

/// Conditional section that only appears if conditions are met
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConditionalSection {
    pub name: String,
    pub condition: String, // "field_exists:tmdb_overview" or "field_equals:category:Movie"
    pub section: TemplateSection,
}

impl Default for TemplateLayout {
    fn default() -> Self {
        let mut section_colors = HashMap::new();
        section_colors.insert("synopsis".to_string(), "#6C3483".to_string());
        section_colors.insert("cast".to_string(), "#2E86C1".to_string());
        section_colors.insert("details".to_string(), "#117A65".to_string());
        section_colors.insert("default".to_string(), "#2E86C1".to_string());

        Self {
            title_color: "#2E86C1".to_string(),
            author_color: "#117A65".to_string(),
            section_colors,
            image_layout: "two_column".to_string(),
            image_width: 720,
            max_images: 8,
            include_footer: true,
            custom_footer: None,
        }
    }
}

/// Template processor that applies templates to metadata
pub struct TemplateProcessor {
    templates: HashMap<String, DescriptionTemplate>,
}

impl TemplateProcessor {
    pub fn new() -> Self {
        Self {
            templates: HashMap::new(),
        }
    }

    /// Create a new TemplateProcessor with default templates loaded
    pub fn with_defaults() -> Result<Self, String> {
        let mut processor = Self::new();
        processor.load_default_templates()?;
        Ok(processor)
    }

    /// Load all default templates from the templates directory
    pub fn load_default_templates(&mut self) -> Result<(), String> {
        let template_files = [
            ("video", include_str!("video_template.yaml")),
            ("audio", include_str!("audio_template.yaml")),
            ("game", include_str!("game_template.yaml")),
            ("ebook", include_str!("ebook_template.yaml")),
            ("hobby", include_str!("hobby_template.yaml")),
        ];

        log::info!("Loading {} default templates", template_files.len());

        for (media_type, template_content) in &template_files {
            log::info!(
                "Loading template for {}, content length: {}",
                media_type,
                template_content.len()
            );
            match self.load_template_from_yaml(template_content) {
                Ok(_) => log::info!("✅ Successfully loaded default template for {}", media_type),
                Err(e) => {
                    log::error!(
                        "❌ Failed to load default template for {}: {}",
                        media_type,
                        e
                    );
                    log::debug!("Template content for {}: {}", media_type, template_content);
                }
            }
        }

        log::info!("Total templates loaded: {}", self.templates.len());
        for (key, template) in &self.templates {
            log::info!("  - {}: {} v{}", key, template.name, template.version);
        }

        Ok(())
    }

    /// Load template from file path
    pub fn load_template_from_file(&mut self, file_path: &str) -> Result<(), String> {
        let content = std::fs::read_to_string(file_path)
            .map_err(|e| format!("Failed to read template file {}: {}", file_path, e))?;

        if file_path.ends_with(".yaml") || file_path.ends_with(".yml") {
            self.load_template_from_yaml(&content)
        } else if file_path.ends_with(".json") {
            self.load_template_from_json(&content)
        } else {
            Err("Unsupported template file format. Use .yaml, .yml, or .json".to_string())
        }
    }

    /// Load template from YAML string
    pub fn load_template_from_yaml(&mut self, yaml_content: &str) -> Result<(), String> {
        let template: DescriptionTemplate = serde_yaml::from_str(yaml_content)
            .map_err(|e| format!("Failed to parse template YAML: {}", e))?;

        let key = format!("{}_{}", template.media_type, template.name);
        self.templates.insert(key, template);
        Ok(())
    }

    /// Load template from JSON string
    pub fn load_template_from_json(&mut self, json_content: &str) -> Result<(), String> {
        let template: DescriptionTemplate = serde_json::from_str(json_content)
            .map_err(|e| format!("Failed to parse template JSON: {}", e))?;

        let key = format!("{}_{}", template.media_type, template.name);
        self.templates.insert(key, template);
        Ok(())
    }

    /// Get template for a specific media type and name
    pub fn get_template(
        &self,
        media_type: &str,
        template_name: &str,
    ) -> Option<&DescriptionTemplate> {
        let key = format!("{}_{}", media_type, template_name);
        log::debug!("Looking for template with key: {}", key);
        log::debug!(
            "Available templates: {:?}",
            self.templates.keys().collect::<Vec<_>>()
        );

        let result = self.templates.get(&key);
        if result.is_some() {
            log::info!("✅ Found template: {}", key);
        } else {
            log::warn!("Template not found: {}", key);
        }
        result
    }

    /// List available templates for a media type
    pub fn list_templates(&self, media_type: &str) -> Vec<&DescriptionTemplate> {
        self.templates
            .values()
            .filter(|t| t.media_type == media_type)
            .collect()
    }

    /// Generate description using default template for media type
    pub fn generate_description(
        &self,
        media_type: &str,
        metadata: &Value,
        enriched_metadata: Option<&HashMap<String, String>>,
    ) -> Result<String, String> {
        let template = self
            .get_template(media_type, "default")
            .ok_or_else(|| format!("No default template found for media type: {}", media_type))?;

        self.apply_template(template, metadata, enriched_metadata)
    }

    /// Apply template to metadata and generate description
    pub fn apply_template(
        &self,
        template: &DescriptionTemplate,
        metadata: &Value,
        enriched_metadata: Option<&HashMap<String, String>>,
    ) -> Result<String, String> {
        use crate::processing::description::{DescriptionBuilder, DescriptionConfig};

        // Convert template layout to DescriptionConfig
        let mut config = DescriptionConfig::default();
        config.title_color = template.layout.title_color.clone();
        config.author_color = template.layout.author_color.clone();
        config.section_colors = template.layout.section_colors.clone();
        config.image_width = template.layout.image_width;
        config.max_images = template.layout.max_images;
        config.include_footer = template.layout.include_footer;
        config.custom_footer = template.layout.custom_footer.clone();

        // Convert image layout string to enum
        config.image_layout = match template.layout.image_layout.as_str() {
            "grid2x2" => ImageLayout::Grid2x2,
            "two_column" => ImageLayout::TwoColumn,
            "single_column" => ImageLayout::SingleColumn,
            "gallery" => ImageLayout::Gallery,
            _ => ImageLayout::TwoColumn,
        };

        // Determine MediaType from template
        let media_type = match template.media_type.as_str() {
            "video" => MediaType::Video(crate::core::VideoType::Mkv),
            "audio" => MediaType::Audio(crate::core::AudioType::Mp3),
            "game" => MediaType::Game(crate::core::GameType::Directory),
            "ebook" => MediaType::Ebook(crate::core::EbookType::Epub),
            "hobby" => MediaType::Hobby(crate::core::HobbyType::Directory),
            _ => MediaType::Video(crate::core::VideoType::Mkv),
        };

        let mut builder = DescriptionBuilder::with_config(media_type, config);

        // Sort sections by order
        let mut sections = template.sections.clone();
        sections.sort_by_key(|s| s.order);

        // Process each section
        for section in &sections {
            if !self.should_include_section(section, metadata, enriched_metadata) {
                continue;
            }

            match section.section_type.as_str() {
                "title" => {
                    if let Some(title) =
                        self.resolve_variable(&section.content, metadata, enriched_metadata)
                    {
                        builder = builder.title(&title);
                    }
                }
                "author" => {
                    if let Some(author) =
                        self.resolve_variable(&section.content, metadata, enriched_metadata)
                    {
                        builder = builder.author(&author);
                    }
                }
                "synopsis" => {
                    if let Some(synopsis) =
                        self.resolve_variable(&section.content, metadata, enriched_metadata)
                    {
                        builder = builder.synopsis(&synopsis);
                    }
                }
                "images" => {
                    let images = self.extract_images(metadata, enriched_metadata);
                    if !images.is_empty() {
                        builder = builder.images(images);
                    }
                }
                "table" => {
                    let table_rows =
                        self.build_table_rows(&section.table_fields, metadata, enriched_metadata);
                    if !table_rows.is_empty() {
                        builder = builder.add_component(crate::core::DescriptionComponent::Table {
                            rows: table_rows,
                        });
                    }
                }
                "custom" => {
                    if let Some(content) =
                        self.resolve_variable(&section.content, metadata, enriched_metadata)
                    {
                        let format = match section.format.as_str() {
                            "quoted" => SectionFormat::Quoted,
                            "spoiler" => SectionFormat::Spoiler,
                            "colored" => {
                                if let Some(color) = &section.color {
                                    SectionFormat::Colored {
                                        color: color.clone(),
                                    }
                                } else {
                                    SectionFormat::Plain
                                }
                            }
                            _ => SectionFormat::Plain,
                        };
                        builder = builder.custom_section(&section.name, &content, format);
                    }
                }
                "raw" => {
                    if let Some(content) =
                        self.resolve_variable(&section.content, metadata, enriched_metadata)
                    {
                        builder = builder.raw(&content);
                    }
                }
                _ => {} // Unknown section type
            }
        }

        // Process conditional sections
        for conditional in &template.conditionals {
            if self.evaluate_condition(&conditional.condition, metadata, enriched_metadata) {
                // Process the conditional section based on its section_type
                match conditional.section.section_type.as_str() {
                    "raw" => {
                        if let Some(content) =
                            self.resolve_variable(&conditional.section.content, metadata, enriched_metadata)
                        {
                            builder = builder.raw(&content);
                        }
                    }
                    "custom" | _ => {
                        if let Some(content) =
                            self.resolve_variable(&conditional.section.content, metadata, enriched_metadata)
                        {
                            let format = match conditional.section.format.as_str() {
                                "quoted" => SectionFormat::Quoted,
                                "spoiler" => SectionFormat::Spoiler,
                                _ => SectionFormat::Plain,
                            };
                            builder = builder.custom_section(&conditional.section.name, &content, format);
                        }
                    }
                }
            }
        }

        Ok(builder.build())
    }

    /// Check if a section should be included based on required fields
    fn should_include_section(
        &self,
        section: &TemplateSection,
        metadata: &Value,
        enriched_metadata: Option<&HashMap<String, String>>,
    ) -> bool {
        if section.required_fields.is_empty() {
            return true;
        }

        for field in &section.required_fields {
            if !self.has_field(field, metadata, enriched_metadata) {
                return false;
            }
        }
        true
    }

    /// Check if a field exists in metadata or enriched metadata
    fn has_field(
        &self,
        field: &str,
        metadata: &Value,
        enriched_metadata: Option<&HashMap<String, String>>,
    ) -> bool {
        if let Some(enriched) = enriched_metadata {
            if enriched.contains_key(field) {
                return true;
            }
        }
        metadata.get(field).is_some()
    }

    /// Resolve template variables like {{title}}, {{tmdb_overview}}
    fn resolve_variable(
        &self,
        template: &str,
        metadata: &Value,
        enriched_metadata: Option<&HashMap<String, String>>,
    ) -> Option<String> {
        if template.is_empty() {
            return None;
        }

        let var_regex = Regex::new(r"\{\{([^}]+)\}\}").unwrap();
        let mut result = template.to_string();

        for capture in var_regex.captures_iter(template) {
            if let Some(var_name) = capture.get(1) {
                let field_spec = var_name.as_str().trim();

                // Handle fallback syntax: field1|field2|default:"value"
                let mut value = None;
                let parts: Vec<&str> = field_spec.split('|').collect();

                for part in parts {
                    let part = part.trim();

                    if part.starts_with("default:") {
                        // Handle default value
                        let default_val = part.strip_prefix("default:").unwrap_or("");
                        // Remove quotes if present
                        let default_val = default_val.trim_matches('"').trim_matches('\'');
                        value = Some(default_val.to_string());
                        break;
                    } else {
                        // Try to get field value
                        if let Some(field_value) =
                            self.get_field_value(part, metadata, enriched_metadata)
                        {
                            if !field_value.is_empty() {
                                value = Some(field_value);
                                break;
                            }
                        }
                    }
                }

                let replacement = value.unwrap_or_default();
                result = result.replace(&format!("{{{{{}}}}}", field_spec), &replacement);
            }
        }

        if result == template && !template.contains("{{") {
            // No variables to resolve, return as-is if not empty
            Some(result)
        } else if result.trim().is_empty() {
            None
        } else {
            Some(result)
        }
    }

    /// Get field value from metadata or enriched metadata
    fn get_field_value(
        &self,
        field: &str,
        metadata: &Value,
        enriched_metadata: Option<&HashMap<String, String>>,
    ) -> Option<String> {
        // Check enriched metadata first
        if let Some(enriched) = enriched_metadata {
            if let Some(value) = enriched.get(field) {
                return Some(value.clone());
            }
        }

        // Check base metadata
        metadata.get(field).and_then(|v| match v {
            Value::String(s) => Some(s.clone()),
            Value::Number(n) => Some(n.to_string()),
            Value::Bool(b) => Some(b.to_string()),
            Value::Array(arr) => {
                // Handle arrays by joining string values
                let strings: Vec<String> = arr
                    .iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect();
                if !strings.is_empty() {
                    Some(strings.join(", "))
                } else {
                    None
                }
            }
            _ => None,
        })
    }

    /// Extract images from metadata
    fn extract_images(
        &self,
        metadata: &Value,
        enriched_metadata: Option<&HashMap<String, String>>,
    ) -> Vec<String> {
        let mut images = Vec::new();

        // Check for various image fields
        let image_fields = [
            "screenshots",
            "images",
            "cover_images",
            "igdb_screenshots",
            "tmdb_poster_url",
            "igdb_cover_url",
        ];

        for field in &image_fields {
            if let Some(value) = self.get_field_value(field, metadata, enriched_metadata) {
                if field.contains("screenshots") && value.contains(",") {
                    // Multiple screenshots separated by commas
                    images.extend(value.split(',').map(|s| s.trim().to_string()));
                } else if !value.is_empty() {
                    images.push(value);
                }
            }
        }

        // Also check arrays
        if let Some(screenshots) = metadata.get("screenshots").and_then(|s| s.as_array()) {
            for screenshot in screenshots {
                if let Some(url) = screenshot.as_str() {
                    images.push(url.to_string());
                }
            }
        }

        images
    }

    /// Build table rows from table field configuration
    fn build_table_rows(
        &self,
        table_fields: &[TableField],
        metadata: &Value,
        enriched_metadata: Option<&HashMap<String, String>>,
    ) -> Vec<Vec<String>> {
        let mut rows = Vec::new();

        for field_config in table_fields {
            let value = self
                .get_field_value(&field_config.field, metadata, enriched_metadata)
                .or_else(|| {
                    // Try fallback field
                    field_config.fallback.as_ref().and_then(|fallback| {
                        self.get_field_value(fallback, metadata, enriched_metadata)
                    })
                });

            if let Some(mut value) = value {
                // Apply formatting if specified
                if let Some(format) = &field_config.format {
                    value = format.replace("{{value}}", &value);
                }

                rows.push(vec![field_config.label.clone(), value]);
            }
        }

        rows
    }

    /// Evaluate conditional expressions
    fn evaluate_condition(
        &self,
        condition: &str,
        metadata: &Value,
        enriched_metadata: Option<&HashMap<String, String>>,
    ) -> bool {
        if condition.starts_with("field_exists:") {
            let field = condition.strip_prefix("field_exists:").unwrap_or("");
            self.has_field(field, metadata, enriched_metadata)
        } else if condition.starts_with("field_equals:") {
            let parts: Vec<&str> = condition
                .strip_prefix("field_equals:")
                .unwrap_or("")
                .split(':')
                .collect();
            if parts.len() == 2 {
                let field = parts[0];
                let expected_value = parts[1];
                if let Some(actual_value) = self.get_field_value(field, metadata, enriched_metadata)
                {
                    actual_value == expected_value
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        }
    }
}

impl Default for TemplateProcessor {
    fn default() -> Self {
        Self::new()
    }
}
