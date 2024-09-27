use actix_web::{get, web, HttpResponse, Responder};
use anyhow::Result;
use image::{imageops::FilterType, ImageFormat};
use reqwest::Client;
use serde::Deserialize;
use std::{fs, path::PathBuf};
use tokio::fs as tokio_fs;

#[derive(Deserialize)]
struct ThumbnailQuery {
    id: String,
}

#[get("/thumbnail")]
pub async fn thumbnail_route(query: web::Query<ThumbnailQuery>) -> impl Responder {
    match fetch_thumbnail(&query.id).await {
        Ok(file_path) => {
            let file = match fs::read(&file_path) {
                Ok(contents) => contents,
                Err(_) => return HttpResponse::InternalServerError().body("Failed to read thumbnail"),
            };
            HttpResponse::Ok()
                .content_type("image/webp")
                .body(file)
        },
        Err(_) => HttpResponse::InternalServerError().body("Failed to fetch thumbnail"),
    }
}

async fn fetch_thumbnail(id: &str) -> Result<PathBuf> {
    let cache_dir = PathBuf::from("cache/thumbnails");
    let cache_file = cache_dir.join(format!("{}.webp", id));

    if cache_file.exists() {
        return Ok(cache_file);
    }

    tokio_fs::create_dir_all(&cache_dir).await?;

    let client = Client::new();
    let response = client
        .get(format!("https://i3.ytimg.com/vi/{}/maxresdefault.jpg", id))
        .send()
        .await?;

    let img_data = response.bytes().await?;
    let img = image::load_from_memory(&img_data)?;

    let size = img.width().min(img.height());
    let left = (img.width() - size) / 2;
    let top = (img.height() - size) / 2;

    let cropped = img.crop_imm(left, top, size, size);
    let resized = cropped.resize(256, 256, FilterType::Lanczos3);

    resized.save_with_format(&cache_file, ImageFormat::WebP)?;

    Ok(cache_file)
}