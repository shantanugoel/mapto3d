# AGENTS.md - Agentic Coding Guidelines for mapto3d

> Generate 3D-printable STL city maps from OpenStreetMap data

**Rust Edition**: 2024 | **MSRV**: 1.92.0+

---

## Build/Lint/Test Commands

```bash
# Build
cargo build                    # Debug build
cargo build --release          # Release build

# Run
cargo run -- -c "Monaco" -C "Monaco" -r 2000

# Lint & Format
cargo clippy                   # Run clippy lints
cargo clippy -- -D warnings    # Treat warnings as errors (CI mode)
cargo fmt                      # Format code
cargo fmt -- --check           # Check formatting (CI mode)

# Test - all tests
cargo test                     # Run all tests

# Test - single test by name
cargo test test_triangle_normal           # Run test containing this string
cargo test mesh::validation::tests::      # Run tests in specific module
cargo test -- --exact test_valid_triangle # Exact match

# Test - with output
cargo test -- --nocapture      # Show println! output

# Fast compile check (no codegen)
cargo check

# Documentation
cargo doc --open               # Build and open docs
```

---

## Code Style Guidelines

### Imports
Order imports: external crates, then `std::`, then local (`use crate::`/`use super::`).
Separate groups with blank lines.

```rust
use anyhow::{Context, Result, bail};      // External crates
use serde::Deserialize;

use std::collections::HashMap;            // Standard library
use std::path::PathBuf;

use crate::domain::RoadClass;             // Crate modules
use super::Triangle;                      // Local module
```

### Naming Conventions

| Item | Convention | Example |
|------|------------|---------|
| Structs/Enums | PascalCase | `RoadSegment`, `RoadClass::Motorway` |
| Functions | snake_case | `parse_roads`, `calculate_normal` |
| Constants | SCREAMING_SNAKE_CASE | `MIN_TRIANGLE_AREA`, `USER_AGENT` |
| Type aliases | PascalCase | `Coord` |

### Numeric Types

| Context | Type | Reason |
|---------|------|--------|
| WGS84 coordinates (lat/lon) | `f64` | Precision for geospatial math |
| Mesh geometry (vertices, normals) | `f32` | STL format requirement |
| Physical dimensions (mm) | `f32` | Model output |
| Counts/indices | `usize` | Rust convention |
| API parameters (radius, etc.) | `u32` | CLI input |

### Error Handling
Use `anyhow` with context for application errors:
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

### Documentation
- `///` for public items, `//!` for module-level docs
- Include examples in doc comments for complex functions

### Testing
Tests in `#[cfg(test)] mod tests` at bottom of file:
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
- Test names: `test_<what_is_being_tested>`
- Helper functions above `#[test]` functions

### Struct Patterns
Builder pattern with `with_*` methods:
```rust
let config = RoadConfig::default()
    .with_scale(1.5)
    .with_map_radius(radius, size);
```

Guard clauses for early returns:
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

## Architecture Overview

```
src/
├── main.rs           # CLI entry point, argument parsing (clap)
├── api/              # External API clients (Nominatim, Overpass)
├── config/           # TOML config parsing, feature heights
├── domain/           # Core types: RoadSegment, WaterPolygon, ParkPolygon
├── geometry/         # Projection (WGS84->meters), scaling, simplification
├── layers/           # Mesh generation: base, roads, water, parks, text
├── mesh/             # Triangle, MeshBuilder, STL writer, validation
└── osm/              # Overpass response parsing
```

**Coordinate Pipeline:**
```
WGS84 (f64) -> Projector -> Local meters (f64) -> Scaler -> Model mm (f32)
```

---

## Gotchas

1. **STL uses f32** - All mesh vertices must be `f32`, but coordinate math uses `f64`
2. **Winding order matters** - CCW = outward-facing normal (right-hand rule)
3. **Holes in polygons** - Use `extrude_polygon` with holes parameter
4. **API rate limits** - Nominatim has strict rate limits; Overpass may timeout
5. **Large maps** - Simplification (`--simplify`) reduces triangle count
