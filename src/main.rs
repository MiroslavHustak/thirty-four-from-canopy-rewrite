/*
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
*/

//chromedriver.exe --port=9515

mod _01_http_client;
mod _02_serialization;
mod _03_scraping_edge;
mod _04_scraping_chrome;

use std::fs;

use _01_http_client::put_to_rest_api;
use _02_serialization::serialize_to_json;
use _03_scraping_edge::scrape_real_results_edge;
use _04_scraping_chrome::scrape_real_results_chrome;

// Filter logic (same as you had)
fn filter_old_links(mut payload: crate::_02_serialization::LinksPayload) -> crate::_02_serialization::LinksPayload {
    payload.list = payload
        .list
        .into_iter()
        .filter(|link| !link.contains("2022") && !link.contains("2023"))
        .collect();
    payload
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting Scraper...");

    // 1. Scrape (using Chrome instead of Edge)
    let payload = match scrape_real_results_chrome().await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Scraping failed: {}. Is chromedriver running on port 9515?", e);
            return Ok(());
        }
    };
/*
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting Scraper...");

    // 1. Scrape (Replaces the Mock)
    let payload = match scrape_real_results_chrome().await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Scraping failed: {}. Is msedgedriver running?", e);
            return Ok(());
        }
    };

 */

    // 2. Filter
    let filtered = filter_old_links(payload);

    // 3. Save
    fs::create_dir_all("CanopyResults")?;
    serialize_to_json(&filtered, "CanopyResults/canopy_results.json")?;

    // 4. Send to API
    println!("Sending to API...");
    let response = put_to_rest_api().await?;
    println!("Response: {} - {}", response.message1, response.message2);

    Ok(())
}