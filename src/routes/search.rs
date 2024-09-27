use std::collections::HashMap;
use std::fs;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use actix_web::error::ErrorInternalServerError;
use actix_web::http::header::{self, HeaderMap, HeaderName, HeaderValue};
use actix_web::{get, post, web, Error, HttpResponse, Responder};
use chrono::Utc;
use futures::future::try_join_all;
use reqwest::header::{HeaderName as ReqwestHeaderName, CONTENT_TYPE};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::Mutex;

use crate::piped::get_selected_instance;
use crate::types::{Album, Playlist, Song};
use crate::utils::log;

pub const CACHE_FILE: &str = "./cache/search_cache.json";
pub const SEARCH_WEIGHTS_FILE: &str = "./cache/search_weights.json";
const USER_AGENT_STRING: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0.0.0 Safari/537.36";

#[derive(Deserialize)]
struct SearchQuery {
    filter: Option<String>,
    mode: Option<String>,
    query: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct SearchResponse {
    albums: Vec<Album>,
    playlists: Vec<Playlist>,
    songs: Vec<Song>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SearchCacheItem {
    results: SearchResponse,
    timestamp: i64,
}

pub struct AppState {
    pub search_cache: Arc<Mutex<HashMap<String, SearchCacheItem>>>,
    pub search_weights: Arc<Mutex<HashMap<String, HashMap<String, u32>>>>,
}

fn get_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(header::ACCEPT, HeaderValue::from_static("text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7"));
    headers.insert(
        header::ACCEPT_LANGUAGE,
        HeaderValue::from_static("en,de;q=0.9,de-CH;q=0.8"),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("max-age=0"));
    headers.insert(
        HeaderName::from_static("dnt"),
        HeaderValue::from_static("1"),
    );
    headers.insert(
        header::IF_MODIFIED_SINCE,
        HeaderValue::from_str(&Utc::now().format("%a, %d %b %Y %H:%M:%S GMT").to_string()).unwrap(),
    );
    headers.insert(
        HeaderName::from_static("priority"),
        HeaderValue::from_static("u=0, i"),
    );
    headers.insert(
        HeaderName::from_static("sec-ch-ua"),
        HeaderValue::from_static("\"Chromium\";v=\"129\", \"Not=A?Brand\";v=\"8\""),
    );
    headers.insert(
        HeaderName::from_static("sec-ch-ua-mobile"),
        HeaderValue::from_static("?0"),
    );
    headers.insert(
        HeaderName::from_static("sec-ch-ua-platform"),
        HeaderValue::from_static("\"macOS\""),
    );
    headers.insert(
        HeaderName::from_static("sec-fetch-dest"),
        HeaderValue::from_static("document"),
    );
    headers.insert(
        HeaderName::from_static("sec-fetch-mode"),
        HeaderValue::from_static("navigate"),
    );
    headers.insert(
        HeaderName::from_static("sec-fetch-site"),
        HeaderValue::from_static("none"),
    );
    headers.insert(
        HeaderName::from_static("sec-fetch-user"),
        HeaderValue::from_static("?1"),
    );
    headers.insert(
        HeaderName::from_static("sec-gpc"),
        HeaderValue::from_static("1"),
    );
    headers.insert(
        HeaderName::from_static("upgrade-insecure-requests"),
        HeaderValue::from_static("1"),
    );
    headers.insert(
        header::USER_AGENT,
        HeaderValue::from_static(USER_AGENT_STRING),
    );
    headers
}

#[get("/search")]
pub async fn search_route(
    query: web::Query<SearchQuery>,
    data: web::Data<AppState>,
    client: web::Data<Client>,
) -> Result<HttpResponse, Error> {
    let search_cache = &data.search_cache;

    let instance = get_selected_instance().ok_or_else(|| {
        ErrorInternalServerError("No Piped instance selected")
    })?;
    let is_full_mode = query.mode.as_deref() != Some("minimal");
    let start_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();

    let filters: HashMap<&str, &str> = [
        ("albums", "music_albums"),
        ("playlists", "music_playlists"),
        ("songs", "music_songs"),
    ]
    .iter()
    .cloned()
    .collect();

    let filters_to_search: Vec<&str> = match &query.filter {
        Some(f) => vec![f.as_str()],
        None => vec!["albums", "playlists", "songs"],
    };

    let mut results = SearchResponse {
        albums: Vec::new(),
        playlists: Vec::new(),
        songs: Vec::new(),
    };

    let mut is_cached = false;
    {
        let cache = search_cache.lock().await;
        if let Some(cached_item) = cache.get(&query.query) {
            if filters_to_search.iter().all(|&f| match f {
                "albums" => !cached_item.results.albums.is_empty(),
                "playlists" => !cached_item.results.playlists.is_empty(),
                "songs" => !cached_item.results.songs.is_empty(),
                _ => false,
            }) {
                results = cached_item.results.clone();
                is_cached = true;
            }
        }
    }

    if !is_cached {
        let mut album_futures: Vec<(
            String,
            Pin<Box<dyn Future<Output = Result<Vec<Song>, Error>> + Send>>,
        )> = Vec::new();
        let mut playlist_futures: Vec<(
            String,
            Pin<Box<dyn Future<Output = Result<Vec<Song>, Error>> + Send>>,
        )> = Vec::new();
        let mut avatar_futures: Vec<(String, String)> = Vec::new();

        let headers = get_headers();

        let search_futures: Vec<_> = filters_to_search
            .iter()
            .map(|&f| {
                let url = format!("{}/search", instance);
                let mut request = client.get(&url);

                for (key, value) in headers.iter() {
                    if let (Ok(header_name), Ok(header_value)) = (
                        ReqwestHeaderName::from_bytes(key.as_ref()),
                        reqwest::header::HeaderValue::from_str(value.to_str().unwrap_or_default()),
                    ) {
                        request = request.header(header_name, header_value);
                    }
                }

                request
                    .query(&[
                        ("_internalType", f),
                        ("filter", filters[f]),
                        ("q", &query.query),
                    ])
                    .send()
            })
            .collect();

        match try_join_all(search_futures).await {
            Ok(responses) => {
                let raw_results: Vec<Value> =
                    try_join_all(responses.into_iter().map(|response| async {
                        let internal_type = response
                            .url()
                            .query_pairs()
                            .find(|(k, _)| k == "_internalType")
                            .map(|(_, v)| v.to_string());
                        let content_type = response
                            .headers()
                            .get(CONTENT_TYPE)
                            .and_then(|v| v.to_str().ok())
                            .unwrap_or("")
                            .to_string();
                        let url = response.url().clone();
                        let response_text = response
                            .text()
                            .await
                            .map_err(|e| ErrorInternalServerError(e.to_string()))?;

                        if !content_type.starts_with("application/json") {
                            log(&format!(
                                "💥 Unexpected content type for \"{}\": {}. Response: {}",
                                url, content_type, response_text
                            ));
                            return Ok(Vec::new());
                        }

                        serde_json::from_str::<Value>(&response_text)
                            .map_err(|e| {
                                log(&format!(
                                    "💥 JSON parsing error for \"{}\": {}. Response: {}",
                                    query.query, e, response_text
                                ));
                                ErrorInternalServerError(
                                    "An error occurred while parsing the search results",
                                )
                            })
                            .and_then(|json| {
                                json["items"]
                                    .as_array()
                                    .map(|items| {
                                        items
                                            .iter()
                                            .map(|item| {
                                                let mut item = item.clone();
                                                if let Some(t) = &internal_type {
                                                    item["_internalType"] = json!(t);
                                                }
                                                item
                                            })
                                            .collect::<Vec<_>>()
                                    })
                                    .ok_or_else(|| {
                                        ErrorInternalServerError("Invalid response structure")
                                    })
                            })
                    }))
                    .await?
                    .into_iter()
                    .flatten()
                    .collect();

                for item in raw_results {
                    let id = extract_id(&item["url"].as_str().unwrap_or_default());
                    if id.is_empty() {
                        continue;
                    }

                    match item["_internalType"].as_str() {
                        Some("albums") => {
                            let album = Album {
                                artist: item["uploaderName"]
                                    .as_str()
                                    .unwrap_or_default()
                                    .to_string(),
                                artist_cover: String::new(),
                                cover: item["thumbnail"].as_str().unwrap_or_default().to_string(),
                                id: id.clone(),
                                name: item["name"].as_str().unwrap_or_default().to_string(),
                                songs: Vec::new(),
                            };
                            results.albums.push(album);
                            if is_full_mode {
                                let client = client.clone();
                                let instance = instance.clone();
                                let id_clone = id.clone();
                                album_futures.push((
                                    id.clone(),
                                    Box::pin(async move {
                                        fetch_songs(&client, &instance, &id_clone, "album").await
                                    }),
                                ));
                            }
                            avatar_futures.push((
                                id.clone(),
                                item["uploaderUrl"].as_str().unwrap_or_default().to_string(),
                            ));
                        }
                        Some("playlists") => {
                            let playlist = Playlist {
                                artist: item["uploaderName"]
                                    .as_str()
                                    .unwrap_or_default()
                                    .to_string(),
                                artist_cover: item["artistCover"]
                                    .as_str()
                                    .unwrap_or_default()
                                    .to_string(),
                                cover: item["thumbnail"].as_str().unwrap_or_default().to_string(),
                                id: id.clone(),
                                name: item["name"].as_str().unwrap_or_default().to_string(),
                                songs: Vec::new(),
                            };
                            results.playlists.push(playlist);
                            if is_full_mode {
                                let client = client.clone();
                                let instance = instance.clone();
                                let id_clone = id.clone();
                                playlist_futures.push((
                                    id.clone(),
                                    Box::pin(async move {
                                        fetch_songs(&client, &instance, &id_clone, "playlist").await
                                    }),
                                ));
                            }
                        }
                        Some("songs") => {
                            let song = Song {
                                album: String::new(),
                                artist: item["uploaderName"]
                                    .as_str()
                                    .unwrap_or_default()
                                    .to_string(),
                                artist_cover: item["artistCover"]
                                    .as_str()
                                    .unwrap_or_default()
                                    .to_string(),
                                cover: item["thumbnail"].as_str().unwrap_or_default().to_string(),
                                duration: item["duration"].as_i64().unwrap_or_default() as i32,
                                id: id.clone(),
                                title: item["title"].as_str().unwrap_or_default().to_string(),
                            };
                            results.songs.push(song);
                        }
                        _ => {}
                    }
                }

                if is_full_mode {
                    for (id, future) in album_futures {
                        if let Ok(songs) = future.await {
                            if let Some(album) = results.albums.iter_mut().find(|a| a.id == id) {
                                album.songs = songs;
                            }
                        }
                    }
                    for (id, future) in playlist_futures {
                        if let Ok(songs) = future.await {
                            if let Some(playlist) = results.playlists.iter_mut().find(|p| p.id == id) {
                                playlist.songs = songs;
                            }
                        }
                    }
                }

                for (id, uploader_url) in avatar_futures {
                    if let Ok(avatar_url) =
                        fetch_avatar_url(&client, &instance, &uploader_url).await
                    {
                        if let Some(album) = results.albums.iter_mut().find(|a| a.id == id) {
                            album.artist_cover = avatar_url;
                        }
                    }
                }

                // Update file cache
                let cache_item = SearchCacheItem {
                    results: results.clone(),
                    timestamp: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs() as i64,
                };

                let mut cache = search_cache.lock().await;
                cache.insert(query.query.clone(), cache_item);
                if let Ok(json) = serde_json::to_string(&*cache) {
                    let _ = fs::write(CACHE_FILE, json);
                }
            }
            Err(e) => {
                log(&format!("💥 Search error for instance {}: {}", instance, e));
                return Ok(HttpResponse::InternalServerError().json(json!({
                    "error": "Failed to perform search",
                    "message": e.to_string()
                })));
            }
        }
    }

    // Get the weights for the current query
    let weights = if let Ok(weights_content) = fs::read_to_string(SEARCH_WEIGHTS_FILE) {
        serde_json::from_str::<HashMap<String, HashMap<String, u32>>>(&weights_content)
            .unwrap_or_default()
    } else {
        HashMap::new()
    };
    let query_weights = weights.get(&query.query).cloned().unwrap_or_default();

    // Sort and weight the results
    results.albums = sort_and_weight(results.albums, &query_weights);
    results.playlists = sort_and_weight(results.playlists, &query_weights);
    results.songs = sort_and_weight(results.songs, &query_weights);

    if let Some(filter) = &query.filter {
        let filtered_results = SearchResponse {
            albums: if filter == "albums" { results.albums } else { Vec::new() },
            playlists: if filter == "playlists" { results.playlists } else { Vec::new() },
            songs: if filter == "songs" { results.songs } else { Vec::new() },
        };
        results = filtered_results;
    }

    let end_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let duration = end_time - start_time;

    if is_cached {
        log(&format!(
            "✅ Search (cached): \"{}\" | Filter: {} | Mode: {} | Duration: {} ms",
            query.query,
            query.filter.as_deref().unwrap_or("all"),
            if is_full_mode { "full" } else { "minimal" },
            duration
        ));
    } else {
        log(&format!(
            "✅ Search: \"{}\" | Filters: {} | Mode: {} | Duration: {} ms",
            query.query,
            filters_to_search.join(", "),
            if is_full_mode { "full" } else { "minimal" },
            duration
        ));
    }

    Ok(HttpResponse::Ok().json(results))
}

fn extract_id(url: &str) -> String {
    if url.contains("list=") {
        url.split("list=").nth(1).unwrap_or_default().to_string()
    } else if url.contains("v=") {
        url.split("v=").nth(1).unwrap_or_default().to_string()
    } else {
        url.split('/').last().unwrap_or_default().to_string()
    }
}

async fn fetch_avatar_url(
    client: &Client,
    instance: &str,
    uploader_url: &str,
) -> Result<String, Error> {
    if let Some(channel_id) = uploader_url.split('/').last() {
        let headers = get_headers();
        let mut request = client.get(&format!("{}/channel/{}", instance, channel_id));

        for (key, value) in headers.iter() {
            if let (Ok(header_name), Ok(header_value)) = (
                ReqwestHeaderName::from_bytes(key.as_ref()),
                reqwest::header::HeaderValue::from_str(value.to_str().unwrap_or_default()),
            ) {
                request = request.header(header_name, header_value);
            }
        }

        let response = request
            .send()
            .await
            .map_err(|e| ErrorInternalServerError(e.to_string()))?;
        let data: Value = response
            .json()
            .await
            .map_err(|e| ErrorInternalServerError(e.to_string()))?;
        Ok(data["avatarUrl"].as_str().unwrap_or_default().to_string())
    } else {
        Ok(String::new())
    }
}

async fn fetch_songs(
    client: &Client,
    instance: &str,
    id: &str,
    type_: &str,
) -> Result<Vec<Song>, Error> {
    let headers = get_headers();
    let mut request = client.get(&format!("{}/playlists/{}", instance, id));

    for (key, value) in headers.iter() {
        if let (Ok(header_name), Ok(header_value)) = (
            ReqwestHeaderName::from_bytes(key.as_ref()),
            reqwest::header::HeaderValue::from_str(value.to_str().unwrap_or_default()),
        ) {
            request = request.header(header_name, header_value);
        }
    }

    let response = request
        .send()
        .await
        .map_err(|e| ErrorInternalServerError(e.to_string()))?;
    let data: Value = response
        .json()
        .await
        .map_err(|e| ErrorInternalServerError(e.to_string()))?;

    if let Some(related_streams) = data["relatedStreams"].as_array() {
        let songs: Vec<Song> = related_streams
            .iter()
            .map(|stream| Song {
                album: if type_ == "album" {
                    data["name"].as_str().unwrap_or_default().to_string()
                } else {
                    String::new()
                },
                artist: stream["uploaderName"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                artist_cover: String::new(),
                cover: stream["thumbnail"].as_str().unwrap_or_default().to_string(),
                duration: stream["duration"].as_i64().unwrap_or_default() as i32,
                id: extract_id(stream["url"].as_str().unwrap_or_default()),
                title: stream["title"].as_str().unwrap_or_default().to_string(),
            })
            .collect();

        Ok(songs)
    } else {
        Ok(Vec::new())
    }
}

fn sort_and_weight<T: Clone + Serialize>(
    mut items: Vec<T>,
    weights: &HashMap<String, u32>,
) -> Vec<T> {
    items.sort_by(|a, b| {
        let a_value = serde_json::to_value(a).unwrap();
        let b_value = serde_json::to_value(b).unwrap();
        let a_id = a_value["id"].as_str().unwrap_or("");
        let b_id = b_value["id"].as_str().unwrap_or("");
        let a_weight = weights.get(a_id).cloned().unwrap_or(0);
        let b_weight = weights.get(b_id).cloned().unwrap_or(0);
        b_weight.cmp(&a_weight).then_with(|| a_id.cmp(b_id))
    });
    items
}

#[derive(Deserialize)]
struct UpdateWeightRequest {
    query: String,
    selected_id: String,
}

#[post("/search/update-weight")]
pub async fn update_weight_route(
    req: web::Query<UpdateWeightRequest>,
    data: web::Data<AppState>,
) -> impl Responder {
    let search_weights = &data.search_weights;
    let mut weights = search_weights.lock().await;
    let query_weights = weights
        .entry(req.query.clone())
        .or_insert_with(HashMap::new);
    *query_weights.entry(req.selected_id.clone()).or_insert(0) += 1;

    if let Ok(json) = serde_json::to_string(&*weights) {
        let _ = fs::write(SEARCH_WEIGHTS_FILE, json);
    }

    HttpResponse::Ok().json(json!({"success": true}))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(search_route)
       .service(update_weight_route);
}