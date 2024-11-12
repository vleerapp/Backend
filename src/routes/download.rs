use actix_web::{get, web, Error, HttpResponse};
use futures::StreamExt;
use rustypipe::param::StreamFilter;
use rustypipe_downloader::DownloaderBuilder;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;
use tokio::fs::{create_dir_all, File, read_dir, remove_file};
use tokio_util::io::ReaderStream;
use std::time::Instant;

use crate::utils::log_with_table;

#[derive(Deserialize, Serialize)]
struct DownloadQuery {
    id: String,
    quality: String,
}

#[get("/download")]
async fn download_route(query: web::Query<DownloadQuery>) -> Result<HttpResponse, Error> {
    let start_time = Instant::now();
    let DownloadQuery { id, quality } = query.into_inner();

    if id.is_empty() || (quality != "compressed" && quality != "lossless") {
        let _ = log_with_table("❌ Invalid request parameters", vec![
            ("ID", id.clone()),
            ("Quality", quality.to_string())
        ])?;
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
        let _ = log_with_table(
            "📥 Download started",
            vec![
                ("ID", id.clone()),
                ("Quality", quality.to_string()),
                ("File Path", file_path.display().to_string()),
                ("Format", file_extension.to_string())
            ],
        )?;
        match download_with_rustypipe(id.clone(), &cache_dir, &quality).await {
            Ok(_) => {
                let _ = log_with_table("✅ Download completed", vec![
                    ("ID", id.clone()),
                    ("File", file_path.display().to_string())
                ])?;
            }
            Err(e) => {
                let _ = log_with_table("💥 Download failed", vec![
                    ("ID", id.clone()),
                    ("Error", e.to_string()),
                    ("File", file_path.display().to_string())
                ])?;
                return Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": "Failed to download video",
                    "details": e.to_string()
                })));
            }
        }
    } else {
        let _ = log_with_table("ℹ️ Using cached file", vec![
            ("ID", id.clone()),
            ("File", file_path.display().to_string())
        ])?;
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
                                    let _ = log_with_table("💥 Cache cleanup failed", vec![
                                        ("File", entry.path().display().to_string()),
                                        ("Error", e.to_string())
                                    ]);
                                }
                            }
                        }
                    }
                }
            }
        }
    });

    let duration = start_time.elapsed().as_millis();
    let _ = log_with_table(
        &format!("✅ Download completed {}", if file_path.exists() { "(cached)" } else { "" }),
        vec![
            ("ID", id.clone()),
            ("Quality", quality.to_string()),
            ("Duration", format!("{} ms", duration)),
            ("Cached", file_path.exists().to_string())
        ],
    )?;

    Ok(response)
}

async fn download_with_rustypipe(
    id: String,
    cache_dir: &Path,
    quality: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let dl = DownloaderBuilder::new().audio_tag().crop_cover().build();
    let filter_audio = StreamFilter::new().no_video();
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
        let error_msg = String::from_utf8_lossy(&output.stderr);
        let _ = log_with_table("💥 FFmpeg conversion failed", vec![
            ("ID", id),
            ("Error", error_msg.to_string())
        ]);
        return Err(format!("FFmpeg conversion failed: {}", error_msg).into());
    }

    Ok(())
}
