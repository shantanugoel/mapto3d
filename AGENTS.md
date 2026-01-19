# AGENTS.md - Agentic Coding Guidelines for mapto3d

> mapto3d: Generate 3D-printable STL city maps from OpenStreetMap data

## Project Overview

A Rust CLI tool that:
1. Geocodes city names via Nominatim API
2. Fetches roads, water, and parks from Overpass API
3. Projects WGS84 coordinates to local meters (Transverse Mercator)
4. Generates 3D mesh geometry (roads as ribbons, areas as extruded polygons)
5. Outputs binary STL files for 3D printing

**Rust Edition**: 2024 | **MSRV**: 1.92.0+

---

## Build/Lint/Test Commands

```bash
# Build
cargo build                    # Debug build
cargo build --release          # Release build

# Run
cargo run -- -c "Monaco" -C "Monaco" -r 2000

# Lint
cargo clippy                   # Run clippy lints
cargo clippy -- -D warnings    # Treat warnings as errors

# Format
cargo fmt                      # Format code
cargo fmt -- --check           # Check formatting (CI)

# Test - all tests
cargo test                     # Run all tests

# Test - single test by name
cargo test test_triangle_normal           # Run test containing this string
cargo test mesh::validation::tests::      # Run tests in specific module
cargo test -- --exact test_valid_triangle # Exact match

# Test - single module
cargo test --lib mesh::validation         # Tests in validation module

# Test with output
cargo test -- --nocapture                 # Show println! output

# Check (fast compile check, no codegen)
cargo check

# Documentation
cargo doc --open                          # Build and open docs
```

---

## Project Structure

```
src/
├── main.rs           # CLI entry point, argument parsing (clap)
├── lib.rs            # Public module exports
├── api/              # External API clients
│   ├── nominatim.rs  # Geocoding API
│   └── overpass.rs   # OSM data API, RoadDepth enum
├── config/           # TOML config parsing
├── domain/           # Domain types
│   ├── road.rs       # RoadSegment, RoadClass
│   ├── water.rs      # WaterPolygon
│   └── park.rs       # ParkPolygon
├── geometry/         # Coordinate math
│   ├── projection.rs # WGS84 -> local meters (Projector)
│   ├── scaling.rs    # Bounds, Scaler
│   └── simplify.rs   # Douglas-Peucker simplification
├── layers/           # Mesh generation per feature type
│   ├── base.rs       # Base plate generation
│   ├── roads.rs      # Road ribbon extrusion
│   ├── water.rs      # Water area extrusion
│   ├── parks.rs      # Park area extrusion
│   └── text.rs       # Text label rendering
├── mesh/             # Core mesh types and STL output
│   ├── builder.rs    # Triangle, MeshBuilder
│   ├── extrusion.rs  # Polygon extrusion (with holes)
│   ├── ribbon.rs     # Polyline -> 3D ribbon
│   ├── triangulation.rs  # Polygon triangulation (earcutr)
│   ├── validation.rs # Mesh validation and repair
│   └── stl.rs        # Binary STL writer
└── osm/              # OSM data parsing
    └── parser.rs     # Overpass response -> domain types
```

---

## Code Style Guidelines

### Imports

Order imports in this sequence, separated by blank lines:
1. `use super::` / `use crate::` (local)
2. External crates
3. `std::` library

```rust
use super::Triangle;                      // Local module
use crate::domain::RoadClass;             // Crate modules

use anyhow::{Context, Result, bail};      // External crates
use serde::Deserialize;

use std::collections::HashMap;            // Standard library
use std::path::PathBuf;
```

### Types and Naming

- **Structs**: PascalCase (`RoadSegment`, `ValidationResult`)
- **Enums**: PascalCase with PascalCase variants (`RoadClass::Motorway`)
- **Functions**: snake_case (`parse_roads`, `calculate_normal`)
- **Constants**: SCREAMING_SNAKE_CASE (`MIN_TRIANGLE_AREA`, `USER_AGENT`)
- **Type aliases**: PascalCase

