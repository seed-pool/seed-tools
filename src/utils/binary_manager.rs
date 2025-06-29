use std::fs;
use std::path::{Path, PathBuf};
use std::io::{self, Write};
use log::{info, debug};
use reqwest;
use crate::processing::extraction::extract_single_archive;

#[derive(Debug, Clone)]
pub struct BinaryInfo {
    pub name: &'static str,
    pub version: &'static str,
    pub download_urls: BinaryUrls,
    pub executable_name: &'static str,
    pub verify_args: &'static [&'static str],
}

#[derive(Debug, Clone)]
pub struct BinaryUrls {
    pub linux_x64: &'static str,
    pub windows_x64: &'static str,
    pub macos_x64: &'static str,
    pub macos_arm64: &'static str,
}

pub struct BinaryManager {
    bin_dir: PathBuf,
    client: reqwest::Client,
}

impl BinaryManager {
    pub fn new(bin_dir: impl Into<PathBuf>) -> Self {
        let bin_dir = bin_dir.into();
        let client = reqwest::Client::builder()
            .user_agent("seedbrr/1.0")
            .timeout(std::time::Duration::from_secs(300)) // 5 minutes timeout
            .build()
            .expect("Failed to create HTTP client");

        Self { bin_dir, client }
    }

    /// Check if all required binaries are present and working
    pub async fn check_binaries(&self) -> Result<Vec<&'static str>, String> {
        let required_binaries = get_required_binaries();
        let mut missing = Vec::new();

        // Ensure bin directory exists
        if let Err(e) = fs::create_dir_all(&self.bin_dir) {
            return Err(format!("Failed to create bin directory: {}", e));
        }

        for binary in &required_binaries {
            let binary_path = self.bin_dir.join(binary.executable_name);
            
            // First check if we have it in our bin directory
            if self.is_binary_working(&binary_path, binary.verify_args).await {
                continue;
            }
            
            // For MediaInfo, check if it's available system-wide first
            if binary.name == "mediainfo" {
                if self.is_system_binary_working("mediainfo", binary.verify_args).await {
                    info!("✅ Found system-wide MediaInfo, creating symlink...");
                    // Create a symlink to the system binary
                    if let Ok(system_path) = which::which("mediainfo") {
                        if let Err(e) = std::os::unix::fs::symlink(system_path, &binary_path) {
                            info!("⚠️ Failed to create symlink, will download: {}", e);
                            missing.push(binary.name);
                        }
                    } else {
                        missing.push(binary.name);
                    }
                    continue;
                }
            }
            
            missing.push(binary.name);
        }

