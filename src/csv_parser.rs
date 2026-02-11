use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

/// Represents a track from Spotify/Exportify
#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub duration_ms: Option<u64>,
    pub spotify_id: Option<String>,
    pub track_number: Option<u32>,
    pub disc_number: Option<u32>,
    pub explicit: Option<bool>,
}

/// CSV parser for Exportify format
pub struct CsvParser;

impl CsvParser {
    /// Parse an Exportify CSV file
    pub fn parse_exportify<P: AsRef<Path>>(csv_path: P) -> Result<Vec<Track>> {
        let path = csv_path.as_ref();

        // Create CSV reader
        let mut reader = csv::Reader::from_path(path)
            .with_context(|| format!("Failed to open CSV file: {}", path.display()))?;

        let mut tracks = Vec::new();
        let mut line_number = 0;

        // Parse each record
        for result in reader.deserialize() {
            line_number += 1;

            let record: ExportifyRecord = match result {
                Ok(record) => record,
                Err(e) => {
                    eprintln!("Warning: Failed to parse line {}: {}", line_number, e);
                    continue;
                }
            };

            // CORRECT: This let statement is inside the function
            let track = Track {
                title: record.track_name,
                artist: record.artist_name,
                album: Some(record.album_name),
                duration_ms: record.duration_ms.parse::<u64>().ok(),
                spotify_id: Some(record.track_uri),  // Changed from track_id
                track_number: record.track_number.and_then(|s| s.parse().ok()),
                disc_number: record.disc_number.and_then(|s| s.parse().ok()),
                explicit: record.explicit.map(|s| s.to_lowercase() == "true"),
            };

            tracks.push(track);
        }

        if tracks.is_empty() {
            return Err(anyhow::anyhow!(
                "No tracks found in CSV file. Is the file in Exportify format?"
            ));
        }

        Ok(tracks)
    }

    /// Try to parse CSV with flexible column detection
    pub fn parse_generic<P: AsRef<Path>>(csv_path: P) -> Result<Vec<Track>> {
        let path = csv_path.as_ref();

        let mut reader = csv::Reader::from_path(path)
            .with_context(|| format!("Failed to open CSV file: {}", path.display()))?;

        let headers = reader.headers()
            .with_context(|| "Failed to read CSV headers")?
            .clone();

        // Detect column indices
        let title_idx = Self::detect_column(&headers, &["track", "title", "name", "song"]);
        let artist_idx = Self::detect_column(&headers, &["artist", "artist_name", "performer", "singer"]);
        let album_idx = Self::detect_column(&headers, &["album", "album_name"]);

        if title_idx.is_none() || artist_idx.is_none() {
            return Err(anyhow::anyhow!(
                "Could not detect required columns (title and artist) in CSV"
            ));
        }

        let mut tracks = Vec::new();
        let mut line_number = 0;

        for result in reader.records() {
            line_number += 1;

            let record = match result {
                Ok(record) => record,
                Err(e) => {
                    eprintln!("Warning: Failed to parse line {}: {}", line_number, e);
                    continue;
                }
            };

            let title = record.get(title_idx.unwrap())
                .unwrap_or("")
                .to_string();

            let artist = record.get(artist_idx.unwrap())
                .unwrap_or("")
                .to_string();

            let album = album_idx.and_then(|idx| record.get(idx).map(|s| s.to_string()));

            // Skip empty tracks
            if title.trim().is_empty() || artist.trim().is_empty() {
                eprintln!("Warning: Line {} has empty title or artist, skipping", line_number);
                continue;
            }

            let track = Track {  // CORRECT: Inside function
                title,
                artist,
                album,
                duration_ms: None,
                spotify_id: None,
                track_number: None,
                disc_number: None,
                explicit: None,
            };

            tracks.push(track);
        }

        if tracks.is_empty() {
            return Err(anyhow::anyhow!("No valid tracks found in CSV file"));
        }

        Ok(tracks)
    }

    /// Detect column index by trying multiple possible names
    fn detect_column(headers: &csv::StringRecord, possible_names: &[&str]) -> Option<usize> {
        for name in possible_names {
            if let Some(idx) = headers.iter().position(|h|
                h.to_lowercase().contains(name)
            ) {
                return Some(idx);
            }
        }
        None
    }

    /// Parse CSV with automatic format detection
    pub fn parse_auto<P: AsRef<Path>>(csv_path: P) -> Result<Vec<Track>> {
        let path = csv_path.as_ref();

        // First try Exportify format
        match Self::parse_exportify(path) {
            Ok(tracks) => return Ok(tracks),
            Err(e) => {
                eprintln!("Not Exportify format, trying generic parser: {}", e);
            }
        }

        // Fall back to generic parser
        Self::parse_generic(path)
    }
}

