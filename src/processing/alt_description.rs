use crate::core::{DescriptionComponent, SectionFormat};
use image::{ImageBuffer, Rgb, RgbImage, imageops, ImageFormat, DynamicImage};
use imageproc::drawing::draw_text_mut;
use ab_glyph::{FontRef, PxScale};
use std::path::Path;
use std::io::Cursor;
use qrcode::QrCode;

/// Save an image with DPI metadata by adding pHYs chunk to PNG
fn save_image_with_dpi(img: &DynamicImage, output_path: &str, dpi: u32) -> Result<(), String> {
    log::info!("🎨 Saving image with {} DPI to: {}", dpi, output_path);
    
    // First save to a buffer
    let mut buffer = Vec::new();
    img.write_to(&mut Cursor::new(&mut buffer), ImageFormat::Png)
        .map_err(|e| format!("Failed to encode PNG: {}", e))?;
    
    // Add pHYs chunk for DPI
    let buffer_with_dpi = add_png_dpi(buffer, dpi)?;
    
    // Write the final buffer to file
    std::fs::write(output_path, buffer_with_dpi)
        .map_err(|e| format!("Failed to write PNG file: {}", e))?;
    
    log::info!("✅ Successfully saved image with {} DPI metadata", dpi);
    Ok(())
}

/// Add pHYs chunk to PNG data for DPI setting
fn add_png_dpi(png_data: Vec<u8>, dpi: u32) -> Result<Vec<u8>, String> {
    const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
    const IHDR_SIZE: usize = 25; // 8 bytes signature + 4 length + 4 type + 13 data + 4 CRC
    
    if !png_data.starts_with(PNG_SIGNATURE) {
        return Err("Not a valid PNG file".to_string());
    }
    
    // Convert DPI to pixels per meter (PNG uses meters)
    let pixels_per_meter = (dpi as f64 * 39.37008) as u32;
    
    // Create pHYs chunk data (9 bytes)
    let mut phys_data = Vec::new();
    phys_data.extend_from_slice(&pixels_per_meter.to_be_bytes()); // X pixels per meter
    phys_data.extend_from_slice(&pixels_per_meter.to_be_bytes()); // Y pixels per meter  
    phys_data.push(1); // Unit: meters
    
    // Create complete pHYs chunk with length, type, data, and CRC
    let mut phys_chunk = Vec::new();
    phys_chunk.extend_from_slice(&9u32.to_be_bytes()); // Length
    phys_chunk.extend_from_slice(b"pHYs"); // Type
    phys_chunk.extend_from_slice(&phys_data); // Data
    
    // Calculate CRC32 over type + data
    let mut type_and_data = Vec::new();
    type_and_data.extend_from_slice(b"pHYs");
    type_and_data.extend_from_slice(&phys_data);
    let crc = crc32fast::hash(&type_and_data);
    phys_chunk.extend_from_slice(&crc.to_be_bytes()); // CRC
    
    // Insert pHYs chunk after IHDR (at position 33: 8 signature + 25 IHDR)
    let insert_pos = PNG_SIGNATURE.len() + IHDR_SIZE;
    let mut result = Vec::new();
    result.extend_from_slice(&png_data[..insert_pos]);
    result.extend_from_slice(&phys_chunk);
    result.extend_from_slice(&png_data[insert_pos..]);
    
    Ok(result)
}

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

/// Create a music-optimized image configuration with flexible width based on title length
fn create_music_image_config(title_length: usize) -> ImageDescriptionConfig {
    // Calculate width based on title length - minimum 720px, expand for longer titles
    let base_width = 720;
    let extra_width_per_char = 8; // Extra pixels per character beyond base length
    let base_char_count = 50; // Characters that fit in base width
    
    let final_width = if title_length > base_char_count {
        base_width + ((title_length - base_char_count) * extra_width_per_char)
    } else {
        base_width
    };
    
    // Cap maximum width to prevent extremely wide images
    let final_width = final_width.min(1200);
    
    log::info!("🎵 Dynamic width calculation: title_length={}, final_width={}px", title_length, final_width);
    
    // Use a more reasonable fixed height for music albums
    let final_height = 300; // Keep it compact like original
    
    log::info!("🎵 Dynamic height calculation: final={}px", final_height);
    
    ImageDescriptionConfig {
        canvas_width: (final_width * 12) as u32, // 12x DPI for ultra high resolution
        canvas_height: (final_height * 12) as u32, // Dynamic height for content
        padding: 96,         // Scaled padding for ultra high resolution
        line_spacing: 48,    // Scaled line spacing
        
        title_font_size: 192.0,   // Scaled title for ultra high resolution
        subtitle_font_size: 144.0, // Scaled subtitle
        body_font_size: 120.0,    // Scaled body text
        
        background_color: Rgb([0, 0, 0]),     // Black background
        title_color: Rgb([100, 180, 255]),    // Light blue
        text_color: Rgb([220, 220, 220]),     // Light gray
        accent_color: Rgb([255, 180, 100]),   // Orange
        
        section_spacing: 72, // Scaled section spacing
    }
}

