use std::fs;
use std::path::Path;
use std::process::Command;
use log::{info, error, debug};
use rand::Rng;

use crate::types::{PathsConfig, VideoSettings};
use crate::naming::generate_release_name;

pub fn find_video_files<T>(
    input_path: &str,
    _paths: &PathsConfig,
    settings: &T,
) -> Result<(Vec<String>, Option<String>), String>
where
    T: VideoSettings,
{
    let supported_extensions = ["mkv", "mp4", "ts", "avi", "mov", "flv", "wmv"];
    let path = Path::new(input_path);

    let mut video_files = Vec::new();
    let mut nfo_file: Option<String> = None;

    let exclusions_enabled = settings.stripshit_from_videos();
    info!("Exclusions enabled: {}", exclusions_enabled);

    fn process_path(
        file_path: &Path,
        video_files: &mut Vec<String>,
        nfo_file: &mut Option<String>,
        supported_extensions: &[&str],
        exclusions_enabled: bool,
    ) -> Result<(), String> {
        if file_path.is_dir() {
            for entry in fs::read_dir(file_path).map_err(|e| format!("Failed to read directory: {}", e))? {
                let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
                let entry_path = entry.path();
                process_path(&entry_path, video_files, nfo_file, supported_extensions, exclusions_enabled)?;
            }
        } else {
            debug!("Processing file: {}", file_path.display());
            process_file(file_path, video_files, nfo_file, supported_extensions, exclusions_enabled)?;
        }
        Ok(())
    }

    process_path(path, &mut video_files, &mut nfo_file, &supported_extensions, exclusions_enabled)?;

    if video_files.is_empty() {
        error!("No valid video files detected after exclusions.");
        return Err("No valid video files detected.".to_string());
    }

    info!("Final NFO file: {:?}", nfo_file);

    Ok((video_files, nfo_file))
}

pub fn generate_mediainfo(video_file: &str, mediainfo_path: &str) -> Result<String, String> {
    let output = Command::new(mediainfo_path)
        .args(&["--Output=TEXT", video_file])
        .output()
        .map_err(|e| format!("Failed to run mediainfo: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "Mediainfo command failed with status: {}",
            output.status
        ));
    }

    let mut result = String::from_utf8(output.stdout)
        .map_err(|e| format!("Failed to parse mediainfo output: {}", e))?;

    // Sanitize the "Complete name" field
    if let Some(start) = result.find("Complete name") {
        if let Some(end) = result[start..].find('\n') {
            let full_line = &result[start..start + end];
            if let Some(_separator) = full_line.find(':') {
                let sanitized_line = format!(
                    "Complete name                            : {}",
                    Path::new(video_file).file_name().unwrap_or_default().to_string_lossy()
                );
                result = result.replace(full_line, &sanitized_line);
            }
        }
    }

    Ok(result)
}

pub fn process_file(
    file_path: &Path,
    video_files: &mut Vec<String>,
    nfo_file: &mut Option<String>,
    supported_extensions: &[&str],
    exclusions_enabled: bool,
) -> Result<(), String> {
    let file_name = file_path.file_name().unwrap_or_default().to_string_lossy().to_string();

    if let Some(ext) = file_path.extension() {
        let ext = ext.to_string_lossy().to_lowercase();
        if supported_extensions.contains(&ext.as_str()) {
            video_files.push(file_path.to_string_lossy().to_string());
        } else if ext == "nfo" && nfo_file.is_none() {
            *nfo_file = Some(file_path.to_string_lossy().to_string());
        }
    } else if exclusions_enabled && contains_excluded_keywords(&file_name) {
        info!("Excluding file due to keywords: {}", file_name);
    }

    Ok(())
}

pub fn contains_excluded_keywords(name: &str) -> bool {
    let keywords = ["sample", "screens", "screenshots", "proof"];
    let lowercase_name = name.to_lowercase();
    let result = keywords.iter().any(|keyword| lowercase_name.contains(keyword));
    info!("Checking if '{}' contains excluded keywords: {}", name, result);
    result
}

pub fn generate_sample(
    video_file: &str,
    screenshots_dir: &str,
    remote_path: &str,
    image_path: &str,
    ffmpeg_path: &str,
    input_name: &str,
    dry_run: bool,
) -> Result<String, String> {
    let sanitized_input_name = generate_release_name(input_name);
    let sample_file = format!("{}/{}.sample.mkv", screenshots_dir, sanitized_input_name);

    // Generate the sample file
    let ffmpeg_command = format!(
        "{} -i \"{}\" -ss 00:05:00 -t 00:00:20 -map 0 -c copy \"{}\"",
        ffmpeg_path, video_file, sample_file
    );
    let output = Command::new("sh")
        .arg("-c")
        .arg(ffmpeg_command)
        .output()
        .map_err(|e| format!("Failed to execute ffmpeg: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "Failed to generate sample file. ffmpeg output: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    // Set permissions to 777
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&sample_file, fs::Permissions::from_mode(0o777))
            .map_err(|e| format!("Failed to set permissions for sample file '{}': {}", sample_file, e))?;
    }

    // Upload the sample file
    if !dry_run {
        crate::utils::upload_to_cdn(
            &sample_file,
            &format!("{}/previews/", remote_path.trim_end_matches('/'))
        )?;
        info!("Sample file uploaded to CDN.");
    } else {
        info!("[DRY RUN] Would upload sample to CDN: {} {}", &format!("{}/previews/", remote_path.trim_end_matches('/')), sanitized_input_name);
    }

    // Return the public-facing URL for the sample
    Ok(format!("{}/{}.sample.mkv", image_path, sanitized_input_name))
}

