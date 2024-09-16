use actix_web::{get, HttpResponse, Responder};

#[get("/download")]
pub async fn download_route() -> impl Responder {
    HttpResponse::Ok().body("Download route")
}