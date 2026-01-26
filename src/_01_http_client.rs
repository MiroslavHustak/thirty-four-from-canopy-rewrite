use reqwest;
use serde::Deserialize;
use crate::_02_serialization::{LinksPayload, deserialize_from_json};

#[derive(Deserialize)]
pub struct ResponsePut {
    #[serde(rename = "Message1")]
    pub message1: String,
    #[serde(rename = "Message2")]
    pub message2: String,
}

pub async fn put_to_rest_api() -> Result<ResponsePut, Box<dyn std::error::Error>> {
    let url = "https://rust-rest-api-endpoints.onrender.com/api/canopy";
    let api_key = "test747646s5d4fvasfd645654asgasga654a6g13a2fg465a4fg4a3";

    // Read strongly-typed payload
    let payload: LinksPayload =
        deserialize_from_json("CanopyResults/canopy_results.json")?;

    let client = reqwest::Client::new();
    let response = client
        .put(url)
        .header("X-API-KEY", api_key)
        .json(&payload) // ✅ correct
        .send()
        .await?;

    let result: ResponsePut = response.json().await?;
    Ok(result)
}