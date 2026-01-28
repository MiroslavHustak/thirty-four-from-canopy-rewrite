use thirtyfour::prelude::*;
use serde_json::json;
//use futures::stream::{StreamExt};
use std::time::Duration;
use crate::_02_serialization::LinksPayload;
use crate::_05_links::{MAIN_URLS, CHANGES_BASE_URL, get_change_ids};

/// ===================== Helper: Wait for elements =====================
async fn wait_for_elements(
    driver: &WebDriver,
    by: By,
    total_timeout: Duration,
    poll_interval: Duration,
) -> bool {
    let start = tokio::time::Instant::now();
    while start.elapsed() < total_timeout {
        if let Ok(els) = driver.find_all(by.clone()).await {
            if !els.is_empty() {
                return true;
            }
        }
        tokio::time::sleep(poll_interval).await;
    }
    false
}

/// ===================== Chrome driver setup =====================
async fn start_chrome_driver() -> WebDriverResult<WebDriver> {
    let mut caps = DesiredCapabilities::chrome();
    let chrome_options = json!({
        "args": [
            "--headless=new",
            "--disable-gpu",
            "--no-sandbox",
            "--disable-dev-shm-usage",
            "--disable-blink-features=AutomationControlled",
            "--window-size=1920,1080"
        ]
    });
    caps.insert("goog:chromeOptions".to_string(), chrome_options);
    WebDriver::new("http://localhost:9515", caps).await
}

/// ===================== Extract PDF links =====================
async fn extract_pdf_links(driver: &WebDriver) -> WebDriverResult<Vec<String>> {
    let tags = driver.find_all(By::Tag("a")).await?;
    let hrefs = futures::future::join_all(tags.into_iter().map(|tag| async move {
        match tag.attr("href").await {
            Ok(Some(href)) if href.ends_with(".pdf") => Some(href),
            _ => None,
        }
    }))
        .await;
    Ok(hrefs.into_iter().flatten().collect())
}

/// ===================== Scrape changes links =====================
pub async fn scrape_changes_links(driver: &WebDriver) -> WebDriverResult<Vec<String>> {
    let change_ids = get_change_ids();
    let mut all_links = Vec::new();

    for id in change_ids {
        let url = format!("{}{}", CHANGES_BASE_URL, id);
        if driver.goto(&url).await.is_ok() {
            // mimic F# Thread.Sleep 50ms
            tokio::time::sleep(Duration::from_millis(50)).await;

            // wait up to 45s for cards
            let cards_present = wait_for_elements(
                driver,
                By::Css("ul > li > div"),
                Duration::from_secs(45),
                Duration::from_millis(400)
            ).await;

            if cards_present {
                let mut links = extract_pdf_links(driver).await?;
                links.retain(|l| l.contains("kodis-files.s3.eu-central-1.amazonaws.com/"));
                // filter out excluded years
                links.retain(|l| !["2022","2023","2024"].iter().any(|y| l.contains(y)));
                all_links.extend(links);
            }
        }
    }

    Ok(all_links)
}

/// ===================== Scrape current/future pages =====================
async fn scrape_with_future_buttons(driver: &WebDriver) -> WebDriverResult<Vec<String>> {
    tokio::time::sleep(Duration::from_secs(25)).await; // wait like Canopy

    let cards_shown = wait_for_elements(
        driver,
        By::Css(".Card_actions__HhB_f"),
        Duration::from_secs(45),
        Duration::from_millis(400)
    ).await;

    if !cards_shown {
        return Ok(Vec::new());
    }

    let buttons = driver.find_all(By::Css("button[title='Budoucí jízdní řády']")).await?;
    let last_index = buttons.len().saturating_sub(1);

    let mut all_links = Vec::new();

    for (i, button) in buttons.into_iter().enumerate() {
        // retry click up to 3 times
        for attempt in 0..3 {
            if attempt > 0 {
                tokio::time::sleep(Duration::from_millis(800)).await;
            }
            let _ = button.click().await;
        }
        tokio::time::sleep(Duration::from_secs(3)).await;

        let extracted = extract_pdf_links(driver).await.unwrap_or_default();

        if i == last_index {
            // mimic Canopy re-click menu button
            if let Ok(menu_button) = driver.find(By::Css("button[title='Budoucí jízdní řády']")).await {
                let _ = menu_button.click().await;
                tokio::time::sleep(Duration::from_secs(3)).await;
            }
        } else {
            // click menu again instead of history.back()
            if let Ok(menu_button) = driver.find(By::Css("button[title='Budoucí jízdní řády']")).await {
                let _ = menu_button.click().await;
                tokio::time::sleep(Duration::from_secs(3)).await;
            }
        }

        all_links.extend(extracted);
    }

    Ok(all_links)
}

