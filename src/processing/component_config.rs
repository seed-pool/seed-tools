// Component configuration structures for the processing pipeline

use crate::processing::components::screenshots::ScreenshotLayout;

/// Configuration for all upload components
#[derive(Debug, Clone, Default)]
pub struct ComponentConfig {
    pub screenshot: ScreenshotConfig,
    pub mediainfo: MediaInfoConfig,
    pub nfo: NfoConfig,
    pub sample: SampleConfig,
    pub cover_art: CoverArtConfig,
}

/// Screenshot component configuration
#[derive(Debug, Clone)]
pub struct ScreenshotConfig {
    pub enabled: bool,
    pub count: usize,
    pub layout: ScreenshotLayout,
}

impl Default for ScreenshotConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            count: 4,
            layout: ScreenshotLayout::Grid2x2,
        }
    }
}

/// MediaInfo component configuration
#[derive(Debug, Clone)]
pub struct MediaInfoConfig {
    pub enabled: bool,
}

impl Default for MediaInfoConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// NFO component configuration
#[derive(Debug, Clone)]
pub struct NfoConfig {
    pub enabled: bool,
}

impl Default for NfoConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// Sample component configuration
#[derive(Debug, Clone)]
pub struct SampleConfig {
    pub enabled: bool,
    pub duration: u32,           // Duration in seconds
    pub start_time: Option<u32>, // Start time in seconds (None = auto)
}

impl Default for SampleConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            duration: 20,
            start_time: None,
        }
    }
}

/// Cover art component configuration
#[derive(Debug, Clone)]
pub struct CoverArtConfig {
    pub enabled: bool,
}

impl Default for CoverArtConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

impl ComponentConfig {
    /// Create component config from UI state
    pub fn from_ui_state(
        enable_screenshots: bool,
        screenshot_count: usize,
        screenshot_layout: ScreenshotLayout,
        enable_mediainfo: bool,
        enable_nfo: bool,
        enable_sample: bool,
        enable_cover_art: bool,
    ) -> Self {
        Self {
            screenshot: ScreenshotConfig {
                enabled: enable_screenshots,
                count: screenshot_count,
                layout: screenshot_layout,
            },
            mediainfo: MediaInfoConfig {
                enabled: enable_mediainfo,
            },
            nfo: NfoConfig {
                enabled: enable_nfo,
            },
            sample: SampleConfig {
                enabled: enable_sample,
                ..Default::default()
            },
            cover_art: CoverArtConfig {
                enabled: enable_cover_art,
            },
        }
    }
}
