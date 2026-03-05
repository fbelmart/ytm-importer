use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

use crate::Track;

/// Search result from YouTube Music
#[derive(Debug, Clone)]
pub struct YouTubeTrack {
    pub video_id: String,
    pub title: String,
    pub artists: Vec<String>,
    pub duration_seconds: Option<u32>,
    pub published_at: Option<String>,
}

/// Match result with confidence score
#[derive(Debug, Clone)]
pub struct MatchResult {
    pub original_track: Track,
    pub matched_track: Option<YouTubeTrack>,
    pub confidence: f64,
    pub search_queries_used: Vec<String>,
}

/// Track matcher with configurable parameters
pub struct TrackMatcher {
    api_client: Client,
    min_confidence: f64,
    max_retries: u32,
    cache: HashMap<String, Vec<YouTubeTrack>>, // Cache search results
}

impl TrackMatcher {
    /// Create a new track matcher
    pub fn new(api_client: Client, min_confidence: f64, max_retries: u32) -> Self {
        Self {
            api_client,
            min_confidence,
            max_retries,
            cache: HashMap::new(),
        }
    }

    /// Search for a track on YouTube Music
    pub async fn search_track(&mut self, track: &Track) -> Result<MatchResult> {
        let mut search_queries = Vec::new();

        // Build search queries in order of specificity
        let queries = self.build_search_queries(track);

        for (i, query) in queries.iter().enumerate() {
            search_queries.push(query.clone());

            // Check cache first
            let results = if let Some(cached) = self.cache.get(query) {
                cached.clone()
            } else {
                // Perform search
                let results = self.perform_search(query).await?;
                self.cache.insert(query.clone(), results.clone());
                results
            };

            if results.is_empty() {
                continue;
            }

            // Score each result
            let mut scored_results: Vec<(f64, &YouTubeTrack)> = results
                .iter()
                .map(|result| (self.calculate_score(result, track, i), result))
                .collect();

            // Sort by score (highest first)
            scored_results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

            // After scoring results, add this debug output
            if i == 0 { // Only for first query attempt
                for (score, result) in &scored_results {
                    println!("    Score {:.2}: {} - {:?}", score, result.title, result.artists);
                }
            }

            if let Some((score, best_match)) = scored_results.first() {
                if *score >= self.min_confidence {
                    return Ok(MatchResult {
                        original_track: track.clone(),
                        matched_track: Some((*best_match).clone()),
                        confidence: *score,
                        search_queries_used: search_queries,
                    });
                }
            }
        }

        // No match found with sufficient confidence
        Ok(MatchResult {
            original_track: track.clone(),
            matched_track: None,
            confidence: 0.0,
            search_queries_used: search_queries,
        })
    }

    /// Build search queries in order of specificity
    fn build_search_queries(&self, track: &Track) -> Vec<String> {
        let mut queries = Vec::new();

        // Clean the title and artist
        let clean_title = self.clean_string(&track.title);
        let clean_artist = self.clean_string(&track.artist);

        // 1. Most specific: Artist + Title (with album if available)
        if let Some(album) = &track.album {
            queries.push(format!("{} {} {}", clean_artist, clean_title, album));
        }

        // 2. Artist + Title
        queries.push(format!("{} {}", clean_artist, clean_title));

        // 3. Title only (for well-known songs)
        queries.push(clean_title.clone());

        // 4. Handle multiple artists
        if track.artist.contains(';') || track.artist.contains(',') || track.artist.contains('&') {
            let primary_artist = track.artist
                .split([';', ',', '&'])
                .next()
                .unwrap_or(&track.artist)
                .trim();
            queries.push(format!("{} {}", primary_artist, clean_title));
        }

        // Remove duplicates while preserving order
        let mut seen = std::collections::HashSet::new();
        queries.into_iter()
            .filter(|q| seen.insert(q.clone()))
            .collect()
    }

