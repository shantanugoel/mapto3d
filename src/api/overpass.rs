use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::collections::HashMap;
use std::time::Duration;

use crate::api::cache::{CachePolicy, get_or_fetch_with_parser};
use crate::config::OverpassConfig;

const USER_AGENT: &str = "mapto3d/0.1.0 (https://github.com/shantanugoel/mapto3d)";
const CACHE_NAMESPACE_ROADS: &str = "overpass_roads";
const CACHE_NAMESPACE_WATER: &str = "overpass_water";
const CACHE_NAMESPACE_PARKS: &str = "overpass_parks";

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
    pub members: Option<Vec<Member>>,
    #[serde(default)]
    pub tags: Option<HashMap<String, String>>,
    #[serde(default)]
    pub lat: Option<f64>,
    #[serde(default)]
    pub lon: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct Member {
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(rename = "ref")]
    pub ref_id: u64,
    #[serde(default)]
    pub role: String,
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

/// Fetch road data with configurable depth
pub fn fetch_roads_with_depth(
    center: (f64, f64),
    radius_m: u32,
    depth: RoadDepth,
    config: &OverpassConfig,
) -> Result<OverpassResponse> {
    fetch_roads_with_depth_and_cache(center, radius_m, depth, config, &CachePolicy::default())
}

pub fn fetch_roads_with_depth_and_cache(
    center: (f64, f64),
    radius_m: u32,
    depth: RoadDepth,
    config: &OverpassConfig,
    cache_policy: &CachePolicy,
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

    execute_overpass_query_with_cache(&query, config, cache_policy, CACHE_NAMESPACE_ROADS)
}

/// Fetch water features from Overpass API
///
/// Fetches water bodies including:
/// - natural=water (lakes, ponds)
/// - waterway=riverbank (river banks, deprecated but still used)
/// - waterway=river/canal (linear waterways)
/// - water=* (generic water tag)
/// - landuse=reservoir/basin (man-made water storage)
/// - natural=wetland (swamps, marshes)
pub fn fetch_water_with_cache(
    center: (f64, f64),
    radius_m: u32,
    config: &OverpassConfig,
    cache_policy: &CachePolicy,
) -> Result<OverpassResponse> {
    let (south, west, north, east) = calculate_bbox(center, radius_m);

    let query = format!(
        r#"[out:json][timeout:180];
(
  way["natural"="water"]({south},{west},{north},{east});
  relation["natural"="water"]({south},{west},{north},{east});
  way["natural"="coastline"]({south},{west},{north},{east});
  relation["natural"="coastline"]({south},{west},{north},{east});
  way["waterway"="riverbank"]({south},{west},{north},{east});
  relation["waterway"="riverbank"]({south},{west},{north},{east});
  way["waterway"="river"]({south},{west},{north},{east});
  relation["waterway"="river"]({south},{west},{north},{east});
  way["water"]({south},{west},{north},{east});
  relation["water"]({south},{west},{north},{east});
  way["landuse"="reservoir"]({south},{west},{north},{east});
  relation["landuse"="reservoir"]({south},{west},{north},{east});
);
out body;
>;
out skel qt;"#,
        south = south,
        west = west,
        north = north,
        east = east
    );

    execute_overpass_query_with_cache(&query, config, cache_policy, CACHE_NAMESPACE_WATER)
}

/// Fetch park features from Overpass API
///
/// Fetches green areas including:
/// - leisure=park/garden/nature_reserve/recreation_ground
/// - landuse=grass/meadow/forest
/// - natural=wood/grassland (natural vegetation)
pub fn fetch_parks_with_cache(
    center: (f64, f64),
    radius_m: u32,
    config: &OverpassConfig,
    cache_policy: &CachePolicy,
) -> Result<OverpassResponse> {
    let (south, west, north, east) = calculate_bbox(center, radius_m);

    let query = format!(
        r#"[out:json][timeout:180];
(
  way["leisure"="park"]({south},{west},{north},{east});
  relation["leisure"="park"]({south},{west},{north},{east});
  way["leisure"="garden"]({south},{west},{north},{east});
  relation["leisure"="garden"]({south},{west},{north},{east});
  way["leisure"="nature_reserve"]({south},{west},{north},{east});
  relation["leisure"="nature_reserve"]({south},{west},{north},{east});
  way["landuse"="grass"]({south},{west},{north},{east});
  relation["landuse"="grass"]({south},{west},{north},{east});
  way["landuse"="meadow"]({south},{west},{north},{east});
  relation["landuse"="meadow"]({south},{west},{north},{east});
  way["landuse"="forest"]({south},{west},{north},{east});
  relation["landuse"="forest"]({south},{west},{north},{east});
  way["natural"="wood"]({south},{west},{north},{east});
  relation["natural"="wood"]({south},{west},{north},{east});
);
out body;
>;
out skel qt;"#,
        south = south,
        west = west,
        north = north,
        east = east
    );

    execute_overpass_query_with_cache(&query, config, cache_policy, CACHE_NAMESPACE_PARKS)
}

