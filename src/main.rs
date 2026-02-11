use ytm_importer::{Cli, CsvParser, ParseStats};
use clap::Parser;
use anyhow::{Result, Context};
use indicatif::{ProgressBar, ProgressStyle};

mod config;
mod validation;

fn main() -> anyhow::Result<()> {
    // Parse CLI arguments
    let cli = Cli::parse();

    // Initialize logging based on verbosity
    init_logging(cli.verbosity);

    print_banner();

    // Validate inputs
    validate_inputs(&cli)?;

    // Create configuration
    let app_config = config::AppConfig::from_cli(&cli);
    let import_config = config::ImportConfig::new(app_config.clone());

    // Create output directory structure
    setup_output_structure(&import_config)?;

    // Display configuration
    display_configuration(&app_config);

    // Ask for confirmation (unless skipped)
    if !app_config.skip_confirmation {
        if !ask_for_confirmation()? {
            println!("Import cancelled by user.");
            return Ok(());
        }
    }

    println!("\n🚀 Starting import process...\n");

    // Parse CSV file with progress indicator
    let tracks = parse_csv_file(&app_config)?;

    // Display parse statistics
    let stats = ParseStats::from_tracks(&tracks);
    stats.print_summary();

    // Apply track limit if specified
    let tracks = if let Some(limit) = app_config.limit_tracks {
        println!("\n⚠️  Limiting to first {} tracks (for testing)", limit);
        tracks.into_iter().take(limit).collect()
    } else {
        tracks
    };

    // Display sample of tracks
    if cli.verbosity > 0 {
        display_track_sample(&tracks);
    }

    println!("\n✅ CSV parsing completed!");
    println!("   Next step: Searching for tracks on YouTube Music...");

    // TODO: Add YouTube Music API integration
    // TODO: Track matching logic
    // TODO: Playlist creation

    Ok(())
}

/// Parse CSV file with optional spinner
fn parse_csv_file(config: &config::AppConfig) -> Result<Vec<ytm_importer::Track>> {
    let start_time = std::time::Instant::now();

    // Show spinner only in verbose mode
    let pb = if config.verbosity > 0 {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.green} {msg}")?
                .tick_strings(&["⣾", "⣽", "⣻", "⢿", "⡿", "⣟", "⣯", "⣷"]),
        );
        pb.set_message("Parsing CSV...");
        pb.enable_steady_tick(std::time::Duration::from_millis(100));
        Some(pb)
    } else {
        None
    };

    let tracks = CsvParser::parse_auto(&config.csv_path)
        .with_context(|| format!("Failed to parse CSV file: {}", config.csv_path.display()))?;

    let duration = start_time.elapsed();

    // Finish progress bar if it exists
    if let Some(pb) = pb {
        pb.finish_with_message(format!("✅ Parsed {} tracks in {:.2?}", tracks.len(), duration));
    } else if config.verbosity > 0 {
        println!("  Parsed {} tracks in {:.2?}", tracks.len(), duration);
    }

    Ok(tracks)
}

/// Display sample of parsed tracks
fn display_track_sample(tracks: &[ytm_importer::Track]) {
    let display_count = std::cmp::min(5, tracks.len());

    if display_count == 0 {
        return;
    }

    println!("\n🎵 Sample of parsed tracks (first {}):", display_count);
    println!("┌─────────────────────────────────────────────────────────────┐");

    for (i, track) in tracks.iter().take(display_count).enumerate() {
        let album_info = track.album.as_deref().unwrap_or("Unknown album");
        let duration_info = track.duration_ms
            .map(|ms| format!("{:.1}m", ms as f64 / 60000.0))
            .unwrap_or_else(|| "Unknown".to_string());

        println!("│ {:2}. {:35.35} - {:20.20}",
            i + 1, track.title, track.artist);
        println!("│    Album: {:45.45} Duration: {:>8}",
            album_info, duration_info);

        if i < display_count - 1 && i < tracks.len() - 1 {
            println!("│");
        }
    }

    println!("└─────────────────────────────────────────────────────────────┘");

    if tracks.len() > display_count {
        println!("   ... and {} more tracks", tracks.len() - display_count);
    }
}

/// Initialize logging based on verbosity level
fn init_logging(verbosity: u8) {
    // Simple logging to stderr based on verbosity
    match verbosity {
        0 => {} // No logging
        1 => eprintln!("[INFO] Verbose mode level 1"),
        2 => eprintln!("[DEBUG] Verbose mode level 2"),
        _ => eprintln!("[TRACE] Verbose mode level {}", verbosity),
    }
}

/// Print application banner
fn print_banner() {
    println!("┌─────────────────────────────────────────────┐");
    println!("│    YouTube Music Playlist Importer v1.0     │");
    println!("└─────────────────────────────────────────────┘");
    println!();
}

/// Validate all inputs
fn validate_inputs(cli: &Cli) -> Result<()> {
    println!("Validating inputs...");

    // Validate CSV file
    validation::ConfigValidator::validate_csv_file(&cli.csv_file)
        .context("CSV file validation failed")?;

    // Validate CSV format
    let sample_lines = validation::ConfigValidator::validate_csv_format(&cli.csv_file)
        .context("CSV format validation failed")?;

    if cli.verbosity > 0 {
        println!("  CSV format looks good");
        println!("  Sample header: {}", sample_lines[0]);
        if sample_lines.len() > 1 {
            println!("  Sample data: {}", sample_lines[1]);
        }
    }

    // Validate configuration values
    validation::ConfigValidator::validate_config(cli.min_confidence, cli.max_retries)
        .context("Configuration validation failed")?;

    // Validate and create output directory
    validation::ConfigValidator::validate_output_dir(&cli.output_dir)
        .context("Output directory validation failed")?;

    println!("✅ All inputs validated successfully.\n");
    Ok(())
}

/// Setup output directory structure
fn setup_output_structure(import_config: &config::ImportConfig) -> Result<()> {
    let output_config = import_config.output_config();

    // Create logs directory
    std::fs::create_dir_all(&output_config.logs_dir)
        .context("Failed to create logs directory")?;

    if import_config.config.verbosity > 0 {
        println!("Output directory structure created:");
        println!("  Logs: {}", output_config.logs_dir.display());
        println!("  Unmatched tracks: {}", output_config.unmatched_csv.display());
        println!("  Summary: {}", output_config.summary_json.display());
        println!();
    }

    Ok(())
}

/// Display configuration to user
fn display_configuration(config: &config::AppConfig) {
    println!("Import Configuration:");
    println!("─────────────────────");
    println!("  CSV file:          {}", config.csv_path.display());
    println!("  Playlist name:     {}", config.get_playlist_name());
    println!("  Visibility:        {}", config.get_visibility());
    println!("  Min confidence:    {:.2}", config.min_confidence);
    println!("  Max retries:       {}", config.max_retries);
    println!("  Output directory:  {}", config.output_dir.display());

    if let Some(limit) = config.limit_tracks {
        println!("  Track limit:       {}", limit);
    }

    println!();
}

/// Ask user for confirmation
fn ask_for_confirmation() -> Result<bool> {
    use std::io::{self, Write};

    print!("Proceed with import? [y/N]: ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let input = input.trim().to_lowercase();
    Ok(input == "y" || input == "yes")
}
