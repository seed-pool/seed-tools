// Common metadata types and traits

use crate::core::error::Result;
use serde::{Deserialize, Serialize};

/// Trait for metadata providers
pub trait MetadataProvider {
    /// Search for metadata
    fn search(&self, query: &str) -> Result<Vec<MetadataResult>>;

    /// Get detailed metadata by ID
    fn get_details(&self, id: &str) -> Result<MetadataResult>;

    /// Provider name
    fn name(&self) -> &'static str;
}

/// Common metadata result structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataResult {
    pub id: String,
    pub title: String,
    pub year: Option<u32>,
    pub overview: Option<String>,
    pub genres: Vec<String>,
    pub rating: Option<f64>,
    pub poster_url: Option<String>,
    pub backdrop_url: Option<String>,
    pub external_ids: ExternalIds,
    pub additional_data: serde_json::Value,
}

/// External IDs from various databases
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExternalIds {
    pub tmdb_id: Option<u32>,
    pub imdb_id: Option<String>,
    pub tvdb_id: Option<u32>,
    pub igdb_id: Option<u64>,
    pub musicbrainz_id: Option<String>,
}

/// Metadata type
#[derive(Debug, Clone, PartialEq)]
pub enum MetadataType {
    Movie,
    TvShow,
    Game,
    Music,
    Book,
}
