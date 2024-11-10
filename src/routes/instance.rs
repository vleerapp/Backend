use actix_web::{get, HttpResponse, Responder};
use serde::Serialize;

use crate::piped::get_instances;

#[derive(Serialize)]
struct InstanceLatencyResponse {
    instances: Vec<InstanceLatency>,
}

#[derive(Serialize)]
struct InstanceLatency {
    name: String,
    api_url: String,
}

#[get("/instances")]
pub async fn instances_route() -> impl Responder {
    let instances = get_instances();

    HttpResponse::Ok().json(InstanceLatencyResponse {
        instances: instances
            .into_iter()
            .map(|instance| InstanceLatency {
                name: instance.name,
                api_url: instance.api_url,
            })
            .collect(),
    })
}