/// Create a video-optimized image configuration for movies/TV shows (fixed 1440x200 dimensions)
fn create_video_image_config(_title_length: usize) -> ImageDescriptionConfig {
    // Fixed dimensions for video content as requested: 1440x200 final size
    let final_width = 1440; // Fixed width as requested
    let final_height = 200; // Reduced height as requested
    
    log::info!("🎬 Creating video-optimized description image: {}x{}px (fixed dimensions)", final_width, final_height);
    
    ImageDescriptionConfig {
        canvas_width: (final_width * 12) as u32, // 12x DPI for ultra high resolution - 17280px canvas
        canvas_height: (final_height * 12) as u32, // 12x DPI for ultra high resolution - 2400px canvas
        padding: 96,         // Scaled padding for ultra high resolution
        line_spacing: 48,    // Scaled line spacing
        
        title_font_size: 192.0,   // Scaled title for ultra high resolution
        subtitle_font_size: 144.0, // Scaled subtitle
        body_font_size: 120.0,    // Scaled body text
        
        background_color: Rgb([0, 0, 0]),     // Black background
        title_color: Rgb([100, 180, 255]),    // Light blue
        text_color: Rgb([220, 220, 220]),     // Light gray
        accent_color: Rgb([255, 180, 100]),   // Orange
        
        section_spacing: 72, // Scaled section spacing
    }
}

