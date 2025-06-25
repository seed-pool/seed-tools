use std::fmt::Debug;
use crate::types::{
    VideoCategory, VideoSourceType, AudioCategory, AudioSourceType,
    EbookCategory, GameCategory, HobbyCategory
};

/// A single mapping entry that maps internal categories/types to tracker-specific IDs
#[derive(Debug, Clone)]
pub struct CategoryMapping {
    /// Internal category name (e.g., "VideoCategory::Movie", "AudioCategory::Album")
    pub internal_category: &'static str,
    /// Internal type/source name (e.g., "VideoSourceType::BluRay", "AudioSourceType::CD")
    /// Can be None if only category matters
    pub internal_type: Option<&'static str>,
    /// Tracker's category ID
    pub external_category: u32,
    /// Tracker's type ID (if applicable)
    pub external_type: Option<u32>,
}

impl CategoryMapping {
    pub fn new(
        internal_category: &'static str,
        internal_type: Option<&'static str>,
        external_category: u32,
        external_type: Option<u32>,
    ) -> Self {
        Self {
            internal_category,
            internal_type,
            external_category,
            external_type,
        }
    }
}

/// Helper struct to manage category mappings for a tracker
#[derive(Clone)]
pub struct TrackerMappingEngine {
    mappings: Vec<CategoryMapping>,
}

impl TrackerMappingEngine {
    pub fn new(mappings: Vec<CategoryMapping>) -> Self {
        Self { mappings }
    }
    
    /// Find a mapping by category and optional type
    pub fn find_mapping(
        &self,
        category: &str,
        source_type: Option<&str>,
    ) -> Option<(u32, Option<u32>)> {
        // First try to find exact match with both category and type
        if let Some(source) = source_type {
            if let Some(mapping) = self.mappings.iter().find(|m| {
                m.internal_category == category && m.internal_type.as_deref() == Some(source)
            }) {
                return Some((mapping.external_category, mapping.external_type));
            }
        }
        
        // If no exact match, try category-only match
        if let Some(mapping) = self.mappings.iter().find(|m| {
            m.internal_category == category && m.internal_type.is_none()
        }) {
            return Some((mapping.external_category, mapping.external_type));
        }
        
        None
    }
    
    /// Find a mapping with a default fallback
    pub fn find_mapping_or_default(
        &self,
        category: &str,
        source_type: Option<&str>,
        default_category: u32,
        default_type: Option<u32>,
    ) -> (u32, Option<u32>) {
        self.find_mapping(category, source_type)
            .unwrap_or((default_category, default_type))
    }
    
    /// Map a VideoCategory with optional source type
    pub fn map_video(&self, category: &VideoCategory, source: Option<&VideoSourceType>) -> Option<(u32, Option<u32>)> {
        let cat_str = format!("VideoCategory::{:?}", category);
        let type_str = source.map(|s| format!("VideoSourceType::{:?}", s));
        self.find_mapping(&cat_str, type_str.as_deref())
    }
    
    /// Map an AudioCategory with optional source type
    pub fn map_audio(&self, category: &AudioCategory, source: Option<&AudioSourceType>) -> Option<(u32, Option<u32>)> {
        let cat_str = format!("AudioCategory::{:?}", category);
        let type_str = source.map(|s| format!("AudioSourceType::{:?}", s));
        self.find_mapping(&cat_str, type_str.as_deref())
    }
    
    /// Map an EbookCategory
    pub fn map_ebook(&self, category: &EbookCategory) -> Option<(u32, Option<u32>)> {
        let cat_str = format!("EbookCategory::{:?}", category);
        self.find_mapping(&cat_str, None)
    }
    
    /// Map a GameCategory
    pub fn map_game(&self, category: &GameCategory) -> Option<(u32, Option<u32>)> {
        let cat_str = format!("GameCategory::{:?}", category);
        self.find_mapping(&cat_str, None)
    }
    
    /// Map a HobbyCategory
    pub fn map_hobby(&self, category: &HobbyCategory) -> Option<(u32, Option<u32>)> {
        let cat_str = format!("HobbyCategory::{:?}", category);
        self.find_mapping(&cat_str, None)
    }
}

/// Macro to simplify creating category mappings
#[macro_export]
macro_rules! mapping {
    // With both category and type
    ($cat:expr, $type:expr => $ext_cat:expr, $ext_type:expr) => {
        CategoryMapping::new($cat, Some($type), $ext_cat, Some($ext_type))
    };
    // Category and type to external category only
    ($cat:expr, $type:expr => $ext_cat:expr) => {
        CategoryMapping::new($cat, Some($type), $ext_cat, None)
    };
    // Category only to both external category and type
    ($cat:expr => $ext_cat:expr, $ext_type:expr) => {
        CategoryMapping::new($cat, None, $ext_cat, Some($ext_type))
    };
    // Category only to external category only
    ($cat:expr => $ext_cat:expr) => {
        CategoryMapping::new($cat, None, $ext_cat, None)
    };
}

