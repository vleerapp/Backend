use futures::future::join_all;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::RwLock;
use std::time::{Duration, Instant};

#[derive(Debug, Serialize, Deserialize)]
pub struct PipedInstance {
    pub api_url: String,
    pub name: String,
    pub country: Vec<String>,
}

static SELECTED_INSTANCE: RwLock<Option<String>> = RwLock::new(None);

async fn ping_instance(client: &Client, instance: &PipedInstance) -> Option<Duration> {
    let start = Instant::now();
    match client
        .get(&format!("{}/healthcheck", instance.api_url))
        .timeout(Duration::from_secs(5))
        .send()
        .await
    {
        Ok(_response) => {
            let duration = start.elapsed();
            Some(duration)
        }
        Err(_) => {
            None
        }
    }
}

async fn ping_instance_multiple(client: &Client, instance: &PipedInstance, count: usize) -> Option<Duration> {
    let pings = join_all((0..count).map(|_i| {
        let client = client.clone();
        let instance = instance;
        async move {
            let result = ping_instance(&client, &instance).await;
            result
        }
    })).await;
    
    let valid_pings: Vec<Duration> = pings.into_iter().flatten().collect();
    
    if valid_pings.is_empty() {
        println!("❌ No valid pings for {}", instance.name);
        None
    } else {
        let total_millis: u128 = valid_pings.iter().map(|d| d.as_millis()).sum();
        let avg_duration = Duration::from_millis((total_millis / valid_pings.len() as u128) as u64);
        println!("✅ Average ping for {}: {}ms", instance.name, avg_duration.as_millis());
        Some(avg_duration)
    }
}

pub fn get_instances() -> Vec<PipedInstance> {
    vec![
        PipedInstance {
            api_url: "https://api.piped.privacydev.net".to_string(),
            name: "privacydev.net".to_string(),
            country: vec!["france".to_string()],
        },
        PipedInstance {
            api_url: "https://pipedapi.adminforge.de".to_string(),
            name: "adminforge.de".to_string(),
            country: vec!["germany".to_string()],
        },
        PipedInstance {
            api_url: "https://pipedapi.leptons.xyz".to_string(),
            name: "leptons.xyz".to_string(),
            country: vec!["austria".to_string()],
        },
        PipedInstance {
            api_url: "https://api.piped.private.coffee".to_string(),
            name: "private.coffee".to_string(),
            country: vec!["austria".to_string()],
        },
        PipedInstance {
            api_url: "https://pipedapi.lunar.icu".to_string(),
            name: "lunar.icu".to_string(),
            country: vec!["germany".to_string()],
        },
        PipedInstance {
            api_url: "https://pipedapi.wireway.ch".to_string(),
            name: "wireway.ch".to_string(),
            country: vec!["switzerland".to_string()],
        },
        PipedInstance {
            api_url: "https://piped.smnz.de".to_string(),
            name: "smnz.de".to_string(),
            country: vec!["germany".to_string()],
        },
        PipedInstance {
            api_url: "https://api.piped.yt".to_string(),
            name: "piped.yt".to_string(),
            country: vec!["germany".to_string()],
        },
        PipedInstance {
            api_url: "https://pipedapi.reallyaweso.me".to_string(),
            name: "reallyaweso.me".to_string(),
            country: vec!["germany".to_string()],
        },
        PipedInstance {
            api_url: "https://pipedapi-libre.kavin.rocks/".to_string(),
            name: "kavin.rocks".to_string(),
            country: vec!["netherlands".to_string()],
        },
        PipedInstance {
            api_url: "https://pipedapi.ducks.party".to_string(),
            name: "ducks.party".to_string(),
            country: vec!["netherlands".to_string()],
        },
        PipedInstance {
            api_url: "https://piped-api.codespace.cz".to_string(),
            name: "codespace.cz".to_string(), 
            country: vec!["czech".to_string()],
        },
        PipedInstance {
            api_url: "https://pipedapi.drgns.space".to_string(),
            name: "drgns.space".to_string(),
            country: vec!["us".to_string()],
        },
        PipedInstance {
            api_url: "https://piapi.ggtyler.dev".to_string(),
            name: "ggtyler.dev".to_string(),
            country: vec!["us".to_string()],
        },
    ]
}

pub async fn select_best_piped_instance() {
    let client = Client::new();
    let instances = get_instances();

    const PING_COUNT: usize = 5;

    let ping_results = join_all(instances.iter().map(|instance| {
        let client = client.clone();
        async move {
            match ping_instance_multiple(&client, instance, PING_COUNT).await {
                Some(avg_ping_time) => {
                    Some((instance, avg_ping_time))
                }
                None => {
                    None
                }
            }
        }
    }))
    .await;

    let valid_results: Vec<_> = ping_results.into_iter().flatten().collect();


    let best_instance = valid_results
        .iter()
        .min_by_key(|(_, ping_time)| *ping_time)
        .map(|(instance, ping_time)| (instance.api_url.clone(), *ping_time));

    match best_instance {
        Some((api_url, _ping_time)) => {
            let mut selected = SELECTED_INSTANCE.write().unwrap();
            *selected = Some(api_url.clone());
        }
        None => {
            let fallback_instance = "https://pipedapi.wireway.ch".to_string();
            let mut selected = SELECTED_INSTANCE.write().unwrap();
            *selected = Some(fallback_instance.clone());
        }
    }

    match get_selected_instance() {
        Some(instance) => {
            let ping_time = valid_results
                .iter()
                .find(|(inst, _)| inst.api_url == instance)
                .map(|(_, time)| time.as_millis())
                .unwrap_or(0);
            println!("🏁 Final selected Piped instance: {} ({}ms)", instance, ping_time);
        }
        None => println!("❌ No Piped instance selected (this should never happen)"),
    }
}

pub fn get_selected_instance() -> Option<String> {
    SELECTED_INSTANCE.read().unwrap().clone()
}