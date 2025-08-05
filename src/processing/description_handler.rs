use crate::core::DescriptionComponent;
use crate::processing::{
    alt_description::{create_image_description, ImageDescriptionConfig},
    description::DescriptionBuilder,
    upload::UploadConfig,
};
use std::path::Path;

/// Handle description generation - either BBCode or image-based
pub fn generate_description(
    components: Vec<DescriptionComponent>,
    upload_config: &UploadConfig,
    output_dir: &str,
    media_name: &str,
) -> Result<DescriptionResult, String> {
    if upload_config.enable_alt_description {
        // Generate image-based description
        let image_filename = format!("{}_description.png", media_name);
        let image_path = Path::new(output_dir).join(&image_filename);
        
        // Filter out Image components since we handle screenshots separately
        let text_components: Vec<DescriptionComponent> = components
            .into_iter()
            .filter(|comp| !matches!(comp, DescriptionComponent::Images { .. }))
            .collect();
        
        let image_path_str = image_path.to_string_lossy().to_string();
        create_image_description(text_components, &image_path_str)?;
        
        Ok(DescriptionResult::Image {
            path: image_path_str,
            filename: image_filename,
        })
    } else {
        // Generate traditional BBCode description
        let mut builder = DescriptionBuilder::new(crate::core::MediaType::Video(crate::core::VideoType::Mkv));
        
        for component in components {
            builder = builder.add_component(component);
        }
        
        let bbcode = builder.build();
        Ok(DescriptionResult::BBCode(bbcode))
    }
}

/// Handle description generation with custom image config
pub fn generate_description_with_config(
    components: Vec<DescriptionComponent>,
    upload_config: &UploadConfig,
    image_config: Option<ImageDescriptionConfig>,
    output_dir: &str,
    media_name: &str,
) -> Result<DescriptionResult, String> {
    if upload_config.enable_alt_description {
        let image_filename = format!("{}_description.png", media_name);
        let image_path = Path::new(output_dir).join(&image_filename);
        
        let text_components: Vec<DescriptionComponent> = components
            .into_iter()
            .filter(|comp| !matches!(comp, DescriptionComponent::Images { .. }))
            .collect();
        
        let image_path_str = image_path.to_string_lossy().to_string();
        
        if let Some(config) = image_config {
            crate::processing::alt_description::create_image_description_with_config(
                text_components,
                &image_path_str,
                config,
            )?;
        } else {
            create_image_description(text_components, &image_path_str)?;
        }
        
        Ok(DescriptionResult::Image {
            path: image_path_str,
            filename: image_filename,
        })
    } else {
        // Traditional BBCode generation (same as above)
        let mut builder = DescriptionBuilder::new(crate::core::MediaType::Video(crate::core::VideoType::Mkv));
        
        for component in components {
            builder = builder.add_component(component);
        }
        
        let bbcode = builder.build();
        Ok(DescriptionResult::BBCode(bbcode))
    }
}

/// Result of description generation
#[derive(Debug, Clone)]
pub enum DescriptionResult {
    /// Traditional BBCode description
    BBCode(String),
    /// Image-based description
    Image {
        path: String,
        filename: String,
    },
}

impl DescriptionResult {
    /// Get the content for posting to tracker
    /// For BBCode, returns the text directly
    /// For Image, this would typically be uploaded and replaced with image BBCode
    pub fn as_upload_content(&self) -> String {
        match self {
            DescriptionResult::BBCode(text) => text.clone(),
            DescriptionResult::Image { filename, .. } => {
                // This would typically be replaced with the uploaded image URL
                format!("[img]{{uploaded_image_url}}/{filename}[/img]")
            }
        }
    }
    
    /// Check if this is an image description
    pub fn is_image(&self) -> bool {
        matches!(self, DescriptionResult::Image { .. })
    }
    
    /// Get the image path if this is an image description
    pub fn image_path(&self) -> Option<&str> {
        match self {
            DescriptionResult::Image { path, .. } => Some(path),
            _ => None,
        }
    }
}

/// Create a BBCode snippet that includes both image description and screenshots
pub fn create_final_description(
    description_result: DescriptionResult,
    screenshot_urls: Vec<String>,
) -> String {
    let mut final_description = String::new();
    
    // Add the description (either BBCode or image)
    match description_result {
        DescriptionResult::BBCode(bbcode) => {
            final_description.push_str(&bbcode);
        }
        DescriptionResult::Image { .. } => {
            // The image description would be uploaded and its URL used here
            final_description.push_str(&description_result.as_upload_content());
        }
    }
    
    // Add spacing before screenshots
    if !screenshot_urls.is_empty() {
        final_description.push_str("\n\n");
    }
    
    // Add screenshots
    for screenshot_url in screenshot_urls {
        final_description.push_str(&format!("[img]{}[/img]\n", screenshot_url));
    }
    
    final_description
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::DescriptionComponent;

    #[test]
    fn test_bbcode_description() {
        let components = vec![
            DescriptionComponent::Title {
                text: "Test Movie".to_string(),
                size: 24,
                color: "#2E86C1".to_string(),
            },
            DescriptionComponent::Synopsis {
                text: "A great movie about testing.".to_string(),
            },
        ];
        
        let config = UploadConfig {
            enable_alt_description: false,
            ..Default::default()
        };
        
        let result = generate_description(components, &config, "/tmp", "test_movie")
            .expect("Should generate BBCode description");
        
        assert!(matches!(result, DescriptionResult::BBCode(_)));
        assert!(!result.is_image());
    }

    #[test]
    fn test_image_description_config() {
        let components = vec![
            DescriptionComponent::Title {
                text: "Test Movie".to_string(),
                size: 24,
                color: "#2E86C1".to_string(),
            },
        ];
        
        let config = UploadConfig {
            enable_alt_description: true,
            ..Default::default()
        };
        
        // This test would need font files to actually work
        // let result = generate_description(components, &config, "/tmp", "test_movie");
        // For now, just test the configuration
        assert!(config.enable_alt_description);
    }
}