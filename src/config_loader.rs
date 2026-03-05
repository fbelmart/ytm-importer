use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize, Clone)]
pub struct OAuth2Config {
    pub client_id: String,
    pub client_secret: String,
    #[serde(default = "default_cache_dir")]
    pub cache_dir: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub oauth2: OAuth2Config,
}

fn default_cache_dir() -> String {
    "./output/auth".to_string()
}

impl Config {
    /// Load configuration from a TOML file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path.as_ref())
            .with_context(|| format!("Failed to read config file: {}", path.as_ref().display()))?;

        let config: Config = toml::from_str(&content)
            .with_context(|| "Failed to parse config file")?;

        Ok(config)
    }

    /// Load with automatic path detection (./config.toml or ./config/config.toml)
    pub fn from_default_paths() -> Result<Self> {
        let paths = [
            Path::new("./config.toml"),
            Path::new("./config/config.toml"),
            Path::new("./.config/config.toml"),
        ];

        for path in paths {
            if path.exists() {
                return Self::from_file(path);
            }
        }

        anyhow::bail!("No config.toml found in default locations")
    }
}
