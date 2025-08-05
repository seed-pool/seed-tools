use crate::core::{DescriptionComponent, SectionFormat};
use image::{ImageBuffer, Rgb, RgbImage, imageops};
use imageproc::drawing::draw_text_mut;
use ab_glyph::{FontRef, PxScale};
use std::path::Path;

/// Configuration for image-based description generation
#[derive(Debug, Clone)]
pub struct ImageDescriptionConfig {
    pub canvas_width: u32,
    pub canvas_height: u32,
    pub padding: u32,
    pub line_spacing: u32,
    
    // Font sizes for rusttype Scale
    pub title_font_size: f32,
    pub subtitle_font_size: f32,
    pub body_font_size: f32,
    
    // Colors
    pub background_color: Rgb<u8>,
    pub title_color: Rgb<u8>,
    pub text_color: Rgb<u8>,
    pub accent_color: Rgb<u8>,
    
    // Layout
    pub section_spacing: u32,
}

impl Default for ImageDescriptionConfig {
    fn default() -> Self {
        Self {
            canvas_width: 4320, // 3x DPI for ultra-sharp text rendering (1440 * 3)
            canvas_height: 600,  // 3x DPI maintaining aspect ratio (200 * 3)
            padding: 45,         // 3x padding for high DPI
            line_spacing: 18,    // 3x line spacing for high DPI
            
            title_font_size: 72.0,  // 3x font sizes for crisp high-DPI rendering
            subtitle_font_size: 54.0,
            body_font_size: 42.0,
            
            background_color: Rgb([0, 0, 0]),     // Black background
            title_color: Rgb([100, 180, 255]),    // Light blue
            text_color: Rgb([220, 220, 220]),     // Light gray
            accent_color: Rgb([255, 180, 100]),   // Orange
            
            section_spacing: 36, // 3x section spacing for high DPI
        }
    }
}

/// Builder for creating image-based descriptions
pub struct ImageDescriptionBuilder {
    config: ImageDescriptionConfig,
    components: Vec<DescriptionComponent>,
    use_simple_text: bool,
}

impl ImageDescriptionBuilder {
    /// Create a new image description builder
    pub fn new() -> Self {
        Self {
            config: ImageDescriptionConfig::default(),
            components: Vec::new(),
            use_simple_text: true, // Start with simple approach
        }
    }
    
    /// Create with custom config
    pub fn with_config(config: ImageDescriptionConfig) -> Self {
        Self {
            config,
            components: Vec::new(),
            use_simple_text: true,
        }
    }
    
    /// Add components to the builder
    pub fn with_components(mut self, components: Vec<DescriptionComponent>) -> Self {
        self.components = components;
        self
    }
    
    /// Extract poster URL using IMDb ID via OMDb API
    fn extract_poster_url(metadata: &serde_json::Value) -> Option<String> {
        // First try to get IMDb ID from metadata
        let imdb_id = if let Some(id) = metadata.get("imdb_id").and_then(|v| v.as_str()) {
            id.to_string()
        } else if let Some(id) = metadata.get("tmdb_imdb_id").and_then(|v| v.as_str()) {
            format!("tt{}", id)
        } else {
            log::info!("📷 No IMDb ID found in metadata for poster lookup");
            return None;
        };
        
        log::info!("🎬 Fetching poster for IMDb ID: {}", imdb_id);
        
        // Use OMDb API to get poster URL
        // Using the 'trilogy' demo API key for OMDb
        let omdb_url = format!("http://www.omdbapi.com/?i={}&plot=short&apikey=trilogy", imdb_id);
        
        if let Ok(response) = reqwest::blocking::get(&omdb_url) {
            if let Ok(text) = response.text() {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(poster_url) = json.get("Poster").and_then(|v| v.as_str()) {
                        if poster_url != "N/A" && !poster_url.is_empty() {
                            log::info!("✅ Found poster URL: {}", poster_url);
                            return Some(poster_url.to_string());
                        }
                    }
                }
            }
        }
        