/// Create the default seedbrr footer for alt descriptions (matching regular descriptions)
fn create_default_alt_description_footer() -> String {
    "[center][b][size=12][color=#757575]Created with mkbrr, ffmpeg, and mediainfo. Posted to this fine tracker with seedbrr.[/color][/size][/b]

    [url=https://github.com/seed-pool/seed-tools][img]https://cdn.seedpool.org/sp.png[/img][/url]  \
    [url=https://github.com/autobrr/mkbrr][img]https://cdn.seedpool.org/mkbrr.png[/img][/url]  \
    [url=https://www.rust-lang.org][img]https://cdn.seedpool.org/rust.png[/img][/url][/center]".to_string()
}

/// Generate a realistic UPC-style barcode image from a barcode string
fn generate_upc_barcode_image(barcode: &str, width: u32, height: u32) -> Result<RgbImage, String> {
    // Clean the barcode string (remove any non-digits)
    let clean_barcode: String = barcode.chars().filter(|c| c.is_ascii_digit()).collect();
    
    if clean_barcode.is_empty() {
        return Err("No valid digits in barcode".to_string());
    }
    
    log::info!("🔢 Generating realistic UPC barcode for: {}", clean_barcode);
    
    // Use full requested width for proper sizing
    let actual_width = width;
    
    // Calculate bar dimensions based on available width
    let available_width = actual_width - 20; // Leave margins
    let total_bars = clean_barcode.len() as u32 * 7; // Approximate bars per digit
    let bar_width = std::cmp::max(3, available_width / total_bars); // Minimum 3px bars
    
    log::info!("🔢 UPC barcode: width={}, bar_width={}, digits={}", actual_width, bar_width, clean_barcode.len());
    
    // Create result image with full requested width
    let mut result_img = ImageBuffer::new(actual_width, height);
    for pixel in result_img.pixels_mut() {
        *pixel = Rgb([255, 255, 255]); // White background
    }
    
    let barcode_height = height * 9 / 10; // Use 90% of available height
    let start_y = (height - barcode_height) / 10; // Small margin at top
    
    let mut current_x = 3; // Smaller left margin
    
    // Start guard pattern (3 bars) - use calculated bar width
    current_x += draw_barcode_bars(&mut result_img, current_x, start_y, barcode_height, &[bar_width, bar_width/2, bar_width, bar_width/2, bar_width]);
    current_x += bar_width; // Spacing
    
    // Generate bars for each digit using a realistic pattern
    for (i, digit_char) in clean_barcode.chars().enumerate() {
        if let Some(digit) = digit_char.to_digit(10) {
            // Create a realistic bar pattern based on the digit using calculated bar width
            let gap = bar_width / 2;
            let thick = bar_width * 2;
            let pattern = match digit % 10 {
                0 => vec![bar_width, gap*2, thick, gap, bar_width],
                1 => vec![thick, gap, bar_width, gap*2, bar_width],
                2 => vec![bar_width, gap, thick, gap, thick],
                3 => vec![thick, gap*2, bar_width, gap, thick],
                4 => vec![bar_width, gap, bar_width, gap, thick*2],
                5 => vec![thick, gap, bar_width*2, gap, bar_width],
                6 => vec![bar_width*2, gap, thick, gap, bar_width],
                7 => vec![thick, gap, thick, gap, bar_width],
                8 => vec![bar_width, gap*2, bar_width, gap, thick],
                9 => vec![thick*2, gap, bar_width, gap, bar_width],
                _ => vec![bar_width, gap, bar_width, gap, bar_width],
            };
            
            current_x += draw_barcode_bars(&mut result_img, current_x, start_y, barcode_height, &pattern);
            current_x += 1; // Smaller gap between digit groups
            
            // Add center guard pattern after 6 digits (for UPC-A style)
            if i == 5 && clean_barcode.len() > 6 {
                current_x += draw_barcode_bars(&mut result_img, current_x, start_y, barcode_height, &[gap, bar_width, gap, bar_width, gap]);
                current_x += bar_width;
            }
        }
        
        if current_x >= actual_width - 15 { // Leave space for end pattern
            break;
        }
    }
    
    // End guard pattern (3 bars) - use calculated bar width
    let gap = bar_width / 2;
    draw_barcode_bars(&mut result_img, current_x, start_y, barcode_height, &[bar_width, gap, bar_width, gap, bar_width]);
    
    // Debug: count black pixels to ensure bars were drawn
    let mut black_pixels = 0;
    for pixel in result_img.pixels() {
        if pixel[0] < 128 && pixel[1] < 128 && pixel[2] < 128 {
            black_pixels += 1;
        }
    }
    
    log::info!("✅ Generated compact UPC barcode: {}x{} (actual width: {}, black pixels: {})", width, height, actual_width, black_pixels);
    Ok(result_img)
}

/// Helper function to draw barcode bars
fn draw_barcode_bars(img: &mut RgbImage, start_x: u32, start_y: u32, height: u32, pattern: &[u32]) -> u32 {
    let mut x = start_x;
    let mut is_bar = true; // Start with a bar
    
    for &width in pattern {
        if is_bar {
            // Draw black bar
            for bar_x in x..(x + width).min(img.width()) {
                for bar_y in start_y..(start_y + height).min(img.height()) {
                    img.put_pixel(bar_x, bar_y, Rgb([0, 0, 0]));
                }
            }
        }
        // If !is_bar, it's a white space - do nothing (already white background)
        
        x += width;
        is_bar = !is_bar; // Alternate between bars and spaces
    }
    
    x - start_x // Return the total width used
}

/// Generate a QR code image for a URL
fn generate_qr_code_image(url: &str, size: u32) -> Result<RgbImage, String> {
    log::info!("📱 Generating QR code for: {}", url);
    
    // Generate QR code
    let qr_code = QrCode::new(url)
        .map_err(|e| format!("Failed to create QR code: {}", e))?;
    
    // Get QR code modules
    let modules = qr_code.render::<image::Luma<u8>>().build();
    
    // Convert to RGB and scale to target size
    let qr_size = modules.width();
    
    // Calculate scale to fit target size, ensuring at least 1 pixel per module
    let scale = if qr_size > size {
        1 // If QR is bigger than target, use minimum scale
    } else {
        std::cmp::max(1, size / qr_size) // Otherwise scale up to fit
    };
    let final_size = qr_size * scale;
    
    log::info!("📱 QR code raw size: {}x{}, target: {}, scale: {}, final size: {}x{}", qr_size, qr_size, size, scale, final_size, final_size);
    
    let mut result_img = ImageBuffer::new(final_size, final_size);
    
    // Fill with white background first
    for pixel in result_img.pixels_mut() {
        *pixel = Rgb([255, 255, 255]);
    }
    
    // Scale up the QR code
    for (x, y, pixel) in modules.enumerate_pixels() {
        let color = if pixel[0] == 0 { 
            Rgb([0, 0, 0])    // Black modules
        } else { 
            Rgb([255, 255, 255]) // White modules
        };
        
        // Scale up each module to multiple pixels
        for dx in 0..scale {
            for dy in 0..scale {
                let final_x = x * scale + dx;
                let final_y = y * scale + dy;
                if final_x < final_size && final_y < final_size {
                    result_img.put_pixel(final_x, final_y, color);
                }
            }
        }
    }
    
    log::info!("✅ Generated QR code image: {}x{}", final_size, final_size);
    Ok(result_img)
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
    
    /// Extract poster/cover art URL from metadata
    fn extract_poster_url(metadata: &serde_json::Value) -> Option<String> {
        // First check if we have a direct cover art URL (for music)
        if let Some(cover_url) = metadata.get("cover_art_url").and_then(|v| v.as_str()) {
            log::info!("🎨 Using provided cover art URL: {}", cover_url);
            return Some(cover_url.to_string());
        }
        
        // For video content, try to get IMDb ID from metadata
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
    
    /// Add a UPC barcode to the center bottom of the image
    fn add_upc_barcode_to_image(&self, img: &mut RgbImage, barcode: &str) {
        // Define barcode dimensions (scaled for 12x DPI canvas) - much larger for visibility
        let max_barcode_width = 3000; // Maximum width (250px final) - much larger
        let barcode_height = 600;     // 50px final height (600/12) - much taller
        
        match generate_upc_barcode_image(barcode, max_barcode_width, barcode_height) {
            Ok(barcode_img) => {
                // Use actual barcode image dimensions
                let actual_barcode_width = barcode_img.width();
                let actual_barcode_height = barcode_img.height();
                
                // Position barcode in center bottom closer to edge
                let padding = self.config.padding; // Reduced padding to get closer to bottom
                let start_x = (self.config.canvas_width - actual_barcode_width) / 2; // Center horizontally
                let start_y = self.config.canvas_height.saturating_sub(actual_barcode_height + padding);
                
                log::info!("🔢 Adding UPC barcode to image at position ({}, {}) with dimensions {}x{}", 
                          start_x, start_y, actual_barcode_width, actual_barcode_height);
                
                // First, draw a white background rectangle with a border to make the barcode stand out
                let border_width = 4; // 2px border in final image
                let bg_start_x = start_x.saturating_sub(border_width);
                let bg_start_y = start_y.saturating_sub(border_width);
                let bg_width = actual_barcode_width + (border_width * 2);
                let bg_height = actual_barcode_height + (border_width * 2);
                
                // Draw white background with dark border
                for y in 0..bg_height {
                    for x in 0..bg_width {
                        let img_x = bg_start_x + x;
                        let img_y = bg_start_y + y;
                        
                        if img_x < self.config.canvas_width && img_y < self.config.canvas_height {
                            // Draw border
                            if x < border_width || x >= bg_width - border_width ||
                               y < border_width || y >= bg_height - border_width {
                                img.put_pixel(img_x, img_y, Rgb([80, 80, 80])); // Dark gray border
                            } else {
                                img.put_pixel(img_x, img_y, Rgb([255, 255, 255])); // White background
                            }
                        }
                    }
                }
                
                // Then overlay the barcode
                for y in 0..actual_barcode_height {
                    for x in 0..actual_barcode_width {
                        let img_x = start_x + x;
                        let img_y = start_y + y;
                        
                        if img_x < self.config.canvas_width && img_y < self.config.canvas_height {
                            if let Some(barcode_pixel) = barcode_img.get_pixel_checked(x, y) {
                                // Overlay all pixels - the barcode should have its own clean background
                                img.put_pixel(img_x, img_y, *barcode_pixel);
                            }
                        }
                    }
                }
                
                log::info!("✅ Successfully added UPC barcode to image");
            }
            Err(e) => {
                log::warn!("⚠️ Failed to generate UPC barcode: {}", e);
            }
        }
    }
    
    /// Add a QR code to the bottom right corner of the image
    fn add_qr_code_to_image(&self, img: &mut RgbImage, url: &str) {
        // Define QR code dimensions (scaled for 12x DPI canvas) - much larger for visibility
        let qr_size = 1200; // 100px final size (1200/12) - much larger
        
        match generate_qr_code_image(url, qr_size) {
            Ok(qr_img) => {
                // Position QR code in bottom right corner closer to edge
                let padding = self.config.padding; // Reduced padding to get closer to bottom
                let start_x = self.config.canvas_width.saturating_sub(qr_img.width() + padding);
                let start_y = self.config.canvas_height.saturating_sub(qr_img.height() + padding);
                
                log::info!("📱 Adding QR code to image at position ({}, {}) with dimensions {}x{}", 
                          start_x, start_y, qr_img.width(), qr_img.height());
                
                // Overlay the QR code onto the main image
                for y in 0..qr_img.height() {
                    for x in 0..qr_img.width() {
                        let img_x = start_x + x;
                        let img_y = start_y + y;
                        
                        if img_x < self.config.canvas_width && img_y < self.config.canvas_height {
                            if let Some(qr_pixel) = qr_img.get_pixel_checked(x, y) {
                                // Overlay both black and white pixels to ensure clean QR code
                                img.put_pixel(img_x, img_y, *qr_pixel);
                            }
                        }
                    }
                }
                
                log::info!("✅ Successfully added QR code to image");
            }
            Err(e) => {
                log::warn!("⚠️ Failed to generate QR code: {}", e);
            }
        }
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
        
        log::info!("🎵 ALT_DESC: Total components to process: {}", self.components.len());
        
        for (i, component) in self.components.iter().enumerate() {
            match component {
                DescriptionComponent::Synopsis { text } => {
                    log::info!("🎵 ALT_DESC: Found Synopsis component #{}: {} chars", i, text.len());
                    synopsis_component = Some(component);
                }
                DescriptionComponent::Title { text, .. } => {
                    log::info!("🎵 ALT_DESC: Found Title component #{}: {}", i, text);
                    left_components.push(component);
                }
                DescriptionComponent::CustomSection { title, content, .. } => {
                    log::info!("🎵 ALT_DESC: Found CustomSection component #{}: {} - {} chars", i, title, content.len());
                    left_components.push(component);
                }
                _ => {
                    log::info!("🎵 ALT_DESC: Found Other component #{}", i);
                    left_components.push(component);
                }
            }
        }
        
        // Check if this is audio content (music albums) to use single column layout for synopsis
        let is_audio_content = if let Some(meta) = metadata {
            meta.get("musicbrainz_artist_name").is_some() ||
            meta.get("artist").is_some() ||
            meta.get("album").is_some() ||
            meta.get("musicbrainz_title").is_some()
        } else {
            false
        };
        
        log::info!("🎵 ALT_DESC: is_audio_content = {}", is_audio_content);
        
        // Render left column (metadata info) - adjusted for poster
        let mut current_y = self.config.padding;
        for component in left_components {
            current_y = self.render_component_left_column_with_offset(&mut img, component, current_y, text_start_x);
        }
        
        // Render synopsis - for music content, always place below track info in single column
        if let Some(synopsis) = synopsis_component {
            log::info!("🎵 ALT_DESC: Rendering synopsis component, is_audio_content = {}", is_audio_content);
            if is_audio_content {
                // For music content, place synopsis with moderate spacing after track info
                current_y += self.config.section_spacing * 2; // Extra spacing before synopsis
                
                log::info!("🎵 ALT_DESC: Rendering synopsis in single column at y = {} (canvas height = {})", current_y, self.config.canvas_height);
                
                // Check if the synopsis component is actually being rendered
                match synopsis {
                    DescriptionComponent::Synopsis { text } => {
                        log::info!("🎵 ALT_DESC: Synopsis text content: '{}'", text);
                        
                        // Render synopsis text directly since render_component_left_column_with_offset might not handle Synopsis
                        self.draw_bold_text(&mut img, "Release Information:", text_start_x as i32, current_y as i32,
                                           self.config.subtitle_font_size, self.config.accent_color);
                        current_y += (self.config.subtitle_font_size as u32) + self.config.line_spacing;
                        
                        // Split synopsis text into lines and render
                        let max_chars_per_line = 80;
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
                                    self.draw_pure_text(&mut img, &current_line, (text_start_x + 20) as i32, current_y as i32,
                                                       self.config.body_font_size, self.config.text_color);
                                    current_y += (self.config.body_font_size as u32) + self.config.line_spacing;
                                }
                                current_line = word.to_string();
                            }
                        }
                        
                        // Draw the last line
                        if !current_line.is_empty() {
                            self.draw_pure_text(&mut img, &current_line, (text_start_x + 20) as i32, current_y as i32,
                                               self.config.body_font_size, self.config.text_color);
                        }
                        
                        log::info!("✅ ALT_DESC: Successfully rendered synopsis text");
                    }
                    _ => {
                        log::warn!("🎵 ALT_DESC: Synopsis component is not a Synopsis type!");
                    }
                }
            } else {
                // For video content, use right column
                log::info!("🎵 ALT_DESC: Rendering synopsis in right column");
                self.render_synopsis_right_column(&mut img, synopsis);
            }
        } else {
            log::info!("🎵 ALT_DESC: No synopsis component found");
        }
        
        // Add barcodes for music content if available
        if is_audio_content {
            if let Some(meta) = metadata {
                // Add UPC barcode if available
                if let Some(barcode) = meta.get("musicbrainz_barcode").and_then(|v| v.as_str()) {
                    log::info!("🎵 Adding UPC barcode to image: {}", barcode);
                    self.add_upc_barcode_to_image(&mut img, barcode);
                }
                
                // Add QR code to MusicBrainz page if release ID available
                if let Some(release_id) = meta.get("musicbrainz_release_id").and_then(|v| v.as_str()) {
                    let musicbrainz_url = format!("https://musicbrainz.org/release/{}", release_id);
                    log::info!("🎵 Adding QR code to image: {}", musicbrainz_url);
                    self.add_qr_code_to_image(&mut img, &musicbrainz_url);
                } else {
                    log::info!("🎵 No MusicBrainz release ID found for QR code");
                }
            }
        }
        
        // Resize from high-DPI rendering down to target size for sharp fonts
        // Calculate target size as 1/12 of canvas size (since canvas is 12x DPI)
        let target_width = self.config.canvas_width / 12;
        let target_height = self.config.canvas_height / 12;
        let resized_img = image::DynamicImage::ImageRgb8(img)
            .resize_exact(target_width, target_height, imageops::FilterType::Lanczos3);
        
        // Save the resized image with high DPI setting
        save_image_with_dpi(&resized_img, output_path, 300)?;
        
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
                next_y += (self.config.subtitle_font_size as u32) + self.config.line_spacing;
                
                // Draw content with extended length for cast and other details
                let content_text = if content.len() > 120 {
                    format!("{}...", &content[..117])
                } else {
                    content.clone()
                };
                
                // Split long content into multiple lines if needed for left column
                let max_chars_per_line = 80; // Increased for better space utilization
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
            // Calculate right column start based on canvas width (better spacing for flexible widths)
            let right_column_start = (self.config.canvas_width * 3) / 5; // Use 60% of width for better balance
            let mut current_y = self.config.padding;
            
            // Draw "Synopsis:" label in bold
            self.draw_bold_text(img, "Synopsis:", right_column_start as i32, current_y as i32,
                               self.config.subtitle_font_size, self.config.accent_color);
            current_y += (self.config.subtitle_font_size as u32) + self.config.line_spacing;
            
            // Draw synopsis with word wrapping to fit right column (adjusted for flexible width)
            let max_chars_per_line = 70; // Increased for better space utilization
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
    
    // Check if this is audio content to use custom configuration
    let is_audio = metadata.get("artist").is_some() 
        || metadata.get("album").is_some() 
        || metadata.get("musicbrainz_artist_name").is_some();
    
    if is_audio {
        // Note: MusicBrainz data check is now handled earlier in the upload flow
        
        // Get title for width calculation
        let title = metadata.get("title")
            .or_else(|| metadata.get("musicbrainz_title"))
            .or_else(|| metadata.get("album"))
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown Album");
        
        log::info!("🎵 Creating audio-optimized description image with flexible width for title: '{}'", title);
        // Use custom config for music albums with flexible width based on title length
        let music_config = create_music_image_config(title.len());
        
        // Try to fetch cover art for audio content
        let mut enriched_metadata = metadata.clone();
        if let Some(release_id) = metadata.get("musicbrainz_release_id").and_then(|v| v.as_str()) {
            match crate::metadata::musicbrainz::get_cover_art_url(release_id) {
                Ok(Some(cover_url)) => {
                    log::info!("🎨 Adding cover art to metadata: {}", cover_url);
                    enriched_metadata["cover_art_url"] = serde_json::json!(cover_url);
                }
                Ok(None) => {
                    log::info!("🎨 No cover art available for this release");
                }
                Err(e) => {
                    log::warn!("⚠️ Failed to fetch cover art: {}", e);
                }
            }
        }
        
        // Create image builder with music config and enriched metadata
        let builder = ImageDescriptionBuilder::with_config(music_config)
            .with_components(components);
        builder.build_with_metadata(&output_path_str, Some(&enriched_metadata))?;
    } else {
        // Video/Movie content - use video-optimized config with same dynamic sizing as music
        // Get title for width calculation (same logic as music)
        let title = metadata.get("title")
            .or_else(|| metadata.get("tmdb_title"))
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown Title");
        
        log::info!("🎬 Creating video-optimized description image with flexible width for title: '{}'", title);
        let video_config = create_video_image_config(title.len());
        let builder = ImageDescriptionBuilder::with_config(video_config)
            .with_components(components);
        builder.build_with_metadata(&output_path_str, Some(metadata))?;
    }
    
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
            } else if key.starts_with("musicbrainz") {
                log::info!("🎵 ALT_DESC: MusicBrainz field '{}': {:?}", key, value);
            }
        }
    }
    
    // Check if this is audio content (look for audio-specific fields)
    let is_audio = metadata.get("artist").is_some() 
        || metadata.get("album").is_some() 
        || metadata.get("musicbrainz_artist_name").is_some();
    
    if is_audio {
        log::info!("🎵 ALT_DESC: Detected audio content, using music-specific components");
        return create_audio_components(metadata);
    }
    
    // Video/Movie content handling (existing logic)
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
    
    // Note: Custom descriptions and footer are now handled in upload.rs for BBCode output
    // They are no longer rendered into the alt description image itself
    
    log::info!("🎯 ALT_DESC: Final component count: {}", components.len());
    components
}

