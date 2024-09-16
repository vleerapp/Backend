use actix_web::{get, HttpResponse, Responder};

#[get("/search-spotify")]
pub async fn search_spotify_route() -> impl Responder {
    HttpResponse::Ok().body("Search Spotify route")
}