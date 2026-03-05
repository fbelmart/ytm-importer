# Getting Google OAuth2 Credentials

1. Go to https://console.cloud.google.com/
2. Create a new project or select existing
3. Enable the YouTube Data API v3
4. Go to "APIs & Services" → "Credentials"
5. Click "Create Credentials" → "OAuth client ID"
6. Application type: "Desktop app"
7. Name: "YouTube Music Importer"
8. Add redirect URI: `http://localhost:8080`
9. Copy Client ID and Client Secret
10. Create `config.toml` from example
