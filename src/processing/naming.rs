use regex::Regex;

/// Functions for generating release names and sanitizing file/path names

pub fn generate_release_name(base_name: &str) -> String {
    let mut release_name = base_name.to_string();

    // Remove file extensions
    if let Ok(re) = Regex::new(r"\.(epub|mobi|pdf|txt|mkv|mp4|m4b|avi|mov|flv|wmv|ts)$") {
        release_name = re.replace(&release_name, "").to_string();
    }

    // First, remove spaces after dashes to preserve release group format (- PULS3 becomes -PULS3)
    if let Ok(re) = Regex::new(r"-\s+") {
        release_name = re.replace_all(&release_name, "-").to_string();
    }
    
    // Replace non-alphanumeric characters with dots
    if let Ok(re) = Regex::new(r"[^A-Za-z0-9+\-]") {
        release_name = re.replace_all(&release_name, ".").to_string();
    }

    // Replace multiple dots with a single dot
    if let Ok(re) = Regex::new(r"\.\.+") {
        release_name = re.replace_all(&release_name, ".").to_string();
    }

    // Replace mixed dot-dash patterns
    if let Ok(re) = Regex::new(r"-\.+|\.-+") {
        release_name = re.replace_all(&release_name, "-").to_string();
    }

    // Remove trailing dots
    if let Ok(re) = Regex::new(r"\.$") {
        release_name = re.replace(&release_name, "").to_string();
    }

    // Remove leading dots
    release_name.trim_start_matches('.').to_string()
}