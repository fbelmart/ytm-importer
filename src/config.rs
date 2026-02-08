use serde::{Serialize, Deserialize};
use chrono::prelude::*;
use std::path::PathBuf;

/// Application configuration
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub csv_path: PathBuf,
    pub playlist_name: Option<String>,
    pub make_public: bool,
    pub skip_confirmation: bool,
    pub output_dir: PathBuf,
    pub min_confidence: f64,
    pub max_retries: u8,
    pub limit_tracks: Option<usize>,
    pub verbosity: u8,
}

/// Import job configuration
#[derive(Debug, Clone)]
pub struct ImportConfig {
    pub job_id: String,
    pub started_at: DateTime<Local>,
    pub config: AppConfig,
}

/// Configuration for output files
#[derive(Debug, Clone)]
pub struct OutputConfig {
    pub logs_dir: PathBuf,
    pub unmatched_csv: PathBuf,
    pub summary_json: PathBuf,
    pub debug_log: PathBuf,
}

impl AppConfig {
    /// Create from CLI arguments
    pub fn from_cli(cli: &crate::Cli) -> Self {
        Self {
            csv_path: cli.csv_file.clone(),
            playlist_name: cli.playlist_name.clone(),
            make_public: cli.make_public,
            skip_confirmation: cli.skip_confirmation,
            output_dir: cli.output_dir.clone(),
            min_confidence: cli.min_confidence,
            max_retries: cli.max_retries,
            limit_tracks: cli.limit_tracks,
            verbosity: cli.verbosity,
        }
    }

    /// Generate default playlist name if not provided
    pub fn get_playlist_name(&self) -> String {
        self.playlist_name.clone().unwrap_or_else(|| {
            let base_name = self.csv_path.file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "Imported Playlist".to_string());

            let timestamp = Local::now().format("%Y-%m-%d %H:%M");
            format!("{} ({})", base_name, timestamp)
        })
    }

    /// Get playlist visibility
    pub fn get_visibility(&self) -> &'static str {
        if self.make_public {
            "PUBLIC"
        } else {
            "PRIVATE"
        }
    }
}

impl ImportConfig {
    /// Create new import configuration
    pub fn new(config: AppConfig) -> Self {
        let job_id = format!("job_{}", Local::now().format("%Y%m%d_%H%M%S"));

        Self {
            job_id,
            started_at: Local::now(),
            config,
        }
    }

    /// Get output configuration for this job
    pub fn output_config(&self) -> OutputConfig {
        let logs_dir = self.config.output_dir.join("logs").join(&self.job_id);
        let unmatched_csv = logs_dir.join("unmatched_tracks.csv");
        let summary_json = logs_dir.join("import_summary.json");
        let debug_log = logs_dir.join("debug.log");

        OutputConfig {
            logs_dir,
            unmatched_csv,
            summary_json,
            debug_log,
        }
    }

    /// Get import summary template
    pub fn create_summary(&self) -> ImportSummary {
        ImportSummary {
            job_id: self.job_id.clone(),
            started_at: self.started_at,
            completed_at: None,
            csv_file: self.config.csv_path.display().to_string(),
            playlist_name: self.config.get_playlist_name(),
            visibility: self.config.get_visibility().to_string(),
            total_tracks: 0,
            matched_tracks: 0,
            unmatched_tracks: 0,
            success_rate: 0.0,
            duration_seconds: None,
            errors: Vec::new(),
            settings: SummarySettings {
                min_confidence: self.config.min_confidence,
                max_retries: self.config.max_retries,
                limit_tracks: self.config.limit_tracks,
            },
        }
    }
}

/// Import summary for JSON output
#[derive(Debug, Serialize, Deserialize)]
pub struct ImportSummary {
    pub job_id: String,
    pub started_at: DateTime<Local>,
    pub completed_at: Option<DateTime<Local>>,
    pub csv_file: String,
    pub playlist_name: String,
    pub visibility: String,
    pub total_tracks: usize,
    pub matched_tracks: usize,
    pub unmatched_tracks: usize,
    pub success_rate: f64,
    pub duration_seconds: Option<f64>,
    pub errors: Vec<String>,
    pub settings: SummarySettings,
}

/// Settings used for the import
#[derive(Debug, Serialize, Deserialize)]
pub struct SummarySettings {
    pub min_confidence: f64,
    pub max_retries: u8,
    pub limit_tracks: Option<usize>,
}
