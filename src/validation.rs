use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ValidationError {
    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Invalid file extension: {0}. Expected .csv")]
    InvalidExtension(String),

    #[error("File is not readable: {0}")]
    NotReadable(String),

    #[error("File is empty: {0}")]
    EmptyFile(String),

    #[error("Invalid confidence value: {0}. Must be between 0.0 and 1.0")]
    InvalidConfidence(f64),

    #[error("Output directory cannot be created: {0}")]
    OutputDirError(String),
}

pub struct ConfigValidator;

impl ConfigValidator {
    /// Validate the CSV file
    pub fn validate_csv_file(csv_path: &Path) -> Result<()> {
        // Check if file exists
        if !csv_path.exists() {
            return Err(ValidationError::FileNotFound(
                csv_path.display().to_string()
            ).into());
        }

        // Check file extension
        if let Some(ext) = csv_path.extension() {
            if ext.to_ascii_lowercase() != "csv" {
                return Err(ValidationError::InvalidExtension(
                    ext.to_string_lossy().to_string()
                ).into());
            }
        } else {
            return Err(ValidationError::InvalidExtension(
                "no extension".to_string()
            ).into());
        }

        // Check if file is readable
        if let Err(e) = std::fs::File::open(csv_path) {
            return Err(ValidationError::NotReadable(
                format!("{}: {}", csv_path.display(), e)
            ).into());
        }

        // Check if file is empty
        let metadata = std::fs::metadata(csv_path)
            .with_context(|| format!("Failed to get metadata for {}", csv_path.display()))?;

        if metadata.len() == 0 {
            return Err(ValidationError::EmptyFile(
                csv_path.display().to_string()
            ).into());
        }

        Ok(())
    }

    /// Validate configuration values
    pub fn validate_config(min_confidence: f64, max_retries: u8) -> Result<()> {
        if !(0.0..=1.0).contains(&min_confidence) {
            return Err(ValidationError::InvalidConfidence(min_confidence).into());
        }

        if max_retries > 10 {
            return Err(anyhow::anyhow!("Max retries cannot exceed 10"));
        }

        Ok(())
    }

    /// Validate and create output directory
    pub fn validate_output_dir(output_dir: &Path) -> Result<PathBuf> {
        if output_dir.exists() {
            // Check if it's a directory
            if !output_dir.is_dir() {
                return Err(anyhow::anyhow!(
                    "Output path exists but is not a directory: {}",
                    output_dir.display()
                ));
            }

            // Check if writable
            let test_file = output_dir.join(".writable_test");
            if std::fs::write(&test_file, "").is_err() {
                return Err(ValidationError::OutputDirError(
                    format!("Directory is not writable: {}", output_dir.display())
                ).into());
            }
            let _ = std::fs::remove_file(test_file);
        } else {
            // Create directory
            std::fs::create_dir_all(output_dir)
                .with_context(|| format!("Failed to create output directory: {}", output_dir.display()))?;
        }

        Ok(output_dir.to_path_buf())
    }

    /// Read and validate the first few lines of CSV to ensure proper format
    pub fn validate_csv_format(csv_path: &Path) -> Result<Vec<String>> {
        use std::fs::File;
        use std::io::{BufRead, BufReader};

        let file = File::open(csv_path)
            .with_context(|| format!("Failed to open CSV file: {}", csv_path.display()))?;

        let reader = BufReader::new(file);
        let mut lines = Vec::new();

        // Read first 5 lines or until EOF
        for (i, line_result) in reader.lines().enumerate().take(5) {
            let line = line_result
                .with_context(|| format!("Failed to read line {} from CSV", i + 1))?;
            lines.push(line);

            // Stop if we have header and one data row
            if i >= 2 && lines.len() > 1 {
                break;
            }
        }

        if lines.is_empty() {
            return Err(ValidationError::EmptyFile(
                csv_path.display().to_string()
            ).into());
        }

        // Check if it looks like Exportify format
        let header = &lines[0];
        let exportify_columns = vec![
            "Track Name",
            "Artist Name(s)",
            "Album Name",
            "Track ID",
        ];

        let mut found_columns = 0;
        for column in &exportify_columns {
            if header.contains(column) {
                found_columns += 1;
            }
        }

        if found_columns < 2 {
            eprintln!("Warning: CSV header doesn't look like standard Exportify format");
            eprintln!("Header: {}", header);
            eprintln!("Expected columns like: Track Name, Artist Name(s), Album Name, Track ID");
        }

        Ok(lines)
    }
}
