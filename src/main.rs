use ytm_importer::Cli;
use clap::Parser;
use anyhow::{Result, Context};

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

    // TODO: Add CSV parsing and import logic
    println!("[TODO] CSV parsing and import logic will be implemented in next steps");

    // TODO: Generate summary
    println!("\n✅ Import completed!");

    Ok(())
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
