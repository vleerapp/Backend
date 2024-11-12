use actix_web::error::ErrorInternalServerError;
use actix_web::{get, post, web, Error, HttpResponse, Responder};
use base64::Engine;
use chrono::Utc;
use futures::future::try_join_all;
use futures::future::Either;
use reqwest::header::{HeaderName as ReqwestHeaderName, CONTENT_TYPE};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH, Instant};
use tokio::fs;
use tokio::sync::{Mutex, Notify};

use crate::piped::get_selected_instance;
use crate::types::{Album, Playlist, Song};
use crate::utils::log_with_table;

pub const CACHE_FILE: &str = "./cache/search_cache.json";
pub const SEARCH_WEIGHTS_FILE: &str = "./cache/search_weights.json";
const USER_AGENT_STRING: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0.0.0 Safari/537.36";
pub const IMAGE_CACHE_DIR: &str = "./cache/images";

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
    pub search_cancel: Arc<Notify>,
}

#[derive(Deserialize)]
struct UpdateWeightRequest {
    query: String,
    selected_id: String,
}

fn get_headers() -> reqwest::header::HeaderMap {
    let mut headers = reqwest::header::HeaderMap::new();
    let now = Utc::now();
    let formatted_date = now.format("%a, %d %b %Y %H:%M:%S GMT").to_string();

    if let Ok(date_value) = reqwest::header::HeaderValue::from_str(&formatted_date) {
        headers.insert(reqwest::header::DATE, date_value);
    }

    headers.insert(
        reqwest::header::CONTENT_TYPE,
        reqwest::header::HeaderValue::from_static("application/json"),
    );
    headers.insert(
        reqwest::header::TRANSFER_ENCODING,
        reqwest::header::HeaderValue::from_static("chunked"),
    );
    headers.insert(
        reqwest::header::CONNECTION,
        reqwest::header::HeaderValue::from_static("keep-alive"),
    );
    headers.insert(
        reqwest::header::HeaderName::from_static("vary"),
        reqwest::header::HeaderValue::from_static("Accept-Encoding"),
    );
    headers.insert(
        reqwest::header::HeaderName::from_static("access-control-allow-origin"),
        reqwest::header::HeaderValue::from_static("*"),
    );
    headers.insert(
        reqwest::header::CACHE_CONTROL,
        reqwest::header::HeaderValue::from_static("public, max-age=600"),
    );
    headers.insert(
        reqwest::header::HeaderName::from_static("access-control-allow-methods"),
        reqwest::header::HeaderValue::from_static("*"),
    );
    headers.insert(
        reqwest::header::HeaderName::from_static("access-control-allow-headers"),
        reqwest::header::HeaderValue::from_static("*, Authorization"),
    );
    headers.insert(
        reqwest::header::HeaderName::from_static("strict-transport-security"),
        reqwest::header::HeaderValue::from_static("max-age=31536000; includeSubDomains; preload"),
    );
    headers.insert(
        reqwest::header::HeaderName::from_static("x-content-type-options"),
        reqwest::header::HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        reqwest::header::HeaderName::from_static("x-xss-protection"),
        reqwest::header::HeaderValue::from_static("1; mode=block"),
    );
    headers.insert(
        reqwest::header::HeaderName::from_static("referrer-policy"),
        reqwest::header::HeaderValue::from_static("same-origin"),
    );
    headers.insert(
        reqwest::header::USER_AGENT,
        reqwest::header::HeaderValue::from_static(USER_AGENT_STRING),
    );

    if let Ok(last_modified_value) = reqwest::header::HeaderValue::from_str(&formatted_date) {
        headers.insert(reqwest::header::LAST_MODIFIED, last_modified_value);
    }

    headers
}

async fn ensure_cache_dir() -> std::io::Result<()> {
    if !Path::new(IMAGE_CACHE_DIR).exists() {
        fs::create_dir_all(IMAGE_CACHE_DIR).await?;
    }
    Ok(())
}

