use std::fs;
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct LinksPayload {
    pub list: Vec<String>,
}

/// Serialize LinksPayload → JSON file
pub fn serialize_to_json(
    payload: &LinksPayload,
    path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::to_string_pretty(payload)?;
    fs::write(path, json)?;
    Ok(())
}

/// Deserialize JSON file → LinksPayload
pub fn deserialize_from_json(
    path: &str,
) -> Result<LinksPayload, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path)?;
    let payload: LinksPayload = serde_json::from_str(&content)?;
    Ok(payload)
}

//    cd c:\temp\driver
//    chromedriver.exe --port=9515
