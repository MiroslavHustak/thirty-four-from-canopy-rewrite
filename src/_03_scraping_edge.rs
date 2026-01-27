use thirtyfour::prelude::*;
use serde_json::json;
use std::time::Duration;
use crate::_02_serialization::LinksPayload;
use crate::_05_links::{MAIN_URLS, CHANGES_BASE_URL, get_change_ids, get_main_urls_owned};

use futures::stream::{self, StreamExt};
use thirtyfour::By;
use futures::future::join_all;

pub async fn scrape_real_results_edge() -> Result<LinksPayload, Box<dyn std::error::Error>> {
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
    let driver = start_edge_driver().await?;

    let all_links = stream::iter(get_change_ids())
        .then(|id| {
            let driver = &driver;
            async move {
                let url = format!("{}{}", CHANGES_BASE_URL, id);

                match driver.goto(&url).await {
                    Ok(_) => {
                        tokio::time::sleep(Duration::from_millis(50)).await;

                        let links_appeared = stream::iter(0..20)
                            .then(|_| async {
                                match driver.find_all(By::Css("ul > li > div")).await {
                                    Ok(elements) if !elements.is_empty() => true,
                                    _ => {
                                        tokio::time::sleep(Duration::from_millis(250)).await;
                                        false
                                    }
                                }
                            })
                            .any(|found| futures::future::ready(found))
                            .await;

                        match links_appeared {
                            true => match extract_pdf_links(driver).await {
                                Ok(links) => Some(
                                    links
                                        .into_iter()
                                        .filter(|l| {
                                            l.contains("kodis-files.s3.eu-central-1.amazonaws.com/")
                                        })
                                        .collect::<Vec<_>>(),
                                ),
                                Err(_) => None,
                            },
                            false => None,
                        }
                    }
                    Err(_) => None,
                }
            }
        })
        .filter_map(|x| async { x })
        .flat_map(stream::iter)
        .filter(|link| {
            let keep = !link.contains("2022") && !link.contains("2023");
            futures::future::ready(keep)
        })
        .collect::<Vec<_>>()
        .await;

    driver.quit().await?;

    Ok(all_links)
}

// Matches F# currentAndFutureLinks()
async fn scrape_current_and_future_links() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let driver = start_edge_driver().await?;
    let mut all_links = Vec::new();

    for url in MAIN_URLS {
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
    let driver = start_edge_driver().await?;

    let all_links = stream::iter(MAIN_URLS)
        .then(|url| {
            let driver = &driver;
            async move {
                match scrape_url_current_only(driver, url).await {
                    Ok(links) => {
                        println!("✓ {}: {} links", url, links.len());
                        Some(links)
                    }
                    Err(e) => {
                        eprintln!("Na tomto odkazu to opravdu nezvládl: {}", url);
                        None
                    }
                }
            }
        })
        .filter_map(|x| async { x })
        .flat_map(stream::iter)
        .filter(|link| {
            let keep = !link.contains("2022") && !link.contains("2023");
            futures::future::ready(keep)
        })
        .collect::<Vec<_>>()
        .await;

    driver.quit().await?;

    Ok(all_links)
}

// Helper function to scrape a URL with current AND future timetables
async fn scrape_url_current_and_future(
    driver: &WebDriver,
    url: &str,
) -> Result<Vec<String>, WebDriverError> {
    driver.goto(url).await?;

    let first_page = scrape_with_future_buttons(driver).await?;

    let paginated = stream::unfold((), |_| async {
        match check_next_button_clickable(driver).await {
            false => None,
            true => match driver.find(By::LinkText("Další")).await {
                Ok(btn) => {
                    let _ = btn.click().await;
                    tokio::time::sleep(Duration::from_millis(2_000)).await;

                    match scrape_with_future_buttons(driver).await {
                        Ok(links) => Some((links, ())),
                        Err(_) => None,
                    }
                }
                Err(_) => None,
            },
        }
    })
        .flat_map(stream::iter)
        .collect::<Vec<_>>()
        .await;

    Ok(first_page.into_iter().chain(paginated).collect())
}

