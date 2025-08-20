// Command-line interface and argument parsing

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "Automated tool for processing and uploading releases to trackers.",
    long_about = None
)]
pub struct Cli {
    /// Enable sync mode
    #[arg(long, conflicts_with_all = ["sp", "tl", "custom_cat_type"])]
    pub sync: bool,

    /// Upload to Seedpool
    #[arg(long = "SP", requires = "input_path")]
    pub sp: bool,

    /// Upload to TorrentLeech
    #[arg(long = "TL", requires = "input_path")]
    pub tl: bool,

    /// Custom category/type code (4 digits)
    #[arg(short = 'c', long, value_name = "CAT_TYPE", requires = "input_path")]
    pub custom_cat_type: Option<String>,

    /// Launch UI mode
    #[arg(long, conflicts_with_all = ["sync", "sp", "tl", "custom_cat_type"])]
    pub ui: bool,

    /// Run preflight check
    #[arg(long, conflicts_with_all = ["sync", "sp", "tl", "custom_cat_type"])]
    pub pre: bool,

    /// Enable dry-run mode - simulate uploads without actually uploading
    #[arg(
        long,
        help = "Enable dry-run mode - simulate uploads without actually uploading"
    )]
    pub dry_run: bool,

    /// Subcommands
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Input path for processing
    #[arg(index = 1)]
    pub input_path: Option<PathBuf>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Check for duplicates across trackers
    Check {
        /// Path to the media file or directory
        #[arg(index = 1)]
        input_path: PathBuf,
    },

    /// Upload to trackers
    Upload {
        /// Path to the media file or directory
        #[arg(index = 1)]
        input_path: PathBuf,

        /// Tracker to upload to
        #[arg(short = 't', long)]
        tracker: String,

        /// Category/type code
        #[arg(short = 'c', long)]
        code: Option<String>,

        /// Dry run mode
        #[arg(long)]
        dry_run: bool,
    },

    /// List available categories and types
    Categories {
        /// Tracker name
        #[arg(short = 't', long)]
        tracker: Option<String>,
    },
}
