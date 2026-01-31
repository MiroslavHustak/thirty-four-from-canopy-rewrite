use reqwest;
use serde::Deserialize;
use crate::_02_serialization::{LinksPayload, deserialize_from_json};

//#[derive(Serialize)] uses procedural macros that generate code at compile time, not runtime reflection.
#[derive(Deserialize)]   //#[derive(Deserialize)] is a procedural macro that automatically generates code to convert data (like JSON, YAML, etc.) into your Rust struct.
pub struct ResponsePut {
    #[serde(rename = "Message1")]  //The #[serde(rename = "...")] annotations handle the mismatch between Rust naming conventions and the JSON field names.
    pub message1: String,
    #[serde(rename = "Message2")]
    pub message2: String,
}

pub async fn put_to_rest_api() -> Result<ResponsePut, Box<dyn std::error::Error>> {
    let url = "https://rust-rest-api-endpoints.onrender.com/api/canopy";

    dotenvy::dotenv().ok(); //loads environment variables from a .env file
    let api_key = std::env::var("API_KEY")?;

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