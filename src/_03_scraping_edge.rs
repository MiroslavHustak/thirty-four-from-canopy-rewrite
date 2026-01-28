use thirtyfour::prelude::*;
use serde_json::json;
use std::time::Duration;
use futures::stream::{self, StreamExt};

//
// ======================================================
// Canopy-style wait primitive
// ======================================================
//

async fn wait_until<F>(
    timeout: Duration,
    mut condition: F,
) -> bool
where
    F: FnMut() -> futures::future::BoxFuture<'static, bool>,
{
    let start = tokio::time::Instant::now();

    while start.elapsed() < timeout {
        if condition().await {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    false
}

//
// ======================================================
// Page readiness condition (React-safe)
// ======================================================
//

fn page_ready(driver: WebDriver)
              -> impl FnMut() -> futures::future::BoxFuture<'static, bool>
{
    move || {
        let driver = driver.clone();
        Box::pin(async move {
            driver
                .find_all(By::Css(".Card_actions__HhB_f"))
                .await
                .map(|els| !els.is_empty())
                .unwrap_or(false)
        })
    }
}

//
// ======================================================
// Deterministic PDF extraction
// ======================================================
//

async fn extract_pdf_links(
    driver: &WebDriver,
) -> Result<Vec<String>, WebDriverError> {
    let anchors = driver
        .find_all(By::Css("a[href$='.pdf']"))
        .await?;

    let mut out = Vec::new();
    for a in anchors {
        if let Some(href) = a.attr("href").await? {
            out.push(href);
        }
    }
    Ok(out)
}

//
// ======================================================
// Browser DSL (Edge + Canopy semantics)
// ======================================================
//

#[derive(Clone)]
struct Browser {
    driver: WebDriver,
}

impl Browser {
    async fn goto(&self, url: &str) {
        let _ = self.driver.goto(url).await;
        self.wait_page().await;
    }

    async fn wait_page(&self) {
        wait_until(
            Duration::from_secs(20),
            page_ready(self.driver.clone()),
        )
            .await;
    }

    async fn click(&self, by: By) {
        if let Ok(el) = self.driver.find(by).await {
            let _ = el.click().await;
            self.wait_page().await;
        }
    }

    async fn back(&self) {
        let _ = self.driver.back().await;
        self.wait_page().await;
    }

    async fn pdf_links(&self) -> Vec<String> {
        extract_pdf_links(&self.driver)
            .await
            .unwrap_or_default()
    }

    async fn has_next(&self) -> bool {
        match self.driver.find(By::LinkText("Další")).await {
            Ok(btn) => {
                btn.is_displayed().await.unwrap_or(false)
                    && btn.is_enabled().await.unwrap_or(false)
            }
            Err(_) => false,
        }
    }

    async fn next(&self) {
        self.click(By::LinkText("Další")).await;
    }
}

//
// ======================================================
// scrape_current_page (Canopy equivalent)
// ======================================================
//

async fn scrape_current_page(browser: &Browser) -> Vec<String> {
    browser.wait_page().await;
    browser.pdf_links().await
}

//
// ======================================================
// scrape_with_future_buttons (strictly sequential)
// ======================================================
//

async fn scrape_with_future_buttons(browser: &Browser) -> Vec<String> {
    browser.wait_page().await;

    let buttons = match browser
        .driver
        .find_all(By::Css("button[title='Budoucí jízdní řády']"))
        .await
    {
        Ok(b) => b,
        Err(_) => return Vec::new(),
    };

    let last = buttons.len().saturating_sub(1);

    stream::iter(buttons.into_iter().enumerate())
        .fold(Vec::new(), |mut acc, (i, button)| {
            let browser = browser.clone();
            async move {
                let _ = button.click().await;
                browser.wait_page().await;

                acc.extend(browser.pdf_links().await);

                if i == last {
                    let _ = wait_until(Duration::from_secs(5), || {
                        let d = browser.driver.clone();
                        Box::pin(async move {
                            d.find(By::Css("[id*='headlessui-menu-item']"))
                                .await
                                .is_ok()
                        })
                    })
                        .await;

                    let _ = button.click().await;
                    browser.wait_page().await;
                } else {
                    browser.back().await;
                }

                acc
            }
        })
        .await
}

//
// ======================================================
// scrape_current_and_future (pagination)
// ======================================================
//

async fn scrape_current_and_future(
    browser: &Browser,
    url: &str,
) -> Vec<String> {
    browser.goto(url).await;

    let first = scrape_with_future_buttons(browser).await;

    let rest = stream::unfold(browser.clone(), |b| async move {
        match b.has_next().await {
            false => None,
            true => {
                b.next().await;
                Some((scrape_with_future_buttons(&b).await, b))
            }
        }
    })
        .flat_map(stream::iter)
        .collect::<Vec<_>>()
        .await;

    first.into_iter().chain(rest).collect()
}

//
// ======================================================
// scrape_current_only (pagination)
// ======================================================
//

async fn scrape_current_only(
    browser: &Browser,
    url: &str,
) -> Vec<String> {
    browser.goto(url).await;

    let first = scrape_current_page(browser).await;

    let rest = stream::unfold(browser.clone(), |b| async move {
        match b.has_next().await {
            false => None,
            true => {
                b.next().await;
                Some((scrape_current_page(&b).await, b))
            }
        }
    })
        .flat_map(stream::iter)
        .collect::<Vec<_>>()
        .await;

    first.into_iter().chain(rest).collect()
}

//
// ======================================================
// Edge driver bootstrap (explicit, stable)
// ======================================================
//

async fn start_edge_driver() -> Result<WebDriver, Box<dyn std::error::Error>> {
    let mut caps = DesiredCapabilities::edge();

    let edge_opts = json!({
        "args": [
            "--headless=new",
            "--disable-gpu",
            "--no-sandbox",
            "--disable-dev-shm-usage",
            "--window-size=1920,1080",
            "--disable-blink-features=AutomationControlled"
        ]
    });

    caps.insert("ms:edgeOptions".into(), edge_opts);

    Ok(WebDriver::new("http://localhost:4444", caps).await?)
}

//
// ======================================================
// PUBLIC ENTRY POINT (stress-test ready)
// ======================================================
//

pub async fn scrape_all(
    main_urls: &[&str],
) -> Result<Vec<String>, Box<dyn std::error::Error>> {

    let driver = start_edge_driver().await?;
    let browser = Browser { driver };

    let mut all_links = Vec::new();

    for &url in main_urls {
        let mut cf = scrape_current_and_future(&browser, url).await;
        let mut co = scrape_current_only(&browser, url).await;
        all_links.append(&mut cf);
        all_links.append(&mut co);
    }

    all_links.sort();
    all_links.dedup();

    browser.driver.quit().await?;

    Ok(all_links)
}