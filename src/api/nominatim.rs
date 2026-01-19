use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::thread;
use std::time::Duration;

const NOMINATIM_URL: &str = "https://nominatim.openstreetmap.org/search";
const USER_AGENT: &str = "mapto3d/0.1.0 (https://github.com/shantanugoel/mapto3d)";

#[derive(Debug, Deserialize)]
struct NominatimResult {
    lat: String,
    lon: String,
    display_name: String,
}

/// Geocode a city name to latitude/longitude coordinates.
///
/// Uses the Nominatim API to convert "{city}, {country}" to (lat, lon).
/// Includes a 1 second delay for rate limiting (Nominatim ToS).
///
/// # Arguments
/// * `city` - City name (e.g., "San Francisco")
/// * `country` - Country name (e.g., "USA")
///
/// # Returns
/// * `Ok((lat, lon))` - Coordinates as f64 tuple
/// * `Err` - If city not found or API error
pub fn geocode_city(city: &str, country: &str) -> Result<(f64, f64)> {
    // Rate limiting - Nominatim requires max 1 request per second
    thread::sleep(Duration::from_secs(1));

    let query = format!("{}, {}", city, country);

    let client = reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(30))
        .build()
        .context("Failed to create HTTP client")?;

    let response = client
        .get(NOMINATIM_URL)
        .query(&[
            ("q", &query),
            ("format", &"json".to_string()),
            ("limit", &"1".to_string()),
        ])
        .send()
        .context("Failed to send request to Nominatim API")?;

    if !response.status().is_success() {
        bail!("Nominatim API returned error status: {}", response.status());
    }

    let results: Vec<NominatimResult> = response
        .json()
        .context("Failed to parse Nominatim JSON response")?;

    let result = results
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("City not found: {}, {}", city, country))?;

    let lat: f64 = result
        .lat
        .parse()
        .context("Failed to parse latitude from Nominatim response")?;
    let lon: f64 = result
        .lon
        .parse()
        .context("Failed to parse longitude from Nominatim response")?;

    Ok((lat, lon))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_nominatim_response() {
        // Sample response from Nominatim
        let json = r#"[{"lat":"37.7790262","lon":"-122.4199061","display_name":"San Francisco, California, USA"}]"#;
        let results: Vec<NominatimResult> = serde_json::from_str(json).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].lat, "37.7790262");
        assert_eq!(results[0].lon, "-122.4199061");
    }
}
