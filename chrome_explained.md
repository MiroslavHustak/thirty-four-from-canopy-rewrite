```rust
# Pragmatic Architect’s Checklist

## Core Warning Signals when using Rust for high-level tasks (like web scraping)

### 1. Async infection / architectural commitment
- The library forces `async fn` everywhere.
- Async spreads through large parts of the codebase even when you don't conceptually need concurrency.

### 2. Emulating higher-level abstractions
- You end up building polling loops, custom streams, `AsyncSeq`-like helpers, or wrappers just to express straightforward sequential logic.
- The language fights your desired abstraction level.

### 3. Readability suffers compared to F# / similar languages
- Control flow is obscured by `.await`, `.map()`, `.next()`, heavy error handling, and helper types.
- Simple workflows become visually noisy and harder to follow.

### 4. Negligible performance payoff in I/O-bound work
- Network latency, site response times, and rate limits dominate.
- Rust rarely delivers meaningful speed gains over F#, Python, or Go in typical scraping or automation scenarios.

### 5. Primarily used for language study rather than problem-solving
- You're learning Rust syntax, ownership, or async patterns.
- Pushing it into production-grade scraping architecture adds unnecessary complexity when the goal is understanding, not optimal deployment.

### 6. Abstractions hit a ceiling quickly
- Libraries like `thirtyfour` already feel "high enough".
- Adding more layers (custom combinators, streams, etc.) often makes code worse, not better.

### 7. Low-level guarantees are mostly unused
- You don't need zero-cost abstractions, manual memory control, no GC, deterministic layout, extreme throughput, or thread safety at scale.
- Rust's biggest strengths remain idle.

### 8. Runtime choice locks you in early
- Tokio (or async-std / smol) becomes a permanent decision.
- Switching or mixing ecosystems is painful and leads to subtle bugs.

### 9. Cancellation & drop surprises
- Async code is often cancellation-unsafe.
- Dropping futures can leak resources or leave inconsistent state, requiring extra mental models not present in synchronous or F# async.

### 10. Compile times become a bottleneck
- Even moderate async projects (tokio + reqwest + serde + futures + anyhow) produce long incremental compiles.
- Slows down experimentation and iteration.

### 11. Error handling noise overwhelms business logic
- Frequent `.map_err`, custom `From` impls, `anyhow!`/`thiserror` boilerplate.
- 40–60% of code can become error-conversion glue instead of domain logic.

### 12. Async ecosystem fragmentation
- No unified async iterators/streams story (`AsyncIterator` unstable, multiple competing crates).
- Leads to glue code and broken composability.

### 13. Testing async logic is noticeably harder
- Timer mocking, cancellation behavior, task spawning, and flakiness require more ceremony.
- Unit/integration tests feel heavier than in F#/ .NET.

### 14. Constant sync/async context switching tax
- You repeatedly ask: "Is this safe to call in async context? Will it block? Do I need spawn_blocking?"
- Subtle performance footguns appear easily.

### 15. Higher long-term maintenance cost for frequently changing code
- More explicit lifetimes, trait bounds, wrapper types accumulate.
- Fixing a broken selector or adding retry logic takes longer than it should in dynamic scripts.

use thirtyfour::prelude::*;
use serde_json::json;
use std::time::Duration;

use crate::_02_serialization::LinksPayload;
use crate::_05_links::{MAIN_URLS, CHANGES_BASE_URL, get_change_ids};

//Am I using async because the problem is naturally concurrent,
//or because the library forces me to?

//If the answer is: “Because the library forces me to” …then Rust may be the wrong layer. - coz tady asi je

/// ===================== Helper: Wait for elements =====================
// Rust ma tady async, ale nema ekvivalent Async.Seq...., takze nelze jedoduse udelat ekvivalentmeho kodu v F#
async fn wait_for_elements(
    driver: &WebDriver, //IWebDriver (aliased as browser)
    by: By,
    total_timeout: Duration,
    poll_interval: Duration,
) -> bool {
    let start = tokio::time::Instant::now();  //std::time::Instant::now() by mohlo byt, ale tokio verze umoznuje testing
    while start.elapsed() < total_timeout {
        match driver.find_all(by.clone()).await { //driver odpovida canopy.classic.browser
            Ok(elements) if !elements.is_empty() => return true,
            Ok(_) => { /* elements were empty → keep waiting */ }
            Err(_) => { /* network or timeout error → keep waiting */ }
        }
        tokio::time::sleep(poll_interval).await;  //do! Async.Sleep poll_interval
    }
    false
}

/*
// wait_for_elements je priblizny (ale async) ekvivalent tohoto:
let waitForWithTimeout (timeoutSeconds : float) (condition : unit -> bool option) =
    let timeout = System.TimeSpan.FromSeconds timeoutSeconds
    let sw = System.Diagnostics.Stopwatch.StartNew()

    Seq.initInfinite id
    |> Seq.takeWhile (fun _ -> sw.Elapsed < timeout)
    |> Seq.tryPick 
        (fun _ 
            -> 
            condition () 
            |> Option.orElse (System.Threading.Thread.Sleep 250; None)
        )
    |> Option.defaultValue false                           
*/

/// ===================== Chrome driver setup =====================
async fn start_chrome_driver() -> WebDriverResult<WebDriver> {
    let mut caps = DesiredCapabilities::chrome();
    let chrome_options = json!({  //json!/format!...macros, umoznuji mj. overloading, ktere Rust normalne nema, stejne jako F#. Ekv. to custom CEs umoznujicim overloading behaviousr
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
//Async<'T> in F# ≈ Future<Output = T> in Rust.
//move: forces the closure to take ownership of all variables it uses. Without move, the closure borrows variables from the surrounding scope.

async fn extract_pdf_links(driver: &WebDriver) -> WebDriverResult<Vec<String>> {
    let tags = driver.find_all(By::Tag("a")).await?;  //driver odpovida canopy.classic...
    let hrefs = futures::future::join_all(tags.into_iter().map(|tag| async move { //Async.Parallel is like join! / join_all / spawn 
        match tag.attr("href").await {
            Ok(Some(href)) if href.ends_with(".pdf") => Some(href),
            _ => None,
        }
    })).await;
    Ok(hrefs.into_iter().flatten().collect())
    //.into_iter().flatten() = List.choose id
    //.collect() = materialize into concrete collection, like List.ofSeq.
}
/*
 let scrapeGeneral () = 
    safeElements "a"  // canopy.classic.elements selector
    |> List.map 
        (fun item 
            ->                                                     
            let href = string <| item.GetAttribute("href")
            match href.EndsWith("pdf") with
            | true  -> Some href     
            | false -> None                                                                    
        )    
 */

/// ===================== Scrape changes links =====================

// tady je kod "nefunkcionalni" - async + ownership model introduces constraints that don’t exist in F#
Rust encourages mutation for sequential, side-effect-heavy code
for loops with Vec::extend are perfectly idiomatic.
Trying to force everything into a functional style can make code more complex, less readable, and error-prone.
Functional patterns work best for pure, concurrent, or independent computations
Mapping over collections that don’t need sequential awaits.
Transforming data without side effects.
Ownership + async = practical limitations
Futures capture ownership (async move) → pipelines are less natural than F#’s AsyncSeq.
Sequential operations with side effects are easier with mutable state.

pub async fn scrape_changes_links(driver: &WebDriver) -> WebDriverResult<Vec<String>> {
    let change_ids = get_change_ids();
    let mut all_links = Vec::new();

    for id in change_ids {
        let url = format!("{}{}", CHANGES_BASE_URL, id);
        if driver.goto(&url).await.is_ok() { //driver.goto(&url) = canopy.classic.url url
            tokio::time::sleep(Duration::from_millis(50)).await;

            let cards_present = wait_for_elements(
                driver,
                By::Css("ul > li > div"),
                Duration::from_secs(45),
                Duration::from_millis(400),
            ).await;

            if cards_present {
                let mut links = extract_pdf_links(driver).await?;
                links.retain(|l| l.contains("kodis-files.s3.eu-central-1.amazonaws.com/"));
                //retain is the mutable equivalent of filter in functional languages.
                links.retain(|l| !["2022", "2023", "2024"].iter().any(|y| l.contains(y)));
                all_links.extend(links);
            }
        }
    }

    Ok(all_links)
}

/*
let changesLinks () = 

    match startHeadlessEdge () with
    | Error _-> []
    | Ok _   ->
        try
            try
                let linksShown () = 
                    Some (safeElements "ul > li > div" |> Seq.length >= 1)
  
                let scrapeUrl (url : string) =

                    try
                        canopy.classic.url url
                        Thread.Sleep 50  
                                                    
                        let waitForWithTimeout (timeoutSeconds : float) (condition : unit -> bool option) =
                    
                            let timeout = System.TimeSpan.FromSeconds timeoutSeconds
                            let sw = System.Diagnostics.Stopwatch.StartNew()
                                                    
                            Seq.initInfinite id
                            |> Seq.takeWhile (fun _ -> sw.Elapsed < timeout)
                            |> Seq.tryPick (fun _ -> condition () |> Option.orElse (Thread.Sleep 250; None))
                            |> Option.defaultValue false                           
                    
                        match waitForWithTimeout 5.0 linksShown with
                        | true 
                            ->
                            scrapeGeneral ()
                            |> List.choose id  
                            |> List.distinct
                            |> List.filter (fun item -> item.Contains urlKodis)
                        | false 
                            ->
                            []
                    with
                    | _ -> []

                urlsChanges 
                |> List.collect scrapeUrl
                |> List.filter (fun item -> not (excludeYears |> List.exists item.Contains))
            with
            | _ -> []

        finally                     
            /// some code
*/
/// ===================== Scrape current/future pages =====================
async fn scrape_with_future_buttons(driver: &WebDriver) -> WebDriverResult<Vec<String>> {
    tokio::time::sleep(Duration::from_secs(25)).await;

    let cards_shown = wait_for_elements(
        driver,
        By::Css(".Card_actions__HhB_f"),
        Duration::from_secs(45),
        Duration::from_millis(400),
    ).await;

    match cards_shown {
    false => return Ok(Vec::new()),
    true => {} }

    let buttons = driver.find_all(By::Css("button[title='Budoucí jízdní řády']")).await?;
    let last_index = buttons.len().saturating_sub(1);

     let mut all_links = Vec::new();  //v Rustu mutable, nova kopie jako v F# sice lze, ale neni to idiomaticke

    for (i, button) in buttons.into_iter().enumerate() {
        for attempt in 0..3 {
            if attempt > 0 {
                tokio::time::sleep(Duration::from_millis(800)).await;
            }
            let _ = button.click().await;
        }
        tokio::time::sleep(Duration::from_secs(3)).await;

        let extracted = extract_pdf_links(driver).await.unwrap_or_default();

        match i == last_index {
            true => {
                match driver.find(By::Css("button[title='Budoucí jízdní řády']")).await.is_ok() {
                    true => {
                        let menu_button =
                            driver.find(By::Css("button[title='Budoucí jízdní řády']")).await.unwrap();
                        let _ = menu_button.click().await;
                        tokio::time::sleep(Duration::from_secs(3)).await;
                    }
                    false => {
                        // do nothing
                    }
                }
                /*
                match i == last_index {
                    true => {
                        match driver.find(By::Css("button[title='Budoucí jízdní řády']")).await {
                            Ok(menu_button) => {
                                let _ = menu_button.click().await;
                                tokio::time::sleep(Duration::from_secs(3)).await;
                            }
                            Err(_) => {
                                // do nothing
                            }
                        }
                    }
                    false => {
                        // do nothing
                    }
                }

                if let Ok(menu_button) =
                    driver.find(By::Css("button[title='Budoucí jízdní řády']")).await
                {
                    let _ = menu_button.click().await;
                    tokio::time::sleep(Duration::from_secs(3)).await;
                }
                */
            }
            false => {
                // do nothing
            }
        }

        all_links.extend(extracted);
        //all_links @ extracted
        //Seq.fold (fun acc x -> acc @ x) [] sequences
    }

    Ok(all_links)
}

async fn scrape_current_page(driver: &WebDriver) -> WebDriverResult<Vec<String>> {
    tokio::time::sleep(Duration::from_secs(25)).await;

    let cards_shown = wait_for_elements(
        driver,
        By::Css(".Card_actions__HhB_f"),
        Duration::from_secs(45),
        Duration::from_millis(400),
    ).await;

    if !cards_shown {
        return Ok(Vec::new());
    }

    extract_pdf_links(driver).await
}

/// ===================== Scrape current/future url =====================
async fn scrape_url_current_and_future(
    driver: &WebDriver,
    url: &str,
) -> WebDriverResult<Vec<String>> {
    driver.goto(url).await?;
    /*
    In Rust, an async fn returns a Future, which is just a value that represents a computation that might not be done yet.

    The executor (like tokio) repeatedly asks the future:
    
    “Are you done yet?”
    
    This asking is called polling.
    
    Each time the future is polled, it either:
    
    Returns Ready(value) → the computation is complete
    
    Returns Pending → the computation is not done, the executor should try again later
    
    So polling = repeatedly checking the future’s state until it finishes.
    */

    let mut all_links = scrape_with_future_buttons(driver).await?; //neco jako Async.Start |> Result.mapRust // await |> ?
    /*
    let mapRust res = //Ok je dole, tohle se v F# takto nerobi, neb se to nezkompiluje
    match res with
    | Ok value -> value
    | Error err -> Error err
    
    match x {
        Ok(v) => v,              // Vec<String>
        Err(e) => return Err(e),  // type ! ("never")
    };
    
    Rust and F# both require “all branches must have the same type”, but what “type” means is slightly different because Rust has a never type (!).
    
    Rust mental model
    match expr {
        produce_value_type => "produces Vec<String>"
        never_returns    => "type !, can act as Vec<String> here"
    }
    */

    loop {
        let next_clickable = match driver.find(By::LinkText("Další")).await {
            Ok(btn) => btn.is_displayed().await.unwrap_or(false) && btn.is_enabled().await.unwrap_or(false),
            Err(_) => false,
        };

        if !next_clickable {
            break;
        }

        if let Ok(btn) = driver.find(By::LinkText("Další")).await {
            let _ = btn.click().await;
            let _ = wait_for_elements(
                driver,
                By::Css(".Card_actions__HhB_f"),
                Duration::from_secs(25),
                Duration::from_millis(500),
            ).await;
            all_links.extend(scrape_with_future_buttons(driver).await?);
        }
    }

    Ok(all_links)
}

async fn scrape_url_current_only(
    driver: &WebDriver,
    url: &str,
) -> WebDriverResult<Vec<String>> {
    driver.goto(url).await?;

    let mut all_links = scrape_current_page(driver).await?;

    loop {
        let next_clickable = match driver.find(By::LinkText("Další")).await {
            Ok(btn) => btn.is_displayed().await.unwrap_or(false) && btn.is_enabled().await.unwrap_or(false),
            Err(_) => false,
        };

        if !next_clickable {
            break;
        }

        if let Ok(btn) = driver.find(By::LinkText("Další")).await {
            let _ = btn.click().await;
            let _ = wait_for_elements(
                driver,
                By::Css(".Card_actions__HhB_f"),
                Duration::from_secs(25),
                Duration::from_millis(500),
            ).await;
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

    let _ = driver.quit().await;

    all_links.sort(); //Array.sort
    all_links.dedup(); //Array.distinct  //delete duplicates

    println!("=== Total unique links: {} ===", all_links.len());

    Ok(LinksPayload { list: all_links })
}

| F# Function / Module       | Rust Equivalent (Iterators)                      | Notes / Comments |
|----------------------------|--------------------------------------------------|-----------------|
| `List.map f`               | `.map(f)`                                        | Transforms each element. Lazy in Rust iterator. |
| `Seq.map f`                | `.map(f)`                                        | Same as List.map but lazy. |
| `List.filter pred`         | `.filter(pred)`                                  | Keep elements that satisfy the predicate. |
| `Seq.filter pred`          | `.filter(pred)`                                  | Lazy, like Seq. |
| `List.choose f`            | `.filter_map(f)` / `.flatten()` with Option      | f returns `Some(x)` or `None`. Flatten unwraps Some. |
| `Seq.choose f`             | `.filter_map(f)` / `.flatten()` with Option      | Lazy version. |
| `List.fold f init lst`     | `.fold(init, f)`                                 | Accumulate a value over iterator. |
| `Seq.fold f init seq`      | `.fold(init, f)`                                 | Lazy version. |
| `List.iter f`              | `.for_each(f)`                                   | Apply function, return `()`. Iterator must be consumed. |
| `Seq.iter f`               | `.for_each(f)`                                   | Lazy; must be consumed to run. |
| `List.sum`                 | `.sum::<T>()`                                    | Sum elements; T must implement `Sum`. |
| `List.min` / `List.max`    | `.min()` / `.max()`                              | Returns `Option<T>` (None if empty). |
| `List.distinct`            | `.unique()` via itertools crate or `HashSet`     | Rust std iter doesn’t have distinct; often `.collect::<HashSet<_>>()`. |
| `List.append a b`          | `.chain(b)`                                      | Lazy concatenation of iterators. |
| `List.collect f lst`       | `.flat_map(f)`                                   | Map and flatten in one step. |
| `Seq.toList` / `Seq.toArray` | `.collect::<Vec<_>>()`                         | Materialize iterator into concrete collection. |
| `List.exists pred`         | `.any(pred)`                                     | Returns true if any element satisfies predicate. |
| `List.forall pred`         | `.all(pred)`                                     | True if all elements satisfy predicate. |
| `List.tryFind pred`        | `.find(pred)`                                    | Returns `Option<T>`. |
| `List.tryPick f`           | `.filter_map(f).next()`                          | f returns Option; pick first Some. |
| `Seq.init n f`             | `(0..n).map(f)`                                  | Creates sequence of n elements. |
| `List.zip a b`             | `a.zip(b)`                                       | Pairs elements of two iterators. |
| `List.unzip`               | `.unzip()`                                       | Splits iterator of tuples into two collections. |
| `List.rev`                 | `.rev()`                                         | Reverse iterator. Lazy. |
| `List.partition pred`      | `.partition(pred)`                               | Splits into (matching, not matching). |
| `List.splitAt n`           | `.take(n)` / `.skip(n)` + collect                | Not exact; split iterator manually. |
| `List.tryHead`             | `.next()`                                        | Returns `Option<T>` of first element. |
| `Seq.iteri f`              | `.enumerate().for_each(|(i, x)| f(i, x))`        | Add index while iterating. |

Key points in Rust:

Iterator chains are always lazy:

vec.iter().map(...).filter(...).flat_map(...) → nothing executes until you call .collect(), .for_each(), .fold(), or .next().

Concrete collections are eager:

.collect::<Vec<_>>() materializes the iterator into a fully evaluated vector.

Like converting Seq to List in F#.

Infinite sequences are possible:

(0..), std::iter::repeat(...), std::iter::repeat_with(...) → lazy, can generate infinite sequences.

Methods like .map(), .filter(), .flat_map() do not allocate memory immediately:

Just build the pipeline. Execution happens on consumption.

F# vs Rust Laziness Summary
Feature	F#	Rust
Lazy sequence	Seq	Iterator
Eager list	List	Vec<T>
Infinite sequences	Seq.initInfinite	(0..) or repeat_with()
Map/filter/collect	Lazy only for Seq	Always lazy on iterators; eager when .collect()

💡 Mental shortcut:

F#’s Seq ≈ Rust’s Iterator.
F#’s List / Array ≈ Rust’s Vec<T> / [T; N].

Everything else (map, filter, flat_map) is just pipeline building and is lazy in Rust, unlike F# where you have to explicitly choose Seq for laziness.


Non-iterators:

F# Function / Module          | Rust Method / Exists On                           | What it Does / Notes
------------------------------|---------------------------------------------------|-----------------------------------------------
List.length / Array.length    | .len() (Vec, slice, array)                        | Returns number of elements
List.isEmpty                  | .is_empty() (Vec, slice)                          | Returns true if empty
List.head / Array.head        | .first() (Vec, slice)                             | Returns Option<&T>; panics if using [0] on empty Vec
List.tail                     | &vec[1..] slice                                   | Returns slice without first element
List.append a b               | .extend(b) (Vec) or [a, b].concat()              | Adds elements of b to a (mutable); concat creates new Vec
List.rev                      | .into_iter().rev().collect() or .reverse()       | .reverse() is in-place (mutable); .rev() for iterator
List.insert i x list          | .insert(index, value) (Vec)                      | Inserts element at index; shifts others right
List.remove i list            | .remove(index) (Vec)                             | Removes element at index; returns it; shifts others left
List.item i                   | [i] / .get(i) (Vec, slice)                       | [i] panics if out of bounds; .get(i) returns Option<&T>
List.tryHead                  | .first() (Vec, slice)                            | Returns Option<&T>
List.tryLast                  | .last() (Vec, slice)                             | Returns Option<&T>
List.take n                   | &vec[..n] or .iter().take(n)                     | .truncate(n) modifies in place; slicing/iterator is non-destructive
List.skip n                   | &vec[n..] or .iter().skip(n)                     | .split_off(n) splits Vec; slicing/iterator is non-destructive
List.findIndex pred           | .iter().position(|x| pred(x))                    | Returns Option<usize>
List.contains x               | .contains(&x) (Vec, slice)                       | Returns true if element exists
List.copy / Array.copy        | .clone() / .to_vec()                             | Deep copy (clone) of Vec; .to_vec() for slices
Array.clear                   | .clear() (Vec)                                   | Removes all elements in place
List.append [x] (at end)      | .push(x) (Vec)                                   | Add element at end
N/A (mutable operation)       | .pop() (Vec)                                     | Remove last element; returns Option<T>
Array.set arr i x             | vec[i] = x                                       | Mutable assignment; panics if out of bounds
List.sort                     | .sort() (Vec)                                    | Sort in place; requires T: Ord
List.sortBy f                 | .sort_by_key(|x| f(x)) or .sort_by()            | Sort using key function or comparator
List.sortDescending           | .sort_by(|a,b| b.cmp(a))                        | Reverse order sort
List.max / List.min           | .iter().max() / .iter().min()                   | Returns Option<&T>; requires T: Ord
List.fold f init              | .iter().fold(init, |acc, x| f(acc, x))          | Left fold accumulator
List.iter f                   | .iter().for_each(|x| f(x))                      | Apply function to each element for side effects
List.tryFind pred             | .iter().find(|x| pred(*x))                      | Returns Option<&T>
List.partition pred           | .into_iter().partition(|x| pred(x))             | Returns (Vec<T>, Vec<T>); consumes original
List.splitAt n                | .split_at(n) (slice)                            | Returns (&[T], &[T]) tuple of slices
List.zip a b                  | a.iter().zip(b).collect::<Vec<_>>()             | Pairs elements; stops at shorter length
List.unzip                    | .iter().cloned().unzip() or .into_iter().unzip()| Splits Vec<(A,B)> into (Vec<A>, Vec<B>)
List.rev / Array.rev          | .reverse() or .iter().rev()                     | In-place (mutable) or iterator-based
Array.blit src dst            | .copy_from_slice() (slice)                      | Copies slice data into another mutable slice