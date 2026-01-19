use anyhow::{Context, Result, bail};
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::PathBuf;
use std::time::Instant;

mod api;
mod config;
mod domain;
mod geometry;
mod layers;
mod mesh;
mod osm;

use api::{RoadDepth, fetch_parks, fetch_roads_with_depth, fetch_water, geocode_city};
use config::FileConfig;
use domain::{ParkPolygon, WaterPolygon};
use geometry::{Bounds, Projector, Scaler};
use layers::{
    RoadConfig, TextRenderer, generate_base_plate, generate_park_meshes, generate_road_meshes,
    generate_water_meshes,
};
use mesh::{stl::estimate_stl_size, validate_and_fix, write_stl};
use osm::{parse_parks, parse_roads, parse_water};

/// Generate 3D-printable STL city maps from OpenStreetMap data
///
/// Examples:
///   # Generate San Francisco map with default settings
///   mapto3d -c "San Francisco" -C "USA"
///   
///   # Generate Tokyo with larger radius
///   mapto3d -c "Tokyo" -C "Japan" -r 15000 -o tokyo.stl
///   
///   # Generate Venice (small, detailed) with all roads
///   mapto3d -c "Venice" -C "Italy" -r 4000 --road-scale 1.5 --road-depth all
///
///   # Generate using coordinates directly with custom labels
///   mapto3d --lat 37.7749 --lon -122.4194 -r 5000 --primary-text "SF BAY" --secondary-text "CALIFORNIA"
///
///   # Use a config file
///   mapto3d --config my-settings.toml
#[derive(Parser, Debug)]
#[command(name = "mapto3d")]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to config file (optional, auto-searches mapto3d.toml if not provided)
    #[arg(long)]
    config: Option<PathBuf>,

    /// City name (optional if --lat and --lon are provided)
    #[arg(short = 'c', long)]
    city: Option<String>,

    /// Country name (optional if --lat and --lon are provided)
    #[arg(short = 'C', long)]
    country: Option<String>,

    /// Latitude for direct coordinate input (use with --lon)
    #[arg(long, requires = "lon")]
    lat: Option<f64>,

    /// Longitude for direct coordinate input (use with --lat)
    #[arg(long, requires = "lat")]
    lon: Option<f64>,

    /// Map radius in meters
    #[arg(short = 'r', long, default_value = "10000")]
    radius: u32,

    /// Output STL file path (defaults to {city}.stl or map.stl)
    #[arg(short = 'o', long)]
    output: Option<PathBuf>,

    /// Physical size in mm (width/height of the square output)
    #[arg(short = 's', long, default_value = "220.0")]
    size: f32,

    /// Base plate thickness in mm
    #[arg(long, default_value = "2.0")]
    base_height: f32,

    /// Road height multiplier
    #[arg(long, default_value = "1.0")]
    road_scale: f32,

    /// Road depth level: motorway, primary, secondary, tertiary, or all
    #[arg(long, default_value = "primary")]
    road_depth: RoadDepth,

    /// Primary text label (large, defaults to city name in uppercase)
    #[arg(long)]
    primary_text: Option<String>,

    /// Secondary text label (small, defaults to coordinates)
    #[arg(long)]
    secondary_text: Option<String>,

    /// Enable verbose logging
    #[arg(short = 'v', long)]
    verbose: bool,

    /// Road simplification level: 0=off (default), 1=light, 2=medium, 3=aggressive
    /// Higher values reduce triangle count but may lose curve detail
    #[arg(long, default_value = "0", value_parser = clap::value_parser!(u8).range(0..=3))]
    simplify: u8,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let total_start = Instant::now();

    let file_config = if let Some(ref config_path) = args.config {
        if config_path.exists() {
            let contents = std::fs::read_to_string(config_path)
                .context(format!("Failed to read config file: {:?}", config_path))?;
            Some(toml::from_str(&contents).context("Failed to parse config file")?)
        } else {
            bail!("Config file not found: {:?}", config_path);
        }
    } else {
        FileConfig::load()
    };

    let city = args
        .city
        .clone()
        .or_else(|| file_config.as_ref().and_then(|c| c.city.clone()));
    let country = args
        .country
        .clone()
        .or_else(|| file_config.as_ref().and_then(|c| c.country.clone()));
    let lat = args
        .lat
        .or_else(|| file_config.as_ref().and_then(|c| c.lat));
    let lon = args
        .lon
        .or_else(|| file_config.as_ref().and_then(|c| c.lon));
    let radius = if args.radius != 10000 {
        args.radius
    } else {
        file_config.as_ref().map(|c| c.radius).unwrap_or(10000)
    };
    let size = if (args.size - 220.0).abs() > 0.01 {
        args.size
    } else {
        file_config.as_ref().map(|c| c.size).unwrap_or(220.0)
    };
    let base_height = if (args.base_height - 2.0).abs() > 0.01 {
        args.base_height
    } else {
        file_config.as_ref().map(|c| c.base_height).unwrap_or(2.0)
    };
    let road_scale = if (args.road_scale - 1.0).abs() > 0.01 {
        args.road_scale
    } else {
        file_config.as_ref().map(|c| c.road_scale).unwrap_or(1.0)
    };
    let road_depth = if args.road_depth != RoadDepth::Primary {
        args.road_depth
    } else {
        file_config
            .as_ref()
            .map(|c| c.road_depth)
            .unwrap_or(RoadDepth::Primary)
    };
    let simplify = if args.simplify != 0 {
        args.simplify
    } else {
        file_config.as_ref().map(|c| c.simplify).unwrap_or(0)
    };
    let verbose = args.verbose || file_config.as_ref().map(|c| c.verbose).unwrap_or(false);
    let primary_text = args
        .primary_text
        .clone()
        .or_else(|| file_config.as_ref().and_then(|c| c.primary_text.clone()));
    let secondary_text = args
        .secondary_text
        .clone()
        .or_else(|| file_config.as_ref().and_then(|c| c.secondary_text.clone()));
    let output = args
        .output
        .clone()
        .or_else(|| file_config.as_ref().and_then(|c| c.output.clone()));

    let overpass_config = file_config
        .as_ref()
        .and_then(|c| c.overpass.clone())
        .unwrap_or_default();

    if city.is_none() && lat.is_none() {
        bail!("Must provide either --city/-c and --country/-C, or --lat and --lon");
    }
    if city.is_some() && country.is_none() {
        bail!("--city requires --country");
    }

    println!("mapto3d - City Map STL Generator");
    println!("================================");
    println!();

    let output_path = output.clone().unwrap_or_else(|| {
        if let Some(ref c) = city {
            PathBuf::from(format!("{}.stl", c.to_lowercase().replace(' ', "_")))
        } else {
            PathBuf::from("map.stl")
        }
    });

    let display_name = city
        .clone()
        .unwrap_or_else(|| "Custom Location".to_string());

    if verbose {
        println!("Configuration:");
        if let Some(ref c) = city {
            println!("  City: {}", c);
            println!("  Country: {}", country.as_ref().unwrap());
        }
        if let Some(lt) = lat {
            println!("  Coordinates: ({:.4}, {:.4})", lt, lon.unwrap());
        }
        println!("  Radius: {}m", radius);
        println!("  Size: {}mm", size);
        println!("  Base height: {}mm", base_height);
        println!("  Road scale: {}", road_scale);
        println!("  Road depth: {:?}", road_depth);
        println!("  Simplify level: {}", simplify);
        println!("  Output: {}", output_path.display());
        println!("  Overpass mirrors: {}", overpass_config.urls.len());
        println!();
    }

    let center = if let (Some(lt), Some(ln)) = (lat, lon) {
        println!("Using provided coordinates: ({:.4}, {:.4})", lt, ln);
        (lt, ln)
    } else {
        let c = city.as_ref().unwrap();
        let co = country.as_ref().unwrap();
        let spinner = create_spinner("Geocoding city...");
        let start = Instant::now();
        let coords = geocode_city(c, co).context("Failed to geocode city")?;
        spinner.finish_with_message(format!(
            "Geocoded: {}, {} -> ({:.4}, {:.4}) [{:.1}s]",
            c,
            co,
            coords.0,
            coords.1,
            start.elapsed().as_secs_f32()
        ));
        coords
    };

    let spinner = create_spinner("Fetching roads from OpenStreetMap...");
    let start = Instant::now();
    let roads_response = fetch_roads_with_depth(center, radius, road_depth, &overpass_config)
        .context("Failed to fetch roads from Overpass API")?;
    spinner.finish_with_message(format!(
        "Fetched {} road elements [{:.1}s]",
        roads_response.elements.len(),
        start.elapsed().as_secs_f32()
    ));

    let spinner = create_spinner("Parsing road data...");
    let start = Instant::now();
    let roads = parse_roads(&roads_response);
    if roads.is_empty() {
        bail!(
            "No roads found in the specified area. Try increasing the radius or using --road-depth all"
        );
    }
    spinner.finish_with_message(format!(
        "Parsed {} road segments [{:.1}s]",
        roads.len(),
        start.elapsed().as_secs_f32()
    ));

    let spinner = create_spinner("Fetching water features...");
    let start = Instant::now();
    let water_response =
        fetch_water(center, radius, &overpass_config).context("Failed to fetch water data")?;
    spinner.finish_with_message(format!(
        "Fetched {} water elements [{:.1}s]",
        water_response.elements.len(),
        start.elapsed().as_secs_f32()
    ));

    let water: Vec<WaterPolygon> = parse_water(&water_response);
    if verbose {
        println!("  Parsed {} water polygons", water.len());
    }

    let spinner = create_spinner("Fetching park features...");
    let start = Instant::now();
    let parks_response =
        fetch_parks(center, radius, &overpass_config).context("Failed to fetch park data")?;
    spinner.finish_with_message(format!(
        "Fetched {} park elements [{:.1}s]",
        parks_response.elements.len(),
        start.elapsed().as_secs_f32()
    ));

    let parks: Vec<ParkPolygon> = parse_parks(&parks_response);
    if verbose {
        println!("  Parsed {} park polygons", parks.len());
    }

    let spinner = create_spinner("Setting up coordinate projection...");
    let projector = Projector::new(center);

    let mut all_projected_points: Vec<(f64, f64)> = Vec::new();
    for road in &roads {
        let projected = projector.project_points(&road.points);
        all_projected_points.extend(projected);
    }

    let bounds = Bounds::from_points(&all_projected_points)
        .context("Failed to compute bounds from road points")?;

    let scaler = Scaler::from_bounds(&bounds, size as f64);
    spinner.finish_with_message(format!(
        "Map area: {:.0}m x {:.0}m -> {:.0}mm x {:.0}mm",
        bounds.width(),
        bounds.height(),
        size,
        size
    ));

    let spinner = create_spinner("Generating mesh layers...");
    let start = Instant::now();

    let mut all_triangles = Vec::new();

    let base_triangles = generate_base_plate(size, base_height);
    if verbose {
        println!("  Base plate: {} triangles", base_triangles.len());
    }
    all_triangles.extend(base_triangles);

    let water_triangles = generate_water_meshes(&water, &projector, &scaler);
    if verbose {
        println!("  Water: {} triangles", water_triangles.len());
    }
    all_triangles.extend(water_triangles);

    let park_triangles = generate_park_meshes(&parks, &projector, &scaler);
    if verbose {
        println!("  Parks: {} triangles", park_triangles.len());
    }
    all_triangles.extend(park_triangles);

    let road_config = RoadConfig::default()
        .with_scale(road_scale)
        .with_map_radius(radius, size)
        .with_simplify_level(simplify);
    let road_triangles = generate_road_meshes(&roads, &projector, &scaler, &road_config);
    if verbose {
        println!("  Roads: {} triangles", road_triangles.len());
    }
    all_triangles.extend(road_triangles);

    let text_triangles = generate_text_layer(
        &display_name,
        center,
        size,
        primary_text.as_deref(),
        secondary_text.as_deref(),
    );
    if verbose {
        println!("  Text: {} triangles", text_triangles.len());
    }
    all_triangles.extend(text_triangles);

    spinner.finish_with_message(format!(
        "Generated {} triangles [{:.1}s]",
        all_triangles.len(),
        start.elapsed().as_secs_f32()
    ));

    let spinner = create_spinner("Validating and cleaning mesh...");
    let start = Instant::now();
    let original_count = all_triangles.len();
    let (all_triangles, validation_report) = validate_and_fix(all_triangles);
    let removed = original_count - all_triangles.len();
    if removed > 0 || verbose {
        spinner.finish_with_message(format!(
            "Validated: {} triangles, {} degenerate removed, {} normals fixed [{:.1}s]",
            all_triangles.len(),
            removed,
            validation_report.invalid_normal,
            start.elapsed().as_secs_f32()
        ));
    } else {
        spinner.finish_with_message(format!(
            "Mesh valid: {} triangles [{:.1}s]",
            all_triangles.len(),
            start.elapsed().as_secs_f32()
        ));
    }

    let spinner = create_spinner("Writing STL file...");
    let start = Instant::now();
    write_stl(&output_path, &all_triangles).context("Failed to write STL file")?;
    let file_size = estimate_stl_size(all_triangles.len());
    spinner.finish_with_message(format!(
        "Wrote {} ({:.1} KB) [{:.1}s]",
        output_path.display(),
        file_size as f64 / 1024.0,
        start.elapsed().as_secs_f32()
    ));

    println!();
    println!(
        "Done! Total time: {:.1}s",
        total_start.elapsed().as_secs_f32()
    );
    println!();
    println!("Output: {}", output_path.display());
    println!("  Triangles: {}", all_triangles.len());
    println!("  File size: {:.1} KB", file_size as f64 / 1024.0);
    println!();
    println!("Open in a 3D slicer to verify and print!");

    Ok(())
}

