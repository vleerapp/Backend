use crate::routes::search::AppState;
use actix_cors::Cors;
use actix_web::{http::header, web, App, HttpServer};
use env_logger::Env;
use futures::lock::Mutex;
use piped::select_best_piped_instance;
use std::{collections::HashMap, sync::Arc};
use utils::clear_log;

mod piped;
mod routes;
mod types;
mod utils;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    clear_log();
    env_logger::init_from_env(Env::default().default_filter_or("debug"));

    let app_state = web::Data::new(AppState {
        search_cache: Arc::new(Mutex::new(HashMap::new())),
        search_weights: Arc::new(Mutex::new(HashMap::new())),
    });

    let server = HttpServer::new(move || {
        let cors = Cors::default()
            .allowed_origin("http://localhost:3000")
            .allowed_methods(vec!["GET", "POST"])
            .allowed_headers(vec![header::AUTHORIZATION, header::ACCEPT])
            .allowed_header(header::CONTENT_TYPE)
            .max_age(3600);

        App::new()
            .wrap(cors)
            .service(routes::download::download_route)
            .service(routes::index::index_route)
            .service(routes::search::search_route)
            .service(routes::search_spotify::search_spotify_route)
            .service(routes::stream::stream_route)
            .service(routes::thumbnail::thumbnail_route)
            .app_data(app_state.clone())
            .configure(routes::search::config)
    })
    .bind(("0.0.0.0", 3001))?;

    select_best_piped_instance().await;

    server.run().await
}
