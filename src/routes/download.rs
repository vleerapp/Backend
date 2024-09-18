use actix_web::{get, web, Error, HttpResponse};
use futures::StreamExt;
use rustypipe::param::StreamFilter;
use rustypipe_downloader::DownloaderBuilder;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;
use tokio::fs::{create_dir_all, File, read_dir, remove_file};
use tokio_util::io::ReaderStream;

use crate::utils::log;

#[derive(Deserialize, Serialize)]
struct DownloadQuery {
    id: String,
    quality: String,
}

#[get("/download")]
async fn download_route(query: web::Query<DownloadQuery>) -> Result<HttpResponse, Error> {
    let DownloadQuery { id, quality } = query.into_inner();

    if id.is_empty() || (quality != "compressed" && quality != "lossless") {
        return Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Invalid or missing id or quality parameter"
        })));
    }

    let cache_dir = Path::new("cache").join(if quality == "compressed" {
        "compressed"
    } else {
        "lossless"
    });
    create_dir_all(&cache_dir).await?;

    let file_extension = if quality == "compressed" {
        "mp3"
    } else {
        "flac"
    };
    let file_path = cache_dir.join(format!("{}.{}", id, file_extension));

    if !file_path.exists() {
        log(&format!("📥 Downloading video: {}", id));
        match download_with_rustypipe(id.clone(), &cache_dir, &quality).await {
            Ok(_) => log("Download completed successfully"),
            Err(e) => {
                log(&format!("Error downloading video: {}", e));
                return Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": "Failed to download video",
                    "details": e.to_string()
                })));
            }
        }
    } else {
        log(&format!("Serving cached file: {:?}", file_path));
    }

    let file = File::open(&file_path).await?;
    let stream = ReaderStream::new(file);
    let mapped_stream = stream.map(|result| result.map(web::Bytes::from));

    let content_type = if quality == "compressed" {
        "audio/mpeg"
    } else {
        "audio/flac"
    };
    let response = HttpResponse::Ok()
        .content_type(content_type)
        .streaming(mapped_stream);

    tokio::spawn(async move {
        if let Ok(mut entries) = read_dir(&cache_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                if let Ok(file_type) = entry.file_type().await {
                    if file_type.is_file() {
                        if let Some(extension) = entry.path().extension() {
                            if extension != "flac" && extension != "mp3" {
                                if let Err(e) = remove_file(entry.path()).await {
                                    log(&format!("Failed to remove file: {:?}. Error: {}", entry.path(), e));
                                }
                            }
                        }
                    }
                }
            }
        }
    });

    Ok(response)
}

async fn download_with_rustypipe(
    id: String,
    cache_dir: &Path,
    quality: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("Building downloader for rustypipe");
    let dl = DownloaderBuilder::new().audio_tag().crop_cover().build();
    let filter_audio = StreamFilter::new().no_video();
    println!("Starting download with rustypipe for id: {}", id);
    let audio_path = cache_dir.join(format!("{}.opus", id));
    dl.id(&id)
        .stream_filter(filter_audio)
        .to_file(audio_path.to_str().unwrap())
        .download()
        .await?;

    let output_extension = if quality == "compressed" {
        "mp3"
    } else {
        "flac"
    };
    println!("Converting {}.opus to {}.{}", id, id, output_extension);
    let output_path = cache_dir.join(format!("{}.{}", id, output_extension));
    let output = Command::new("ffmpeg")
        .args(&[
            "-i",
            audio_path.to_str().unwrap(),
            output_path.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute ffmpeg");

    if !output.status.success() {
        println!(
            "FFmpeg conversion failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(())
}
