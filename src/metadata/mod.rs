// External metadata services for enriching media information

pub mod tmdb;
pub mod igdb;
pub mod musicbrainz;
pub mod common;

// Re-export main metadata functions
pub use tmdb::{fetch_tmdb_id, fetch_external_ids, fetch_tmdb_details, extract_tmdb_metadata};
pub use igdb::{search_igdb_game, get_igdb_game_details, clean_game_title_for_search, extract_igdb_companies, extract_igdb_cover_url, extract_igdb_metadata};
pub use musicbrainz::{search_musicbrainz_release, get_musicbrainz_release_details, extract_musicbrainz_metadata, search_musicbrainz_artist, get_musicbrainz_artist_details};
pub use common::{MetadataProvider, MetadataResult};