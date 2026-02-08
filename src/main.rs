use anyhow::{Context, Result, bail};
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use std::collections::HashMap;
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
    CachePolicy, RoadDepth, fetch_parks_with_cache, fetch_roads_with_depth_and_cache,
    fetch_water_with_cache, geocode_city_with_cache,
};
use config::{FeatureHeights, FileConfig};
use geometry::{Bounds, Projector, Scaler};
use layers::{
    RoadConfig, TextRenderer, generate_base_plate, generate_park_meshes, generate_road_meshes,
    generate_water_meshes,
};
use mesh::{stl::estimate_stl_size, validate_and_fix, write_stl};
use osm::{parse_parks, parse_roads, parse_water};

type QuantizedVertex = (i64, i64, i64);
type QuantizedEdge = (QuantizedVertex, QuantizedVertex);

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
    /// Higher values reduce triangle count but may lose curve detail
    #[arg(long, default_value = "0", value_parser = clap::value_parser!(u8).range(0..=3))]
    simplify: u8,

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
    let cache_enabled = if args.no_cache {
        false
    } else {
        file_config
            .as_ref()
            .map(|c| c.cache_enabled)
            .unwrap_or(true)
    };
    let cache_ttl_hours = if args.cache_ttl_hours != 24 {
        args.cache_ttl_hours
    } else {
        file_config
            .as_ref()
            .map(|c| c.cache_ttl_hours)
            .unwrap_or(24)
    };
    let cache_dir = args
        .cache_dir
        .clone()
        .or_else(|| file_config.as_ref().and_then(|c| c.cache_dir.clone()));
    let cache_policy = CachePolicy::new(
        cache_enabled,
        args.refresh,
        cache_ttl_hours.saturating_mul(3600),
        cache_dir,
    );
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
    let font_path = args.font.clone();

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
        println!(
            "  Water features: {}",
            if args.water { "enabled" } else { "disabled" }
        );
        println!(
            "  Park features: {}",
            if args.parks { "enabled" } else { "disabled" }
        );
        println!("  Output: {}", output_path.display());
        println!("  Overpass mirrors: {}", overpass_config.urls.len());
        println!(
            "  HTTP cache: {}",
            if cache_policy.enabled {
                "enabled"
            } else {
                "disabled"
            }
        );
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
        let coords =
            geocode_city_with_cache(c, co, &cache_policy).context("Failed to geocode city")?;
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
    let roads_response = fetch_roads_with_depth_and_cache(
        center,
        radius,
        road_depth,
        &overpass_config,
        &cache_policy,
    )
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

    let water = if args.water {
        let spinner = create_spinner("Fetching water features...");
        let start = Instant::now();
        let water_response =
            fetch_water_with_cache(center, radius, &overpass_config, &cache_policy)
                .context("Failed to fetch water data")?;
        spinner.finish_with_message(format!(
            "Fetched {} water elements [{:.1}s]",
            water_response.elements.len(),
            start.elapsed().as_secs_f32()
        ));

        let parsed = parse_water(&water_response);
        if verbose {
            println!("  Parsed {} water polygons", parsed.len());
        }
        parsed
    } else {
        Vec::new()
    };

    let parks = if args.parks {
        let spinner = create_spinner("Fetching park features...");
        let start = Instant::now();
        let parks_response =
            fetch_parks_with_cache(center, radius, &overpass_config, &cache_policy)
                .context("Failed to fetch park data")?;
        spinner.finish_with_message(format!(
            "Fetched {} park elements [{:.1}s]",
            parks_response.elements.len(),
            start.elapsed().as_secs_f32()
        ));

        let parsed = parse_parks(&parks_response);
        if verbose {
            println!("  Parsed {} park polygons", parsed.len());
        }
        parsed
    } else {
        Vec::new()
    };

    let feature_heights = FeatureHeights::new(base_height, args.water, args.parks);

    let spinner = create_spinner("Setting up coordinate projection...");
    let projector = Projector::new(center);

    let mut all_projected_points: Vec<(f64, f64)> = Vec::new();
    for road in &roads {
        let projected = projector.project_points(&road.points);
        all_projected_points.extend(projected);
    }

    let bounds = Bounds::from_points(&all_projected_points)
        .context("Failed to compute bounds from road points")?;

    let text_margin_mm = 20.0;
    let scaler = Scaler::from_bounds_with_margin(&bounds, size as f64, text_margin_mm);
    spinner.finish_with_message(format!(
        "Map area: {:.0}m x {:.0}m -> {:.0}mm x {:.0}mm (with {:.0}mm text margin)",
        bounds.width(),
        bounds.height(),
        size,
        size - text_margin_mm as f32,
        text_margin_mm
    ));

    let spinner = create_spinner("Generating mesh layers...");
    let start = Instant::now();

    let base_triangles = generate_base_plate(size, base_height);
    if verbose {
        println!("  Base plate: {} triangles", base_triangles.len());
    }

    let water_triangles = if args.water {
        let triangles =
            generate_water_meshes(&water, &projector, &scaler, feature_heights.water_z_top);
        if verbose {
            println!("  Water: {} triangles", triangles.len());
        }
        triangles
    } else {
        Vec::new()
    };

    let park_triangles = if args.parks {
        let triangles =
            generate_park_meshes(&parks, &projector, &scaler, feature_heights.park_z_top);
        if verbose {
            println!("  Parks: {} triangles", triangles.len());
        }
        triangles
    } else {
        Vec::new()
    };

    let road_config = RoadConfig::default()
        .with_scale(road_scale)
        .with_map_radius(radius, size)
        .with_simplify_level(simplify)
        .with_z_top(feature_heights.road_z_top);
    let road_triangles = generate_road_meshes(&roads, &projector, &scaler, &road_config);
    if verbose {
        println!("  Roads: {} triangles", road_triangles.len());
    }

    let text_triangles = generate_text_layer(
        &display_name,
        center,
        size,
        (primary_text.as_deref(), secondary_text.as_deref()),
        font_path.as_deref(),
        feature_heights.text_z_top,
        !args.no_text_fallback,
    );
    if verbose {
        println!("  Text: {} triangles", text_triangles.len());
    }

    let total_triangles = base_triangles.len()
        + water_triangles.len()
        + park_triangles.len()
        + road_triangles.len()
        + text_triangles.len();

    spinner.finish_with_message(format!(
        "Generated {} triangles [{:.1}s]",
        total_triangles,
        start.elapsed().as_secs_f32()
    ));

    let spinner = create_spinner("Validating and writing STL file...");
    let start = Instant::now();

    let mut all_triangles = Vec::new();
    all_triangles.extend(base_triangles);
    all_triangles.extend(water_triangles);
    all_triangles.extend(park_triangles);
    all_triangles.extend(road_triangles);
    all_triangles.extend(text_triangles);

    let (validated, _) = validate_and_fix(all_triangles);
    let file_size = estimate_stl_size(validated.len());

    write_stl(&output_path, &validated).context("Failed to write STL file")?;

    spinner.finish_with_message(format!(
        "Wrote {} triangles ({:.1} KB) [{:.1}s]",
        validated.len(),
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
    println!();
    print_color_change_guide(&feature_heights);

    Ok(())
}

fn print_color_change_guide(heights: &FeatureHeights) {
    use mapto3d::config::heights::LAYER_HEIGHT;

    let base_layers = (heights.base_height / LAYER_HEIGHT).round() as i32;
    let roads_top_layers = (heights.road_z_top / LAYER_HEIGHT).round() as i32;
    let text_top_layers = (heights.text_z_top / LAYER_HEIGHT).round() as i32;

    println!("Multi-Color FDM Printing Guide (0.2mm layer height)");
    println!("====================================================");
    println!();
    println!("Solid column architecture - all features start at z=0, differ in height:");
    println!(
        "  Base:    0.0mm -> {:.1}mm ({} layers)",
        heights.base_height, base_layers
    );

    let mut color_num = 1;

    if heights.water_enabled {
        let water_top_layers = (heights.water_z_top / LAYER_HEIGHT).round() as i32;
        println!(
            "  Water:   0.0mm -> {:.1}mm ({} layers)",
            heights.water_z_top, water_top_layers
        );
    }

    if heights.parks_enabled {
        let parks_top_layers = (heights.park_z_top / LAYER_HEIGHT).round() as i32;
        println!(
            "  Parks:   0.0mm -> {:.1}mm ({} layers)",
            heights.park_z_top, parks_top_layers
        );
    }

    println!(
        "  Roads:   0.0mm -> {:.1}mm ({} layers)",
        heights.road_z_top, roads_top_layers
    );
    println!(
        "  Text:    0.0mm -> {:.1}mm ({} layers - tallest)",
        heights.text_z_top, text_top_layers
    );
    println!();
    println!(
        "Total height: {:.1}mm = {} layers",
        heights.text_z_top, text_top_layers
    );
    println!();
    println!("Color change schedule (based on absolute feature heights):");
    println!(
        "  Layers 1-{}: Base only (Color {})",
        base_layers, color_num
    );
    color_num += 1;
    let mut prev_layers = base_layers;

    if heights.water_enabled {
        let water_top_layers = (heights.water_z_top / LAYER_HEIGHT).round() as i32;
        println!(
            "  Layers {}-{}: Water tops out at {:.1}mm (Color {} for water areas)",
            prev_layers + 1,
            water_top_layers,
            heights.water_z_top,
            color_num
        );
        color_num += 1;
        prev_layers = water_top_layers;
    }

    if heights.parks_enabled {
        let parks_top_layers = (heights.park_z_top / LAYER_HEIGHT).round() as i32;
        println!(
            "  Layers {}-{}: Parks top out at {:.1}mm (Color {} for park areas)",
            prev_layers + 1,
            parks_top_layers,
            heights.park_z_top,
            color_num
        );
        color_num += 1;
        prev_layers = parks_top_layers;
    }

    println!(
        "  Layers {}-{}: Roads top out at {:.1}mm (Color {} for road areas)",
        prev_layers + 1,
        roads_top_layers,
        heights.road_z_top,
        color_num
    );
    color_num += 1;

    println!(
        "  Layers {}-{}: Text tops out at {:.1}mm (Color {} for text)",
        roads_top_layers + 1,
        text_top_layers,
        heights.text_z_top,
        color_num
    );
    println!();
    println!("NOTE: With solid columns, features overlap in XY space.");
    println!("The slicer will show mixed colors on layers where features coexist.");
    println!("For clean color separation, use a multi-material slicer like PrusaSlicer");
    println!("with separate STL files per feature, or accept blended colors.");
    println!();

    if heights.water_enabled && heights.parks_enabled {
        println!("Color palette suggestions:");
        println!("  Classic:    White base, Blue water, Green parks, Gray roads, Black text");
        println!("  Earth:      Tan base, Blue water, Forest green parks, Brown roads, Black text");
        println!(
            "  Monochrome: Light gray base, Medium gray water, Gray parks, Dark gray roads, Black text"
        );
        println!("  Night:      Black base, Navy water, Dark green parks, White roads, Gold text");
    } else if heights.water_enabled {
        println!("Color palette suggestions:");
        println!("  Classic:    White base, Blue water, Gray roads, Black text");
        println!("  Ocean:      Sand base, Cyan water, Coral roads, White text");
        println!("  Night:      Black base, Navy water, White roads, Gold text");
    } else if heights.parks_enabled {
        println!("Color palette suggestions:");
        println!("  Classic:    White base, Green parks, Gray roads, Black text");
        println!("  Earth:      Tan base, Forest green parks, Brown roads, Black text");
        println!("  Night:      Black base, Dark green parks, White roads, Gold text");
    } else {
        println!("Color palette suggestions:");
        println!("  Classic:    White base, Gray roads, Black text");
        println!("  Monochrome: Light gray base, Dark gray roads, Black text");
        println!("  Night:      Black base, White roads, Gold text");
    }
}

fn generate_text_layer(
    city: &str,
    coords: (f64, f64),
    size_mm: f32,
    labels: (Option<&str>, Option<&str>),
    font_path: Option<&std::path::Path>,
    text_z_top: f32,
    allow_fallback: bool,
) -> Vec<mesh::Triangle> {
    let (primary_text, secondary_text) = labels;
    let text_z = 0.0;
    let renderer = TextRenderer::new(font_path, text_z_top);
    let mut triangles = render_text_triangles(
        &renderer,
        city,
        coords,
        size_mm,
        primary_text,
        secondary_text,
        text_z,
    );

    let (boundary_edges, non_manifold_edges) = edge_topology_counts(&triangles);
    if allow_fallback && (boundary_edges > 0 || non_manifold_edges > 0) {
        let fallback_renderer =
            layers::text::TextRenderer::Stroke(layers::text::StrokeTextRenderer::new(text_z_top));
        let fallback_triangles = render_text_triangles(
            &fallback_renderer,
            city,
            coords,
            size_mm,
            primary_text,
            secondary_text,
            text_z,
        );
        let fallback_metrics = edge_topology_counts(&fallback_triangles);

        if fallback_metrics.0 + fallback_metrics.1 <= boundary_edges + non_manifold_edges {
            eprintln!(
                "Warning: text mesh from TTF had topology issues ({} boundary, {} non-manifold edges); using stroke fallback",
                boundary_edges, non_manifold_edges
            );
            triangles = fallback_triangles;
        }
    }

    triangles
}

fn render_text_triangles(
    renderer: &TextRenderer,
    city: &str,
    coords: (f64, f64),
    size_mm: f32,
    primary_text: Option<&str>,
    secondary_text: Option<&str>,
    text_z: f32,
) -> Vec<mesh::Triangle> {
    let mut triangles = Vec::new();
    let primary = primary_text
        .map(|s| s.to_uppercase())
        .unwrap_or_else(|| city.to_uppercase());

    let target_primary_width = size_mm * 0.75;
    let primary_scale = renderer.calculate_scale_for_width(&primary, target_primary_width);
    let primary_y = 12.0 * (size_mm / 220.0);
    triangles.extend(renderer.render_text_centered(
        &primary,
        size_mm / 2.0,
        primary_y,
        text_z,
        primary_scale,
    ));

    let secondary = secondary_text.map(|s| s.to_string()).unwrap_or_else(|| {
        let (lat, lon) = coords;
        let lat_dir = if lat >= 0.0 { "N" } else { "S" };
        let lon_dir = if lon >= 0.0 { "E" } else { "W" };
        format!("{:.4}{} / {:.4}{}", lat.abs(), lat_dir, lon.abs(), lon_dir)
    });

    let target_secondary_width = size_mm * 0.40;
    let secondary_scale = renderer.calculate_scale_for_width(&secondary, target_secondary_width);
    let secondary_y = 4.0 * (size_mm / 220.0);
    triangles.extend(renderer.render_text_centered(
        &secondary,
        size_mm / 2.0,
        secondary_y,
        text_z,
        secondary_scale,
    ));

    triangles
}

fn edge_topology_counts(triangles: &[mesh::Triangle]) -> (usize, usize) {
    let mut counts: HashMap<QuantizedEdge, usize> = HashMap::new();

    for triangle in triangles {
        let vertices = triangle.vertices.map(quantize_vertex);
        for (a, b) in [(0, 1), (1, 2), (2, 0)] {
            let edge = ordered_edge(vertices[a], vertices[b]);
            *counts.entry(edge).or_insert(0) += 1;
        }
    }

    let boundary_edges = counts.values().filter(|&&count| count == 1).count();
    let non_manifold_edges = counts.values().filter(|&&count| count > 2).count();
    (boundary_edges, non_manifold_edges)
}

fn quantize_vertex(vertex: [f32; 3]) -> QuantizedVertex {
    const SCALE: f32 = 10_000.0;
    (
        (vertex[0] * SCALE).round() as i64,
        (vertex[1] * SCALE).round() as i64,
        (vertex[2] * SCALE).round() as i64,
    )
}

fn ordered_edge(a: QuantizedVertex, b: QuantizedVertex) -> QuantizedEdge {
    if a <= b { (a, b) } else { (b, a) }
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
