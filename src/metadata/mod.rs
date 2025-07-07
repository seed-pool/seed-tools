// External metadata services for enriching media information

pub mod common;
pub mod igdb;
pub mod musicbrainz;
pub mod tmdb;

// Re-export main metadata functions
pub use common::{MetadataProvider, MetadataResult};
pub use igdb::{
    clean_game_title_for_search, extract_igdb_companies, extract_igdb_cover_url,
    extract_igdb_metadata, get_igdb_game_details, search_igdb_game,
};
pub use musicbrainz::{
    extract_musicbrainz_metadata, get_musicbrainz_artist_details, get_musicbrainz_release_details,
    search_musicbrainz_artist, search_musicbrainz_release,
};
pub use tmdb::{extract_tmdb_metadata, fetch_external_ids, fetch_tmdb_details, fetch_tmdb_id};