fn execute_overpass_query_with_cache(
    query: &str,
    config: &OverpassConfig,
    cache_policy: &CachePolicy,
    cache_namespace: &str,
) -> Result<OverpassResponse> {
    execute_overpass_query_with_fetcher(query, config, cache_policy, cache_namespace, |q, c| {
        fetch_overpass_payload(q, c)
    })
}

fn execute_overpass_query_with_fetcher<F>(
    query: &str,
    config: &OverpassConfig,
    cache_policy: &CachePolicy,
    cache_namespace: &str,
    fetcher: F,
) -> Result<OverpassResponse>
where
    F: FnOnce(&str, &OverpassConfig) -> Result<String>,
{
    let request_payload = cache_request_payload(query);
    get_or_fetch_with_parser(
        cache_policy,
        cache_namespace,
        &request_payload,
        || fetcher(query, config),
        |payload| serde_json::from_str(payload).context("Failed to parse Overpass JSON response"),
    )
}

fn fetch_overpass_payload(query: &str, config: &OverpassConfig) -> Result<String> {
    let client = reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(config.timeout_secs))
        .build()
        .context("Failed to create HTTP client")?;

    let urls = if config.urls.is_empty() {
        // Fallback to defaults if somehow empty
        vec![
            // "https://maps.mail.ru/osm/tools/overpass/api/interpreter".to_string(),
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
                    let payload = response
                        .text()
                        .context("Failed to read Overpass response body")?;
                    return Ok(payload);
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

fn cache_request_payload(query: &str) -> String {
    format!("query={query}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::cache::{self, CacheLookup};
    use std::cell::Cell;

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

    #[test]
    fn test_overpass_uses_cache_hit_without_network_fetch() {
        let temp = tempfile::tempdir().unwrap();
        let policy = CachePolicy::new(true, false, 24 * 60 * 60, Some(temp.path().to_path_buf()));
        let query = "[out:json];way(0,0,1,1);out;";
        let request_payload = cache_request_payload(query);
        let cached_payload = r#"{"elements":[{"type":"node","id":1,"lat":1.0,"lon":2.0}]}"#;
        cache::store(
            &policy,
            CACHE_NAMESPACE_ROADS,
            &request_payload,
            cached_payload,
        )
        .unwrap();

        let calls = Cell::new(0);
        let response = execute_overpass_query_with_fetcher(
            query,
            &OverpassConfig::default(),
            &policy,
            CACHE_NAMESPACE_ROADS,
            |_, _| {
                calls.set(calls.get() + 1);
                Ok(r#"{"elements":[]}"#.to_string())
            },
        )
        .unwrap();

        assert_eq!(calls.get(), 0);
        assert_eq!(response.elements.len(), 1);
        assert_eq!(response.elements[0].id, 1);
    }

    #[test]
    fn test_overpass_fetches_and_populates_cache_on_miss() {
        let temp = tempfile::tempdir().unwrap();
        let policy = CachePolicy::new(true, false, 24 * 60 * 60, Some(temp.path().to_path_buf()));
        let query = "[out:json];way(0,0,1,1);out;";

        let calls = Cell::new(0);
        let response = execute_overpass_query_with_fetcher(
            query,
            &OverpassConfig::default(),
            &policy,
            CACHE_NAMESPACE_PARKS,
            |_, _| {
                calls.set(calls.get() + 1);
                Ok(r#"{"elements":[{"type":"node","id":42,"lat":3.0,"lon":4.0}]}"#.to_string())
            },
        )
        .unwrap();

        assert_eq!(calls.get(), 1);
        assert_eq!(response.elements.len(), 1);
        assert_eq!(response.elements[0].id, 42);

        let request_payload = cache_request_payload(query);
        let lookup = cache::lookup(&policy, CACHE_NAMESPACE_PARKS, &request_payload).unwrap();
        assert!(matches!(lookup, CacheLookup::Hit { .. }));
    }
}
