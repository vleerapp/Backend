use actix_web::{get, web, HttpRequest, HttpResponse};
use crate::utils::log_with_table;
use rustypipe::param::StreamFilter;
use rustypipe_downloader::DownloaderBuilder;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs::{self, File};
use tokio::io::{AsyncReadExt, AsyncSeekExt, SeekFrom};
use tokio::process::Command;
use futures::stream::{self};
use actix_web::web::Bytes;
use std::io::{self, ErrorKind};
use std::time::Instant;

const CHUNK_SIZE: u64 = 1000 * 1024; // 1000 KB chunks

#[derive(Deserialize, Serialize)]
struct StreamQuery {
    id: String,
    quality: String,
}

#[get("/stream")]
async fn stream_route(
    query: web::Query<StreamQuery>,
    req: HttpRequest,
) -> Result<HttpResponse, actix_web::Error> {
    let start_time = Instant::now();
    let StreamQuery { id, quality } = query.into_inner();

    let cache_dir = PathBuf::from("cache").join(&quality);
    let file_extension = if quality == "compressed" { "mp3" } else { "flac" };
    let cached_file_path = cache_dir.join(format!("{}.{}", id, file_extension));

    fs::create_dir_all(&cache_dir).await.unwrap();

    let is_cached = cached_file_path.exists();
    if !is_cached {
        let _ = log_with_table("ℹ️ Starting new download and conversion", vec![
            ("ID", id.clone()),
            ("Quality", quality.clone()),
            ("Cached", "false".to_string()),
            ("Duration (ms)", start_time.elapsed().as_millis().to_string())
        ]).map_err(actix_web::error::ErrorInternalServerError)?;

        match download_with_rustypipe(id.clone(), &cache_dir, &quality).await {
            Ok(_) => {
                let _ = log_with_table("✅ Processing complete", vec![
                    ("ID", id.clone()),
                    ("Quality", quality.clone()),
                    ("Cached", "false".to_string()),
                    ("Duration (ms)", start_time.elapsed().as_millis().to_string())
                ]).map_err(actix_web::error::ErrorInternalServerError)?;
            }
            Err(e) => {
                let _ = log_with_table("💥 Failed to download or process file.", vec![
                    ("Error", e.to_string()),
                    ("Cached", "false".to_string()),
                    ("Duration (ms)", start_time.elapsed().as_millis().to_string())
                ]).map_err(actix_web::error::ErrorInternalServerError)?;
                return Ok(HttpResponse::InternalServerError().finish());
            }
        }
    } else {
        let _ = log_with_table("ℹ️ Using cached file", vec![
            ("ID", id.clone()),
            ("Quality", quality.clone()),
            ("Cached", "true".to_string()),
            ("Duration (ms)", start_time.elapsed().as_millis().to_string())
        ]).map_err(actix_web::error::ErrorInternalServerError)?;
    }

    let file = match File::open(&cached_file_path).await {
        Ok(file) => file,
        Err(e) => {
            let _ = log_with_table("💥 Failed to open cached file.", vec![
                ("Error", e.to_string()),
                ("Cached", is_cached.to_string()),
                ("Duration (ms)", start_time.elapsed().as_millis().to_string())
            ]);
            return Ok(HttpResponse::InternalServerError().finish());
        }
    };

    let metadata = match file.metadata().await {
        Ok(metadata) => metadata,
        Err(e) => {
            let _ = log_with_table("💥 Failed to read file metadata.", vec![
                ("Error", e.to_string()),
                ("Cached", is_cached.to_string()),
                ("Duration (ms)", start_time.elapsed().as_millis().to_string())
            ]);
            return Ok(HttpResponse::InternalServerError().finish());
        }
    };

    let total_length = metadata.len();
    let range_header = req.headers().get("range").and_then(|v| v.to_str().ok());

    let (start, end) = match range_header {
        Some(range) => match parse_range(range, total_length) {
            Ok(range) => range,
            Err(_) => return Ok(HttpResponse::BadRequest().finish()),
        },
        None => (0, total_length - 1),
    };

    let content_type = if quality == "compressed" { "audio/mpeg" } else { "audio/flac" };

    let stream = stream::unfold((file, start, end), |(mut file, position, end)| async move {
        if position > end {
            return None;
        }

        let read_length = std::cmp::min(CHUNK_SIZE, end - position + 1);
        let mut buffer = vec![0; read_length as usize];
        if let Err(_) = file.seek(SeekFrom::Start(position)).await {
            return Some((Err(io::Error::new(ErrorKind::Other, "Seek error")), (file, position, end)));
        }
        match file.read_exact(&mut buffer).await {
            Ok(_) => {
                let next_position = position + read_length;
                Some((Ok(Bytes::from(buffer)), (file, next_position, end)))
            },
            Err(e) => Some((Err(io::Error::new(io::ErrorKind::Other, e)), (file, position, end)))
        }
    });

    Ok(HttpResponse::PartialContent()
        .insert_header(("Content-Type", content_type))
        .insert_header(("Content-Range", format!("bytes {}-{}/{}", start, end, total_length)))
        .insert_header(("Accept-Ranges", "bytes"))
        .streaming(stream))
}

fn parse_range(range: &str, total_length: u64) -> Result<(u64, u64), actix_web::error::Error> {
    let range = range.trim_start_matches("bytes=");
    let mut parts = range.split('-');
    let start = parts.next().and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
    let end = parts.next().and_then(|s| s.parse::<u64>().ok()).unwrap_or(total_length - 1);

    if start >= total_length || end >= total_length || start > end {
        let _ = log_with_table("💥 Invalid range.", vec![("Requested range", range.to_string())]);
        return Err(actix_web::error::ErrorBadRequest("Requested range not satisfiable"));
    }

    Ok((start, end))
}

async fn download_with_rustypipe(
    id: String,
    cache_dir: &PathBuf,
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

    let output_extension = if quality == "compressed" { "mp3" } else { "flac" };
    let output_path = cache_dir.join(format!("{}.{}", id, output_extension));
    let output = Command::new("ffmpeg")
        .args(&[
            "-i",
            audio_path.to_str().unwrap(),
            output_path.to_str().unwrap(),
        ])
        .output()
        .await
        .expect("Failed to execute ffmpeg");

    if !output.status.success() {
        let _ = log_with_table("💥 FFmpeg conversion failed.", vec![("Error", String::from_utf8_lossy(&output.stderr).to_string())]);
    }

    fs::remove_file(audio_path).await?;

    Ok(())
}