/// Create audio-specific description components from metadata
fn create_audio_components(metadata: &serde_json::Value) -> Vec<DescriptionComponent> {
    let mut components = Vec::new();
    
    // Helper function to get value from metadata with fallback
    let get_value = |key: &str| -> Option<&str> {
        metadata.get(key).and_then(|v| v.as_str())
    };
    
    // Add album title and artist
    let title = if let (Some(album), Some(artist)) = (get_value("album"), get_value("artist")) {
        format!("{} - {}", artist, album)
    } else if let Some(title) = get_value("title") {
        title.to_string()
    } else if let Some(album) = get_value("album") {
        album.to_string()
    } else if let Some(artist) = get_value("artist") {
        artist.to_string()
    } else {
        "Unknown Album".to_string()
    };
    
    // Add year if available
    let title_with_year = if let Some(year) = get_value("year").or_else(|| get_value("musicbrainz_date")) {
        format!("{} ({})", title, year)
    } else {
        title
    };
    
    components.push(DescriptionComponent::Title {
        text: title_with_year.clone(),
        size: 22,
        color: "#64B4FF".to_string(),
    });
    log::info!("✅ ALT_DESC: Added audio title component: {}", title_with_year);
    
    // Add artist as "author"
    if let Some(artist) = get_value("artist").or_else(|| get_value("musicbrainz_artist_name")) {
        components.push(DescriptionComponent::Author {
            name: artist.to_string(),
            color: "#50C8B4".to_string(),
        });
        log::info!("✅ ALT_DESC: Added artist component: {}", artist);
    }
    
    // Add album information section with more MusicBrainz data
    let mut album_info = Vec::new();
    
    if let Some(labels) = get_value("label").or_else(|| get_value("musicbrainz_labels")) {
        album_info.push(format!("Labels: {}", labels));
    }
    
    if let Some(catalog) = get_value("catalog_number").or_else(|| get_value("musicbrainz_catalog_numbers")) {
        album_info.push(format!("Catalog: {}", catalog));
    }
    
    if let Some(release_type) = get_value("musicbrainz_primary_type") {
        album_info.push(format!("Type: {}", release_type));
    }
    
    // Add disambiguation (e.g., "deluxe edition")
    if let Some(disambiguation) = get_value("musicbrainz_disambiguation") {
        if !disambiguation.is_empty() {
            album_info.push(format!("Edition: {}", disambiguation));
        }
    }
    
    // Add packaging information
    if let Some(packaging) = get_value("musicbrainz_packaging") {
        album_info.push(format!("Packaging: {}", packaging));
    }
    
    if let Some(country) = get_value("musicbrainz_country") {
        album_info.push(format!("Country: {}", country));
    }
    
    if let Some(genre) = get_value("genre").or_else(|| get_value("musicbrainz_genre")) {
        album_info.push(format!("Genre: {}", genre));
    }
    
    // Add audio format info
    if let Some(format) = get_value("format") {
        album_info.push(format!("Format: {}", format));
    }
    
    if get_value("is_lossless").map(|v| v == "true").unwrap_or(false) {
        album_info.push("Quality: Lossless".to_string());
    }
    
    if let Some(sample_rate) = get_value("sample_rate") {
        album_info.push(format!("Sample Rate: {}Hz", sample_rate));
    }
    
    if let Some(bitrate) = get_value("bitrate") {
        album_info.push(format!("Bitrate: {} kbps", bitrate));
    }
    
    // Add MusicBrainz IDs
    let mut mb_ids = Vec::new();
    if let Some(mb_artist_id) = get_value("musicbrainz_artist_id") {
        mb_ids.push(format!("MB Artist: {}", mb_artist_id));
    }
    if let Some(mb_album_id) = get_value("musicbrainz_album_id") {
        mb_ids.push(format!("MB Release: {}", mb_album_id));
    }
    
    if !album_info.is_empty() {
        let full_info = if !mb_ids.is_empty() {
            format!("{} | {}", album_info.join(" | "), mb_ids.join(" | "))
        } else {
            album_info.join(" | ")
        };
        
        components.push(DescriptionComponent::CustomSection {
            title: "Album Information".to_string(),
            content: full_info.clone(),
            format: SectionFormat::Plain,
        });
        log::info!("✅ ALT_DESC: Added album info component: {}", full_info);
    }
    
    // Add track count and length info with media format details
    let mut track_info = Vec::new();
    
    if let Some(track_count) = get_value("musicbrainz_track_count").or_else(|| get_value("track_count")) {
        track_info.push(format!("Tracks: {}", track_count));
    }
    
    if let Some(total_length) = get_value("musicbrainz_total_length").or_else(|| get_value("total_length")) {
        track_info.push(format!("Duration: {}", total_length));
    }
    
    if let Some(disc_count) = get_value("disc_count") {
        if disc_count != "1" {
            track_info.push(format!("Discs: {}", disc_count));
        }
    }
    
    // Add media format information
    if let Some(media_format) = get_value("musicbrainz_media_format") {
        track_info.push(format!("Media: {}", media_format));
    }
    
    // Add language information
    if let Some(language) = get_value("musicbrainz_language") {
        track_info.push(format!("Language: {}", language));
    }
    
    if !track_info.is_empty() {
        components.push(DescriptionComponent::CustomSection {
            title: "Track Information".to_string(),
            content: track_info.join(" | "),
            format: SectionFormat::Plain,
        });
        log::info!("✅ ALT_DESC: Added track info component");
    }
    
    // Add MusicBrainz release information instead of tracklist (which is already in the table)
    let mut release_info = Vec::new();
    
    if let Some(date) = get_value("musicbrainz_date") {
        release_info.push(format!("Release Date: {}", date));
    }
    
    if let Some(status) = get_value("musicbrainz_status") {
        release_info.push(format!("Status: {}", status));
    }
    
    if let Some(barcode) = get_value("musicbrainz_barcode") {
        release_info.push(format!("Barcode: {}", barcode));
    }
    
    if !release_info.is_empty() {
        components.push(DescriptionComponent::Synopsis {
            text: format!("Release Information:\n{}", release_info.join("\n")),
        });
        log::info!("✅ ALT_DESC: Added MusicBrainz release info component ({} items)", release_info.len());
    } else if let Some(description) = get_value("description") {
        components.push(DescriptionComponent::Synopsis {
            text: description.to_string(),
        });
        log::info!("✅ ALT_DESC: Added description component ({} chars)", description.len());
    } else {
        // Add a basic description with available info
        let artist_name = get_value("artist").or_else(|| get_value("musicbrainz_artist")).unwrap_or("Unknown Artist");
        let album_name = get_value("album").or_else(|| get_value("musicbrainz_title")).unwrap_or("Unknown Album");
        let basic_desc = format!("Music release: {} by {}", album_name, artist_name);
        components.push(DescriptionComponent::Synopsis {
            text: basic_desc.clone(),
        });
        log::info!("✅ ALT_DESC: Added fallback description: {}", basic_desc);
    }
    
    // Note: Custom descriptions and footer are now handled in upload.rs for BBCode output  
    // They are no longer rendered into the alt description image itself
    
    log::info!("🎯 ALT_DESC: Final audio component count: {}", components.len());
    components
}