    /// Perform actual search on YouTube Music
    async fn perform_search(&self, query: &str) -> Result<Vec<YouTubeTrack>> {
        // CORRECT YouTube Music InnerTube API endpoint
        let url = "https://music.youtube.com/youtubei/v1/search";

        // Build the request body with proper InnerTube format
        let body = serde_json::json!({
            "context": {
                "client": {
                    "clientName": "WEB_REMIX",
                    "clientVersion": "1.20240801.01.00",
                    "hl": "en",
                    "gl": "US",
                    "utcOffsetMinutes": 0
                }
            },
            "query": query,
            "params": "EgWKAQIIAWoKEAoQCRADEAQQBQ%3D%3D"
        });

        println!("🔍 Searching for: '{}'", query);

        // Make the request with proper YouTube Music headers
        let response = self.api_client
            .post(url)
            .header("Content-Type", "application/json")
            .header("User-Agent", "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .header("Origin", "https://music.youtube.com")
            .header("Referer", "https://music.youtube.com")
            .query(&[("key", "AIzaSyA8eiZmM1FaDVjRy-df2KTyQ_vz_yYM39w")]) // Add API key as query param
            .json(&body)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .with_context(|| format!("Failed to search YouTube Music for: '{}'", query))?;

        // Check if the request was successful
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            println!("⚠️  Search failed with status: {} - {}", status, error_text);
            return Ok(Vec::new());
        }

        // Parse the JSON response
        let json: serde_json::Value = response.json().await
            .with_context(|| format!("Failed to parse YouTube Music response for query: '{}'", query))?;

        // For debugging - save first response
        static QUERY_COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let count = QUERY_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if count == 0 {
            if let Ok(json_str) = serde_json::to_string_pretty(&json) {
                let _ = std::fs::write("debug_response.json", json_str);
                println!("📝 Saved debug response to: debug_response.json");
            }
        }

        let mut results = Vec::new();

        // Parse the response - YouTube Music returns results in a specific structure
        if let Some(contents) = json["contents"]["tabbedSearchResultsRenderer"]["tabs"][0]
            ["tabRenderer"]["content"]["sectionListRenderer"]["contents"]
            .as_array()
        {
            for section in contents {
                if let Some(music_shelf) = section.get("musicShelfRenderer") {
                    if let Some(items) = music_shelf["contents"].as_array() {
                        for item in items {
                            if let Some(renderer) = item.get("musicResponsiveListItemRenderer") {
                                // Extract video ID
                                let video_id = renderer["overlay"]
                                    ["musicItemThumbnailOverlayRenderer"]["content"]
                                    ["musicPlayButtonRenderer"]["playNavigationEndpoint"]
                                    ["watchEndpoint"]["videoId"]
                                    .as_str()
                                    .unwrap_or("")
                                    .to_string();

                                if video_id.is_empty() {
                                    continue;
                                }

                                // Extract title
                                let title = renderer["flexColumns"][0]
                                    ["musicResponsiveListItemFlexColumnRenderer"]["text"]
                                    ["runs"][0]["text"]
                                    .as_str()
                                    .unwrap_or("")
                                    .to_string();

                                // Extract artists
                                let mut artists = Vec::new();
                                if let Some(runs) = renderer["flexColumns"][1]
                                    ["musicResponsiveListItemFlexColumnRenderer"]["text"]["runs"]
                                    .as_array()
                                {
                                    for run in runs {
                                        if let Some(text) = run["text"].as_str() {
                                            if text != " • " && text != ", " && !text.is_empty() && text != " & " {
                                                artists.push(text.to_string());
                                            }
                                        }
                                    }
                                }

                                results.push(YouTubeTrack {
                                    video_id,
                                    title,
                                    artists,
                                    duration_seconds: None,
                                    published_at: None,
                                });
                            }
                        }
                    }
                }
            }
        }

        println!("  Found {} results for '{}'", results.len(), query);
        Ok(results)
    }

