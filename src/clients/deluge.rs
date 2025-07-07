// Deluge client implementation

use super::{TorrentClient, TorrentInfo};
use crate::core::DelugeConfig;
use reqwest::blocking::Client;

pub struct DelugeClient {
    config: DelugeConfig,
    client: Client,
    session_id: Option<String>,
}

impl DelugeClient {
    pub fn new(config: DelugeConfig) -> Result<Self, String> {
        let client = Client::new();
        let mut deluge_client = Self {
            config,
            client,
            session_id: None,
        };
        deluge_client.login()?;
        Ok(deluge_client)
    }

    fn login(&mut self) -> Result<(), String> {
        // TODO: Implement Deluge login
        // Extract from torrent.rs add_torrent_to_deluge
        unimplemented!("Deluge login")
    }
}

impl TorrentClient for DelugeClient {
    fn add_torrent(&self, torrent_data: &[u8], save_path: &str) -> Result<String, String> {
        // TODO: Implement add torrent
        unimplemented!("Add torrent to Deluge")
    }

    fn get_torrent_info(&self, hash: &str) -> Result<TorrentInfo, String> {
        // TODO: Implement get torrent info
        unimplemented!("Get torrent info from Deluge")
    }

    fn remove_torrent(&self, hash: &str, delete_files: bool) -> Result<(), String> {
        // TODO: Implement remove torrent
        unimplemented!("Remove torrent from Deluge")
    }

    fn list_torrents(&self) -> Result<Vec<TorrentInfo>, String> {
        // TODO: Implement list torrents
        unimplemented!("List torrents from Deluge")
    }
}
