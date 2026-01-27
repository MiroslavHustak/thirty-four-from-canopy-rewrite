//cd c:/temp
//chromedriver.exe --port=9515

mod _01_http_client;
mod _02_serialization;
mod _03_scraping_edge;
mod _04_scraping_chrome;
mod _05_links;

use std::fs;
use std::time::{SystemTime, UNIX_EPOCH, Instant, Duration};
use chrono::{DateTime, Local, Utc};

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

fn format_duration(duration: Duration) -> String {
    let total_seconds = duration.as_secs();
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    let millis = duration.subsec_millis();

    match (hours, minutes) {
        (h, m, ..) if h > 0 => format!("{}h {}m {}s", h, m, seconds),
        (_, m, ..) if m > 0 => format!("{}m {}s", m, seconds),
        _ => format!("{}.{:03}s", seconds, millis),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting Scraper...");
    let start = Instant::now();

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

    let elapsed = start.elapsed();
    println!("Took: {}", format_duration(elapsed));

    Ok(())
}