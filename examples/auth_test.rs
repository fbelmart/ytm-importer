use ytm_importer::auth::YouTubeMusicAuth;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // Load config from file or environment
    let config = load_config()?;

    let mut auth = YouTubeMusicAuth::new(
        &config.client_id,
        &config.client_secret,
        &std::path::PathBuf::from("./output/auth"),
    )?;

    auth.authorize().await?;

    println!("✅ Access token: {}", auth.get_access_token()?);

    // Test API client
    let api_client = auth.create_api_client()?;
    println!("✅ API client created with connection pooling");

    Ok(())
}

fn load_config() -> Result<Config> {
    // Simple config loading - you can expand this
    Ok(Config {
        client_id: std::env::var("GOOGLE_CLIENT_ID")
            .unwrap_or_else(|_| "YOUR_CLIENT_ID".to_string()),
        client_secret: std::env::var("GOOGLE_CLIENT_SECRET")
            .unwrap_or_else(|_| "YOUR_CLIENT_SECRET".to_string()),
    })
}

struct Config {
    client_id: String,
    client_secret: String,
}
