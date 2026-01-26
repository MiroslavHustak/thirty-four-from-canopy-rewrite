mod _02_serialization;
mod _01_http_client;

use std::fs;
use _02_serialization::{LinksPayload, serialize_to_json};
use _01_http_client::put_to_rest_api;

// Mock canopy results
fn mock_canopy_results() -> LinksPayload {
    LinksPayload {
        list: vec![
            "https://kodis-files.s3.eu-central-1.amazonaws.com/288_2025_12_14_2026_12_12_211a7a5d18.pdf".to_string(),
            "https://kodis-files.s3.eu-central-1.amazonaws.com/293_2025_12_14_2026_12_12_eb56d8fc05.pdf".to_string(),
            "https://kodis-files.s3.eu-central-1.amazonaws.com/901_2025_12_14_2026_12_12_c98921935e.pdf".to_string(),
        ],
    }
}

// Filter logic
fn filter_old_links(mut payload: LinksPayload) -> LinksPayload {
    payload.list = payload
        .list
        .into_iter()
        .filter(|link| !link.contains("2022") && !link.contains("2023"))
        .collect();

    payload
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {  

    let payload = mock_canopy_results();
    let filtered = filter_old_links(payload);

    fs::create_dir_all("CanopyResults")?;

    serialize_to_json(&filtered, "CanopyResults/canopy_results.json")?;

    let response = put_to_rest_api().await?;

    println!("Response: {} - {}", response.message1, response.message2);

    Ok(())
}