/// Example function showing how to create Seedpool mappings
/// This would be defined in the tracker implementation
pub fn create_seedpool_mappings() -> Vec<CategoryMapping> {
    vec![
        // Video mappings with source types
        mapping!("VideoCategory::Movie", "VideoSourceType::UHDBluRay" => 10, 7),
        mapping!("VideoCategory::Movie", "VideoSourceType::BluRay" => 1, 8),
        mapping!("VideoCategory::Movie", "VideoSourceType::Remux" => 1, 2),
        mapping!("VideoCategory::Movie", "VideoSourceType::WebDL" => 1, 4),
        mapping!("VideoCategory::Movie", "VideoSourceType::WebRip" => 1, 5),
        mapping!("VideoCategory::Movie", "VideoSourceType::HDTV" => 1, 6),
        mapping!("VideoCategory::Movie" => 1, 22), // Default movie
        
        mapping!("VideoCategory::TvShow", "VideoSourceType::BluRay" => 2, 8),
        mapping!("VideoCategory::TvShow", "VideoSourceType::WebDL" => 2, 4),
        mapping!("VideoCategory::TvShow" => 2, 24), // Default TV
        
        mapping!("VideoCategory::Anime" => 6, 27),
        mapping!("VideoCategory::Sports" => 8, 19),
        mapping!("VideoCategory::Documentary" => 1, 22),
        
        // Audio mappings
        mapping!("AudioCategory::Album", "AudioSourceType::CD" => 5, 11),
        mapping!("AudioCategory::Album", "AudioSourceType::Web" => 5, 13),
        mapping!("AudioCategory::Album", "AudioSourceType::Vinyl" => 5, 11),
        mapping!("AudioCategory::Audiobook" => 9, 21),
        mapping!("AudioCategory::Soundtrack" => 5, 10),
        
        // Ebook mappings
        mapping!("EbookCategory::Novel" => 7, 20),
        mapping!("EbookCategory::Comic" => 7, 9),
        mapping!("EbookCategory::Technical" => 15, 32),
        
        // Game mappings
        mapping!("GameCategory::PCGame" => 14, 16),
        mapping!("GameCategory::PS4Game" => 14, 28),
        mapping!("GameCategory::PS5Game" => 14, 28),
        mapping!("GameCategory::XboxGame" => 14, 35),
        mapping!("GameCategory::NintendoSwitch" => 14, 15),
        mapping!("GameCategory::Retro" => 19, 17),
        
        // Hobby mappings
        mapping!("HobbyCategory::Tutorial" => 15, 32),
        mapping!("HobbyCategory::Documents" => 12, 17),
        // ... etc
    ]
}

/// Example function showing how to create TorrentLeech mappings
pub fn create_torrentleech_mappings() -> Vec<CategoryMapping> {
    vec![
        // TorrentLeech only uses category IDs, no type IDs
        mapping!("VideoCategory::Movie", "VideoSourceType::UHDBluRay" => 47),
        mapping!("VideoCategory::Movie", "VideoSourceType::BluRay" => 13),
        mapping!("VideoCategory::Movie", "VideoSourceType::DVD" => 12),
        mapping!("VideoCategory::Movie", "VideoSourceType::WebDL" => 37),
        mapping!("VideoCategory::Movie", "VideoSourceType::WebRip" => 37),
        mapping!("VideoCategory::Movie" => 13), // Default
        
        mapping!("VideoCategory::TvShow" => 32),
        mapping!("VideoCategory::Anime" => 34),
        mapping!("VideoCategory::Sports" => 30),
        mapping!("VideoCategory::Documentary" => 29),
        
        mapping!("AudioCategory::Audiobook" => 35),
        mapping!("AudioCategory::Album" => 17), // Music Videos
        
        mapping!("EbookCategory::Novel" => 45), // E-Learning
        
        mapping!("GameCategory::PCGame" => 42),
        mapping!("GameCategory::PS4Game" => 40),
        mapping!("GameCategory::PS5Game" => 40),
        mapping!("GameCategory::XboxGame" => 41),
        mapping!("GameCategory::NintendoSwitch" => 39),
        
        mapping!("HobbyCategory::Tutorial" => 45),
        // ... etc
    ]
}