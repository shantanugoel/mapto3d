use serde::Deserialize;
use std::path::PathBuf;

use crate::api::RoadDepth;

fn default_radius() -> u32 {
    10000
}
fn default_size() -> f32 {
    220.0
}
fn default_base_height() -> f32 {
    2.0
}
fn default_road_scale() -> f32 {
    1.0
}
fn default_road_depth() -> RoadDepth {
    RoadDepth::Primary
}
fn default_simplify() -> u8 {
    0
}
fn default_verbose() -> bool {
    false
}

#[derive(Debug, Deserialize, Default)]
pub struct FileConfig {
    #[serde(default)]
    pub city: Option<String>,
    #[serde(default)]
    pub country: Option<String>,
    #[serde(default)]
    pub lat: Option<f64>,
    #[serde(default)]
    pub lon: Option<f64>,
    #[serde(default = "default_radius")]
    pub radius: u32,
    #[serde(default)]
    pub output: Option<PathBuf>,
    #[serde(default = "default_size")]
    pub size: f32,
    #[serde(default = "default_base_height")]
    pub base_height: f32,
    #[serde(default = "default_road_scale")]
    pub road_scale: f32,
    #[serde(default = "default_road_depth")]
    pub road_depth: RoadDepth,
    #[serde(default)]
    pub primary_text: Option<String>,
    #[serde(default)]
    pub secondary_text: Option<String>,
    #[serde(default = "default_verbose")]
    pub verbose: bool,
    #[serde(default = "default_simplify")]
    pub simplify: u8,
    #[serde(default)]
    pub overpass: Option<OverpassConfig>,
}

fn default_overpass_urls() -> Vec<String> {
    vec![
        "https://overpass.private.coffee/api/interpreter".to_string(),
        "https://overpass-api.de/api/interpreter".to_string(),
        "https://maps.mail.ru/osm/tools/overpass/api/interpreter".to_string(),
    ]
}

fn default_timeout_secs() -> u64 {
    200
}

fn default_max_retries() -> u32 {
    3
}

#[derive(Debug, Deserialize, Clone)]
pub struct OverpassConfig {
    #[serde(default = "default_overpass_urls")]
    pub urls: Vec<String>,
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
}

impl Default for OverpassConfig {
    fn default() -> Self {
        Self {
            urls: default_overpass_urls(),
            timeout_secs: default_timeout_secs(),
            max_retries: default_max_retries(),
        }
    }
}

impl FileConfig {
    pub fn load() -> Option<Self> {
        let config_paths = get_config_paths();

        for path in config_paths {
            if path.exists()
                && let Ok(contents) = std::fs::read_to_string(&path)
            {
                match toml::from_str(&contents) {
                    Ok(config) => return Some(config),
                    Err(e) => {
                        eprintln!("Warning: Failed to parse config file {:?}: {}", path, e);
                    }
                }
            }
        }
        None
    }
}

fn get_config_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    paths.push(PathBuf::from("mapto3d.toml"));
    paths.push(PathBuf::from(".mapto3d.toml"));

    if let Some(config_dir) = dirs::config_dir() {
        paths.push(config_dir.join("mapto3d").join("config.toml"));
        paths.push(config_dir.join("mapto3d.toml"));
    }

    if let Some(home) = dirs::home_dir() {
        paths.push(home.join(".mapto3d.toml"));
        paths.push(home.join(".config").join("mapto3d").join("config.toml"));
    }

    paths
}