async fn scrape_current_page(driver: &WebDriver) -> WebDriverResult<Vec<String>> {
    tokio::time::sleep(Duration::from_secs(25)).await;

    let cards_shown = wait_for_elements(
        driver,
        By::Css(".Card_actions__HhB_f"),
        Duration::from_secs(45),
        Duration::from_millis(400)
    ).await;

    if !cards_shown {
        return Ok(Vec::new());
    }

    extract_pdf_links(driver).await
}

/// ===================== Scrape current/future url =====================
async fn scrape_url_current_and_future(driver: &WebDriver, url: &str) -> WebDriverResult<Vec<String>> {
    driver.goto(url).await?;
    let mut all_links = scrape_with_future_buttons(driver).await?;

    // pagination like Canopy
    loop {
        let next_clickable = match driver.find(By::LinkText("Další")).await {
            Ok(btn) => btn.is_displayed().await.unwrap_or(false) && btn.is_enabled().await.unwrap_or(false),
            Err(_) => false,
        };

        if !next_clickable { break; }

        if let Ok(btn) = driver.find(By::LinkText("Další")).await {
            let _ = btn.click().await;
            // wait for cards after page change
            let _ = wait_for_elements(driver, By::Css(".Card_actions__HhB_f"), Duration::from_secs(25), Duration::from_millis(500)).await;
            all_links.extend(scrape_with_future_buttons(driver).await?);
        }
    }

    Ok(all_links)
}

async fn scrape_url_current_only(driver: &WebDriver, url: &str) -> WebDriverResult<Vec<String>> {
    driver.goto(url).await?;
    let mut all_links = scrape_current_page(driver).await?;

    loop {
        let next_clickable = match driver.find(By::LinkText("Další")).await {
            Ok(btn) => btn.is_displayed().await.unwrap_or(false) && btn.is_enabled().await.unwrap_or(false),
            Err(_) => false,
        };

        if !next_clickable { break; }

        if let Ok(btn) = driver.find(By::LinkText("Další")).await {
            let _ = btn.click().await;
            let _ = wait_for_elements(driver, By::Css(".Card_actions__HhB_f"), Duration::from_secs(25), Duration::from_millis(500)).await;
            all_links.extend(scrape_current_page(driver).await?);
        }
    }

    Ok(all_links)
}

/// ===================== Main scraper entry =====================
pub async fn scrape_real_results_chrome() -> Result<LinksPayload, Box<dyn std::error::Error>> {
    let driver = start_chrome_driver().await?;

    println!("=== Starting changesLinks() ===");
    let mut all_links = scrape_changes_links(&driver).await.unwrap_or_default();

    println!("=== Starting currentAndFutureLinks() ===");
    for url in MAIN_URLS {
        all_links.extend(scrape_url_current_and_future(&driver, url).await.unwrap_or_default());
    }

    println!("=== Starting currentLinks() ===");
    for url in MAIN_URLS {
        all_links.extend(scrape_url_current_only(&driver, url).await.unwrap_or_default());
    }

    driver.quit().await?;

    all_links.sort();
    all_links.dedup();

    println!("=== Total unique links: {} ===", all_links.len());
    Ok(LinksPayload { list: all_links })
}