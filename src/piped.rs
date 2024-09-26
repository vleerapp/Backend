use futures::future::join_all;
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
        Ok(_response) => start.elapsed(),
        Err(e) => {
            warn!("Failed to ping {}: {}", instance.name, e);
            Duration::from_secs(u64::MAX)
        }
    }
}

pub async fn select_best_piped_instance() {
    let client = Client::new();

    let instances = match client
        .get("https://piped-instances.kavin.rocks/")
        .timeout(Duration::from_secs(5))
        .send()
        .await
    {
        Ok(response) => match response.json::<Vec<PipedInstance>>().await {
            Ok(instances) => instances,
            Err(e) => {
                error!("Failed to parse instances JSON: {}", e);
                vec![] // Return an empty vector to continue with fallback instance
            }
        },
        Err(error) => {
            error!("💥 Error fetching Piped instances: {}", error);
            vec![] // Return an empty vector to continue with fallback instance
        }
    };

    info!("Fetched {} instances", instances.len());

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

    info!("Filtered to {} instances", filtered_instances.len());

    let mut instances_to_test = filtered_instances;
    instances_to_test.push(PipedInstance {
        api_url: "https://pipedapi.wireway.ch".to_string(),
        name: "wireway.ch".to_string(),
    });

    info!("Testing {} instances", instances_to_test.len());

    let ping_results = join_all(instances_to_test.iter().map(|instance| {
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

    let best_instance = ping_results
        .into_iter()
        .min_by_key(|(_, ping_time)| *ping_time)
        .map(|(instance, ping_time)| (instance.api_url.clone(), ping_time));

    match best_instance {
        Some((api_url, ping_time)) => {
            let mut selected = SELECTED_INSTANCE.write().unwrap();
            *selected = Some(api_url.clone());
            info!(
                "🌐 Selected Piped instance: {} ({}ms)",
                api_url,
                ping_time.as_millis()
            );
        }
        None => {
            warn!("No suitable Piped instance found, using fallback");
            let fallback_instance = "https://pipedapi.kavin.rocks".to_string();
            let mut selected = SELECTED_INSTANCE.write().unwrap();
            *selected = Some(fallback_instance.clone());
            info!("🌐 Using fallback Piped instance: {}", fallback_instance);
        }
    }

    match get_selected_instance() {
        Some(instance) => println!("🌐 Selected Piped instance: {}", instance),
        None => println!("❌ No Piped instance selected (this should never happen)"),
    }
}

pub fn get_selected_instance() -> Option<String> {
    SELECTED_INSTANCE.read().unwrap().clone()
}
