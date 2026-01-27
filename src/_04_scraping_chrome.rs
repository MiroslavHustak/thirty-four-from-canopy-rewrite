use thirtyfour::prelude::*;
use serde_json::json;
use std::time::Duration;
use crate::_02_serialization::LinksPayload;

pub async fn scrape_real_results_chrome() -> Result<LinksPayload, Box<dyn std::error::Error>> {
    let mut all_links = Vec::new();

    // F# runs THREE separate browser sessions - we need to do the same
    println!("=== Starting changesLinks() ===");
    match scrape_changes_links().await {
        Ok(links) => {
            println!("changesLinks found {} links", links.len());
            all_links.extend(links);
        }
        Err(e) => eprintln!("changesLinks failed: {}", e),
    }

    println!("\n=== Starting currentAndFutureLinks() ===");
    match scrape_current_and_future_links().await {
        Ok(links) => {
            println!("currentAndFutureLinks found {} links", links.len());
            all_links.extend(links);
        }
        Err(e) => eprintln!("currentAndFutureLinks failed: {}", e),
    }

    println!("\n=== Starting currentLinks() ===");
    match scrape_current_links().await {
        Ok(links) => {
            println!("currentLinks found {} links", links.len());
            all_links.extend(links);
        }
        Err(e) => eprintln!("currentLinks failed: {}", e),
    }

    all_links.sort();
    all_links.dedup();

    println!("\n=== Total unique links: {} ===", all_links.len());
    Ok(LinksPayload { list: all_links })
}

// Matches F# changesLinks()
async fn scrape_changes_links() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let driver = start_chrome_driver().await?;
    let mut all_links = Vec::new();

    let changes_base = "https://www.kodis.cz/changes/";
    let change_ids: Vec<i32> = std::iter::once(2115).chain(2400..2800).collect();

    for id in change_ids {
        let url = format!("{}{}", changes_base, id);

        match driver.goto(&url).await {
            Ok(_) => {
                tokio::time::sleep(Duration::from_millis(50)).await;

                // Wait for links to appear (with timeout)
                let mut links_appeared = false;
                for _ in 0..20 { // 5 second timeout (20 * 250ms)
                    if let Ok(elements) = driver.find_all(By::Css("ul > li > div")).await {
                        if elements.len() >= 1 {
                            links_appeared = true;
                            break;
                        }
                    }
                    tokio::time::sleep(Duration::from_millis(250)).await;
                }

                if links_appeared {
                    if let Ok(links) = extract_pdf_links(&driver).await {
                        let filtered: Vec<String> = links.into_iter()
                            .filter(|link| link.contains("kodis-files.s3.eu-central-1.amazonaws.com/"))
                            .collect();
                        all_links.extend(filtered);
                    }
                }
            }
            Err(_) => continue,
        }
    }

    driver.quit().await?;

    let filtered: Vec<String> = all_links.into_iter()
        .filter(|item| !item.contains("2022") && !item.contains("2023"))
        .collect();

    Ok(filtered)
}

// Matches F# currentAndFutureLinks()
async fn scrape_current_and_future_links() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let driver = start_chrome_driver().await?;
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

    for url in urls {
        match scrape_url_current_and_future(&driver, url).await {
            Ok(links) => {
                println!("✓ {}: {} links", url, links.len());
                all_links.extend(links);
            }
            Err(e) => {
                eprintln!("Na tomto odkazu se buď momentálně nenachází žádné JŘ, anebo to nezvládl: {}", url);
                eprintln!("Zkusíme něco dalšího.");
            }
        }
    }

    driver.quit().await?;

    let filtered: Vec<String> = all_links.into_iter()
        .filter(|item| !item.contains("2022") && !item.contains("2023"))
        .collect();

    Ok(filtered)
}

// Matches F# currentLinks()
async fn scrape_current_links() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let driver = start_chrome_driver().await?;
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

    for url in urls {
        match scrape_url_current_only(&driver, url).await {
            Ok(links) => {
                println!("✓ {}: {} links", url, links.len());
                all_links.extend(links);
            }
            Err(e) => {
                eprintln!("Na tomto odkazu to opravdu nezvládl: {}", url);
            }
        }
    }

    driver.quit().await?;

    let filtered: Vec<String> = all_links.into_iter()
        .filter(|item| !item.contains("2022") && !item.contains("2023"))
        .collect();

    Ok(filtered)
}

