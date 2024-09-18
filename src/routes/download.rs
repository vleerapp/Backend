use actix_web::{get, web, Error, HttpResponse};
use bytes::Bytes;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Read;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::utils::log;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DownloadQuery {
    id: String,
    quality: String,
}

#[get("/download")]
pub async fn download_route(query: web::Query<DownloadQuery>) -> Result<HttpResponse, Error> {
    let DownloadQuery { id, quality } = query.clone().into_inner();

    if id.is_empty() || (quality != "compressed" && quality != "lossless") {
        log(&format!("🚫 Invalid request: {:?}", query));
        return Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Invalid or missing id or quality parameter"
        })));
    }

    log(&format!("📥 Proxying stream for: {}", id));

    let stream = match proxy_stream(&id, &quality).await {
        Ok(stream) => stream,
        Err(e) => {
            log(&format!("Error proxying stream: {:?}", e));
            return Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to proxy stream"
            })));
        }
    };

    Ok(HttpResponse::Ok()
        .content_type(if quality == "compressed" {
            "audio/mpeg"
        } else {
            "audio/flac"
        })
        .streaming(stream))
}

async fn proxy_stream(
    id: &str,
    quality: &str,
) -> Result<impl futures_util::Stream<Item = Result<Bytes, std::io::Error>>, Box<dyn std::error::Error>> {
    let client = Client::new();
    let api_url = format!("https://pipedapi.wireway.ch/streams/{}", id);
    let response = client.get(&api_url).send()?.json::<Value>()?;

    let audio_streams = response["audioStreams"]
        .as_array()
        .ok_or("No audio streams found")?;
    let stream_url = audio_streams
        .iter()
        .find(|stream| {
            if quality == "compressed" {
                stream["itag"].as_i64() == Some(140) // M4A format
            } else {
                stream["itag"].as_i64() == Some(251) // OPUS format (highest quality)
            }
        })
        .and_then(|stream| stream["url"].as_str())
        .ok_or("No suitable audio stream found")?;

    let mut response = client.get(stream_url).send()?;
    let (tx, rx) = mpsc::channel(1024);

    tokio::task::spawn_blocking(move || {
        let mut buffer = [0; 8192];
        loop {
            match response.read(&mut buffer) {
                Ok(0) => break, // EOF
                Ok(n) => {
                    if tx.blocking_send(Ok(Bytes::copy_from_slice(&buffer[..n]))).is_err() {
                        break;
                    }
                }
                Err(e) => {
                    let _ = tx.blocking_send(Err(e));
                    break;
                }
            }
        }
    });

    Ok(ReceiverStream::new(rx))
}
