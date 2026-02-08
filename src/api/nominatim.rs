use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::thread;
use std::time::Duration;

use crate::api::cache::{CachePolicy, get_or_fetch_with_parser};

const NOMINATIM_URL: &str = "https://nominatim.openstreetmap.org/search";
const USER_AGENT: &str = "mapto3d/0.1.0 (https://github.com/shantanugoel/mapto3d)";
const CACHE_NAMESPACE: &str = "nominatim_geocode";

#[derive(Debug, Deserialize)]
struct NominatimResult {
    lat: String,
    lon: String,
    display_name: String,
}

pub fn geocode_city_with_cache(
    city: &str,
    country: &str,
    cache_policy: &CachePolicy,
) -> Result<(f64, f64)> {
    geocode_city_with_fetcher(city, country, cache_policy, fetch_nominatim_payload)
}

fn geocode_city_with_fetcher<F>(
    city: &str,
    country: &str,
    cache_policy: &CachePolicy,
    fetcher: F,
) -> Result<(f64, f64)>
where
    F: FnOnce(&str) -> Result<String>,
{
    let query = format!("{}, {}", city.trim(), country.trim());
    let request_payload = cache_request_payload(city, country);
    get_or_fetch_with_parser(
        cache_policy,
        CACHE_NAMESPACE,
        &request_payload,
        || fetcher(&query),
        |payload| parse_coords_from_payload(payload, city, country),
    )
}

fn fetch_nominatim_payload(query: &str) -> Result<String> {
    // Rate limiting - Nominatim requires max 1 request per second
    thread::sleep(Duration::from_secs(1));

    let client = reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(30))
        .build()
        .context("Failed to create HTTP client")?;

    let response = client
        .get(NOMINATIM_URL)
        .query(&[("q", query), ("format", "json"), ("limit", "1")])
        .send()
        .context("Failed to send request to Nominatim API")?;

    if !response.status().is_success() {
        bail!("Nominatim API returned error status: {}", response.status());
    }

    response
        .text()
        .context("Failed to read Nominatim API response body")
}

fn parse_coords_from_payload(payload: &str, city: &str, country: &str) -> Result<(f64, f64)> {
    let results: Vec<NominatimResult> =
        serde_json::from_str(payload).context("Failed to parse Nominatim JSON response")?;

    let result = results
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("City not found: {}, {}", city, country))?;

    if result.display_name.is_empty() {
        bail!("Nominatim returned an empty display name for {city}, {country}");
    }

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

fn cache_request_payload(city: &str, country: &str) -> String {
    format!(
        "q={city}&country={country}",
        city = city.trim().to_lowercase(),
        country = country.trim().to_lowercase()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::cache::{self, CacheLookup};
    use std::cell::Cell;

    #[test]
    fn test_parse_nominatim_response() {
        // Sample response from Nominatim
        let json = r#"[{"lat":"37.7790262","lon":"-122.4199061","display_name":"San Francisco, California, USA"}]"#;
        let results: Vec<NominatimResult> = serde_json::from_str(json).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].lat, "37.7790262");
        assert_eq!(results[0].lon, "-122.4199061");
    }

    #[test]
    fn test_geocode_uses_cached_payload_when_fresh() {
        let temp = tempfile::tempdir().unwrap();
        let policy = CachePolicy::new(true, false, 24 * 60 * 60, Some(temp.path().to_path_buf()));
        let request_payload = cache_request_payload("Monaco", "Monaco");
        let cached_payload = r#"[{"lat":"43.7384","lon":"7.4246","display_name":"Monaco"}]"#;
        cache::store(&policy, CACHE_NAMESPACE, &request_payload, cached_payload).unwrap();

        let calls = Cell::new(0);
        let coords = geocode_city_with_fetcher("Monaco", "Monaco", &policy, |_| {
            calls.set(calls.get() + 1);
            Ok(r#"[{"lat":"0.0","lon":"0.0","display_name":"wrong"}]"#.to_string())
        })
        .unwrap();

        assert_eq!(calls.get(), 0);
        assert!((coords.0 - 43.7384).abs() < 0.0001);
        assert!((coords.1 - 7.4246).abs() < 0.0001);
    }

    #[test]
    fn test_geocode_fetches_and_populates_cache_on_miss() {
        let temp = tempfile::tempdir().unwrap();
        let policy = CachePolicy::new(true, false, 24 * 60 * 60, Some(temp.path().to_path_buf()));

        let calls = Cell::new(0);
        let coords = geocode_city_with_fetcher("Paris", "France", &policy, |_| {
            calls.set(calls.get() + 1);
            Ok(r#"[{"lat":"48.8566","lon":"2.3522","display_name":"Paris"}]"#.to_string())
        })
        .unwrap();

        assert_eq!(calls.get(), 1);
        assert!((coords.0 - 48.8566).abs() < 0.0001);
        assert!((coords.1 - 2.3522).abs() < 0.0001);

        let request_payload = cache_request_payload("Paris", "France");
        let lookup = cache::lookup(&policy, CACHE_NAMESPACE, &request_payload).unwrap();
        assert!(matches!(lookup, CacheLookup::Hit { .. }));
    }
}