pub fn get_video_duration(video_file: &str, ffprobe_path: &str) -> Result<f64, String> {
    let ffprobe_output = Command::new(ffprobe_path)
        .args(&[
            "-v", "error",
            "-show_entries", "format=duration",
            "-of", "default=noprint_wrappers=1:nokey=1",
            video_file,
        ])
        .output()
        .map_err(|e| format!("Failed to run ffprobe: {}", e))?;

    if !ffprobe_output.status.success() {
        return Err(format!(
            "ffprobe failed with status: {}. Stderr: {}",
            ffprobe_output.status,
            String::from_utf8_lossy(&ffprobe_output.stderr)
        ));
    }

    let duration_str = String::from_utf8_lossy(&ffprobe_output.stdout).trim().to_string();
    duration_str.parse::<f64>().map_err(|_| "Failed to parse video duration.".to_string())
}

pub fn default_non_video_description() -> String {
    format!(
        "[b][size=12][color=#757575]Created with mkbrr, ffmpeg, and mediainfo. Posted to this fine tracker with seed-tools.[/color][/size][/b]
        
        [url=https://github.com/seed-pool/seed-tools][img]https://cdn.seedpool.org/sp.png[/img][/url]  \
        [url=https://github.com/autobrr/mkbrr][img]https://cdn.seedpool.org/mkbrr.png[/img][/url]  \
        [url=https://www.rust-lang.org][img]https://cdn.seedpool.org/rust.png[/img][/url]"
    )
}

pub fn generate_description(
    screenshots: &[String],
    _thumbnails: &[String],
    sample_url: &str,
    _datestamp: &str,
    custom_description: Option<&str>,
    youtube_trailer_url: Option<&str>,
    _base_url: &str,
    _release_name: &str,
) -> String {
    let mut description = String::new();

    // Add screenshots in a 2x2 table pattern
    if !screenshots.is_empty() {
        description.push_str("[center][tr]\n");

        for (i, screenshot) in screenshots.iter().enumerate() {
            description.push_str(&format!(
                "        [td][url={}][img width=720]{}[/img][/url][/td]\n",
                screenshot, screenshot
            ));

            // Add a new row every 2 images
            if (i + 1) % 2 == 0 {
                description.push_str("    [/tr]\n    [tr]\n");
            }
        }

        // Close the last row properly
        if screenshots.len() % 2 != 0 {
            description.push_str("    [/center][/tr]\n");
        }
    }

    // Add a blank line after screenshots
    description.push_str("\n");

    // Add sample link if available
    if !sample_url.is_empty() {
        description.push_str(&format!(
            "[b][spoiler=Sample: {}]{}[/spoiler][/b]\n\n",
            Path::new(sample_url).file_name().unwrap_or_default().to_string_lossy(),
            sample_url
        ));
    }

    // Add YouTube trailer link if available
    if let Some(trailer_url) = youtube_trailer_url {
        description.push_str(&format!(
            "[center][b][url={}][Trailer on YouTube][/url][/b][/center]\n\n",
            trailer_url
        ));
    }

    // Add custom description (not centered)
    if let Some(custom_desc) = custom_description {
        description.push_str(custom_desc);
        description.push_str("\n\n");
    }

    // Append the default non-video description
    description.push_str(&default_non_video_description());

    description
}

pub fn generate_screenshots(
    video_file: &str,
    output_dir: &str,
    ffmpeg_path: &str,
    ffprobe_path: &str,
    remote_path: &str,
    image_path: &str,
    input_name: &str,
    dry_run: bool,
) -> Result<(Vec<String>, Vec<String>), String> {
    let mut screenshots_list = Vec::new();
    let mut thumbnails_list = Vec::new();

    // Ensure the output directory exists
    fs::create_dir_all(output_dir).map_err(|e| format!("Failed to create output directory: {}", e))?;

    let sanitized_input_name = generate_release_name(input_name); // Sanitize the input name
    let duration = get_video_duration(video_file, ffprobe_path)?;
    let timestamps = generate_random_timestamps(duration, 4);

    for (i, shot_time) in timestamps.iter().enumerate() {
        // Generate sanitized filenames for screenshots and thumbnails
        let screenshot_file = format!("{}/{}_{}.jpg", output_dir, sanitized_input_name, i + 1);
        let thumbnail_file = format!("{}/{}_{}_thumb.jpg", output_dir, sanitized_input_name, i + 1);

        // Generate screenshot
        generate_screenshot(video_file, ffmpeg_path, shot_time, &screenshot_file)?;
        generate_thumbnail(ffmpeg_path, &screenshot_file, &thumbnail_file)?;

        // Set permissions to 777 for the screenshot and thumbnail locally
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&screenshot_file, fs::Permissions::from_mode(0o777))
                .map_err(|e| format!("Failed to set permissions for {}: {}", screenshot_file, e))?;
            fs::set_permissions(&thumbnail_file, fs::Permissions::from_mode(0o777))
                .map_err(|e| format!("Failed to set permissions for {}: {}", thumbnail_file, e))?;
        }

        // Upload files to the CDN
        if !dry_run {
            crate::utils::upload_to_cdn(&screenshot_file, &format!("{}/screenshots/", remote_path.trim_end_matches('/')))?;
            crate::utils::upload_to_cdn(&thumbnail_file, &format!("{}/screenshots/", remote_path.trim_end_matches('/')))?;
        } else {
            
            info!("[DRY RUN] Skipping screenshot/thumbnail upload to CDN: {} {}", &format!("{}/screenshots/", remote_path.trim_end_matches('/')), screenshot_file);
        }

        // Add public-facing URLs to the lists
        screenshots_list.push(format!("{}/{}", image_path, Path::new(&screenshot_file).file_name().unwrap().to_string_lossy()));
        thumbnails_list.push(format!("{}/{}", image_path, Path::new(&thumbnail_file).file_name().unwrap().to_string_lossy()));
    }

    Ok((screenshots_list, thumbnails_list))
}

fn generate_random_timestamps(duration: f64, count: usize) -> Vec<u32> {
    let start_time = (duration * 0.15) as u32;
    let end_time = (duration * 0.85) as u32;

    let mut rng = rand::thread_rng();
    let mut timestamps: Vec<u32> = (0..count).map(|_| rng.gen_range(start_time..end_time)).collect();
    timestamps.sort();
    timestamps
}

fn generate_screenshot(video_file: &str, ffmpeg_path: &str, timestamp: &u32, output_file: &str) -> Result<(), String> {
    Command::new(ffmpeg_path)
        .args(&[
            "-y", "-loglevel", "error", "-ss", &timestamp.to_string(),
            "-i", video_file, "-vframes", "1", "-qscale:v", "2", output_file,
        ])
        .status()
        .map_err(|e| format!("Failed to run ffmpeg for screenshot: {}", e))?;
    Ok(())
}

fn generate_thumbnail(ffmpeg_path: &str, input_file: &str, output_file: &str) -> Result<(), String> {
    Command::new(ffmpeg_path)
        .args(&[
            "-y", "-loglevel", "error", "-i", input_file,
            "-vf", "scale=720:-1", output_file,
        ])
        .status()
        .map_err(|e| format!("Failed to run ffmpeg for thumbnail: {}", e))?;
    Ok(())
}


pub fn generate_screenshots_imgbb(
    video_file: &str,
    ffmpeg_path: &Path,
    ffprobe_path: &Path,
    imgbb_api_key: &str,
    dry_run: bool,
) -> Result<(Vec<String>, Vec<String>), String> {
    let mut screenshots = Vec::new();
    let mut thumbnails = Vec::new();

    // Get video duration
    let duration = get_video_duration(video_file, ffprobe_path.to_str().unwrap())?;
    let timestamps = generate_random_timestamps(duration, 4);

    // Generate sanitized base name for screenshots
    let base_name = Path::new(video_file)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let sanitized_base_name = generate_release_name(&base_name);

    for (i, timestamp) in timestamps.iter().enumerate() {
        // Generate screenshot file name
        let screenshot_name = format!("{}_{}.jpg", sanitized_base_name, i + 1);
        let screenshot_path = format!("/tmp/{}", screenshot_name);

        // Generate screenshot
        generate_screenshot(video_file, ffmpeg_path.to_str().unwrap(), timestamp, &screenshot_path)?;

        // Upload screenshot to ImgBB
        let (full_image_url, thumb_url) = crate::utils::upload_to_imgbb(&screenshot_path, imgbb_api_key, dry_run)?;
        screenshots.push(full_image_url); // Use full_image_url for the description
        thumbnails.push(thumb_url);

        // Clean up the local screenshot file
        fs::remove_file(&screenshot_path).map_err(|e| format!("Failed to delete temporary screenshot: {}", e))?;
    }

    Ok((screenshots, thumbnails))
}