use crate::routes::search::{AppState, SearchCacheItem, CACHE_FILE, SEARCH_WEIGHTS_FILE};
use actix_cors::Cors;
use actix_web::middleware::{NormalizePath, TrailingSlash};
use actix_web::{http::header, middleware, web, App, HttpServer};
use piped::select_best_piped_instance;
use reqwest::Client;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, Notify};
use utils::clear_log;

mod piped;
mod routes;
mod types;
mod utils;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    clear_log();

    let search_cache: Arc<Mutex<HashMap<String, SearchCacheItem>>> = Arc::new(Mutex::new(
        std::fs::read_to_string(CACHE_FILE)
            .map(|content| serde_json::from_str(&content).unwrap_or_default())
            .unwrap_or_default(),
    ));

    let search_weights: Arc<Mutex<HashMap<String, HashMap<String, u32>>>> = Arc::new(Mutex::new(
        std::fs::read_to_string(SEARCH_WEIGHTS_FILE)
            .map(|content| serde_json::from_str(&content).unwrap_or_default())
            .unwrap_or_default(),
    ));

    let search_cancel = Arc::new(Notify::new());

    let client = Client::new();

    let app_state = web::Data::new(AppState {
        search_cache: search_cache.clone(),
        search_weights: search_weights.clone(),
        search_cancel: search_cancel.clone(),
    });

    let server = HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allowed_methods(vec!["GET", "POST"])
            .allowed_headers(vec![header::AUTHORIZATION, header::ACCEPT])
            .allowed_header(header::CONTENT_TYPE)
            .max_age(3600);

        App::new()
            .app_data(app_state.clone())
            .app_data(web::Data::new(client.clone()))
            .wrap(cors)
            .wrap(middleware::Logger::default())
            .wrap(NormalizePath::new(TrailingSlash::Trim))
            .wrap(middleware::Compress::default())
            .service(routes::download::download_route)
            .service(routes::index::index_route)
            .service(routes::search_spotify::search_spotify_route)
            .service(routes::stream::stream_route)
            .service(routes::thumbnail::thumbnail_route)
            .service(routes::instance::instances_route)
            .configure(routes::search::config)
    })
    .bind(("0.0.0.0", 3001))?;

    select_best_piped_instance().await;

    server.run().await
}
