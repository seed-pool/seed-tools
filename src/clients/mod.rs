// External service clients - torrent clients, IRC, etc.

pub mod qbittorrent;
pub mod deluge;
pub mod irc;
pub mod sync;

// Re-export main client functionality
pub use irc::launch_irc_client;
pub use sync::sync_qbittorrent;

/// Common trait for torrent clients
pub trait TorrentClient {
    /// Add a torrent
    fn add_torrent(&self, torrent_data: &[u8], save_path: &str) -> Result<String, String>;
    
    /// Get torrent info
    fn get_torrent_info(&self, hash: &str) -> Result<TorrentInfo, String>;
    
    /// Remove torrent
    fn remove_torrent(&self, hash: &str, delete_files: bool) -> Result<(), String>;
    
    /// List all torrents
    fn list_torrents(&self) -> Result<Vec<TorrentInfo>, String>;
}

#[derive(Debug, Clone)]
pub struct TorrentInfo {
    pub hash: String,
    pub name: String,
    pub size: u64,
    pub progress: f32,
    pub status: String,
    pub save_path: String,
    pub category: Option<String>,
    pub tags: Vec<String>,
}