Prefer concrete types over generics unless flexibility is needed:
```rust
// Good: concrete when sufficient
pub fn project_points(&self, points: &[(f64, f64)]) -> Vec<(f64, f64)>

// Good: generic when needed for flexibility
pub fn extend(&mut self, triangles: impl IntoIterator<Item = Triangle>)
```

### Error Handling

Use `anyhow` for application errors with context:
```rust
use anyhow::{Context, Result, bail};

fn load_config(path: &Path) -> Result<Config> {
    let contents = std::fs::read_to_string(path)
        .context(format!("Failed to read config file: {:?}", path))?;
    
    if contents.is_empty() {
        bail!("Config file is empty: {:?}", path);
    }
    
    toml::from_str(&contents).context("Failed to parse config file")
}
```

Use `thiserror` for library/domain errors when needed for pattern matching.

### Documentation

- Add doc comments (`///`) for all public items
- Use `//!` for module-level documentation
- Include examples in doc comments for complex functions:

```rust
/// Calculate the normal vector for a triangle using the cross product
///
/// Uses the right-hand rule: CCW winding = outward normal
fn calculate_normal(vertices: &[[f32; 3]; 3]) -> [f32; 3] { ... }
```

### Testing

Tests live in `#[cfg(test)] mod tests` at bottom of each file:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_triangle() {
        let tri = Triangle::new([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        assert!(!is_degenerate(&tri));
    }
}
```

- Test function names: `test_<what_is_being_tested>`
- Use descriptive assertions with context when helpful
- Helper functions for test setup go above `#[test]` functions

### Numeric Types

- **WGS84 coordinates** (lat/lon): `f64`
- **Mesh geometry** (vertices, normals): `f32` (STL format requirement)
- **Physical dimensions** (mm): `f32`
- **Counts/indices**: `usize`
- **API parameters** (radius, etc.): `u32`

### Struct Patterns

Use builder pattern with `with_*` methods for configuration:
```rust
impl RoadConfig {
    pub fn with_scale(mut self, scale: f32) -> Self {
        self.road_scale = scale;
        self
    }
}

// Usage
let config = RoadConfig::default()
    .with_scale(1.5)
    .with_map_radius(radius, size);
```

### Guard Clauses

Prefer early returns for validation:
```rust
pub fn extrude_polygon(outer: &[(f32, f32)], ...) -> Vec<Triangle> {
    if outer.len() < 3 {
        return Vec::new();  // Early return for invalid input
    }
    // ... main logic
}
```

---

## Key Dependencies

| Crate | Purpose |
|-------|---------|
| `clap` | CLI argument parsing with derive macros |
| `reqwest` | HTTP client (blocking mode) for APIs |
| `serde` / `serde_json` / `toml` | Serialization |
| `geo` | Geometric types and operations |
| `stl_io` | STL file I/O |
| `anyhow` / `thiserror` | Error handling |
| `earcutr` | Polygon triangulation |
| `indicatif` | Progress spinners |

---

## Common Patterns

### Coordinate Pipeline
```
WGS84 (f64) -> Projector -> Local meters (f64) -> Scaler -> Model mm (f32)
```

### Mesh Generation
```rust
// 1. Project coordinates
let projected = projector.project_points(&polygon.points);

// 2. Scale to physical size
let scaled: Vec<(f32, f32)> = projected.iter()
    .map(|&(x, y)| scaler.scale(x, y))
    .collect();

// 3. Extrude to 3D
let triangles = extrude_polygon(&scaled, &holes, z_bottom, z_top);
```

---

## Gotchas

1. **STL uses f32** - All mesh vertices must be `f32`, but coordinate math uses `f64`
2. **Winding order matters** - CCW = outward-facing normal (right-hand rule)
3. **Holes in polygons** - Use `extrude_polygon` with holes parameter
4. **API rate limits** - Nominatim has strict rate limits; Overpass may timeout
5. **Large maps** - Simplification (`--simplify`) reduces triangle count
