use anyhow::{Context, Result, bail};
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::PathBuf;
use std::time::Instant;

mod api;
mod domain;
mod geometry;
mod layers;
mod mesh;
mod osm;

use api::{fetch_roads, geocode_city};
use geometry::{Bounds, Projector, Scaler};
use layers::{RoadConfig, TextRenderer, generate_base_plate, generate_road_meshes};
use mesh::{stl::estimate_stl_size, write_stl};
use osm::parse_roads;

/// Generate 3D-printable STL city maps from OpenStreetMap data
///
/// Examples:
///   # Generate San Francisco map with default settings
///   mapto3d -c "San Francisco" -C "USA"
///   
///   # Generate Tokyo with larger radius
///   mapto3d -c "Tokyo" -C "Japan" -r 15000 -o tokyo.stl
///   
///   # Generate Venice (small, detailed)
///   mapto3d -c "Venice" -C "Italy" -r 4000 --road-scale 1.5
#[derive(Parser, Debug)]
#[command(name = "mapto3d")]
#[command(version, about, long_about = None)]
struct Args {
    /// City name
    #[arg(short = 'c', long)]
    city: String,

    /// Country name
    #[arg(short = 'C', long)]
    country: String,

    /// Map radius in meters
    #[arg(short = 'r', long, default_value = "10000")]
    radius: u32,

    /// Output STL file path (defaults to {city}.stl)
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

    /// Enable verbose logging
    #[arg(short = 'v', long)]
    verbose: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let total_start = Instant::now();

    println!("mapto3d - City Map STL Generator");
    println!("================================");
    println!();

    // Determine output path
    let output_path = args.output.clone().unwrap_or_else(|| {
        PathBuf::from(format!(
            "{}.stl",
            args.city.to_lowercase().replace(' ', "_")
        ))
    });

    if args.verbose {
        println!("Configuration:");
        println!("  City: {}", args.city);
        println!("  Country: {}", args.country);
        println!("  Radius: {}m", args.radius);
        println!("  Size: {}mm", args.size);
        println!("  Base height: {}mm", args.base_height);
        println!("  Road scale: {}", args.road_scale);
        println!("  Output: {}", output_path.display());
        println!();
    }

    // Step 1: Geocode city
    let spinner = create_spinner("Geocoding city...");
    let start = Instant::now();
    let center = geocode_city(&args.city, &args.country).context("Failed to geocode city")?;
    spinner.finish_with_message(format!(
        "Geocoded: {}, {} -> ({:.4}, {:.4}) [{:.1}s]",
        args.city,
        args.country,
        center.0,
        center.1,
        start.elapsed().as_secs_f32()
    ));

    // Step 2: Fetch roads from Overpass API
    let spinner = create_spinner("Fetching roads from OpenStreetMap...");
    let start = Instant::now();
    let overpass_response =
        fetch_roads(center, args.radius).context("Failed to fetch roads from Overpass API")?;
    spinner.finish_with_message(format!(
        "Fetched {} elements [{:.1}s]",
        overpass_response.elements.len(),
        start.elapsed().as_secs_f32()
    ));

    // Step 3: Parse roads
    let spinner = create_spinner("Parsing road data...");
    let start = Instant::now();
    let roads = parse_roads(&overpass_response);
    if roads.is_empty() {
        bail!("No roads found in the specified area. Try increasing the radius.");
    }
    spinner.finish_with_message(format!(
        "Parsed {} road segments [{:.1}s]",
        roads.len(),
        start.elapsed().as_secs_f32()
    ));

    // Step 4: Set up coordinate projection and scaling
    let spinner = create_spinner("Setting up coordinate projection...");
    let projector = Projector::new(center);

    // Project all road points to get bounds
    let mut all_projected_points: Vec<(f64, f64)> = Vec::new();
    for road in &roads {
        let projected = projector.project_points(&road.points);
        all_projected_points.extend(projected);
    }

    let bounds = Bounds::from_points(&all_projected_points)
        .context("Failed to compute bounds from road points")?;

    let scaler = Scaler::from_bounds(&bounds, args.size as f64);
    spinner.finish_with_message(format!(
        "Map area: {:.0}m x {:.0}m -> {:.0}mm x {:.0}mm",
        bounds.width(),
        bounds.height(),
        args.size,
        args.size
    ));

    // Step 5: Generate all mesh layers
    let spinner = create_spinner("Generating mesh layers...");
    let start = Instant::now();

    let mut all_triangles = Vec::new();

    // Layer 1: Base plate
    let base_triangles = generate_base_plate(args.size, args.base_height);
    all_triangles.extend(base_triangles);

    // Layer 2: Roads
    let road_config = RoadConfig::default().with_scale(args.road_scale);
    let road_triangles = generate_road_meshes(&roads, &projector, &scaler, &road_config);
    all_triangles.extend(road_triangles);

    // Layer 3: Text (city name and coordinates)
    let text_triangles = generate_text_layer(&args.city, center, args.size);
    all_triangles.extend(text_triangles);

    spinner.finish_with_message(format!(
        "Generated {} triangles [{:.1}s]",
        all_triangles.len(),
        start.elapsed().as_secs_f32()
    ));

    // Step 6: Write STL file
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

    // Summary
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

fn generate_text_layer(city: &str, coords: (f64, f64), size_mm: f32) -> Vec<mesh::Triangle> {
    let mut triangles = Vec::new();

    let text_scale = size_mm / 220.0;
    let text_z = 0.0;

    let city_renderer = TextRenderer::default().with_scale(2.5 * text_scale);
    let city_upper = city.to_uppercase();
    let spaced_city: String = city_upper
        .chars()
        .map(|c| c.to_string())
        .collect::<Vec<_>>()
        .join(" ");
    let city_y = 15.0 * text_scale;
    triangles.extend(city_renderer.render_text_centered(
        &spaced_city,
        size_mm / 2.0,
        city_y,
        text_z,
    ));

    let coord_renderer = TextRenderer::default().with_scale(1.0 * text_scale);
    let (lat, lon) = coords;
    let lat_dir = if lat >= 0.0 { "N" } else { "S" };
    let lon_dir = if lon >= 0.0 { "E" } else { "W" };
    let coord_text = format!(
        "{:.4} {} / {:.4} {}",
        lat.abs(),
        lat_dir,
        lon.abs(),
        lon_dir
    );
    let coord_y = 5.0 * text_scale;
    triangles.extend(coord_renderer.render_text_centered(
        &coord_text,
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