fn generate_text_layer(
    city: &str,
    coords: (f64, f64),
    size_mm: f32,
    primary_text: Option<&str>,
    secondary_text: Option<&str>,
) -> Vec<mesh::Triangle> {
    let mut triangles = Vec::new();

    let text_scale = size_mm / 220.0;
    let text_z = 0.0;

    let city_renderer = TextRenderer::default().with_scale(2.5 * text_scale);
    let primary = primary_text.map(|s| s.to_uppercase()).unwrap_or_else(|| {
        let city_upper = city.to_uppercase();
        city_upper
            .chars()
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
            .join(" ")
    });
    let city_y = 15.0 * text_scale;
    triangles.extend(city_renderer.render_text_centered(&primary, size_mm / 2.0, city_y, text_z));

    let coord_renderer = TextRenderer::default().with_scale(1.0 * text_scale);
    let secondary = secondary_text.map(|s| s.to_string()).unwrap_or_else(|| {
        let (lat, lon) = coords;
        let lat_dir = if lat >= 0.0 { "N" } else { "S" };
        let lon_dir = if lon >= 0.0 { "E" } else { "W" };
        format!(
            "{:.4} {} / {:.4} {}",
            lat.abs(),
            lat_dir,
            lon.abs(),
            lon_dir
        )
    });
    let coord_y = 5.0 * text_scale;
    triangles.extend(coord_renderer.render_text_centered(
        &secondary,
        size_mm / 2.0,
        coord_y,
        text_z,
    ));

    triangles
}

fn create_spinner(message: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("{spinner:.green} {msg}")
            .unwrap()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );
    pb.set_message(message.to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(80));
    pb
}
