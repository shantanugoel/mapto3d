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

use api::{
    CachePolicy, RoadDepth, fetch_parks_with_cache, fetch_roads_with_depth,
    fetch_roads_with_depth_and_cache, fetch_water_with_cache, geocode_city_with_cache,
};
use config::{FeatureHeights, FileConfig};
use geometry::{Bounds, ClipRect, Projector, Scaler};
use layers::{
    RoadConfig, build_park_polygons, build_road_polygons, build_water_polygons,
    generate_base_plate, generate_park_meshes_from_polygons, generate_road_meshes_from_polygons,
    generate_text_output, generate_water_meshes_from_polygons,
};
use mesh::{stl::estimate_stl_size, validate_and_fix, write_stl};
use osm::{parse_parks, parse_roads, parse_water};

/// Generate 3D-printable STL city maps from OpenStreetMap data
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
    #[arg(long, requires = "lon", allow_hyphen_values = true)]
    lat: Option<f64>,

    /// Longitude for direct coordinate input (use with --lat)
    #[arg(long, requires = "lat", allow_hyphen_values = true)]
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

    /// Road width multiplier
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

    /// Disable HTTP response caching
    #[arg(long)]
    no_cache: bool,

    /// Force refresh from network and bypass fresh cache reads
    #[arg(long)]
    refresh: bool,

    /// Cache TTL in hours
    #[arg(long, default_value = "24")]
    cache_ttl_hours: u64,

    /// Cache directory path
    #[arg(long)]
    cache_dir: Option<PathBuf>,

    /// Road simplification level: 0=off (default), 1=light, 2=medium, 3=aggressive
    #[arg(long, default_value = "0", value_parser = clap::value_parser!(u8).range(0..=3))]
    simplify: u8,

    /// Edge margin for map features (roads/water/parks) in mm
    #[arg(long, default_value = "0.0")]
    edge_margin_mm: f32,

    /// Path to TTF font file for text rendering (defaults to fonts/RobotoSerif.ttf)
    #[arg(long)]
    font: Option<PathBuf>,

    /// Disable topology-based fallback to stroke text and force primary TTF text rendering
    #[arg(long)]
    no_text_fallback: bool,

    /// Enable water features (rivers, lakes, sea)
    #[arg(long)]
    water: bool,

    /// Enable park features (parks, forests, green areas)
    #[arg(long)]
    parks: bool,
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

    let city = args.city.clone().or_else(|| file_config.as_ref().and_then(|c| c.city.clone()));
    let country = args.country.clone().or_else(|| file_config.as_ref().and_then(|c| c.country.clone()));
    let lat = args.lat.or_else(|| file_config.as_ref().and_then(|c| c.lat));
    let lon = args.lon.or_else(|| file_config.as_ref().and_then(|c| c.lon));
    let radius = if args.radius != 10000 { args.radius } else { file_config.as_ref().map(|c| c.radius).unwrap_or(10000) };
    let size = if (args.size - 220.0).abs() > 0.01 { args.size } else { file_config.as_ref().map(|c| c.size).unwrap_or(220.0) };
    let base_height = if (args.base_height - 2.0).abs() > 0.01 { args.base_height } else { file_config.as_ref().map(|c| c.base_height).unwrap_or(2.0) };
    let road_scale = if (args.road_scale - 1.0).abs() > 0.01 { args.road_scale } else { file_config.as_ref().map(|c| c.road_scale).unwrap_or(1.0) };
    let road_depth = if args.road_depth != RoadDepth::Primary { args.road_depth } else { file_config.as_ref().map(|c| c.road_depth).unwrap_or(RoadDepth::Primary) };
    let simplify = if args.simplify != 0 { args.simplify } else { file_config.as_ref().map(|c| c.simplify).unwrap_or(0) };
    let edge_margin_mm = if (args.edge_margin_mm - 0.0).abs() > 0.01 {
        args.edge_margin_mm
    } else {
        file_config.as_ref().map(|c| c.edge_margin_mm).unwrap_or(0.0)
    };
    let verbose = args.verbose || file_config.as_ref().map(|c| c.verbose).unwrap_or(false);
    let cache_enabled = if args.no_cache { false } else { file_config.as_ref().map(|c| c.cache_enabled).unwrap_or(true) };
    let cache_ttl_hours = if args.cache_ttl_hours != 24 { args.cache_ttl_hours } else { file_config.as_ref().map(|c| c.cache_ttl_hours).unwrap_or(24) };
    let cache_dir = args.cache_dir.clone().or_else(|| file_config.as_ref().and_then(|c| c.cache_dir.clone()));
    let cache_policy = CachePolicy::new(cache_enabled, args.refresh, cache_ttl_hours.saturating_mul(3600), cache_dir);
    let primary_text = args.primary_text.clone().or_else(|| file_config.as_ref().and_then(|c| c.primary_text.clone()));
    let secondary_text = args.secondary_text.clone().or_else(|| file_config.as_ref().and_then(|c| c.secondary_text.clone()));
    let output = args.output.clone().or_else(|| file_config.as_ref().and_then(|c| c.output.clone()));
    let font_path = args.font.clone();

    let overpass_config = file_config.as_ref().and_then(|c| c.overpass.clone()).unwrap_or_default();

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

    let display_name = city.clone().unwrap_or_else(|| "Custom Location".to_string());

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
        println!("  Edge margin: {}mm", edge_margin_mm);
        println!("  Water features: {}", if args.water { "enabled" } else { "disabled" });
        println!("  Park features: {}", if args.parks { "enabled" } else { "disabled" });
        println!("  Output: {}", output_path.display());
        println!("  Overpass mirrors: {}", overpass_config.urls.len());
        println!("  HTTP cache: {}", if cache_policy.enabled { "enabled" } else { "disabled" });
        if cache_policy.enabled {
            println!("  Cache refresh: {}", args.refresh);
            println!("  Cache TTL: {}h", cache_ttl_hours);
            println!("  Cache dir: {}", cache_policy.cache_dir.display());
        }
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
        let coords = geocode_city_with_cache(c, co, &cache_policy).context("Failed to geocode city")?;
        spinner.finish_with_message(format!("Geocoded: {}, {} -> ({:.4}, {:.4}) [{:.1}s]", c, co, coords.0, coords.1, start.elapsed().as_secs_f32()));
        coords
    };

    let spinner = create_spinner("Fetching roads from OpenStreetMap...");
    let start = Instant::now();
    let roads_response = if cache_policy.enabled {
        fetch_roads_with_depth_and_cache(center, radius, road_depth, &overpass_config, &cache_policy)
    } else {
        fetch_roads_with_depth(center, radius, road_depth, &overpass_config)
    }.context("Failed to fetch roads from Overpass API")?;
    spinner.finish_with_message(format!("Fetched {} road elements [{:.1}s]", roads_response.elements.len(), start.elapsed().as_secs_f32()));

    let spinner = create_spinner("Parsing road data...");
    let start = Instant::now();
    let roads = parse_roads(&roads_response);
    if roads.is_empty() {
        bail!("No roads found in the specified area. Try increasing the radius or using --road-depth all");
    }
    spinner.finish_with_message(format!("Parsed {} road segments [{:.1}s]", roads.len(), start.elapsed().as_secs_f32()));

    let water = if args.water {
        let spinner = create_spinner("Fetching water features...");
        let start = Instant::now();
        let water_response = fetch_water_with_cache(center, radius, &overpass_config, &cache_policy).context("Failed to fetch water data")?;
        spinner.finish_with_message(format!("Fetched {} water elements [{:.1}s]", water_response.elements.len(), start.elapsed().as_secs_f32()));
        let parsed = parse_water(&water_response);
        if verbose { println!("  Parsed {} water polygons", parsed.len()); }
        parsed
    } else {
        Vec::new()
    };

    let parks = if args.parks {
        let spinner = create_spinner("Fetching park features...");
        let start = Instant::now();
        let parks_response = fetch_parks_with_cache(center, radius, &overpass_config, &cache_policy).context("Failed to fetch park data")?;
        spinner.finish_with_message(format!("Fetched {} park elements [{:.1}s]", parks_response.elements.len(), start.elapsed().as_secs_f32()));
        let parsed = parse_parks(&parks_response);
        if verbose { println!("  Parsed {} park polygons", parsed.len()); }
        parsed
    } else {
        Vec::new()
    };

    let feature_heights = FeatureHeights::new(base_height, args.water, args.parks);
    let projector = Projector::new(center);

    let mut all_projected_points: Vec<(f64, f64)> = Vec::new();
    for road in &roads {
        let projected = projector.project_points(&road.points);
        all_projected_points.extend(projected);
    }

    let bounds = Bounds::from_points(&all_projected_points).context("Failed to compute bounds from road points")?;
    let scaler = Scaler::from_bounds_with_edge_margin(&bounds, size as f64, edge_margin_mm as f64);
    let clip_rect = ClipRect::from_bounds(&bounds, &scaler);
    
    let spinner = create_spinner("Generating mesh layers...");
    let start = Instant::now();

    let road_config = RoadConfig::default()
        .with_scale(road_scale)
        .with_map_radius(radius, size)
        .with_simplify_level(simplify)
        .with_z_top(feature_heights.road_z_top);

    let road_footprint = build_road_polygons(&roads, &projector, &scaler, &road_config);
    let water_footprint = if args.water { build_water_polygons(&water, &projector, &scaler, &clip_rect) } else { geo::MultiPolygon::new(vec![]) };
    let park_footprint = if args.parks { build_park_polygons(&parks, &projector, &scaler, &clip_rect) } else { geo::MultiPolygon::new(vec![]) };
    let text_output = generate_text_output(
        &display_name,
        center,
        size,
        (primary_text.as_deref(), secondary_text.as_deref()),
        font_path.as_deref(),
        feature_heights.text_z_top,
        !args.no_text_fallback,
    );

    let mut all_footprints = Vec::new();
    all_footprints.extend(road_footprint.0.iter().cloned());
    all_footprints.extend(water_footprint.0.iter().cloned());
    all_footprints.extend(park_footprint.0.iter().cloned());
    all_footprints.extend(text_output.footprint.0.iter().cloned());
    let combined_footprint = geometry::union_polygons_batched(all_footprints, 500);

    let base_triangles = generate_base_plate(size, base_height, Some(&combined_footprint));
    let water_triangles = if args.water {
        generate_water_meshes_from_polygons(&water_footprint, feature_heights.water_z_top)
    } else {
        Vec::new()
    };
    let park_triangles = if args.parks {
        generate_park_meshes_from_polygons(&park_footprint, feature_heights.park_z_top)
    } else {
        Vec::new()
    };
    let road_triangles = generate_road_meshes_from_polygons(&road_footprint, feature_heights.road_z_top);
    let text_triangles = text_output.triangles;

    let total_triangles = base_triangles.len() + water_triangles.len() + park_triangles.len() + road_triangles.len() + text_triangles.len();
    spinner.finish_with_message(format!("Generated {} triangles [{:.1}s]", total_triangles, start.elapsed().as_secs_f32()));

    let spinner = create_spinner("Validating and writing STL file...");
    let start = Instant::now();
    let mut all_triangles = Vec::new();
    all_triangles.extend(base_triangles);
    all_triangles.extend(water_triangles);
    all_triangles.extend(park_triangles);
    all_triangles.extend(road_triangles);
    all_triangles.extend(text_triangles);

    let (validated, report) = validate_and_fix(all_triangles);
    if report.has_issues() {
        eprintln!("Warning: {}", report.summary());
        for warning in &report.warnings { eprintln!("  {warning}"); }
    }
    
    write_stl(&output_path, &validated).context("Failed to write STL file")?;
    spinner.finish_with_message(format!("Wrote {} triangles ({:.1} KB) [{:.1}s]", validated.len(), estimate_stl_size(validated.len()) as f64 / 1024.0, start.elapsed().as_secs_f32()));

    println!("\nDone! Total time: {:.1}s\nOutput: {}\n", total_start.elapsed().as_secs_f32(), output_path.display());
    print_color_change_guide(&feature_heights);
    Ok(())
}

