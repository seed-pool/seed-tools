// Logging configuration utilities

use log::LevelFilter;
use simplelog::{Config as SimpleLogConfig, CombinedLogger, WriteLogger, TermLogger, TerminalMode, ColorChoice};
use std::fs::OpenOptions;
use std::path::Path;
use crate::core::error::Result;

/// Setup logging with file and optional console output
pub fn setup_logging(log_file: &str, console_output: bool, log_level: LevelFilter) -> Result<()> {
    let mut loggers: Vec<Box<dyn simplelog::SharedLogger>> = vec![];
    
    // File logger
    let log_path = Path::new(log_file);
    let file_logger = WriteLogger::new(
        log_level,
        SimpleLogConfig::default(),
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)?,
    );
    loggers.push(file_logger);
    
    // Console logger if requested
    if console_output {
        // Try to create terminal logger, fallback if it fails
        let term_logger = TermLogger::new(
            log_level,
            SimpleLogConfig::default(),
            TerminalMode::Mixed,
            ColorChoice::Auto,
        );
        loggers.push(term_logger);
    }
    
    CombinedLogger::init(loggers)
        .map_err(|e| format!("Failed to initialize logging: {}", e))?;
    
    log::info!("Logging initialized at level: {:?}", log_level);
    Ok(())
}

/// Get log level from environment variable or default
pub fn get_log_level() -> LevelFilter {
    match std::env::var("RUST_LOG") {
        Ok(level) => match level.to_lowercase().as_str() {
            "trace" => LevelFilter::Trace,
            "debug" => LevelFilter::Debug,
            "info" => LevelFilter::Info,
            "warn" => LevelFilter::Warn,
            "error" => LevelFilter::Error,
            _ => LevelFilter::Info,
        },
        Err(_) => LevelFilter::Info,
    }
}