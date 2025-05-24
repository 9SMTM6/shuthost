use axum::{
    extract::Query,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use rand::seq::IndexedRandom;
use serde::Deserialize;

const CLIENT_SCRIPT_TEMPLATE: &str = include_str!("shuthost_client.sh");

// Word lists for generating readable client IDs
const ADJECTIVES: &[&str] = &[
    "red", "blue", "swift", "calm", "bold", "wise", "kind", "brave",
];
const NOUNS: &[&str] = &[
    "fox", "bird", "wolf", "bear", "lion", "deer", "hawk", "eagle",
];

#[derive(Deserialize)]
pub struct ClientScriptQuery {
    remote_url: String,
    #[serde(default)]
    client_id: Option<String>,
}

pub async fn download_client_script(Query(params): Query<ClientScriptQuery>) -> impl IntoResponse {
    // TODO: switch to download per script like with the agent.
    // Like with the agent, infer the client name from hostname, and print out the config to hadd to the config file.
    // Both of these have to happen in the download script.
    let client_id = params.client_id.unwrap_or_else(generate_client_id);
    let shared_secret = shuthost_common::generate_secret();

    let script = CLIENT_SCRIPT_TEMPLATE
        .replace("{embedded_remote_url}", &params.remote_url)
        .replace("{client_id}", &client_id)
        .replace("{shared_secret}", &shared_secret);

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/plain")
        .header(
            "Content-Disposition",
            "attachment; filename=\"shuthost-client.sh\"",
        )
        .body(script)
        .unwrap()
}

fn generate_client_id() -> String {
    let mut rng = rand::rng();
    let adjective = ADJECTIVES.choose(&mut rng).unwrap();
    let noun = NOUNS.choose(&mut rng).unwrap();
    format!("{}-{}", adjective, noun)
}
