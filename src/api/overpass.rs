use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::collections::HashMap;
use std::time::Duration;

const OVERPASS_URL: &str = "https://overpass-api.de/api/interpreter";
const USER_AGENT: &str = "mapto3d/0.1.0 (https://github.com/shantanugoel/mapto3d)";

/// Raw Overpass API response
#[derive(Debug, Deserialize)]
pub struct OverpassResponse {
    pub elements: Vec<Element>,
}

/// A single element from Overpass (node or way)
#[derive(Debug, Deserialize)]
pub struct Element {
    #[serde(rename = "type")]
    pub type_: String,
    pub id: u64,
    #[serde(default)]
    pub nodes: Option<Vec<u64>>,
    #[serde(default)]
    pub tags: Option<HashMap<String, String>>,
    #[serde(default)]
    pub lat: Option<f64>,
    #[serde(default)]
    pub lon: Option<f64>,
}

/// Calculate bounding box from center point and radius
fn calculate_bbox(center: (f64, f64), radius_m: u32) -> (f64, f64, f64, f64) {
    let (lat, lon) = center;
    let radius_km = radius_m as f64 / 1000.0;

    // Approximate degrees per km
    // 1 degree latitude ≈ 111 km
    // 1 degree longitude ≈ 111 km * cos(lat)
    let lat_delta = radius_km / 111.0;
    let lon_delta = radius_km / (111.0 * lat.to_radians().cos());

    let south = lat - lat_delta;
    let north = lat + lat_delta;
    let west = lon - lon_delta;
    let east = lon + lon_delta;

    (south, west, north, east)
}

/// Fetch road data from Overpass API
///
/// # Arguments
/// * `center` - (lat, lon) center point
/// * `radius_m` - Radius in meters
///
/// # Returns
/// * `OverpassResponse` containing all highway ways and their nodes
pub fn fetch_roads(center: (f64, f64), radius_m: u32) -> Result<OverpassResponse> {
    let (south, west, north, east) = calculate_bbox(center, radius_m);

    // Overpass QL query for highways
    // Use 180s timeout to match OSMnx's default - 60s is often too short for larger areas
    let query = format!(
        r#"[out:json][timeout:180];
(
  way["highway"]({south},{west},{north},{east});
);
out body;
>;
out skel qt;"#,
        south = south,
        west = west,
        north = north,
        east = east
    );

    execute_overpass_query(&query)
}

/// Execute an Overpass API query with retry logic for 504 errors
fn execute_overpass_query(query: &str) -> Result<OverpassResponse> {
    let client = reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(200)) // Client timeout slightly higher than server's 180s
        .build()
        .context("Failed to create HTTP client")?;

    // Retry logic for 504 Gateway Timeout errors (common with Overpass)
    let max_retries = 3;
    let mut last_error = None;

    for attempt in 0..max_retries {
        if attempt > 0 {
            // Wait before retry - Overpass recommends waiting when overloaded
            let wait_secs = 30 * attempt as u64;
            eprintln!(
                "Overpass API timeout, retrying in {} seconds (attempt {}/{})",
                wait_secs,
                attempt + 1,
                max_retries
            );
            std::thread::sleep(Duration::from_secs(wait_secs));
        }

        // IMPORTANT: Overpass API expects form-encoded POST data, not raw body
        // The query must be sent as: data=<query>
        let response = client
            .post(OVERPASS_URL)
            .form(&[("data", query)])
            .send()
            .context("Failed to send request to Overpass API")?;

        match response.status().as_u16() {
            200 => {
                let result: OverpassResponse = response
                    .json()
                    .context("Failed to parse Overpass JSON response")?;
                return Ok(result);
            }
            429 | 504 => {
                // 429 = Too Many Requests, 504 = Gateway Timeout
                // These are retriable errors
                last_error = Some(format!(
                    "Overpass API returned status {} (attempt {})",
                    response.status(),
                    attempt + 1
                ));
                continue;
            }
            status => {
                bail!("Overpass API returned error status: {}", status);
            }
        }
    }

    bail!(
        "Overpass API failed after {} retries: {}",
        max_retries,
        last_error.unwrap_or_else(|| "Unknown error".to_string())
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_bbox() {
        // San Francisco: (37.7749, -122.4194)
        let (south, west, north, east) = calculate_bbox((37.7749, -122.4194), 10000);

        // 10km radius should give approximately ±0.09 degrees latitude
        assert!((north - south - 0.18).abs() < 0.01);
        // Longitude spread should be slightly larger due to cos(lat)
        assert!(east - west > north - south);
    }

    #[test]
    fn test_parse_overpass_response() {
        let json = r#"{
            "elements": [
                {"type": "node", "id": 1, "lat": 37.77, "lon": -122.42},
                {"type": "way", "id": 2, "nodes": [1, 3], "tags": {"highway": "primary"}}
            ]
        }"#;

        let response: OverpassResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.elements.len(), 2);
        assert_eq!(response.elements[0].type_, "node");
        assert_eq!(response.elements[1].type_, "way");
    }
}