        log::info!("📷 Could not fetch poster from OMDb API for {}", imdb_id);
        None
    }
    

    
    /// Build the final image with two-column layout and optional poster
    pub fn build(self, output_path: &str) -> Result<(), String> {
        self.build_with_metadata(output_path, None)
    }
    
    /// Build with optional metadata for poster embedding
    pub fn build_with_metadata(self, output_path: &str, metadata: Option<&serde_json::Value>) -> Result<(), String> {
        // Create banner-sized image
        let mut img: RgbImage = ImageBuffer::new(self.config.canvas_width, self.config.canvas_height);
        
        // Fill with background color
        for (_x, _y, pixel) in img.enumerate_pixels_mut() {
            *pixel = self.config.background_color;
        }
        
        // Try to download and embed poster if metadata is available
        let mut text_start_x = self.config.padding; // Start position for text
        
        if let Some(meta) = metadata {
            if let Some(poster_url) = Self::extract_poster_url(meta) {
                log::info!("🖼️ Downloading poster: {}", poster_url);
                
                // Use blocking HTTP request for now (since we're in sync context)
                if let Ok(response) = reqwest::blocking::get(&poster_url) {
                    if let Ok(image_bytes) = response.bytes() {
                        if let Ok(poster_img) = image::load_from_memory(&image_bytes) {
                            // Calculate proportional dimensions for poster with padding
                            let poster_padding = 24; // Small padding around poster (3x DPI)
                            let target_height = self.config.canvas_height - (2 * poster_padding); // Leave padding top/bottom
                            let original_height = poster_img.height();
                            let original_width = poster_img.width();
                            let target_width = (original_width * target_height) / original_height;
                            
                            // Resize poster to fit banner height with padding
                            let resized_poster = poster_img.resize_exact(target_width, target_height, imageops::FilterType::Lanczos3);
                            let poster_rgb = resized_poster.to_rgb8();
                            
                            // Composite poster onto the left side of the banner with padding
                            for y in 0..poster_rgb.height() {
                                for x in 0..poster_rgb.width() {
                                    let poster_pixel = poster_rgb.get_pixel(x, y);
                                    let img_x = x + poster_padding; // Add left padding
                                    let img_y = y + poster_padding; // Add top padding
                                    
                                    if img_x < img.width() && img_y < img.height() {
                                        img.put_pixel(img_x, img_y, *poster_pixel);
                                    }
                                }
                            }
                            
                            // Adjust text start position to account for poster and padding
                            text_start_x = target_width + poster_padding + 45; // Spacing after poster (3x DPI)
                            log::info!("✅ Poster embedded successfully");
                        }
                    }
                }
            }
        }
        
        // Split components into left and right columns
        let mut left_components = Vec::new();
        let mut synopsis_component = None;
        
        for component in &self.components {
            match component {
                DescriptionComponent::Synopsis { .. } => {
                    synopsis_component = Some(component);
                }
                _ => {
                    left_components.push(component);
                }
            }
        }
        
        // Render left column (metadata info) - adjusted for poster
        let mut current_y = self.config.padding;
        for component in left_components {
            current_y = self.render_component_left_column_with_offset(&mut img, component, current_y, text_start_x);
        }
        
        // Render right column (synopsis)
        if let Some(synopsis) = synopsis_component {
            self.render_synopsis_right_column(&mut img, synopsis);
        }
        
        // Resize from high-DPI rendering (4320x600) down to target size (1440x200) for sharp fonts
        let target_width = 1440;
        let target_height = 200;
        let resized_img = image::DynamicImage::ImageRgb8(img)
            .resize_exact(target_width, target_height, imageops::FilterType::Lanczos3);
        
        // Save the resized image
        resized_img.save(output_path).map_err(|e| format!("Failed to save image: {}", e))?;
        
        Ok(())
    }
    
    /// Render a component with actual text
    fn render_component_simple(
        &self,
        img: &mut RgbImage,
        component: &DescriptionComponent,
        y_pos: u32,
    ) -> u32 {
        let mut next_y = y_pos;
        
        match component {
            DescriptionComponent::Title { text, .. } => {
                // Draw title text
                self.draw_pure_text(img, text, self.config.padding as i32, next_y as i32, 
                                   self.config.title_font_size, self.config.title_color);
                next_y += (self.config.title_font_size as u32) + self.config.section_spacing;
            }
            
            DescriptionComponent::Author { name, .. } => {
                // Draw author text
                let author_text = format!("Director: {}", name);
                self.draw_pure_text(img, &author_text, self.config.padding as i32, next_y as i32,
                                   self.config.subtitle_font_size, self.config.text_color);
                next_y += (self.config.subtitle_font_size as u32) + self.config.line_spacing;
            }
            
            DescriptionComponent::Synopsis { text } => {
                // Draw "Synopsis:" label first
                self.draw_pure_text(img, "Synopsis:", self.config.padding as i32, next_y as i32,
                                   self.config.subtitle_font_size, self.config.accent_color);
                next_y += (self.config.subtitle_font_size as u32) + self.config.line_spacing;
                
                // Draw synopsis content with better wrapping
                let synopsis_text = if text.len() > 400 {
                    format!("{}...", &text[..397])
                } else {
                    text.clone()
                };
                
                // For long text, split into multiple lines
                let max_chars_per_line = 100;
                let words: Vec<&str> = synopsis_text.split_whitespace().collect();
                let mut current_line = String::new();
                
                for word in words {
                    if current_line.len() + word.len() + 1 <= max_chars_per_line {
                        if !current_line.is_empty() {
                            current_line.push(' ');
                        }
                        current_line.push_str(word);
                    } else {
                        // Draw current line and start new one
                        if !current_line.is_empty() {
                            self.draw_pure_text(img, &current_line, (self.config.padding + 20) as i32, next_y as i32,
                                               self.config.body_font_size, self.config.text_color);
                            next_y += (self.config.body_font_size as u32) + self.config.line_spacing;
                        }
                        current_line = word.to_string();
                    }
                }
                
                // Draw the last line
                if !current_line.is_empty() {
                    self.draw_pure_text(img, &current_line, (self.config.padding + 20) as i32, next_y as i32,
                                       self.config.body_font_size, self.config.text_color);
                    next_y += (self.config.body_font_size as u32) + self.config.section_spacing;
                }
            }
            
            DescriptionComponent::CustomSection { title, content, .. } => {
                // Draw section title
                self.draw_pure_text(img, title, self.config.padding as i32, next_y as i32,
                                   self.config.subtitle_font_size, self.config.accent_color);
                next_y += (self.config.subtitle_font_size as u32) + self.config.line_spacing;
                
                // Draw content with better formatting and longer text
                let content_text = if content.len() > 200 {
                    format!("{}...", &content[..197])
                } else {
                    content.clone()
                };
                
                // Split long content into multiple lines if needed
                let max_chars_per_line = 90;
                if content_text.len() > max_chars_per_line {
                    let words: Vec<&str> = content_text.split_whitespace().collect();
                    let mut current_line = String::new();
                    
                    for word in words {
                        if current_line.len() + word.len() + 1 <= max_chars_per_line {
                            if !current_line.is_empty() {
                                current_line.push(' ');
                            }
                            current_line.push_str(word);
                        } else {
                            // Draw current line and start new one
                            if !current_line.is_empty() {
                                self.draw_pure_text(img, &current_line, (self.config.padding + 20) as i32, next_y as i32,
                                                   self.config.body_font_size, self.config.text_color);
                                next_y += (self.config.body_font_size as u32) + self.config.line_spacing;
                            }
                            current_line = word.to_string();
                        }
                    }
                    
                    // Draw the last line
                    if !current_line.is_empty() {
                        self.draw_pure_text(img, &current_line, (self.config.padding + 20) as i32, next_y as i32,
                                           self.config.body_font_size, self.config.text_color);
                        next_y += (self.config.body_font_size as u32) + self.config.line_spacing;
                    }
                } else {
                    self.draw_pure_text(img, &content_text, (self.config.padding + 20) as i32, next_y as i32,
                                       self.config.body_font_size, self.config.text_color);
                    next_y += (self.config.body_font_size as u32) + self.config.line_spacing;
                }
                
                next_y += self.config.section_spacing;
            }
            
            _ => {
                // For other components, just add some spacing
                next_y += self.config.line_spacing;
            }
        }
        
        next_y
    }
    
    /// Draw text directly (no background bars)
    fn draw_pure_text(
        &self,
        img: &mut RgbImage,
        text: &str,
        x: i32,
        y: i32,
        font_size: f32,
        color: Rgb<u8>,
    ) {
        // Try to load a font and render text
        if let Ok(font_data) = std::fs::read("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf") {
            if let Ok(font) = FontRef::try_from_slice(&font_data) {
                let scale = PxScale::from(font_size);
                draw_text_mut(img, color, x, y, scale, &font, text);
                return;
            }
        }
        
        // Fallback: try other common fonts
        let font_paths = [
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf", 
            "/System/Library/Fonts/Arial.ttf",
        ];
        
        for path in &font_paths {
            if let Ok(font_data) = std::fs::read(path) {
                if let Ok(font) = FontRef::try_from_slice(&font_data) {
                    let scale = PxScale::from(font_size);
                    draw_text_mut(img, color, x, y, scale, &font, text);
                    return;
                }
            }
        }
        
        log::warn!("⚠️ No fonts available for text rendering: {}", text);
    }
    
    /// Draw bold text by rendering multiple times with slight offsets
    fn draw_bold_text(
        &self,
        img: &mut RgbImage,
        text: &str,
        x: i32,
        y: i32,
        font_size: f32,
        color: Rgb<u8>,
    ) {
        // Draw text multiple times with slight offsets to simulate bold (3x DPI)
        let offsets = [
            (0, 0),   // Original position
            (3, 0),   // Right 3px (3x DPI)
            (0, 3),   // Down 3px (3x DPI)
            (3, 3),   // Right 3px, Down 3px (3x DPI)
        ];
        
        for (offset_x, offset_y) in &offsets {
            self.draw_pure_text(img, text, x + offset_x, y + offset_y, font_size, color);
        }
    }
    
    /// Render component in left column (compact format)
    fn render_component_left_column(
        &self,
        img: &mut RgbImage,
        component: &DescriptionComponent,
        y_pos: u32,
    ) -> u32 {
        self.render_component_left_column_with_offset(img, component, y_pos, self.config.padding)
    }
    
    /// Render component in left column with custom x offset (for poster space)
    fn render_component_left_column_with_offset(
        &self,
        img: &mut RgbImage,
        component: &DescriptionComponent,
        y_pos: u32,
        x_offset: u32,
    ) -> u32 {
        let mut next_y = y_pos;
        
        match component {
            DescriptionComponent::Title { text, .. } => {
                // Draw title text in bold
                self.draw_bold_text(img, text, x_offset as i32, next_y as i32, 
                                   self.config.title_font_size, self.config.title_color);
                next_y += (self.config.title_font_size as u32) + self.config.section_spacing;
            }
            
            DescriptionComponent::Author { name, .. } => {
                // Draw author text
                let author_text = format!("Director: {}", name);
                self.draw_pure_text(img, &author_text, x_offset as i32, next_y as i32,
                                   self.config.subtitle_font_size, self.config.text_color);
                next_y += (self.config.subtitle_font_size as u32) + self.config.line_spacing;
            }
            
            DescriptionComponent::CustomSection { title, content, .. } => {
                // Draw section title in bold
                self.draw_bold_text(img, title, x_offset as i32, next_y as i32,
                                   self.config.subtitle_font_size, self.config.accent_color);
                next_y += (self.config.subtitle_font_size as u32) + 2;
                
                // Draw content with extended length for cast and other details
                let content_text = if content.len() > 120 {
                    format!("{}...", &content[..117])
                } else {
                    content.clone()
                };
                
                // Split long content into multiple lines if needed for left column
                let max_chars_per_line = 60; // Adjusted for higher DPI rendering
                if content_text.len() > max_chars_per_line {
                    let words: Vec<&str> = content_text.split_whitespace().collect();
                    let mut current_line = String::new();
                    
                    for word in words {
                        if current_line.len() + word.len() + 1 <= max_chars_per_line {
                            if !current_line.is_empty() {
                                current_line.push(' ');
                            }
                            current_line.push_str(word);
                        } else {
                            // Draw current line and start new one
                            if !current_line.is_empty() {
                                self.draw_pure_text(img, &current_line, (x_offset + 30) as i32, next_y as i32,
                                                   self.config.body_font_size, self.config.text_color);
                                next_y += (self.config.body_font_size as u32) + self.config.line_spacing;
                            }
                            current_line = word.to_string();
                        }
                    }
                    
                    // Draw the last line
                    if !current_line.is_empty() {
                        self.draw_pure_text(img, &current_line, (x_offset + 30) as i32, next_y as i32,
                                           self.config.body_font_size, self.config.text_color);
                        next_y += (self.config.body_font_size as u32) + self.config.line_spacing;
                    }
                } else {
                    self.draw_pure_text(img, &content_text, (x_offset + 30) as i32, next_y as i32,
                                       self.config.body_font_size, self.config.text_color);
                    next_y += (self.config.body_font_size as u32) + self.config.line_spacing;
                }
            }
            
            _ => {
                // For other components, just add some spacing
                next_y += self.config.line_spacing;
            }
        }
        
        next_y
    }
    
    /// Render synopsis in right column with full text
    fn render_synopsis_right_column(
        &self,
        img: &mut RgbImage,
        component: &DescriptionComponent,
    ) {
        if let DescriptionComponent::Synopsis { text } = component {
            let right_column_start = (self.config.canvas_width * 2) / 3 - 450; // Move synopsis 450px left for better balance (3x DPI)
            let mut current_y = self.config.padding;
            
            // Draw "Synopsis:" label in bold
            self.draw_bold_text(img, "Synopsis:", right_column_start as i32, current_y as i32,
                               self.config.subtitle_font_size, self.config.accent_color);
            current_y += (self.config.subtitle_font_size as u32) + self.config.line_spacing;
            
            // Draw synopsis with word wrapping to fit right column
            let max_chars_per_line = 50; // Adjusted for higher DPI rendering
            let words: Vec<&str> = text.split_whitespace().collect();
            let mut current_line = String::new();
            
            for word in words {
                if current_line.len() + word.len() + 1 <= max_chars_per_line {
                    if !current_line.is_empty() {
                        current_line.push(' ');
                    }
                    current_line.push_str(word);
                } else {
                    // Draw current line and start new one
                    if !current_line.is_empty() {
                        self.draw_pure_text(img, &current_line, right_column_start as i32, current_y as i32,
                                           self.config.body_font_size, self.config.text_color);
                        current_y += (self.config.body_font_size as u32) + self.config.line_spacing;
                        
                        // Stop if we're getting close to the bottom
                        if current_y + (self.config.body_font_size as u32) > self.config.canvas_height - self.config.padding {
                            break;
                        }
                    }
                    current_line = word.to_string();
                }
            }
            
            // Draw the last line if there's space
            if !current_line.is_empty() && current_y + (self.config.body_font_size as u32) <= self.config.canvas_height - self.config.padding {
                self.draw_pure_text(img, &current_line, right_column_start as i32, current_y as i32,
                                   self.config.body_font_size, self.config.text_color);
            }
        }
    }
}