    /// Calculate match score between search result and original track
    fn calculate_score(&self, result: &YouTubeTrack, original: &Track, query_depth: usize) -> f64 {
        let mut score = 0.0;

        // Title similarity (weight: 0.5)
        let title_sim = self.string_similarity(&result.title, &original.title);
        score += title_sim * 0.5;

        // Artist similarity (weight: 0.3)
        let result_artists = result.artists.join(" ");
        let artist_sim = self.string_similarity(&result_artists, &original.artist);
        score += artist_sim * 0.3;

        // Duration bonus if available (weight: 0.2)
        if let (Some(result_duration), Some(original_duration)) = (result.duration_seconds, original.duration_ms) {
            let original_seconds = (original_duration / 1000) as u32;
            let duration_diff = (result_duration as i32 - original_seconds as i32).abs();
            if duration_diff < 5 {
                score += 0.2; // Perfect match
            } else if duration_diff < 10 {
                score += 0.15; // Very close
            } else if duration_diff < 30 {
                score += 0.1; // Acceptable
            }
        }

        // Penalize deeper queries (they're less likely to be correct)
        let depth_penalty = (query_depth as f64) * 0.05;
        score = (score - depth_penalty).max(0.0);

        score.min(1.0)
    }

    /// Calculate string similarity (simple implementation)
    fn string_similarity(&self, a: &str, b: &str) -> f64 {
        let a = a.to_lowercase();
        let b = b.to_lowercase();

        // Simple word matching for now
        let a_words: Vec<&str> = a.split_whitespace().collect();
        let b_words: Vec<&str> = b.split_whitespace().collect();

        if a_words.is_empty() || b_words.is_empty() {
            return 0.0;
        }

        let matches = a_words.iter()
            .filter(|&word| b_words.contains(word))
            .count();

        matches as f64 / a_words.len().max(b_words.len()) as f64
    }

    /// Clean string for search
    fn clean_string(&self, s: &str) -> String {
        s.split('(')
            .next()
            .unwrap_or(s)
            .split('[')
            .next()
            .unwrap_or(s)
            .split(" - ")
            .next()
            .unwrap_or(s)
            .trim()
            .to_string()
    }

    /// Clear the search cache
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    /// Get cache size
    pub fn cache_size(&self) -> usize {
        self.cache.len()
    }
}

/// Statistics about the matching process
#[derive(Debug, Default, Serialize)]
pub struct MatchingStats {
    pub total_tracks: usize,
    pub matched_tracks: usize,
    pub unmatched_tracks: usize,
    pub average_confidence: f64,
    pub search_queries_per_track: f64,
    pub cache_hits: usize,
    pub cache_misses: usize,
}

impl MatchingStats {
    pub fn update(&mut self, result: &MatchResult, cache_hit: bool) {
        self.total_tracks += 1;

        if result.matched_track.is_some() {
            self.matched_tracks += 1;
            self.average_confidence = (self.average_confidence * (self.matched_tracks - 1) as f64 + result.confidence) / self.matched_tracks as f64;
        } else {
            self.unmatched_tracks += 1;
        }

        self.search_queries_per_track = (self.search_queries_per_track * (self.total_tracks - 1) as f64 + result.search_queries_used.len() as f64) / self.total_tracks as f64;

        if cache_hit {
            self.cache_hits += 1;
        } else {
            self.cache_misses += 1;
        }
    }

    pub fn print_summary(&self) {
        println!("\n📊 Matching Statistics:");
        println!("  Total tracks processed: {}", self.total_tracks);
        println!("  Matched tracks: {} ({:.1}%)",
            self.matched_tracks,
            (self.matched_tracks as f64 / self.total_tracks as f64) * 100.0
        );
        println!("  Unmatched tracks: {} ({:.1}%)",
            self.unmatched_tracks,
            (self.unmatched_tracks as f64 / self.total_tracks as f64) * 100.0
        );
        println!("  Average confidence: {:.2}", self.average_confidence);
        println!("  Avg search queries/track: {:.2}", self.search_queries_per_track);
        println!("  Cache hits: {} | Cache misses: {}", self.cache_hits, self.cache_misses);
    }
}
