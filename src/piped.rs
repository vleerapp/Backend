use log::{error, info, warn};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::RwLock;
use std::time::{Duration, Instant};

#[derive(Debug, Serialize, Deserialize)]
struct PipedInstance {
    api_url: String,
    name: String,
}

static SELECTED_INSTANCE: RwLock<Option<String>> = RwLock::new(None);

async fn ping_instance(client: &Client, instance: &PipedInstance) -> Duration {
    let start = Instant::now();
    match client
        .get(&format!("{}/healthcheck", instance.api_url))
        .timeout(Duration::from_secs(5))
        .send()
        .await
    {
        Ok(_response) => {
            let duration = start.elapsed();
            duration
        }
        Err(e) => {
            warn!("Failed to ping {}: {}", instance.name, e);
            Duration::from_secs(u64::MAX)
        }
    }
}

pub async fn select_best_piped_instance() {
    let client = Client::new();

    match client
        .get("https://piped-instances.kavin.rocks/")
        .timeout(Duration::from_secs(5))
        .send()
        .await
    {
        Ok(response) => {
            let instances: Vec<PipedInstance> = match response.json().await {
                Ok(instances) => instances,
                Err(e) => {
                    error!("Failed to parse instances JSON: {}", e);
                    return;
                }
            };

            let filtered_instances: Vec<PipedInstance> = instances
                .into_iter()
                .filter(|instance| {
                    ![
                        "adminforge.de",
                        "ehwurscht.at",
                        "ggtyler.dev",
                        "phoenixthrush.com",
                        "piped.yt",
                        "private.coffee",
                        "privacydev.net",
                        "projectsegfau.lt"
                    ]
                    .contains(&instance.name.as_str())
                    && !instance.api_url.contains("kavin.rocks")
                })
                .collect();

            let mut instances_to_test = filtered_instances;
            instances_to_test.push(PipedInstance {
                api_url: "https://pipedapi.wireway.ch".to_string(),
                name: "wireway.ch".to_string(),
            });

            let ping_results =
                futures::future::join_all(instances_to_test.iter().map(|instance| {
                    let client = client.clone();
                    async move {
                        let ping_time = ping_instance(&client, instance).await;
                        info!(
                            "🏓 Ping test for {}: {}ms",
                            instance.name,
                            ping_time.as_millis()
                        );
                        (instance, ping_time)
                    }
                }))
                .await;

            if let Some((best_instance, best_ping)) = ping_results
                .into_iter()
                .min_by_key(|(_, ping_time)| *ping_time)
            {
                let mut selected = SELECTED_INSTANCE.write().unwrap();
                *selected = Some(best_instance.api_url.clone());
                info!(
                    "🌐 Selected Piped instance: {} ({}ms)",
                    best_instance.api_url,
                    best_ping.as_millis()
                );
            } else {
                warn!("No suitable Piped instance found");
            }
        }
        Err(error) => {
            error!("💥 Error fetching Piped instances: {}", error);
        }
    }
}

pub fn get_selected_instance() -> String {
    let instance = SELECTED_INSTANCE
        .read()
        .unwrap()
        .clone()
        .unwrap().to_string();
    instance
}
