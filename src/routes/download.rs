use actix_web::{get, web, Error, HttpResponse};
use bytes::Bytes;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::utils::log;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DownloadQuery {
    id: String,
    quality: String,
}

#[get("/download")]
pub async fn download_route(query: web::Query<DownloadQuery>) -> Result<HttpResponse, Error> {
    let query_clone = query.clone();
    let DownloadQuery { id, quality } = query.into_inner();

    if id.is_empty() || (quality != "compressed" && quality != "lossless") {
        log(&format!("🚫 Invalid request: {:?}", query_clone));
        return Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Invalid or missing id or quality parameter"
        })));
    }

    let cache_dir = Path::new("cache");
    let compressed_dir = cache_dir.join("compressed");
    let lossless_dir = cache_dir.join("lossless");
    let cache_file_path = if quality == "compressed" {
        compressed_dir.join(format!("{}.mp3", id))
    } else {
        lossless_dir.join(format!("{}.flac", id))
    };

    fs::create_dir_all(&compressed_dir).unwrap();
    fs::create_dir_all(&lossless_dir).unwrap();

    log(&format!("📥 Downloading and converting: {}", id));

    let (tx, rx) = mpsc::channel::<Result<Bytes, std::io::Error>>(1024);
    let quality_clone = quality.clone();

    tokio::spawn(async move {
        if let Err(e) = stream_and_cache(&id, &cache_file_path, &quality).await {
            log(&format!("Error streaming video: {:?}", e));
        }
    });

    let stream = ReceiverStream::new(rx);

    Ok(HttpResponse::Ok()
        .content_type(if quality_clone == "compressed" {
            "audio/mpeg"
        } else {
            "audio/flac"
        })
        .streaming(stream))
}

async fn stream_and_cache(
    id: &str,
    output_path: &Path,
    quality: &str,
) -> Result<
    impl futures_util::Stream<Item = Result<Bytes, std::io::Error>>,
    Box<dyn std::error::Error>,
> {
    let client = Client::new();
    let api_url = format!("https://pipedapi.wireway.ch/streams/{}", id);
    let response = client.get(&api_url).send().await?.json::<Value>().await?;

    let audio_streams = response["audioStreams"]
        .as_array()
        .ok_or("No audio streams found")?;
    let stream_url = audio_streams
        .iter()
        .find(|stream| stream["itag"].as_i64() == Some(251))
        .and_then(|stream| stream["url"].as_str())
        .ok_or("No suitable audio stream found")?;

    let mut response = client.get(stream_url).send().await?;
    let (tx, rx) = mpsc::channel::<Result<Bytes, std::io::Error>>(1024);

    let output_path_clone = output_path.to_path_buf();
    let quality_clone = quality.to_string();

    tokio::spawn(async move {
        let mut temp_file = tempfile::NamedTempFile::new().unwrap();
        let mut ffmpeg_process = Command::new("ffmpeg")
            .args(&[
                "-i",
                "pipe:0",
                "-f",
                if quality_clone == "compressed" {
                    "mp3"
                } else {
                    "flac"
                },
                "-acodec",
                if quality_clone == "compressed" {
                    "libmp3lame"
                } else {
                    "flac"
                },
                "-ar",
                "44100",
                "-ac",
                "2",
                "-b:a",
                if quality_clone == "compressed" {
                    "192k"
                } else {
                    "1411k"
                },
                "-loglevel",
                "error",
                "pipe:1",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();

        let mut stdin = ffmpeg_process.stdin.take().unwrap();
        let mut stdout = ffmpeg_process.stdout.take().unwrap();

        // Stream data to FFmpeg
        let mut total_bytes = 0;
        while let Some(chunk) = response.chunk().await.unwrap() {
            stdin.write_all(&chunk).unwrap();
            total_bytes += chunk.len();
            tx.send(Ok(chunk)).await.unwrap();
        }
        drop(stdin);

        // Read from FFmpeg and write to file
        let mut buffer = [0; 8192];
        let mut converted_bytes = 0;
        loop {
            match stdout.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => {
                    temp_file.write_all(&buffer[..n]).unwrap();
                    converted_bytes += n;
                    tx.send(Ok(Bytes::copy_from_slice(&buffer[..n])))
                        .await
                        .unwrap();
                }
                Err(_) => break,
            }
        }

        // Check FFmpeg exit status and error output
        let status = ffmpeg_process.wait().unwrap();
        if !status.success() {
            if let Some(mut stderr) = ffmpeg_process.stderr {
                let mut error_message = String::new();
                stderr.read_to_string(&mut error_message).unwrap();
                eprintln!("FFmpeg error: {}", error_message);
            }
        }

        println!("Total bytes received: {}", total_bytes);
        println!("Total bytes converted: {}", converted_bytes);

        temp_file.persist(output_path_clone).unwrap();
    });

    Ok(tokio_stream::wrappers::ReceiverStream::new(rx))
}