/// Exportify CSV record structure
#[derive(Debug, Deserialize)]
struct ExportifyRecord {
    #[serde(rename = "Track Name")]
    track_name: String,

    #[serde(rename = "Artist Name(s)")]
    artist_name: String,

    #[serde(rename = "Album Name")]
    album_name: String,

    #[serde(rename = "Track URI")]
    track_uri: String,

    #[serde(rename = "Duration (ms)")]
    duration_ms: String,

    #[serde(rename = "Track Number")]
    track_number: Option<String>,

    #[serde(rename = "Disc Number")]
    disc_number: Option<String>,

    #[serde(rename = "Explicit")]
    explicit: Option<String>,

    // Optional fields
    #[serde(rename = "Album Artist Name(s)")]
    #[allow(dead_code)]
    album_artist: Option<String>,

    #[serde(rename = "Album ID")]
    #[allow(dead_code)]
    album_id: Option<String>,

    #[serde(rename = "Album Release Date")]
    #[allow(dead_code)]
    release_date: Option<String>,

    #[serde(rename = "Track Popularity")]
    #[allow(dead_code)]
    popularity: Option<String>,

    #[serde(rename = "Added By")]
    #[allow(dead_code)]
    added_by: Option<String>,

    #[serde(rename = "Added At")]
    #[allow(dead_code)]
    added_at: Option<String>,
}

/// Statistics about parsed tracks
#[derive(Debug, Clone)]
pub struct ParseStats {
    pub total_tracks: usize,
    pub with_album: usize,
    pub with_duration: usize,
    pub with_spotify_id: usize,
    pub unique_artists: usize,
    pub unique_albums: usize,
}

impl ParseStats {
    pub fn from_tracks(tracks: &[Track]) -> Self {
        let total_tracks = tracks.len();
        let with_album = tracks.iter().filter(|t| t.album.is_some()).count();
        let with_duration = tracks.iter().filter(|t| t.duration_ms.is_some()).count();
        let with_spotify_id = tracks.iter().filter(|t| t.spotify_id.is_some()).count();

        let unique_artists = {
            let mut artists = std::collections::HashSet::new();
            for track in tracks {
                artists.insert(track.artist.clone());
            }
            artists.len()
        };

        let unique_albums = {
            let mut albums = std::collections::HashSet::new();
            for track in tracks {
                if let Some(album) = &track.album {
                    albums.insert(album.clone());
                }
            }
            albums.len()
        };

        Self {
            total_tracks,
            with_album,
            with_duration,
            with_spotify_id,
            unique_artists,
            unique_albums,
        }
    }

    pub fn print_summary(&self) {
        println!("📊 CSV Parse Statistics:");
        println!("  Total tracks:        {}", self.total_tracks);
        println!("  Tracks with album:   {} ({:.1}%)",
            self.with_album,
            (self.with_album as f64 / self.total_tracks as f64) * 100.0
        );
        println!("  Tracks with duration: {} ({:.1}%)",
            self.with_duration,
            (self.with_duration as f64 / self.total_tracks as f64) * 100.0
        );
        println!("  Tracks with Spotify ID: {} ({:.1}%)",
            self.with_spotify_id,
            (self.with_spotify_id as f64 / self.total_tracks as f64) * 100.0
        );
        println!("  Unique artists:      {}", self.unique_artists);
        println!("  Unique albums:       {}", self.unique_albums);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[test]
    fn test_parse_exportify_valid() {
        let mut file = NamedTempFile::new().unwrap();

        writeln!(file, "Track Name,Artist Name(s),Album Name,Track URI,Duration (ms),Explicit")
            .unwrap();
        writeln!(file, "Test Song,Test Artist,Test Album,spotify:track:123,180000,false")
            .unwrap();

        let tracks = CsvParser::parse_exportify(file.path()).unwrap();
        assert_eq!(tracks.len(), 1);

        let track = &tracks[0];
        assert_eq!(track.title, "Test Song");
        assert_eq!(track.artist, "Test Artist");
        assert_eq!(track.album.as_deref(), Some("Test Album"));
        assert_eq!(track.spotify_id.as_deref(), Some("spotify:track:123"));
        assert_eq!(track.duration_ms, Some(180000));
    }

    #[test]
    fn test_parse_generic_valid() {
        let mut file = NamedTempFile::new().unwrap();

        writeln!(file, "Title,Artist,Album,Duration").unwrap();
        writeln!(file, "Song 1,Artist 1,Album 1,180000").unwrap();
        writeln!(file, "Song 2,Artist 2,Album 2,240000").unwrap();

        let tracks = CsvParser::parse_generic(file.path()).unwrap();
        assert_eq!(tracks.len(), 2);
        assert_eq!(tracks[0].title, "Song 1");
        assert_eq!(tracks[1].artist, "Artist 2");
    }
}