/// Create an image-based description from components
pub fn create_image_description(
    components: Vec<DescriptionComponent>,
    output_path: &str,
) -> Result<String, String> {
    create_image_description_with_metadata(components, output_path, None)
}

/// Create an image-based description with metadata for poster embedding
pub fn create_image_description_with_metadata(
    components: Vec<DescriptionComponent>,
    output_path: &str,
    metadata: Option<&serde_json::Value>,
) -> Result<String, String> {
    let builder = ImageDescriptionBuilder::new()
        .with_components(components);
    
    builder.build_with_metadata(output_path, metadata)?;
    
    Ok(output_path.to_string())
}

/// Create an image-based description with custom config
pub fn create_image_description_with_config(
    components: Vec<DescriptionComponent>,
    output_path: &str,
    config: ImageDescriptionConfig,
) -> Result<String, String> {
    let builder = ImageDescriptionBuilder::with_config(config)
        .with_components(components);
    
    builder.build(output_path)?;
    
    Ok(output_path.to_string())
}

/// Generate image description from metadata (main entry point)
pub fn generate_from_metadata(
    metadata: &serde_json::Value,
    _upload_config: &crate::processing::upload::UploadConfig,
    output_dir: &str,
    media_name: &str,
) -> Result<crate::processing::description_handler::DescriptionResult, String> {
    // Convert metadata to components
    let components = metadata_to_components(metadata);
    
    // Generate the image description
    let filename = format!("{}_description.png", media_name);
    let output_path = Path::new(output_dir).join(&filename);
    let output_path_str = output_path.to_string_lossy().to_string();
    
    create_image_description_with_metadata(components, &output_path_str, Some(metadata))?;
    
    Ok(crate::processing::description_handler::DescriptionResult::Image {
        path: output_path_str,
        filename,
    })
}

