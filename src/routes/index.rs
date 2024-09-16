use actix_web::{get, HttpResponse, Responder};

#[get("/")]
pub async fn index_route() -> impl Responder {
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body("<html><body><h1>👋🏼</h1></body></html>")
}