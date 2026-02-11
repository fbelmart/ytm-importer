pub mod config;
pub mod validation;
pub mod csv_parser;

use clap::Parser;

/// Import Spotify playlists to YouTube Music
#[derive(Parser)]
#[command(name = "ytm-importer")]
#[command(about = "Import Spotify playlists to YouTube Music from CSV files")]
#[command(version = "1.0")]
pub struct Cli {
    /// Path to the Exportify CSV file
    #[arg(help = "Path to Exportify CSV file")]
    pub csv_file: std::path::PathBuf,

    /// Custom name for the YouTube Music playlist
    #[arg(short = 'n', long = "name")]
    pub playlist_name: Option<String>,

    /// Make the playlist public (default: private)
    #[arg(short = 'p', long = "public")]
    pub make_public: bool,

    /// Skip confirmation prompts
    #[arg(short = 'y', long = "yes", default_value_t = false)]
    pub skip_confirmation: bool,

    /// Output directory for logs and results
    #[arg(short = 'o', long = "output", default_value = "./output")]
    pub output_dir: std::path::PathBuf,

    /// Minimum match confidence (0.0 to 1.0)
    #[arg(short = 'c', long = "confidence", default_value_t = 0.7)]
    pub min_confidence: f64,

    /// Maximum retry attempts per track
    #[arg(short = 'r', long = "retries", default_value_t = 2)]
    pub max_retries: u8,

    /// Limit number of tracks to process (for testing)
    #[arg(long = "limit")]
    pub limit_tracks: Option<usize>,

    /// Verbose output
    #[arg(short = 'v', long = "verbose", action = clap::ArgAction::Count)]
    pub verbosity: u8,
}

/// Application result type
pub type AppResult<T> = anyhow::Result<T>;

// Re-export commonly used items
pub use config::{AppConfig, ImportConfig};
pub use validation::ConfigValidator;
pub use csv_parser::{CsvParser, Track, ParseStats};