/// Convert JSON metadata to description components
fn metadata_to_components(metadata: &serde_json::Value) -> Vec<DescriptionComponent> {
    let mut components = Vec::new();
    
    // DEBUG: Log all available metadata keys
    if let serde_json::Value::Object(map) = metadata {
        log::info!("📊 ALT_DESC: Available metadata keys: {:?}", map.keys().collect::<Vec<_>>());
        for (key, value) in map.iter() {
            if key.starts_with("tmdb") {
                log::info!("🎬 ALT_DESC: TMDB field '{}': {:?}", key, value);
            }
        }
    }
    
    // Add title with year
    if let Some(title) = metadata.get("title").and_then(|v| v.as_str()) {
        let title_with_year = if let Some(year) = metadata.get("tmdb_year").and_then(|v| v.as_str()) {
            format!("{} ({})", title, year)
        } else {
            title.to_string()
        };
        components.push(DescriptionComponent::Title {
            text: title_with_year.clone(),
            size: 22,
            color: "#64B4FF".to_string(),
        });
        log::info!("✅ ALT_DESC: Added title component: {}", title_with_year);
    } else {
        log::info!("❌ ALT_DESC: No title found in metadata");
    }
    
    // Add director
    if let Some(director) = metadata.get("tmdb_directors").and_then(|v| v.as_str()) {
        components.push(DescriptionComponent::Author {
            name: director.to_string(),
            color: "#50C8B4".to_string(),
        });
        log::info!("✅ ALT_DESC: Added director component: {}", director);
    } else {
        log::info!("❌ ALT_DESC: No tmdb_directors found in metadata");
    }
    
    // Add comprehensive TMDB info in multiple sections
    let mut info_parts = Vec::new();
    
    if let Some(rating) = metadata.get("tmdb_rating").and_then(|v| v.as_str()) {
        info_parts.push(format!("Rating: {}/10", rating));
    }
    
    if let Some(runtime) = metadata.get("tmdb_runtime").and_then(|v| v.as_str()) {
        info_parts.push(format!("Runtime: {} min", runtime));
    }
    
    if let Some(genres) = metadata.get("tmdb_genres").and_then(|v| v.as_str()) {
        info_parts.push(format!("Genre: {}", genres));
    }
    
    if let Some(release_date) = metadata.get("tmdb_release_date").and_then(|v| v.as_str()) {
        info_parts.push(format!("Released: {}", release_date));
    }
    
    if let Some(language) = metadata.get("tmdb_original_language").and_then(|v| v.as_str()) {
        info_parts.push(format!("Language: {}", language));
    }
    
    if let Some(budget) = metadata.get("tmdb_budget").and_then(|v| v.as_str()) {
        if budget != "0" && !budget.is_empty() {
            info_parts.push(format!("Budget: ${}", budget));
        }
    }
    
    if let Some(revenue) = metadata.get("tmdb_revenue").and_then(|v| v.as_str()) {
        if revenue != "0" && !revenue.is_empty() {
            info_parts.push(format!("Revenue: ${}", revenue));
        }
    }
    
    // Add database IDs
    let mut db_info = Vec::new();
    if let Some(tmdb_id) = metadata.get("tmdb_id").and_then(|v| v.as_str()) {
        db_info.push(format!("TMDB ID: {}", tmdb_id));
    }
    if let Some(imdb_id) = metadata.get("imdb_id").and_then(|v| v.as_str()) {
        db_info.push(format!("IMDB ID: {}", imdb_id));
    }
    
    if !info_parts.is_empty() {
        let tmdb_content = format!("{} | {}", info_parts.join(" | "), db_info.join(" | "));
        components.push(DescriptionComponent::CustomSection {
            title: "Movie Database Info".to_string(),
            content: tmdb_content.clone(),
            format: SectionFormat::Plain,
        });
        log::info!("✅ ALT_DESC: Added TMDB info component: {}", tmdb_content);
    } else {
        log::info!("❌ ALT_DESC: No TMDB info parts found");
    }
    
    // Add cast if available
    if let Some(cast) = metadata.get("tmdb_cast").and_then(|v| v.as_str()) {
        components.push(DescriptionComponent::CustomSection {
            title: "Cast".to_string(),
            content: cast.to_string(),
            format: SectionFormat::Plain,
        });
        log::info!("✅ ALT_DESC: Added cast component: {}", cast);
    } else {
        log::info!("❌ ALT_DESC: No tmdb_cast found in metadata");
    }
    
    // Add synopsis last (takes most space)
    if let Some(synopsis) = metadata.get("tmdb_overview").and_then(|v| v.as_str()) {
        components.push(DescriptionComponent::Synopsis {
            text: synopsis.to_string(),
        });
        log::info!("✅ ALT_DESC: Added synopsis component ({} chars)", synopsis.len());
    } else if let Some(description) = metadata.get("description").and_then(|v| v.as_str()) {
        components.push(DescriptionComponent::Synopsis {
            text: description.to_string(),
        });
        log::info!("✅ ALT_DESC: Added description component ({} chars)", description.len());
    } else {
        log::info!("❌ ALT_DESC: No synopsis or description found in metadata");
    }
    
    log::info!("🎯 ALT_DESC: Final component count: {}", components.len());
    components
}