// Helper function to scrape a URL with ONLY current timetables
async fn scrape_url_current_only(
    driver: &WebDriver,
    url: &str,
) -> Result<Vec<String>, WebDriverError> {
    driver.goto(url).await?;

    let first_page = scrape_current_page(driver).await?;

    let paginated = stream::unfold((), |_| async {
        match check_next_button_clickable(driver).await {
            false => None,
            true => match driver.find(By::LinkText("Další")).await {
                Ok(btn) => {
                    let _ = btn.click().await;
                    tokio::time::sleep(Duration::from_millis(2_000)).await;

                    match scrape_current_page(driver).await {
                        Ok(links) => Some((links, ())),
                        Err(_) => None,
                    }
                }
                Err(_) => None,
            },
        }
    })
        .flat_map(stream::iter)
        .collect::<Vec<_>>()
        .await;

    Ok(first_page.into_iter().chain(paginated).collect())
}

// This matches the F# pdfLinkList() function with future buttons
use thirtyfour::{WebDriver};
use thirtyfour::error::WebDriverError;

async fn scrape_with_future_buttons(
    driver: &WebDriver,
) -> Result<Vec<String>, WebDriverError> {
    tokio::time::sleep(Duration::from_millis(15_000)).await;

    let links_shown = stream::iter(0..100)
        .then(|_| async {
            match driver.find_all(By::Css(".Card_actions__HhB_f")).await {
                Ok(elements) if !elements.is_empty() => true,
                _ => {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    false
                }
            }
        })
        .any(|shown| futures::future::ready(shown))
        .await;

    match links_shown {
        false => return Ok(Vec::new()),
        true => {}
    }

    let buttons = driver
        .find_all(By::Css("button[title='Budoucí jízdní řády']"))
        .await?;

    let last_index = buttons.len().saturating_sub(1);

    let all_links = stream::iter(buttons.into_iter().enumerate())
        .then(|(i, button)| async move {
            let _ = button.click().await;
            tokio::time::sleep(Duration::from_millis(2_000)).await;

            let extracted = match extract_pdf_links(driver).await {
                Ok(links) => links,
                Err(_) => Vec::new(),
            };

            match i == last_index {
                true => {
                    let _ = stream::iter(0..20)
                        .then(|_| async {
                            match driver
                                .find(By::Css("[id*='headlessui-menu-item']"))
                                .await
                            {
                                Ok(_) => true,
                                Err(_) => {
                                    tokio::time::sleep(Duration::from_millis(100)).await;
                                    false
                                }
                            }
                        })
                        .any(|found| futures::future::ready(found))
                        .await;

                    let _ = button.click().await;
                    tokio::time::sleep(Duration::from_millis(2_000)).await;
                }
                false => {
                    let _ = driver
                        .execute("window.history.back();", vec![])
                        .await;
                }
            }

            extracted
        })
        .flat_map(stream::iter)
        .collect::<Vec<_>>()
        .await;

    Ok(all_links)
}

// This matches the F# pdfLinkList() function without future buttons
async fn scrape_current_page(
    driver: &WebDriver,
) -> Result<Vec<String>, WebDriverError> {
    tokio::time::sleep(Duration::from_millis(15_000)).await;

    let links_shown = stream::iter(0..100)
        .then(|_| async {
            match driver.find_all(By::Css(".Card_actions__HhB_f")).await {
                Ok(elements) if !elements.is_empty() => true,
                _ => {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    false
                }
            }
        })
        .any(|shown| futures::future::ready(shown))
        .await;

    match links_shown {
        false => Ok(Vec::new()),
        true => extract_pdf_links(driver).await,
    }
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

async fn extract_pdf_links(
    driver: &WebDriver,
) -> Result<Vec<String>, WebDriverError> {
    let tags = driver.find_all(By::Tag("a")).await?;

    let hrefs = join_all(
        tags.into_iter().map(|tag| async move {
            match tag.attr("href").await {
                Ok(Some(href)) if href.ends_with(".pdf") => Some(href),
                _ => None,
            }
        }),
    )
        .await;

    Ok(hrefs.into_iter().flatten().collect())
}

async fn start_edge_driver() -> Result<WebDriver, Box<dyn std::error::Error>> {
    let mut caps = DesiredCapabilities::edge();

    let edge_options = json!({
        "args": [
            "--headless",
            "--disable-gpu",
            "--no-sandbox",
            "--disable-dev-shm-usage",
            "--disable-blink-features=AutomationControlled"
        ]
    });
    caps.insert("ms:edgeOptions".to_string(), edge_options);

    let driver = WebDriver::new("http://localhost:4444", caps).await?;
    Ok(driver)
}