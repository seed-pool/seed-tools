/// Tracker-specific type definitions
pub mod seedpool;
pub mod torrentleech;

/// Generic trait for torrent classification across all trackers
pub trait TorrentInfo {
    /// Get the category code as a number
    fn category_code(&self) -> u8;
    
    /// Get the type code as a number  
    fn type_code(&self) -> u8;
    
    /// Get a human-readable description
    fn description(&self) -> String;
    
    /// Get the category name
    fn category_name(&self) -> &'static str;
    
    /// Get the type name
    fn type_name(&self) -> &'static str;
    
    /// Check if this is an ebook category
    fn is_ebook_category(&self) -> bool;
    
    /// Check if this is a game category
    fn is_game_category(&self) -> bool;
    
    /// Check if this is an audio/music category
    fn is_audio_category(&self) -> bool;
    
    /// Check if this is a video category (movies, TV, etc.)
    fn is_video_category(&self) -> bool;
    
    /// Check if this is an audiobook category
    fn is_audiobook_category(&self) -> bool;
    
    /// Check if this is a hobby/miscellaneous category
    fn is_hobby_category(&self) -> bool;
    
    /// Check if this is a sports category
    fn is_sports_category(&self) -> bool;
    
    /// Check if this is an application category
    fn is_application_category(&self) -> bool;
    
    /// Check if this is an "other" or generic category
    fn is_other_category(&self) -> bool;
}