use actix_web::{get, HttpResponse, Responder};

#[get("/thumbnail")]
pub async fn thumbnail_route() -> impl Responder {
    HttpResponse::Ok().body("Thumbnail route")
}