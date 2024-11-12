use actix_web::{get, web, HttpResponse, Responder};
use anyhow::Result;
use image::{imageops::FilterType, ImageFormat};
use reqwest::Client;
use serde::Deserialize;
use std::{fs, path::PathBuf};
use tokio::fs as tokio_fs;
use std::time::Instant;

use crate::utils::log_with_table;

#[derive(Deserialize)]
struct ThumbnailQuery {
    id: String,
}

#[get("/thumbnail")]
pub async fn thumbnail_route(query: web::Query<ThumbnailQuery>) -> impl Responder {
    let start_time = Instant::now();
    
    match fetch_thumbnail(&query.id).await {
        Ok((file_path, is_cached)) => {
            let duration = start_time.elapsed().as_millis();
            let status = if is_cached { "✅ Cached" } else { "✅ Fetched and processed" };
            let _ = log_with_table(
                &format!("{} thumbnail served", status),
                vec![
                    ("ID", query.id.clone()),
                    ("Duration", format!("{} ms", duration)),
                    ("Path", file_path.display().to_string()),
                    ("Cached", is_cached.to_string())
                ]
            );
            
            let file = match fs::read(&file_path) {
                Ok(contents) => contents,
                Err(e) => {
                    let _ = log_with_table("💥 Failed to read thumbnail", vec![
                        ("ID", query.id.clone()),
                        ("Error", e.to_string())
                    ]);
                    return HttpResponse::InternalServerError().body("Failed to read thumbnail");
                }
            };
            HttpResponse::Ok()
                .content_type("image/webp")
                .body(file)
        },
        Err(e) => {
            let duration = start_time.elapsed().as_millis();
            let _ = log_with_table("💥 Failed to fetch thumbnail", vec![
                ("ID", query.id.clone()),
                ("Error", e.to_string()),
                ("Duration", format!("{} ms", duration))
            ]);
            HttpResponse::InternalServerError().body("Failed to fetch thumbnail")
        }
    }
}

async fn fetch_thumbnail(id: &str) -> Result<(PathBuf, bool)> {
    let cache_dir = PathBuf::from("cache/thumbnails");
    let cache_file = cache_dir.join(format!("{}.webp", id));

    if cache_file.exists() {
        return Ok((cache_file, true));
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

    Ok((cache_file, false))
}