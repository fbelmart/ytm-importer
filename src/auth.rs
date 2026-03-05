use anyhow::{Context, Result};
use oauth2::{
    AuthUrl,
    ClientId,
    ClientSecret,
    CsrfToken,
    PkceCodeChallenge,
    RedirectUrl,
    Scope,
    TokenResponse,
    TokenUrl,
};
use oauth2::basic::BasicClient;
use oauth2::reqwest::async_http_client;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
// Remove unused import: use url::Url;

/// YouTube OAuth2 scopes needed for playlist management
const YOUTUBE_SCOPES: [&str; 2] = [
    "https://www.googleapis.com/auth/youtube",
    "https://www.googleapis.com/auth/youtubepartner",
];

/// Google OAuth2 endpoints
const GOOGLE_AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";

/// Stored token information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredToken {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub scopes: Vec<String>,
}

/// YouTube Music authenticated client
pub struct YouTubeMusicAuth {
    client: BasicClient,
    http_client: reqwest::Client,
    token_cache_path: PathBuf,
    current_token: Option<StoredToken>,
}

impl YouTubeMusicAuth {
    /// Create a new authentication instance
    pub fn new(client_id: &str, client_secret: &str, cache_dir: &Path) -> Result<Self> {
        let client = BasicClient::new(
            ClientId::new(client_id.to_string()),
            Some(ClientSecret::new(client_secret.to_string())),
            AuthUrl::new(GOOGLE_AUTH_URL.to_string())?,
            Some(TokenUrl::new(GOOGLE_TOKEN_URL.to_string())?),
        )
        .set_redirect_uri(
            RedirectUrl::new("http://localhost:8080".to_string())?
        );

        let http_client = reqwest::ClientBuilder::new()
            // Following redirects opens the client up to SSRF vulnerabilities
            .redirect(reqwest::redirect::Policy::none())
            // Enable connection pooling for better performance
            .pool_max_idle_per_host(10)
            .tcp_keepalive(std::time::Duration::from_secs(60))
            .build()
            .context("Failed to build HTTP client")?;

        // Create cache directory if it doesn't exist
        fs::create_dir_all(cache_dir)
            .context("Failed to create token cache directory")?;

        let token_cache_path = cache_dir.join("token.json");

        Ok(Self {
            client,
            http_client,
            token_cache_path,
            current_token: None,
        })
    }

    /// Perform OAuth2 authorization flow
    pub async fn authorize(&mut self) -> Result<()> {
        // Try to load existing token first
        if let Some(token) = self.load_cached_token()? {
            if !self.is_token_expired(&token) {
                self.current_token = Some(token);
                println!("✅ Using cached authentication token");
                return Ok(());
            } else if let Some(refresh_token) = &token.refresh_token {
                // Try to refresh the token
                println!("🔄 Token expired, attempting to refresh...");
                if let Ok(new_token) = self.refresh_access_token(refresh_token).await {
                    self.current_token = Some(new_token);
                    self.save_cached_token()?;
                    return Ok(());
                }
            }
        }

        println!("🔐 Starting OAuth2 authorization flow...");

        // Generate PKCE challenge
        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

        // Generate authorization URL
        let mut auth_request = self.client
            .authorize_url(CsrfToken::new_random)
            .set_pkce_challenge(pkce_challenge);

        // Add required scopes
        for scope in &YOUTUBE_SCOPES {
            auth_request = auth_request.add_scope(Scope::new(scope.to_string()));
        }

        let (auth_url, csrf_token) = auth_request.url();

        // Start the local server BEFORE opening the browser
        println!("📡 Starting local callback server on http://localhost:8080...");

        // Use tokio::spawn to run the server in the background
        let (tx, rx) = tokio::sync::oneshot::channel::<Result<String>>();

        let server_handle = tokio::spawn(async move {
            let result = Self::wait_for_callback().await;
            let _ = tx.send(result);
        });

        // Give the server a moment to start
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Now open the browser
        println!("\n🌐 Opening browser for authorization...");
        if let Err(e) = open::that(auth_url.to_string()) {
            println!("   Couldn't open browser automatically. Please open this URL manually:");
            println!("   {}", auth_url);
        } else {
            println!("   Browser opened. Please authorize the application.");
        }
        println!("\n📋 After authorizing, you'll be redirected automatically.");

        // Wait for the callback - FIXED: Handle the Result properly
        let callback_result = rx.await
            .context("Failed to receive callback from server")?;

        let code = match callback_result {
            Ok(code) => code,
            Err(e) => return Err(e).context("Callback server error"),
        };

        // Exchange code for token
        println!("🔄 Exchanging authorization code for access token...");

        let token_response = self.client
            .exchange_code(oauth2::AuthorizationCode::new(code))
            .set_pkce_verifier(pkce_verifier)
            .request_async(async_http_client)
            .await
            .context("Failed to exchange authorization code")?;

        // Store the token
        let stored_token = StoredToken {
            access_token: token_response.access_token().secret().clone(),
            refresh_token: token_response.refresh_token().map(|rt| rt.secret().clone()),
            expires_at: token_response.expires_in().map(|duration| {
                chrono::Utc::now() + chrono::Duration::from_std(duration).unwrap()
            }),
            scopes: YOUTUBE_SCOPES.iter().map(|s| s.to_string()).collect(),
        };

        self.current_token = Some(stored_token);
        self.save_cached_token()?;

        println!("✅ Authorization successful!");

        // Stop the server
        server_handle.abort();

        Ok(())
    }