        Ok(missing)
    }

    /// Download and install all missing binaries
    pub async fn install_missing_binaries(&self, missing: &[&str]) -> Result<(), String> {
        let required_binaries = get_required_binaries();
        
        println!("🚀 Setting up {} required binaries for first-time use...", missing.len());
        println!("   This may take a few minutes depending on your internet connection.");
        println!();
        
        for (i, &missing_name) in missing.iter().enumerate() {
            if let Some(binary) = required_binaries.iter().find(|b| b.name == missing_name) {
                println!("📦 [{}/{}] Installing {}...", i + 1, missing.len(), binary.name);
                info!("📥 Downloading {}...", binary.name);
                self.download_and_install_binary(binary).await?;
                println!("✅ {} installed successfully", binary.name);
                info!("✅ {} installed successfully", binary.name);
            }
        }

        println!();
        println!("🎉 All required binaries installed successfully!");
        Ok(())
    }

    /// Download and install a specific binary
    async fn download_and_install_binary(&self, binary: &BinaryInfo) -> Result<(), String> {
        let download_url = self.get_platform_url(binary)?;
        
        // Determine archive extension from URL
        let archive_extension = if download_url.ends_with(".tar.xz") {
            "tar.xz"
        } else if download_url.ends_with(".tar.gz") {
            "tar.gz"
        } else if download_url.ends_with(".tar.bz2") {
            "tar.bz2"
        } else if download_url.ends_with(".zip") {
            "zip"
        } else if download_url.ends_with(".deb") {
            "deb"
        } else {
            "archive"
        };
        
        let archive_path = self.bin_dir.join(format!("{}.{}", binary.name, archive_extension));
        let final_path = self.bin_dir.join(binary.executable_name);

        info!("🌐 Downloading {} from {}", binary.name, download_url);
        
        // Download with progress
        let response = self.client.get(download_url)
            .send()
            .await
            .map_err(|e| format!("Failed to download {}: {}", binary.name, e))?;

        if !response.status().is_success() {
            return Err(format!("Failed to download {}: HTTP {}", binary.name, response.status()));
        }

        let total_size = response.content_length().unwrap_or(0);
        let mut downloaded = 0u64;
        let mut file = fs::File::create(&archive_path)
            .map_err(|e| format!("Failed to create temp file: {}", e))?;

        let mut stream = response.bytes_stream();
        use futures_util::StreamExt;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| format!("Failed to read chunk: {}", e))?;
            downloaded += chunk.len() as u64;
            
            file.write_all(&chunk)
                .map_err(|e| format!("Failed to write chunk: {}", e))?;

            if total_size > 0 && downloaded % 5242880 == 0 { // Every 5MB
                let progress = (downloaded as f64 / total_size as f64) * 100.0;
                print!("   📊 Downloading: {:.0}% ({}/{})\r", progress, format_bytes(downloaded), format_bytes(total_size));
                io::stdout().flush().ok();
            }
        }
        drop(file); // Ensure file is closed before extraction
        
        // Clear the progress line
        print!("                                                                \r");
        io::stdout().flush().ok();

        info!("📦 Extracting {} archive...", binary.name);
        
        // Extract the archive using existing extraction system
        let temp_extract_dir = self.bin_dir.join(format!("{}_extract", binary.name));
        match extract_single_archive(&archive_path, &temp_extract_dir) {
            Ok(_) => {
                info!("✅ Archive extraction successful");
                // Find and copy the binary from extracted files
                self.find_and_copy_binary(&temp_extract_dir, binary, &final_path).await?;
                // Clean up extraction directory
                fs::remove_dir_all(&temp_extract_dir).ok();
            }
            Err(e) => {
                info!("❌ Archive extraction failed: {}", e);
                return Err(format!("Failed to extract archive: {}", e));
            }
        }
        
        // Clean up archive
        fs::remove_file(&archive_path).ok();

        // Make executable on Unix systems
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&final_path, fs::Permissions::from_mode(0o755))
                .map_err(|e| format!("Failed to set executable permissions: {}", e))?;
        }

        // Verify the binary works
        if !self.is_binary_working(&final_path, binary.verify_args).await {
            fs::remove_file(&final_path).ok();
            return Err(format!("Downloaded {} binary is not working correctly", binary.name));
        }

        Ok(())
    }
    
    /// Recursively find the binary executable in extracted directory
    async fn find_and_copy_binary(
        &self,
        search_dir: &Path,
        binary: &BinaryInfo,
        final_path: &Path,
    ) -> Result<(), String> {
        let target_name = binary.executable_name;
        info!("🔍 Searching for binary '{}' in: {:?}", target_name, search_dir);
        
        // For MediaInfo source distribution, build it with dependencies
        if binary.name == "mediainfo" {
            // Look for the source directory structure
            let source_dirs = [
                search_dir.join("MediaInfo_CLI_GNU_FromSource"),
                search_dir.join("MediaInfo_CLI_25.04_GNU_FromSource"),
            ];
            
            for source_dir in &source_dirs {
                if source_dir.exists() {
                    info!("🔨 Found MediaInfo source directory, building with dependencies...");
                    return self.build_mediainfo_with_dependencies(source_dir, final_path).await;
                }
            }
        }
        
        
        // Walk the directory tree looking for the binary
        for entry in walkdir::WalkDir::new(search_dir) {
            let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
            let path = entry.path();
            
            if let Some(filename) = path.file_name() {
                let filename_str = filename.to_string_lossy();
                info!("  Checking file: {}", filename_str);
                
                if filename == target_name {
                    info!("✅ Found binary at: {:?}", path);
                    // Found the binary, copy it - handle both regular files and symlinks
                    if path.is_file() {
                        fs::copy(path, final_path)
                            .map_err(|e| format!("Failed to copy binary from archive: {}", e))?;
                    } else if path.is_symlink() {
                        // For symlinks, try to resolve and copy the target
                        let target = fs::read_link(path)
                            .map_err(|e| format!("Failed to read symlink: {}", e))?;
                        let absolute_target = if target.is_relative() {
                            path.parent().unwrap().join(target)
                        } else {
                            target
                        };
                        if absolute_target.is_file() {
                            fs::copy(&absolute_target, final_path)
                                .map_err(|e| format!("Failed to copy symlink target: {}", e))?;
                        } else {
                            return Err(format!("Symlink target does not exist or is not a file: {:?}", absolute_target));
                        }
                    } else {
                        return Err(format!("Found binary is neither a regular file nor a symlink: {:?}", path));
                    }
                    info!("📋 Copied binary to: {:?}", final_path);
                    return Ok(());
                }
            }
        }
        
        // List all files found for debugging
        info!("❌ Binary '{}' not found. Files found in archive:", target_name);
        for entry in walkdir::WalkDir::new(search_dir).max_depth(3) {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.is_file() {
                    info!("  📄 {}", path.strip_prefix(search_dir).unwrap_or(path).display());
                }
            }
        }
        
        Err(format!("Binary '{}' not found in extracted archive", target_name))
    }

    /// Build MediaInfo with all required dependencies (libzen, libmediainfo)
    async fn build_mediainfo_with_dependencies(
        &self,
        source_root: &Path,
        final_path: &Path,
    ) -> Result<(), String> {
        info!("🔨 Building MediaInfo with dependencies from: {:?}", source_root);
        
        // Create a build directory for our dependencies
        let build_dir = source_root.join("build_deps");
        if let Err(e) = fs::create_dir_all(&build_dir) {
            return Err(format!("Failed to create build directory: {}", e));
        }
        
        // Download and build libzen
        info!("📦 Step 1/3: Building libzen...");
        let libzen_prefix = build_dir.join("libzen_install");
        self.download_and_build_libzen(&build_dir, &libzen_prefix).await?;
        
        // Download and build libmediainfo  
        info!("📦 Step 2/3: Building libmediainfo...");
        let libmediainfo_prefix = build_dir.join("libmediainfo_install");
        self.download_and_build_libmediainfo(&build_dir, &libzen_prefix, &libmediainfo_prefix).await?;
        
        // Build MediaInfo CLI
        info!("📦 Step 3/3: Building MediaInfo CLI...");
        let cli_dir = source_root.join("MediaInfo/Project/GNU/CLI");
        if !cli_dir.exists() {
            return Err("MediaInfo CLI source directory not found".to_string());
        }
        
        // Set environment variables for the build
        let pkg_config_path = format!(
            "{}:{}",
            libzen_prefix.join("lib/pkgconfig").display(),
            libmediainfo_prefix.join("lib/pkgconfig").display()
        );
        
        // Run autogen.sh if it exists
        let autogen_path = cli_dir.join("autogen.sh");
        if autogen_path.exists() {
            info!("📋 Running autogen.sh...");
            let output = tokio::process::Command::new("bash")
                .arg("autogen.sh")
                .current_dir(&cli_dir)
                .env("PKG_CONFIG_PATH", &pkg_config_path)
                .output()
                .await
                .map_err(|e| format!("Failed to run autogen.sh: {}", e))?;
            
            if !output.status.success() {
                return Err(format!("autogen.sh failed: {}", String::from_utf8_lossy(&output.stderr)));
            }
        }
        
        // Configure the build
        info!("📋 Configuring MediaInfo build...");
        let output = tokio::process::Command::new("./configure")
            .arg("--enable-static")
            .arg("--disable-shared")
            .current_dir(&cli_dir)
            .env("PKG_CONFIG_PATH", &pkg_config_path)
            .env("CPPFLAGS", format!("-I{}/include", libzen_prefix.display()))
            .env("LDFLAGS", format!("-L{}/lib -L{}/lib", libzen_prefix.display(), libmediainfo_prefix.display()))
            .output()
            .await
            .map_err(|e| format!("Failed to run configure: {}", e))?;
        
        if !output.status.success() {
            return Err(format!("Configure failed: {}", String::from_utf8_lossy(&output.stderr)));
        }
        
        // Build the project
        info!("🔨 Building MediaInfo CLI (this may take a few minutes)...");
        let output = tokio::process::Command::new("make")
            .arg("-j4") // Use 4 parallel jobs
            .current_dir(&cli_dir)
            .env("PKG_CONFIG_PATH", &pkg_config_path)
            .output()
            .await
            .map_err(|e| format!("Failed to run make: {}", e))?;
        
        if !output.status.success() {
            return Err(format!("Build failed: {}", String::from_utf8_lossy(&output.stderr)));
        }
        
        // Copy the built binary
        let built_binary = cli_dir.join("mediainfo");
        if built_binary.exists() {
            info!("✅ Build successful, copying binary...");
            fs::copy(&built_binary, final_path)
                .map_err(|e| format!("Failed to copy built binary: {}", e))?;
            info!("📋 Copied built binary to: {:?}", final_path);
            
            // Clean up build directory
            fs::remove_dir_all(&build_dir).ok();
            
            Ok(())
        } else {
            Err("Built binary not found after successful build".to_string())
        }
    }
    
    /// Download and build libzen
    async fn download_and_build_libzen(
        &self,
        build_dir: &Path,
        install_prefix: &Path,
    ) -> Result<(), String> {
        let libzen_url = "https://mediaarea.net/download/binary/libzen0/0.4.41/libzen_0.4.41_GNU_FromSource.tar.xz";
        let libzen_archive = build_dir.join("libzen.tar.xz");
        
        // Download libzen
        info!("📥 Downloading libzen...");
        let response = self.client.get(libzen_url)
            .send()
            .await
            .map_err(|e| format!("Failed to download libzen: {}", e))?;
            
        if !response.status().is_success() {
            return Err(format!("Failed to download libzen: HTTP {}", response.status()));
        }
        
        let bytes = response.bytes().await
            .map_err(|e| format!("Failed to read libzen bytes: {}", e))?;
        fs::write(&libzen_archive, &bytes)
            .map_err(|e| format!("Failed to write libzen archive: {}", e))?;
        
        // Extract libzen
        let libzen_extract_dir = build_dir.join("libzen_extract");
        crate::processing::extraction::extract_single_archive(&libzen_archive, &libzen_extract_dir)
            .map_err(|e| format!("Failed to extract libzen: {}", e))?;
        
        // Find libzen source directory
        let libzen_source = libzen_extract_dir.join("ZenLib_GNU_FromSource/Project/GNU/Library");
        if !libzen_source.exists() {
            return Err("libzen source directory not found".to_string());
        }
        
        // Build libzen
        info!("🔨 Building libzen...");
        
        // Run autogen.sh
        let autogen_path = libzen_source.join("autogen.sh");
        if autogen_path.exists() {
            let output = tokio::process::Command::new("bash")
                .arg("autogen.sh")
                .current_dir(&libzen_source)
                .output()
                .await
                .map_err(|e| format!("Failed to run libzen autogen.sh: {}", e))?;
                
            if !output.status.success() {
                return Err(format!("libzen autogen.sh failed: {}", String::from_utf8_lossy(&output.stderr)));
            }
        }
        
        // Configure
        let output = tokio::process::Command::new("./configure")
            .arg(&format!("--prefix={}", install_prefix.display()))
            .arg("--enable-static")
            .arg("--disable-shared")
            .current_dir(&libzen_source)
            .output()
            .await
            .map_err(|e| format!("Failed to configure libzen: {}", e))?;
            
        if !output.status.success() {
            return Err(format!("libzen configure failed: {}", String::from_utf8_lossy(&output.stderr)));
        }
        
        // Make and install
        let output = tokio::process::Command::new("make")
            .arg("-j4")
            .current_dir(&libzen_source)
            .output()
            .await
            .map_err(|e| format!("Failed to build libzen: {}", e))?;
            
        if !output.status.success() {
            return Err(format!("libzen build failed: {}", String::from_utf8_lossy(&output.stderr)));
        }
        
        let output = tokio::process::Command::new("make")
            .arg("install")
            .current_dir(&libzen_source)
            .output()
            .await
            .map_err(|e| format!("Failed to install libzen: {}", e))?;
            
        if !output.status.success() {
            return Err(format!("libzen install failed: {}", String::from_utf8_lossy(&output.stderr)));
        }
        
        info!("✅ libzen built and installed successfully");
        Ok(())
    }
    
    /// Download and build libmediainfo
    async fn download_and_build_libmediainfo(
        &self,
        build_dir: &Path,
        libzen_prefix: &Path,
        install_prefix: &Path,
    ) -> Result<(), String> {
        let libmediainfo_url = "https://mediaarea.net/download/binary/libmediainfo0/25.04/libmediainfo_25.04_GNU_FromSource.tar.xz";
        let libmediainfo_archive = build_dir.join("libmediainfo.tar.xz");
        
        // Download libmediainfo
        info!("📥 Downloading libmediainfo...");
        let response = self.client.get(libmediainfo_url)
            .send()
            .await
            .map_err(|e| format!("Failed to download libmediainfo: {}", e))?;
            
        if !response.status().is_success() {
            return Err(format!("Failed to download libmediainfo: HTTP {}", response.status()));
        }
        
        let bytes = response.bytes().await
            .map_err(|e| format!("Failed to read libmediainfo bytes: {}", e))?;
        fs::write(&libmediainfo_archive, &bytes)
            .map_err(|e| format!("Failed to write libmediainfo archive: {}", e))?;
        
        // Extract libmediainfo
        let libmediainfo_extract_dir = build_dir.join("libmediainfo_extract");
        crate::processing::extraction::extract_single_archive(&libmediainfo_archive, &libmediainfo_extract_dir)
            .map_err(|e| format!("Failed to extract libmediainfo: {}", e))?;
        
        // Find libmediainfo source directory
        let libmediainfo_source = libmediainfo_extract_dir.join("MediaInfoLib_GNU_FromSource/Project/GNU/Library");
        if !libmediainfo_source.exists() {
            return Err("libmediainfo source directory not found".to_string());
        }
        
        // Build libmediainfo
        info!("🔨 Building libmediainfo...");
        
        let pkg_config_path = libzen_prefix.join("lib/pkgconfig");
        
        // Run autogen.sh
        let autogen_path = libmediainfo_source.join("autogen.sh");
        if autogen_path.exists() {
            let output = tokio::process::Command::new("bash")
                .arg("autogen.sh")
                .current_dir(&libmediainfo_source)
                .env("PKG_CONFIG_PATH", pkg_config_path.display().to_string())
                .output()
                .await
                .map_err(|e| format!("Failed to run libmediainfo autogen.sh: {}", e))?;
                
            if !output.status.success() {
                return Err(format!("libmediainfo autogen.sh failed: {}", String::from_utf8_lossy(&output.stderr)));
            }
        }
        
        // Configure
        let output = tokio::process::Command::new("./configure")
            .arg(&format!("--prefix={}", install_prefix.display()))
            .arg("--enable-static")
            .arg("--disable-shared")
            .current_dir(&libmediainfo_source)
            .env("PKG_CONFIG_PATH", pkg_config_path.display().to_string())
            .env("CPPFLAGS", format!("-I{}/include", libzen_prefix.display()))
            .env("LDFLAGS", format!("-L{}/lib", libzen_prefix.display()))
            .output()
            .await
            .map_err(|e| format!("Failed to configure libmediainfo: {}", e))?;
            
        if !output.status.success() {
            return Err(format!("libmediainfo configure failed: {}", String::from_utf8_lossy(&output.stderr)));
        }
        
        // Make and install
        let output = tokio::process::Command::new("make")
            .arg("-j4")
            .current_dir(&libmediainfo_source)
            .env("PKG_CONFIG_PATH", pkg_config_path.display().to_string())
            .output()
            .await
            .map_err(|e| format!("Failed to build libmediainfo: {}", e))?;
            
        if !output.status.success() {
            return Err(format!("libmediainfo build failed: {}", String::from_utf8_lossy(&output.stderr)));
        }
        
        let output = tokio::process::Command::new("make")
            .arg("install")
            .current_dir(&libmediainfo_source)
            .env("PKG_CONFIG_PATH", pkg_config_path.display().to_string())
            .output()
            .await
            .map_err(|e| format!("Failed to install libmediainfo: {}", e))?;
            
        if !output.status.success() {
            return Err(format!("libmediainfo install failed: {}", String::from_utf8_lossy(&output.stderr)));
        }
        
        info!("✅ libmediainfo built and installed successfully");
        Ok(())
    }

    /// Check if a system binary exists and is working
    async fn is_system_binary_working(&self, binary_name: &str, verify_args: &[&str]) -> bool {
        info!("🔍 Checking for system-wide {}", binary_name);
        
        // Try to run the binary with verification arguments
        match tokio::process::Command::new(binary_name)
            .args(verify_args)
            .output()
            .await
        {
            Ok(output) => {
                let success = output.status.success();
                if success {
                    info!("✅ System binary test successful: {}", binary_name);
                } else {
                    info!("❌ System binary test failed: {}", binary_name);
                }
                success
            },
            Err(e) => {
                info!("❌ Failed to execute system binary {}: {}", binary_name, e);
                false
            }
        }
    }

    /// Check if a binary exists and is working
    async fn is_binary_working(&self, path: &Path, verify_args: &[&str]) -> bool {
        if !path.exists() {
            info!("❌ Binary not found at: {:?}", path);
            return false;
        }

        info!("🧪 Testing binary: {:?} with args: {:?}", path, verify_args);
        
        // Try to run the binary with verification arguments
        match tokio::process::Command::new(path)
            .args(verify_args)
            .output()
            .await
        {
            Ok(output) => {
                let success = output.status.success();
                if success {
                    info!("✅ Binary test successful: {:?}", path);
                } else {
                    info!("❌ Binary test failed: {:?}", path);
                    info!("  stdout: {}", String::from_utf8_lossy(&output.stdout));
                    info!("  stderr: {}", String::from_utf8_lossy(&output.stderr));
                }
                success
            },
            Err(e) => {
                info!("❌ Failed to execute binary {:?}: {}", path, e);
                false
            }
        }
    }

    /// Get the appropriate download URL for the current platform
    fn get_platform_url(&self, binary: &BinaryInfo) -> Result<&str, String> {
        let os = std::env::consts::OS;
        let arch = std::env::consts::ARCH;

        match (os, arch) {
            ("linux", "x86_64") => Ok(binary.download_urls.linux_x64),
            ("windows", "x86_64") => Ok(binary.download_urls.windows_x64),
            ("macos", "x86_64") => Ok(binary.download_urls.macos_x64),
            ("macos", "aarch64") => Ok(binary.download_urls.macos_arm64),
            _ => Err(format!("Unsupported platform: {} {}", os, arch)),
        }
    }
}

