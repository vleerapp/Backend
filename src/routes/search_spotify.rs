use actix_web::{get, web, HttpResponse, Responder};
use chrono::{DateTime, Duration, Utc};
use regex::Regex;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::utils::log_with_table;

const CACHE_FILE: &str = "./cache/spotify_search_cache.json";

#[derive(Clone, Debug, Deserialize, Serialize)]
struct SpotifyAlbum {
    images: Vec<SpotifyImage>,
    name: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct SpotifyArtist {
    name: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct SpotifyAuth {
    access_token: Option<String>,
    client_id: Option<String>,
    token_expiration_time: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct SpotifyImage {
    url: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct SpotifySession {
    access_token: String,
    client_id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct SpotifyTrack {
    album: SpotifyAlbum,
    artists: Vec<SpotifyArtist>,
    duration_ms: u32,
    id: String,
    name: String,
    preview_url: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct MinifiedTrack {
    artist: String,
    duration: u32,
    id: String,
    thumbnail_url: String,
    title: String,
}

struct AppState {
    auth: Arc<Mutex<SpotifyAuth>>,
    client: Client,
    search_cache: Arc<Mutex<HashMap<String, HashMap<String, MinifiedTrack>>>>,
}

#[get("/search-spotify")]
async fn search_spotify_route(
    query: web::Query<HashMap<String, String>>,
    data: web::Data<AppState>,
) -> impl Responder {
    let query = match query.get("query") {
        Some(q) => q,
        None => {
            let _ = log_with_table(
                "💥 Invalid request",
                vec![("Error", "Missing search query parameter".to_string())],
            );
            return HttpResponse::BadRequest().json(json!({"error": "Search query is required"}));
        }
    };

    let start_time = Utc::now();

    {
        let search_cache = data.search_cache.lock().await;
        if let Some(cached_results) = search_cache.get(query) {
            let duration = Utc::now().signed_duration_since(start_time);
            let _ = log_with_table(
                "✅ Spotify search completed",
                vec![
                    ("Query", query.to_string()),
                    ("Duration", format!("{} ms", duration.num_milliseconds())),
                    ("Cache", "Hit".to_string()),
                    ("Results", format!("{} tracks", cached_results.len())),
                ],
            );
            return HttpResponse::Ok().json(cached_results);
        }
    }

    if !auth(&data).await {
        let _ = log_with_table(
            "💥 Spotify authentication failed",
            vec![
                ("Query", query.to_string()),
                ("Error", "Failed to authenticate with Spotify API".to_string()),
            ],
        );
        return HttpResponse::InternalServerError()
            .json(json!({"error": "Failed to authenticate"}));
    }

    match fetch_spotify_results(query, &data).await {
        Ok(minified_results) => {
            let mut search_cache = data.search_cache.lock().await;
            search_cache.insert(query.to_string(), minified_results.clone());
            save_cache(&search_cache);

            let duration = Utc::now().signed_duration_since(start_time);
            let _ = log_with_table(
                "✅ Spotify search completed",
                vec![
                    ("Query", query.to_string()),
                    ("Duration", format!("{} ms", duration.num_milliseconds())),
                    ("Cache", "Miss".to_string()),
                    ("Results", format!("{} tracks", minified_results.len())),
                ],
            );

            HttpResponse::Ok().json(minified_results)
        }
        Err(e) => {
            let _ = log_with_table(
                "💥 Spotify search error",
                vec![
                    ("Query", query.to_string()),
                    ("Error", format!("API request failed: {}", e).to_string()),
                ],
            );
            HttpResponse::InternalServerError()
                .json(json!({"error": "Failed to fetch search results"}))
        }
    }
}

async fn auth(data: &web::Data<AppState>) -> bool {
    let mut auth = data.auth.lock().await;
    if let (Some(_token), Some(expiration)) =
        (auth.access_token.as_ref(), auth.token_expiration_time)
    {
        if Utc::now() < expiration {
            return true;
        }
    }

    let re = Regex::new(
        r#"<script id="session" data-testid="session" type="application/json">({.*?})</script>"#,
    )
    .unwrap();
    let response = match data
        .client
        .get("https://open.spotify.com/search")
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            let _ = log_with_table(
                "💥 Authentication error",
                vec![(
                    "Error",
                    format!("Failed to fetch Spotify session: {}", e).to_string(),
                )],
            );
            return false;
        }
    };

    let body = match response.text().await {
        Ok(body) => body,
        Err(e) => {
            let _ = log_with_table(
                "💥 Authentication error",
                vec![(
                    "Error",
                    format!("Failed to read session response: {}", e).to_string(),
                )],
            );
            return false;
        }
    };

    if let Some(captures) = re.captures(&body) {
        if let Ok(session) = serde_json::from_str::<SpotifySession>(&captures[1]) {
            auth.access_token = Some(session.access_token);
            auth.client_id = Some(session.client_id);
            auth.token_expiration_time = Some(Utc::now() + Duration::hours(1));
            return true;
        }
    }

    let _ = log_with_table(
        "💥 Authentication error",
        vec![("Error", "Failed to parse Spotify session data".to_string())],
    );
    false
}

async fn fetch_spotify_results(
    query: &str,
    data: &web::Data<AppState>,
) -> Result<HashMap<String, MinifiedTrack>, Box<dyn std::error::Error>> {
    let auth = data.auth.lock().await;
    let url = format!(
        "https://api.spotify.com/v1/search?q={}&type=track",
        urlencoding::encode(query)
    );
    let response = data
        .client
        .get(&url)
        .header(
            "Authorization",
            format!("Bearer {}", auth.access_token.as_ref().unwrap()),
        )
        .send()
        .await?;

    let spotify_data: serde_json::Value = response.json().await?;
    let tracks = spotify_data["tracks"]["items"]
        .as_array()
        .ok_or("No tracks found")?;

    let mut minified_results = HashMap::new();
    for track in tracks {
        let track: SpotifyTrack = serde_json::from_value(track.clone())?;
        let minified = MinifiedTrack {
            artist: track
                .artists
                .first()
                .map(|a| a.name.clone())
                .unwrap_or_default(),
            duration: track.duration_ms / 1000,
            id: track.id,
            thumbnail_url: track
                .album
                .images
                .first()
                .map(|i| i.url.clone())
                .unwrap_or_default(),
            title: track.name,
        };
        minified_results.insert(minified.id.clone(), minified);
    }

    Ok(minified_results)
}

fn save_cache(cache: &HashMap<String, HashMap<String, MinifiedTrack>>) {
    if let Ok(json) = serde_json::to_string(cache) {
        let _ = fs::write(CACHE_FILE, json);
    }
}