    /// Start a local server to receive the OAuth2 callback
    async fn wait_for_callback() -> Result<String> {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:8080").await
            .context("Failed to bind to local port 8080")?;

        println!("   Listening for callback on http://localhost:8080...");

        loop {
            let (mut stream, _) = listener.accept().await?;

            let mut reader = BufReader::new(&mut stream);
            let mut request_line = String::new();
            reader.read_line(&mut request_line).await?;

            // Parse the request to get the code
            if let Some(code) = Self::parse_callback_request(&request_line) {
                // Send success response
                let response = "HTTP/1.1 200 OK\r\n\
                    Content-Type: text/html\r\n\
                    \r\n\
                    <html><body style='font-family: sans-serif; padding: 2rem;'>\
                    <h1 style='color: #4CAF50;'>✅ Authorization Successful!</h1>\
                    <p>You can close this window and return to the terminal.</p>\
                    </body></html>";

                stream.write_all(response.as_bytes()).await?;
                stream.flush().await?;

                return Ok(code);
            }

            // Send error response for other requests
            let response = "HTTP/1.1 400 Bad Request\r\n\r\n";
            stream.write_all(response.as_bytes()).await?;
        }
    }

    /// Parse the authorization code from the callback request
    fn parse_callback_request(request_line: &str) -> Option<String> {
        if !request_line.starts_with("GET /?code=") && !request_line.starts_with("GET /?state=") {
            return None;
        }

        // Extract the code parameter
        let parts: Vec<&str> = request_line.split_whitespace().collect();
        if parts.len() < 2 {
            return None;
        }

        let url_path = parts[1];

        // Find the code parameter
        if let Some(code_start) = url_path.find("code=") {
            let code_part = &url_path[code_start + 5..];
            if let Some(code_end) = code_part.find('&') {
                Some(percent_encoding::percent_decode_str(&code_part[..code_end])
                    .decode_utf8()
                    .ok()?
                    .to_string())
            } else {
                Some(code_part.to_string())
            }
        } else {
            None
        }
    }

    /// Check if the stored token is expired
    fn is_token_expired(&self, token: &StoredToken) -> bool {
        match token.expires_at {
            Some(expires) => expires <= chrono::Utc::now(),
            None => true,
        }
    }

    /// Refresh an expired access token
    async fn refresh_access_token(&self, refresh_token: &str) -> Result<StoredToken> {
        // For now, return error - we'll implement this when needed
        Err(anyhow::anyhow!("Token refresh not yet implemented"))
    }

    /// Load cached token from disk
    fn load_cached_token(&self) -> Result<Option<StoredToken>> {
        if !self.token_cache_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&self.token_cache_path)
            .context("Failed to read token cache")?;

        let token = serde_json::from_str(&content)
            .context("Failed to parse token cache")?;

        Ok(Some(token))
    }

    /// Save token to disk cache
    fn save_cached_token(&self) -> Result<()> {
        if let Some(token) = &self.current_token {
            let content = serde_json::to_string_pretty(token)
                .context("Failed to serialize token")?;

            fs::write(&self.token_cache_path, content)
                .context("Failed to write token cache")?;
        }

        Ok(())
    }

    /// Get the current access token
    pub fn get_access_token(&self) -> Result<&str> {
        self.current_token
            .as_ref()
            .map(|t| t.access_token.as_str())
            .ok_or_else(|| anyhow::anyhow!("Not authenticated. Run authorize() first."))
    }

    /// Create an authorized HTTP client for API calls
    pub fn create_api_client(&self) -> Result<reqwest::Client> {
        let token = self.get_access_token()?;

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            reqwest::header::HeaderValue::from_str(&format!("Bearer {}", token))
                .context("Failed to create authorization header")?,
        );
        headers.insert(
            reqwest::header::USER_AGENT,
            reqwest::header::HeaderValue::from_static(
                "YouTubeMusicImporter/1.0 (https://github.com/fbelmart/ytm-importer)"
            ),
        );
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            reqwest::header::HeaderValue::from_static("application/json"),
        );

        reqwest::ClientBuilder::new()
            .default_headers(headers)
            .pool_max_idle_per_host(10)
            .tcp_keepalive(std::time::Duration::from_secs(60))
            .build()
            .context("Failed to build API client")
    }
}

impl Drop for YouTubeMusicAuth {
    fn drop(&mut self) {
        // Ensure token is saved when struct is dropped
        if self.current_token.is_some() {
            let _ = self.save_cached_token();
        }
    }
}
