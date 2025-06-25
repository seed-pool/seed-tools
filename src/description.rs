use std::collections::HashMap;
use crate::types::{MediaType, ImageLayout, SectionFormat, DescriptionComponent};

/// Configuration for description generation
#[derive(Debug, Clone)]
pub struct DescriptionConfig {
    // Color scheme
    pub title_color: String,
    pub author_color: String,
    pub section_colors: HashMap<String, String>,
    
    // Layout preferences
    pub image_width: u32,
    pub max_images: usize,
    pub image_layout: ImageLayout,
    
    // Feature flags
    pub include_footer: bool,
    pub custom_footer: Option<String>,
}

impl Default for DescriptionConfig {
    fn default() -> Self {
        let mut section_colors = HashMap::new();
        section_colors.insert("synopsis".to_string(), "#6C3483".to_string());
        section_colors.insert("author_bio".to_string(), "#F39C12".to_string());
        section_colors.insert("default".to_string(), "#2E86C1".to_string());
        
        Self {
            title_color: "#2E86C1".to_string(),
            author_color: "#117A65".to_string(),
            section_colors,
            image_width: 720,
            max_images: 8,
            image_layout: ImageLayout::TwoColumn,
            include_footer: true,
            custom_footer: None,
        }
    }
}

/// Builder for creating descriptions
pub struct DescriptionBuilder {
    components: Vec<DescriptionComponent>,
    footer: Option<String>,
    media_type: MediaType,
    config: DescriptionConfig,
}

impl DescriptionBuilder {
    /// Create a new description builder for a specific media type
    pub fn new(media_type: MediaType) -> Self {
        Self {
            components: Vec::new(),
            footer: None,
            media_type,
            config: DescriptionConfig::default(),
        }
    }
    
    /// Create a new description builder with custom config
    pub fn with_config(media_type: MediaType, config: DescriptionConfig) -> Self {
        Self {
            components: Vec::new(),
            footer: None,
            media_type,
            config,
        }
    }
    
    /// Add a component to the description
    pub fn add_component(mut self, component: DescriptionComponent) -> Self {
        self.components.push(component);
        self
    }
    
    /// Add a title component
    pub fn title(self, text: &str) -> Self {
        let size = match &self.media_type {
            MediaType::Video(_) => 18,
            MediaType::Ebook(_) => 32,
            _ => 24,
        };
        let color = self.config.title_color.clone();
        
        self.add_component(DescriptionComponent::Title {
            text: text.to_string(),
            size,
            color,
        })
    }
    
    /// Add an author component
    pub fn author(self, name: &str) -> Self {
        let color = self.config.author_color.clone();
        
        self.add_component(DescriptionComponent::Author {
            name: name.to_string(),
            color,
        })
    }
    
    /// Add images with the configured layout
    pub fn images(self, urls: Vec<String>) -> Self {
        let max_images = self.config.max_images;
        let layout = self.config.image_layout.clone();
        let width = self.config.image_width;
        
        let urls = if urls.len() > max_images {
            urls[..max_images].to_vec()
        } else {
            urls
        };
        
        self.add_component(DescriptionComponent::Images {
            urls,
            layout,
            width,
        })
    }
    
    /// Add a sample video/file
    pub fn sample(self, url: &str, filename: &str) -> Self {
        self.add_component(DescriptionComponent::Sample {
            url: url.to_string(),
            filename: filename.to_string(),
        })
    }
    
    /// Add a trailer link
    pub fn trailer(self, url: &str, platform: &str) -> Self {
        self.add_component(DescriptionComponent::Trailer {
            url: url.to_string(),
            platform: platform.to_string(),
        })
    }
    
    /// Add a synopsis/description
    pub fn synopsis(self, text: &str) -> Self {
        self.add_component(DescriptionComponent::Synopsis {
            text: text.to_string(),
        })
    }
    
    /// Add a quoted section
    pub fn quote(self, content: &str) -> Self {
        self.add_component(DescriptionComponent::Quote {
            content: content.to_string(),
        })
    }
    
    /// Add a custom section
    pub fn custom_section(self, title: &str, content: &str, format: SectionFormat) -> Self {
        self.add_component(DescriptionComponent::CustomSection {
            title: title.to_string(),
            content: content.to_string(),
            format,
        })
    }
    
    /// Add raw BBCode content
    pub fn raw(self, content: &str) -> Self {
        self.add_component(DescriptionComponent::Raw {
            content: content.to_string(),
        })
    }
    
