// Category and type mappings for different trackers

use std::collections::HashMap;

/// Get category mappings for a specific tracker
pub fn get_category_mappings(tracker: &str) -> HashMap<String, String> {
    match tracker {
        "seedpool" => seedpool_categories(),
        "torrentleech" => torrentleech_categories(),
        _ => HashMap::new(),
    }
}

/// Get type mappings for a specific tracker
pub fn get_type_mappings(tracker: &str) -> HashMap<String, String> {
    match tracker {
        "seedpool" => seedpool_types(),
        "torrentleech" => torrentleech_types(),
        _ => HashMap::new(),
    }
}

fn seedpool_categories() -> HashMap<String, String> {
    let mut map = HashMap::new();
    map.insert("Movie".to_string(), "01".to_string());
    map.insert("TvShow".to_string(), "02".to_string());
    map.insert("Documentary".to_string(), "03".to_string());
    map.insert("Music".to_string(), "04".to_string());
    map.insert("GamePC".to_string(), "05".to_string());
    map.insert("GameConsole".to_string(), "06".to_string());
    map.insert("Ebook".to_string(), "07".to_string());
    map.insert("Software".to_string(), "08".to_string());
    map.insert("Other".to_string(), "09".to_string());
    // Add more as needed
    map
}

fn seedpool_types() -> HashMap<String, String> {
    let mut map = HashMap::new();
    map.insert("FullDisc".to_string(), "01".to_string());
    map.insert("Remux".to_string(), "02".to_string());
    map.insert("Encode".to_string(), "03".to_string());
    map.insert("WEB-DL".to_string(), "04".to_string());
    map.insert("WEBRip".to_string(), "05".to_string());
    map.insert("HDTV".to_string(), "06".to_string());
    map.insert("UHDBluRay".to_string(), "07".to_string());
    map.insert("BluRay".to_string(), "08".to_string());
    map.insert("Other".to_string(), "17".to_string());
    map.insert("Episode".to_string(), "24".to_string());
    // Season type removed - all TV shows use source types or fallback to Other (17)
    // Add more as needed
    map
}

fn torrentleech_categories() -> HashMap<String, String> {
    let mut map = HashMap::new();
    map.insert("Movies/Bluray".to_string(), "8".to_string());
    map.insert("Movies/4K".to_string(), "41".to_string());
    map.insert("TV/Episodes".to_string(), "26".to_string());
    // Add more as needed
    map
}

fn torrentleech_types() -> HashMap<String, String> {
    // TorrentLeech doesn't use separate type codes
    HashMap::new()
}
