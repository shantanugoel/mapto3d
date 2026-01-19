use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::collections::HashMap;
use std::time::Duration;

use crate::config::OverpassConfig;

const USER_AGENT: &str = "mapto3d/0.1.0 (https://github.com/shantanugoel/mapto3d)";

#[derive(Debug, Deserialize)]
pub struct OverpassResponse {
    pub elements: Vec<Element>,
}

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

fn calculate_bbox(center: (f64, f64), radius_m: u32) -> (f64, f64, f64, f64) {
    let (lat, lon) = center;
    let radius_km = radius_m as f64 / 1000.0;

    let lat_delta = radius_km / 111.0;
    let lon_delta = radius_km / (111.0 * lat.to_radians().cos());

    let south = lat - lat_delta;
    let north = lat + lat_delta;
    let west = lon - lon_delta;
    let east = lon + lon_delta;

    (south, west, north, east)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RoadDepth {
    Motorway,
    #[default]
    Primary,
    Secondary,
    Tertiary,
    All,
}

impl std::str::FromStr for RoadDepth {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "motorway" => Ok(RoadDepth::Motorway),
            "primary" => Ok(RoadDepth::Primary),
            "secondary" => Ok(RoadDepth::Secondary),
            "tertiary" => Ok(RoadDepth::Tertiary),
            "all" => Ok(RoadDepth::All),
            _ => Err(format!(
                "Invalid road depth '{}'. Valid options: motorway, primary, secondary, tertiary, all",
                s
            )),
        }
    }
}

impl RoadDepth {
    /// Get the highway types to include for this depth level
    pub fn highway_filter(&self) -> &'static str {
        match self {
            RoadDepth::Motorway => r#"["highway"~"^(motorway|motorway_link)$"]"#,
            RoadDepth::Primary => {
                r#"["highway"~"^(motorway|motorway_link|trunk|trunk_link|primary|primary_link)$"]"#
            }
            RoadDepth::Secondary => {
                r#"["highway"~"^(motorway|motorway_link|trunk|trunk_link|primary|primary_link|secondary|secondary_link)$"]"#
            }
            RoadDepth::Tertiary => {
                r#"["highway"~"^(motorway|motorway_link|trunk|trunk_link|primary|primary_link|secondary|secondary_link|tertiary|tertiary_link)$"]"#
            }
            RoadDepth::All => r#"["highway"]"#,
        }
    }
}

/// Fetch road data from Overpass API
///
/// # Arguments
/// * `center` - (lat, lon) center point
/// * `radius_m` - Radius in meters
/// * `depth` - Which road classes to include
///
/// # Returns
/// * `OverpassResponse` containing all highway ways and their nodes
#[allow(dead_code)]
pub fn fetch_roads(center: (f64, f64), radius_m: u32) -> Result<OverpassResponse> {
    fetch_roads_with_depth(
        center,
        radius_m,
        RoadDepth::default(),
        &OverpassConfig::default(),
    )
}

/// Fetch road data with configurable depth
pub fn fetch_roads_with_depth(
    center: (f64, f64),
    radius_m: u32,
    depth: RoadDepth,
    config: &OverpassConfig,
) -> Result<OverpassResponse> {
    let (south, west, north, east) = calculate_bbox(center, radius_m);

    // Overpass QL query for highways with depth filter
    // Use 180s timeout to match OSMnx's default - 60s is often too short for larger areas
    let query = format!(
        r#"[out:json][timeout:180];
(
  way{filter}({south},{west},{north},{east});
);
out body;
>;
out skel qt;"#,
        filter = depth.highway_filter(),
        south = south,
        west = west,
        north = north,
        east = east
    );

    execute_overpass_query(&query, config)
}

/// Fetch water features from Overpass API
///
/// Fetches natural=water ways and waterway=riverbank
pub fn fetch_water(
    center: (f64, f64),
    radius_m: u32,
    config: &OverpassConfig,
) -> Result<OverpassResponse> {
    let (south, west, north, east) = calculate_bbox(center, radius_m);

    let query = format!(
        r#"[out:json][timeout:180];
(
  way["natural"="water"]({south},{west},{north},{east});
  way["waterway"="riverbank"]({south},{west},{north},{east});
  way["water"]({south},{west},{north},{east});
  way["landuse"="reservoir"]({south},{west},{north},{east});
);
out body;
>;
out skel qt;"#,
        south = south,
        west = west,
        north = north,
        east = east
    );

    execute_overpass_query(&query, config)
}

/// Fetch park features from Overpass API
///
/// Fetches leisure=park and landuse=grass
pub fn fetch_parks(
    center: (f64, f64),
    radius_m: u32,
    config: &OverpassConfig,
) -> Result<OverpassResponse> {
    let (south, west, north, east) = calculate_bbox(center, radius_m);

    let query = format!(
        r#"[out:json][timeout:180];
(
  way["leisure"="park"]({south},{west},{north},{east});
  way["landuse"="grass"]({south},{west},{north},{east});
  way["leisure"="garden"]({south},{west},{north},{east});
  way["landuse"="meadow"]({south},{west},{north},{east});
);
out body;
>;
out skel qt;"#,
        south = south,
        west = west,
        north = north,
        east = east
    );

    execute_overpass_query(&query, config)
}

/// Execute an Overpass API query with retry logic and URL fallback
fn execute_overpass_query(query: &str, config: &OverpassConfig) -> Result<OverpassResponse> {
    let client = reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(config.timeout_secs))
        .build()
        .context("Failed to create HTTP client")?;

    let urls = if config.urls.is_empty() {
        // Fallback to defaults if somehow empty
        vec![
            "https://overpass.private.coffee/api/interpreter".to_string(),
            "https://overpass-api.de/api/interpreter".to_string(),
        ]
    } else {
        config.urls.clone()
    };

    let mut all_errors: Vec<String> = Vec::new();

    // Try each URL in sequence
    for (url_idx, url) in urls.iter().enumerate() {
        let mut last_error = None;

        // Retry logic for each URL
        for attempt in 0..config.max_retries {
            if attempt > 0 {
                // Wait before retry - Overpass recommends waiting when overloaded
                let wait_secs = 30 * attempt as u64;
                eprintln!(
                    "Overpass API timeout on {}, retrying in {} seconds (attempt {}/{})",
                    url,
                    wait_secs,
                    attempt + 1,
                    config.max_retries
                );
                std::thread::sleep(Duration::from_secs(wait_secs));
            }

            // IMPORTANT: Overpass API expects form-encoded POST data, not raw body
            // The query must be sent as: data=<query>
            let response = match client.post(url).form(&[("data", query)]).send() {
                Ok(resp) => resp,
                Err(e) => {
                    last_error = Some(format!("Request failed: {}", e));
                    continue;
                }
            };

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
                    // Non-retriable error for this URL, try next URL
                    last_error = Some(format!("Overpass API returned error status: {}", status));
                    break;
                }
            }
        }

        // Record error for this URL and try next
        if let Some(err) = last_error {
            all_errors.push(format!("{}: {}", url, err));
            if url_idx + 1 < urls.len() {
                eprintln!("Overpass API {} failed, trying fallback mirror...", url);
            }
        }
    }

    bail!(
        "All Overpass API endpoints failed:\n  {}",
        all_errors.join("\n  ")
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
