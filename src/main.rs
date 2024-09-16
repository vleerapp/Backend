use actix_web::{web, App, HttpServer};
use futures::lock::Mutex;
use piped::select_best_piped_instance;
use utils::clear_log;
use std::{collections::HashMap, sync::Arc};
use env_logger::Env;
use crate::routes::search::AppState;

mod piped;
mod routes;
mod types;
mod utils;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    clear_log();
    env_logger::init_from_env(Env::default().default_filter_or("debug"));
    select_best_piped_instance().await;

    let app_state = web::Data::new(AppState {
        search_cache: Arc::new(Mutex::new(HashMap::new())),
        search_weights: Arc::new(Mutex::new(HashMap::new())),
    });

    HttpServer::new(move || {
        App::new()
            .service(routes::download::download_route)
            .service(routes::index::index_route)
            .service(routes::search::search_route)
            .service(routes::search_spotify::search_spotify_route)
            .service(routes::stream::stream_route)
            .service(routes::thumbnail::thumbnail_route)
            .app_data(app_state.clone())
            .configure(routes::search::config)
    })
    .bind(("0.0.0.0", 3001))?
    .run()
    .await
}
