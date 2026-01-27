use thirtyfour::prelude::*;
use serde_json::json;
use std::time::Duration;
use crate::_02_serialization::LinksPayload;

pub async fn scrape_real_results_chrome() -> Result<LinksPayload, Box<dyn std::error::Error>> {
    // 1. Initialize Chrome Capabilities
    let mut caps = DesiredCapabilities::chrome();

    // 2. Add Chrome-specific options using raw JSON
    let chrome_options = json!({
        "args": [
            "--headless",
            "--disable-gpu",
            "--no-sandbox",
            "--disable-dev-shm-usage",
            "--disable-blink-features=AutomationControlled"
        ]
    });
    caps.insert("goog:chromeOptions".to_string(), chrome_options);

    // 3. Connect to the driver
    let driver = match WebDriver::new("http://localhost:9515", caps).await {
        Ok(d) => d,
        Err(e) => {
            panic!(
                "FAILED TO START DRIVER: {}\n\
                Ensure chromedriver.exe is running on port 9515. Path: c:/temp/driver",
                e
            );
        }
    };

    let mut all_links = Vec::new();

    let urls = vec![
        "https://www.kodis.cz/lines/city?tab=MHD+Ostrava",
        "https://www.kodis.cz/lines/region?tab=75",
        "https://www.kodis.cz/lines/city?tab=MHD+Opava",
        "https://www.kodis.cz/lines/region?tab=232-293",
        "https://www.kodis.cz/lines/city?tab=MHD+Frýdek-Místek",
        "https://www.kodis.cz/lines/region?tab=331-392",
        "https://www.kodis.cz/lines/city?tab=MHD+Havířov",
        "https://www.kodis.cz/lines/region?tab=440-465",
        "https://www.kodis.cz/lines/city?tab=MHD+Karviná",
        "https://www.kodis.cz/lines/city?tab=MHD+Orlová",
        "https://www.kodis.cz/lines/region?tab=531-583",
        "https://www.kodis.cz/lines/city?tab=MHD+Nový+Jičín",
        "https://www.kodis.cz/lines/city?tab=MHD+Studénka",
        "https://www.kodis.cz/lines/region?tab=613-699",
        "https://www.kodis.cz/lines/city?tab=MHD+Třinec",
        "https://www.kodis.cz/lines/city?tab=MHD+Český+Těšín",
        "https://www.kodis.cz/lines/region?tab=731-788",
        "https://www.kodis.cz/lines/city?tab=MHD+Krnov",
        "https://www.kodis.cz/lines/city?tab=MHD+Bruntál",
        "https://www.kodis.cz/lines/region?tab=811-885",
        "https://www.kodis.cz/lines/region?tab=901-990",
        "https://www.kodis.cz/lines/train?tab=S1-S34",
        "https://www.kodis.cz/lines/train?tab=R8-R62",
        "https://www.kodis.cz/lines/city?tab=NAD+MHD",
        "https://www.kodis.cz/lines/region?tab=NAD",
        "https://www.kodis.cz/lines/boat?tab=Lodní+doprava",
    ];

    let changes_base = "https://www.kodis.cz/changes/";
    let change_ids: Vec<i32> = (2115..2116).chain(2400..2800).collect();

    // Scrape changes pages - continue even if individual pages fail
    for id in change_ids {
        let url = format!("{}{}", changes_base, id);
        match scrape_page_pdfs(&driver, &url).await {
            Ok(links) => {
                if !links.is_empty() {
                    println!("✓ Changes {}: {} links", id, links.len());
                    all_links.extend(links);
                }
            }
            Err(e) => {
                eprintln!("✗ Changes {}: {}", id, e);
                // Continue to next change ID
            }
        }
    }

    // Scrape main URLs - continue even if individual pages fail
    for url in urls {
        match scrape_url_with_pagination(&driver, url).await {
            Ok(links) => {
                println!("✓ {} - {} links", url, links.len());
                all_links.extend(links);
            }
            Err(e) => {
                eprintln!("✗ Failed {}: {}", url, e);
                eprintln!("  Continuing with next URL...");
                // Continue to next URL instead of stopping
            }
        }
    }

    driver.quit().await?;

    all_links.sort();
    all_links.dedup();

    println!("\n=== Total unique links found: {} ===", all_links.len());
    Ok(LinksPayload { list: all_links })
}

// New function to handle a single URL with all its pagination
async fn scrape_url_with_pagination(driver: &WebDriver, url: &str) -> WebDriverResult<Vec<String>> {
    let mut page_links = Vec::new();

    // Navigate to URL
    driver.goto(url).await?;
    tokio::time::sleep(Duration::from_millis(2000)).await;

    // Try to click "Future timetables" buttons if they exist
    if let Ok(future_buttons) = driver.find_all(By::Css("button[title='Budoucí jízdní řády']")).await {
        for btn in future_buttons {
            if btn.is_displayed().await.unwrap_or(false) {
                // Click to show future timetables
                let _ = btn.click().await;
                tokio::time::sleep(Duration::from_millis(1500)).await;

                // Extract links while menu is open
                if let Ok(links) = extract_pdf_links(&driver).await {
                    page_links.extend(links);
                }

                // Click again to close
                let _ = btn.click().await;
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }
    }

    // Extract current page links
    if let Ok(links) = extract_pdf_links(&driver).await {
        page_links.extend(links);
    }

    // Paginate through "Další" (Next) buttons
    loop {
        match driver.find(By::LinkText("Další")).await {
            Ok(next_btn) => {
                if next_btn.is_displayed().await.unwrap_or(false)
                    && next_btn.is_enabled().await.unwrap_or(false) {

                    let _ = next_btn.click().await;
                    tokio::time::sleep(Duration::from_millis(2000)).await;

                    if let Ok(links) = extract_pdf_links(&driver).await {
                        page_links.extend(links);
                    }
                } else {
                    break; // Button exists but not clickable
                }
            }
            Err(_) => break, // No more "Next" buttons
        }
    }

    Ok(page_links)
}

async fn scrape_page_pdfs(driver: &WebDriver, url: &str) -> WebDriverResult<Vec<String>> {
    driver.goto(url).await?;
    tokio::time::sleep(Duration::from_millis(1000)).await;
    extract_pdf_links(driver).await
}

async fn extract_pdf_links(driver: &WebDriver) -> WebDriverResult<Vec<String>> {
    let mut results = Vec::new();
    let tags = driver.find_all(By::Tag("a")).await?;

    for tag in tags {
        if let Some(href) = tag.attr("href").await? {
            if href.ends_with(".pdf") && href.contains("kodis-files.s3.eu-central-1.amazonaws.com/") {
                results.push(href);
            }
        }
    }

    Ok(results)
}