    /// Set a custom footer
    pub fn with_custom_footer(mut self, footer: String) -> Self {
        self.footer = Some(footer);
        self
    }
    
    /// Build the final description string
    pub fn build(self) -> String {
        let mut parts = Vec::new();
        
        for component in &self.components {
            let bbcode = match component {
                DescriptionComponent::Title { text, size, color } => {
                    format!("[center][b][size={}][color={}]{}[/color][/size][/b][/center]", 
                        size, color, text)
                }
                
                DescriptionComponent::Author { name, color } => {
                    format!("[center][b][size=16][color={}]By:[/color][/size][/b] [i]{}[/i][/center]", 
                        color, name)
                }
                
                DescriptionComponent::Images { urls, layout, width } => {
                    self.format_images(urls, layout, *width)
                }
                
                DescriptionComponent::Synopsis { text } => {
                    let color = self.config.section_colors.get("synopsis")
                        .unwrap_or(&self.config.section_colors["default"]);
                    format!("[b][size=15][color={}]Synopsis:[/color][/size][/b]\n[quote]{}[/quote]", 
                        color, text)
                }
                
                DescriptionComponent::Sample { url, filename } => {
                    format!("[b][spoiler=Sample: {}]{}[/spoiler][/b]", filename, url)
                }
                
                DescriptionComponent::Trailer { url, platform } => {
                    format!("[center][b][url={}][Trailer on {}][/url][/b][/center]", url, platform)
                }
                
                DescriptionComponent::CustomSection { title, content, format } => {
                    self.format_custom_section(title, content, format)
                }
                
                DescriptionComponent::Table { rows } => {
                    self.format_table(rows)
                }
                
                DescriptionComponent::Quote { content } => {
                    format!("[quote]{}[/quote]", content)
                }
                
                DescriptionComponent::Spoiler { title, content } => {
                    format!("[spoiler={}]{}[/spoiler]", title, content)
                }
                
                DescriptionComponent::Raw { content } => content.clone(),
            };
            
            parts.push(bbcode);
        }
        
        // Add footer if configured
        if self.config.include_footer {
            let footer = self.footer.as_ref()
                .map(|f| f.clone())
                .unwrap_or_else(|| self.default_footer());
            parts.push(footer);
        }
        
        parts.join("\n\n")
    }
    