async fn get_cached_image(url: &str) -> Option<String> {
    let hash = format!("{:x}", md5::compute(url));
    let path = format!("{}/{}", IMAGE_CACHE_DIR, hash);

    if let Ok(data) = fs::read(&path).await {
        Some(base64::engine::general_purpose::STANDARD.encode(data))
    } else {
        None
    }
}

async fn cache_image(client: &Client, url: &str) {
    let hash = format!("{:x}", md5::compute(url));
    let path = format!("{}/{}", IMAGE_CACHE_DIR, hash);

    if let Ok(response) = client.get(url).send().await {
        if let Ok(bytes) = response.bytes().await {
            let _ = fs::write(&path, bytes).await;
        }
    }
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
    cfg.service(search_route).service(update_weight_route);
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

#[get("/search")]
pub async fn search_route(
    query: web::Query<SearchQuery>,
    data: web::Data<AppState>,
    client: web::Data<Client>,
) -> Result<HttpResponse, Error> {
    let start_time = Instant::now();

    let search_cache = &data.search_cache;
    let search_cancel = &data.search_cancel;

    let instance = get_selected_instance()
        .ok_or_else(|| ErrorInternalServerError("No Piped instance selected"))?;
    let is_full_mode = query.mode.as_deref() != Some("minimal");

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
        search_cancel.notify_waiters();

        let search_future = async {
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
                            reqwest::header::HeaderValue::from_str(
                                value.to_str().unwrap_or_default(),
                            ),
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
                                let _ = log_with_table(
                                    "💥 Error: Invalid content type",
                                    vec![
                                        ("URL", url.to_string()),
                                        ("Content-Type", content_type.to_string()),
                                    ],
                                );
                                return Ok(Vec::new());
                            }

                            serde_json::from_str::<Value>(&response_text)
                                .map_err(|e| {
                                    let _ = log_with_table(
                                        "💥 Error: JSON parsing failed",
                                        vec![
                                            ("Query", query.query.clone()),
                                            ("Error", e.to_string()),
                                            ("Response", response_text.clone()),
                                        ],
                                    );
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
                                    cover: item["thumbnail"]
                                        .as_str()
                                        .unwrap_or_default()
                                        .to_string(),
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
                                            fetch_songs(&client, &instance, &id_clone, "album")
                                                .await
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
                                    cover: item["thumbnail"]
                                        .as_str()
                                        .unwrap_or_default()
                                        .to_string(),
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
                                            fetch_songs(&client, &instance, &id_clone, "playlist")
                                                .await
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
                                    cover: item["thumbnail"]
                                        .as_str()
                                        .unwrap_or_default()
                                        .to_string(),
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
                                if let Some(album) = results.albums.iter_mut().find(|a| a.id == id)
                                {
                                    album.songs = songs;
                                }
                            }
                        }
                        for (id, future) in playlist_futures {
                            if let Ok(songs) = future.await {
                                if let Some(playlist) =
                                    results.playlists.iter_mut().find(|p| p.id == id)
                                {
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

                    Ok(results)
                }
                Err(e) => {
                    let _ = log_with_table(
                        "💥 Error: Search failed",
                        vec![
                            ("Instance", instance.clone()),
                            ("Error", e.to_string()),
                        ],
                    );
                    Err(ErrorInternalServerError(format!(
                        "Failed to perform search: {}",
                        e
                    )))
                }
            }
        };

        let search_future = Box::pin(search_future);
        let cancel_future = Box::pin(search_cancel.notified());

        match futures::future::select(search_future, cancel_future).await {
            Either::Left((result, _)) => {
                results = result?;
            }
            Either::Right(_) => {
                return Ok(HttpResponse::Ok().json(json!({
                    "error": "Search cancelled",
                    "message": "A new search request was initiated"
                })));
            }
        }
    }

    let weights = if let Ok(weights_content) = fs::read_to_string(SEARCH_WEIGHTS_FILE).await {
        serde_json::from_str::<HashMap<String, HashMap<String, u32>>>(&weights_content)
            .unwrap_or_default()
    } else {
        HashMap::new()
    };
    let query_weights = weights.get(&query.query).cloned().unwrap_or_default();

    results.albums = sort_and_weight(results.albums, &query_weights);
    results.playlists = sort_and_weight(results.playlists, &query_weights);
    results.songs = sort_and_weight(results.songs, &query_weights);

    if let Some(filter) = &query.filter {
        let filtered_results = SearchResponse {
            albums: if filter == "albums" {
                results.albums
            } else {
                Vec::new()
            },
            playlists: if filter == "playlists" {
                results.playlists
            } else {
                Vec::new()
            },
            songs: if filter == "songs" {
                results.songs
            } else {
                Vec::new()
            },
        };
        results = filtered_results;
    }

    let duration = start_time.elapsed().as_millis();
    let _ = log_with_table(
        &format!("✅ Search completed {}", if is_cached { "(cached)" } else { "" }),
        vec![
            ("Query", query.query.clone()),
            ("Filter", query.filter.as_deref().unwrap_or("all").to_string()),
            ("Mode", if is_full_mode { "full" } else { "minimal" }.to_string()),
            ("Duration", format!("{} ms", duration)),
            ("Cached", is_cached.to_string()),
            ("Results", format!("Albums: {}, Playlists: {}, Songs: {}", 
                results.albums.len(),
                results.playlists.len(), 
                results.songs.len()
            )),
        ],
    );

    ensure_cache_dir().await.map_err(ErrorInternalServerError)?;

    let mut image_futures = Vec::new();

    for album in &mut results.albums {
        if let Some(cached_data) = get_cached_image(&album.cover).await {
            album.cover = format!("data:image/jpeg;base64,{}", cached_data);
        } else {
            let client = client.clone();
            let url = album.cover.clone();
            image_futures.push(tokio::spawn(async move {
                cache_image(&client, &url).await;
            }));
        }

        if let Some(cached_data) = get_cached_image(&album.artist_cover).await {
            album.artist_cover = format!("data:image/jpeg;base64,{}", cached_data);
        } else {
            let client = client.clone();
            let url = album.artist_cover.clone();
            image_futures.push(tokio::spawn(async move {
                cache_image(&client, &url).await;
            }));
        }
    }

    for playlist in &mut results.playlists {
        if let Some(cached_data) = get_cached_image(&playlist.cover).await {
            playlist.cover = format!("data:image/jpeg;base64,{}", cached_data);
        } else {
            let client = client.clone();
            let url = playlist.cover.clone();
            image_futures.push(tokio::spawn(async move {
                cache_image(&client, &url).await;
            }));
        }

        if let Some(cached_data) = get_cached_image(&playlist.artist_cover).await {
            playlist.artist_cover = format!("data:image/jpeg;base64,{}", cached_data);
        } else {
            let client = client.clone();
            let url = playlist.artist_cover.clone();
            image_futures.push(tokio::spawn(async move {
                cache_image(&client, &url).await;
            }));
        }
    }

    for song in &mut results.songs {
        if let Some(cached_data) = get_cached_image(&song.cover).await {
            song.cover = format!("data:image/jpeg;base64,{}", cached_data);
        } else {
            let client = client.clone();
            let url = song.cover.clone();
            image_futures.push(tokio::spawn(async move {
                cache_image(&client, &url).await;
            }));
        }

        if let Some(cached_data) = get_cached_image(&song.artist_cover).await {
            song.artist_cover = format!("data:image/jpeg;base64,{}", cached_data);
        } else {
            let client = client.clone();
            let url = song.artist_cover.clone();
            image_futures.push(tokio::spawn(async move {
                cache_image(&client, &url).await;
            }));
        }
    }

    tokio::spawn(async move {
        futures::future::join_all(image_futures).await;
    });

    Ok(HttpResponse::Ok().json(results))
}