/// Get list of all required binaries with download information
fn get_required_binaries() -> Vec<BinaryInfo> {
    vec![
        BinaryInfo {
            name: "ffmpeg",
            version: "6.1",
            download_urls: BinaryUrls {
                linux_x64: "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-linux64-gpl.tar.xz",
                windows_x64: "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip",
                macos_x64: "https://evermeet.cx/ffmpeg/ffmpeg-116899-gc3427b6c9a.zip",
                macos_arm64: "https://evermeet.cx/ffmpeg/ffmpeg-116899-gc3427b6c9a.zip",
            },
            executable_name: if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" },
            verify_args: &["-version"],
        },
        BinaryInfo {
            name: "ffprobe",
            version: "6.1",
            download_urls: BinaryUrls {
                linux_x64: "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-linux64-gpl.tar.xz",
                windows_x64: "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip",
                macos_x64: "https://evermeet.cx/ffmpeg/ffprobe-116899-gc3427b6c9a.zip",
                macos_arm64: "https://evermeet.cx/ffmpeg/ffprobe-116899-gc3427b6c9a.zip",
            },
            executable_name: if cfg!(windows) { "ffprobe.exe" } else { "ffprobe" },
            verify_args: &["-version"],
        },
        BinaryInfo {
            name: "mediainfo",
            version: "25.04",
            download_urls: BinaryUrls {
                linux_x64: "https://mediaarea.net/download/binary/mediainfo/25.04/MediaInfo_CLI_25.04_GNU_FromSource.tar.xz",
                windows_x64: "https://mediaarea.net/download/binary/mediainfo/25.04/MediaInfo_CLI_25.04_Windows_x64.zip",
                macos_x64: "https://mediaarea.net/download/binary/mediainfo/25.04/MediaInfo_CLI_25.04_Mac.dmg",
                macos_arm64: "https://mediaarea.net/download/binary/mediainfo/25.04/MediaInfo_CLI_25.04_Mac.dmg",
            },
            executable_name: if cfg!(windows) { "mediainfo.exe" } else { "mediainfo" },
            verify_args: &["--version"],
        },
        BinaryInfo {
            name: "mkbrr",
            version: "latest",
            download_urls: BinaryUrls {
                linux_x64: "https://github.com/autobrr/mkbrr/releases/download/v1.13.0/mkbrr_1.13.0_linux_x86_64.tar.gz",
                windows_x64: "https://github.com/autobrr/mkbrr/releases/latest/download/mkbrr_windows_x86_64.zip",
                macos_x64: "https://github.com/autobrr/mkbrr/releases/latest/download/mkbrr_darwin_x86_64.tar.gz",
                macos_arm64: "https://github.com/autobrr/mkbrr/releases/latest/download/mkbrr_darwin_arm64.tar.gz",
            },
            executable_name: if cfg!(windows) { "mkbrr.exe" } else { "mkbrr" },
            verify_args: &["version"],
        },
    ]
}