// Helper function to scrape a URL with current AND future timetables
async fn scrape_url_current_and_future(driver: &WebDriver, url: &str) -> WebDriverResult<Vec<String>> {
    driver.goto(url).await?;

    let mut all_page_links = Vec::new();

    // First pass - click all future buttons
    let pdf_link_list_1 = scrape_with_future_buttons(driver).await?;
    all_page_links.extend(pdf_link_list_1);

    // Paginate through "Další" buttons
    loop {
        if !check_next_button_clickable(driver).await {
            break;
        }

        match driver.find(By::LinkText("Další")).await {
            Ok(btn) => {
                let _ = btn.click().await;
                tokio::time::sleep(Duration::from_millis(2000)).await;

                let links = scrape_with_future_buttons(driver).await?;
                all_page_links.extend(links);
            }
            Err(_) => break,
        }
    }

    Ok(all_page_links)
}

// Helper function to scrape a URL with ONLY current timetables
async fn scrape_url_current_only(driver: &WebDriver, url: &str) -> WebDriverResult<Vec<String>> {
    driver.goto(url).await?;

    let mut all_page_links = Vec::new();

    // First pass - no future buttons
    let pdf_link_list_1 = scrape_current_page(driver).await?;
    all_page_links.extend(pdf_link_list_1);

    // Paginate through "Další" buttons
    loop {
        if !check_next_button_clickable(driver).await {
            break;
        }

        match driver.find(By::LinkText("Další")).await {
            Ok(btn) => {
                let _ = btn.click().await;
                tokio::time::sleep(Duration::from_millis(2000)).await;

                let links = scrape_current_page(driver).await?;
                all_page_links.extend(links);
            }
            Err(_) => break,
        }
    }

    Ok(all_page_links)
}

// This matches the F# pdfLinkList() function with future buttons
async fn scrape_with_future_buttons(driver: &WebDriver) -> WebDriverResult<Vec<String>> {
    tokio::time::sleep(Duration::from_millis(15000)).await;

    // Wait for links to show
    let mut links_shown = false;
    for _ in 0..100 {
        if let Ok(elements) = driver.find_all(By::Css(".Card_actions__HhB_f")).await {
            if elements.len() >= 1 {
                links_shown = true;
                break;
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    if !links_shown {
        return Ok(Vec::new());
    }

    let mut all_links = Vec::new();

    // Get all "Budoucí jízdní řády" buttons
    let buttons = driver.find_all(By::Css("button[title='Budoucí jízdní řády']")).await?;
    let button_count = buttons.len();

    for (i, button) in buttons.into_iter().enumerate() {
        // Click button to show future timetables
        let _ = button.click().await;
        tokio::time::sleep(Duration::from_millis(2000)).await;

        // Extract links
        if let Ok(links) = extract_pdf_links(driver).await {
            all_links.extend(links);
        }

        // CRITICAL: For the last button, wait for popup and click again to reveal "Další"
        if i == button_count - 1 {
            // Wait for headlessui menu item popup
            for _ in 0..20 {
                if let Ok(_) = driver.find(By::Css("[id*='headlessui-menu-item']")).await {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }

            // Click last button again to close and reveal "Další"
            let _ = button.click().await;
            tokio::time::sleep(Duration::from_millis(2000)).await;
        } else {
            // For non-last buttons, navigate back
            let _ = driver.execute("window.history.back();", vec![]).await;
        }
    }

    Ok(all_links)
}

// This matches the F# pdfLinkList() function without future buttons
async fn scrape_current_page(driver: &WebDriver) -> WebDriverResult<Vec<String>> {
    tokio::time::sleep(Duration::from_millis(15000)).await;

    // Wait for links to show
    let mut links_shown = false;
    for _ in 0..100 {
        if let Ok(elements) = driver.find_all(By::Css(".Card_actions__HhB_f")).await {
            if elements.len() >= 1 {
                links_shown = true;
                break;
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    if !links_shown {
        return Ok(Vec::new());
    }

    extract_pdf_links(driver).await
}

async fn check_next_button_clickable(driver: &WebDriver) -> bool {
    match driver.find(By::LinkText("Další")).await {
        Ok(btn) => {
            btn.is_displayed().await.unwrap_or(false)
                && btn.is_enabled().await.unwrap_or(false)
        }
        Err(_) => false,
    }
}

async fn extract_pdf_links(driver: &WebDriver) -> WebDriverResult<Vec<String>> {
    let mut results = Vec::new();
    let tags = driver.find_all(By::Tag("a")).await?;

    for tag in tags {
        if let Some(href) = tag.attr("href").await? {
            if href.ends_with(".pdf") {
                results.push(href);
            }
        }
    }

    Ok(results)
}

async fn start_chrome_driver() -> Result<WebDriver, Box<dyn std::error::Error>> {
    let mut caps = DesiredCapabilities::chrome();

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

    let driver = WebDriver::new("http://localhost:9515", caps).await?;
    Ok(driver)
}
