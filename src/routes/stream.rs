use actix_web::{get, HttpResponse, Responder};

#[get("/stream")]
pub async fn stream_route() -> impl Responder {
    HttpResponse::Ok().body("Stream route")
}