/// Format bytes in a human-readable format
fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    format!("{:.1} {}", size, UNITS[unit_index])
}

/// Check if this is the first run (no binaries exist)
pub async fn is_first_run(bin_dir: &Path) -> bool {
    if !bin_dir.exists() {
        return true;
    }

    let required_binaries = get_required_binaries();
    for binary in &required_binaries {
        let binary_path = bin_dir.join(binary.executable_name);
        if binary_path.exists() {
            return false; // At least one binary exists
        }
    }

    true // No binaries found
}

/// Run first-time setup if needed
pub async fn setup_binaries_if_needed(bin_dir: &Path) -> Result<(), String> {
    let manager = BinaryManager::new(bin_dir);
    
    info!("🔍 Checking required binaries...");
    let missing = manager.check_binaries().await?;
    
    if missing.is_empty() {
        info!("✅ All required binaries are available");
        return Ok(());
    }

    info!("📦 Missing binaries: {}", missing.join(", "));
    
    manager.install_missing_binaries(&missing).await?;
    
    println!("🎯 Setup complete! seedbrr is ready to use.");
    println!();
    info!("🎉 Binary setup complete! All required tools are now available.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_binary_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let manager = BinaryManager::new(temp_dir.path());
        
        // Should create bin directory
        assert!(temp_dir.path().exists());
    }

    #[tokio::test]
    async fn test_format_bytes() {
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1048576), "1.0 MB");
        assert_eq!(format_bytes(1073741824), "1.0 GB");
    }
}