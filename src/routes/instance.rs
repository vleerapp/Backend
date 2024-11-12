use actix_web::{get, HttpResponse, Responder};
use serde::Serialize;

use crate::{piped::get_instances, utils::log_with_table};

#[derive(Serialize)]
struct InstanceLatency {
    name: String,
    api_url: String,
}

#[get("/instances")]
pub async fn instances_route() -> impl Responder {
    let instances = get_instances();

    let table_data = vec![
        ("Total Instances", instances.len().to_string()),
        ("Status", "Available".to_string()),
    ];

    let _ = log_with_table("ℹ️ Fetching available instances", table_data);

    HttpResponse::Ok().json(
        instances
            .into_iter()
            .map(|instance| InstanceLatency {
                name: instance.name,
                api_url: instance.api_url
            })
            .collect::<Vec<InstanceLatency>>()
    )
}