fn print_color_change_guide(heights: &FeatureHeights) {
    const LAYER_HEIGHT: f32 = 0.2;

    let base_layers = (heights.base_height / LAYER_HEIGHT).round() as i32;
    let roads_top_layers = (heights.road_z_top / LAYER_HEIGHT).round() as i32;
    let text_top_layers = (heights.text_z_top / LAYER_HEIGHT).round() as i32;

    println!("Multi-Color FDM Printing Guide (0.2mm layer height)");
    println!("====================================================");
    println!();
    println!("Solid column architecture - all features start at z=0, differ in height:");
    println!("  Base:    0.0mm -> {:.1}mm ({} layers)", heights.base_height, base_layers);
    
    let mut color_num = 1;
    let mut prev_layers = base_layers;

    if heights.water_enabled {
        let water_top_layers = (heights.water_z_top / LAYER_HEIGHT).round() as i32;
        println!("  Water:   0.0mm -> {:.1}mm ({} layers)", heights.water_z_top, water_top_layers);
    }
    if heights.parks_enabled {
        let parks_top_layers = (heights.park_z_top / LAYER_HEIGHT).round() as i32;
        println!("  Parks:   0.0mm -> {:.1}mm ({} layers)", heights.park_z_top, parks_top_layers);
    }
    println!("  Roads:   0.0mm -> {:.1}mm ({} layers)", heights.road_z_top, roads_top_layers);
    println!("  Text:    0.0mm -> {:.1}mm ({} layers - tallest)", heights.text_z_top, text_top_layers);
    
    println!("\nColor change schedule:");
    println!("  Layers 1-{}: Base only (Color {})", base_layers, color_num);
    color_num += 1;

    if heights.water_enabled {
        let water_top_layers = (heights.water_z_top / LAYER_HEIGHT).round() as i32;
        println!("  Layers {}-{}: Water tops out at {:.1}mm (Color {})", prev_layers + 1, water_top_layers, heights.water_z_top, color_num);
        color_num += 1;
        prev_layers = water_top_layers;
    }
    if heights.parks_enabled {
        let parks_top_layers = (heights.park_z_top / LAYER_HEIGHT).round() as i32;
        println!("  Layers {}-{}: Parks top out at {:.1}mm (Color {})", prev_layers + 1, parks_top_layers, heights.park_z_top, color_num);
        color_num += 1;
        prev_layers = parks_top_layers;
    }
    println!("  Layers {}-{}: Roads top out at {:.1}mm (Color {})", prev_layers + 1, roads_top_layers, heights.road_z_top, color_num);
    color_num += 1;
    println!("  Layers {}-{}: Text tops out at {:.1}mm (Color {})", roads_top_layers + 1, text_top_layers, heights.text_z_top, color_num);
}

fn create_spinner(message: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(ProgressStyle::with_template("{spinner:.green} {msg}").unwrap().tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]));
    pb.set_message(message.to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(80));
    pb
}