    /// Format images based on layout
    fn format_images(&self, urls: &[String], layout: &ImageLayout, width: u32) -> String {
        match layout {
            ImageLayout::Grid2x2 => {
                let mut result = String::from("[center][tr]\n");
                for (i, url) in urls.iter().enumerate() {
                    result.push_str(&format!(
                        "        [td][url={}][img width={}]{}[/img][/url][/td]\n",
                        url, width, url
                    ));
                    
                    // Add new row every 2 images
                    if (i + 1) % 2 == 0 && i + 1 < urls.len() {
                        result.push_str("    [/tr]\n    [tr]\n");
                    }
                }
                
                // Close the last row properly
                if urls.len() % 2 != 0 {
                    result.push_str("    [/center][/tr]\n");
                } else {
                    result.push_str("    [/tr][/center]\n");
                }
                
                result
            }
            
            ImageLayout::TwoColumn => {
                let mut result = String::from("[table]\n");
                let mut iter = urls.iter();
                
                while let Some(url1) = iter.next() {
                    result.push_str("[tr]\n");
                    result.push_str(&format!("[td][img width={}]{}[/img][/td]\n", width, url1));
                    
                    if let Some(url2) = iter.next() {
                        result.push_str(&format!("[td][img width={}]{}[/img][/td]\n", width, url2));
                    }
                    
                    result.push_str("[/tr]\n");
                }
                
                result.push_str("[/table]");
                result
            }
            
            ImageLayout::SingleColumn => {
                urls.iter()
                    .map(|url| format!("[center][img width={}]{}[/img][/center]", width, url))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            
            ImageLayout::Gallery => {
                let images = urls.iter()
                    .map(|url| format!("[url={}][img width={}]{}[/img][/url]", url, width / 4, url))
                    .collect::<Vec<_>>()
                    .join(" ");
                format!("[center]{}[/center]", images)
            }
        }
    }
    
    /// Format custom section based on format type
    fn format_custom_section(&self, title: &str, content: &str, format: &SectionFormat) -> String {
        let color = self.config.section_colors.get("default").unwrap();
        let title_formatted = format!("[b][size=15][color={}]{}:[/color][/size][/b]", color, title);
        
        match format {
            SectionFormat::Plain => format!("{}\n{}", title_formatted, content),
            SectionFormat::Quoted => format!("{}\n[quote]{}[/quote]", title_formatted, content),
            SectionFormat::Spoiler => format!("{}\n[spoiler]{}[/spoiler]", title_formatted, content),
            SectionFormat::Colored { color } => {
                format!("{}\n[color={}]{}[/color]", title_formatted, color, content)
            }
        }
    }
    
    /// Format a table
    fn format_table(&self, rows: &[Vec<String>]) -> String {
        let mut result = String::from("[table]\n");
        
        for row in rows {
            result.push_str("[tr]\n");
            for cell in row {
                result.push_str(&format!("[td]{}[/td]\n", cell));
            }
            result.push_str("[/tr]\n");
        }
        
        result.push_str("[/table]");
        result
    }
    
    /// Get the default footer
    fn default_footer(&self) -> String {
        format!(
            "[b][size=12][color=#757575]Created with mkbrr, ffmpeg, and mediainfo. Posted to this fine tracker with seed-tools.[/color][/size][/b]
        
        [url=https://github.com/seed-pool/seed-tools][img]https://cdn.seedpool.org/sp.png[/img][/url]  \
        [url=https://github.com/autobrr/mkbrr][img]https://cdn.seedpool.org/mkbrr.png[/img][/url]  \
        [url=https://www.rust-lang.org][img]https://cdn.seedpool.org/rust.png[/img][/url]"
        )
    }
}

/// Helper function to create video descriptions
pub fn create_video_description(
    title: Option<&str>,
    screenshots: Vec<String>,
    sample_url: Option<&str>,
    trailer_url: Option<&str>,
    custom_desc: Option<&str>,
) -> String {
    let mut builder = DescriptionBuilder::new(MediaType::Video(crate::types::VideoType::Mkv));
    
    if let Some(title) = title {
        builder = builder.title(title);
    }
    
    if !screenshots.is_empty() {
        let mut config = DescriptionConfig::default();
        config.image_layout = ImageLayout::Grid2x2;
        builder = DescriptionBuilder::with_config(
            MediaType::Video(crate::types::VideoType::Mkv), 
            config
        ).images(screenshots);
    }
    
    if let Some((url, filename)) = sample_url.and_then(|u| {
        std::path::Path::new(u).file_name()
            .and_then(|f| f.to_str())
            .map(|f| (u, f))
    }) {
        builder = builder.sample(url, filename);
    }
    
    if let Some(url) = trailer_url {
        builder = builder.trailer(url, "YouTube");
    }
    
    if let Some(desc) = custom_desc {
        builder = builder.raw(desc);
    }
    
    builder.build()
}

/// Helper function to create ebook descriptions
pub fn create_ebook_description(
    title: &str,
    author: Option<&str>,
    synopsis: Option<&str>,
    page_images: Vec<String>,
) -> String {
    let mut builder = DescriptionBuilder::new(MediaType::Ebook(crate::types::EbookType::Epub))
        .title(title);
    
    if let Some(author) = author {
        builder = builder.author(author);
    }
    
    if let Some(synopsis) = synopsis {
        builder = builder.synopsis(synopsis);
    }
    
    if !page_images.is_empty() {
        builder = builder.images(page_images);
    }
    
    builder.build()
}

/// Helper function to create ebook description with Open Library data
pub fn create_ebook_description_with_metadata(
    title: &str,
    author: &str,
    synopsis: Option<&str>,
    author_bio: Option<&str>,
    additional_links: Vec<(&str, &str)>, // (title, url) pairs
) -> String {
    let mut builder = DescriptionBuilder::new(MediaType::Ebook(crate::types::EbookType::Epub))
        .title(title)
        .author(author);
    
    if let Some(synopsis) = synopsis {
        builder = builder.synopsis(synopsis);
    }
    
    if let Some(bio) = author_bio {
        builder = builder.custom_section("About the Author", bio, SectionFormat::Quoted);
    }
    
    if !additional_links.is_empty() {
        let links_content = additional_links.iter()
            .map(|(title, url)| format!("[url={}]{}[/url]", url, title))
            .collect::<Vec<_>>()
            .join("\n");
        
        builder = builder.custom_section("Additional Editions", &links_content, SectionFormat::Plain);
    }
    
    builder.build()
}