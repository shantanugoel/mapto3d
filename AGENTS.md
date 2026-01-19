# mapto3d - Agent Delegation Guide

> Implementation delegation structure for the mapto3d project

This document defines how to break down and delegate the implementation work using specialized AI agents.

---

## Agent Types Available

| Agent | Specialty | When to Use |
|-------|-----------|-------------|
| **Sisyphus-Junior** | Focused task executor | Single-file implementations, specific algorithms |
| **oracle** | Architecture & debugging | Complex design decisions, stuck on hard bugs |
| **explore** | Codebase search | Finding patterns, understanding existing code |
| **librarian** | External research | Crate documentation, OSS examples, API docs |
| **frontend-ui-ux-engineer** | Visual design | N/A for this CLI project |
| **document-writer** | Documentation | README, API docs, user guides |

---

## Implementation Delegation Plan

### Phase 1: Foundation (MVP)

#### Task 1.1: CLI Setup
```
delegate_task(
  category="quick",
  skills=[],
  prompt="""
TASK: Implement CLI argument parsing for mapto3d

FILE: src/main.rs

REQUIREMENTS:
1. Use clap with derive feature
2. Define Args struct with:
   - city: String (required, short: c)
   - country: String (required, short: C)  
   - radius: u32 (default: 10000, short: r)
   - output: Option<PathBuf> (short: o)
   - size: f32 (default: 220.0, short: s)
   - base_height: f32 (default: 2.0)
   - road_scale: f32 (default: 1.0)
   - verbose: bool (flag, short: v)
3. Add proper help text and examples
4. Parse args and print them (placeholder)

MUST DO:
- Add clap = { version = "4", features = ["derive"] } to Cargo.toml
- Use #[derive(Parser)] 
- Include usage examples in help

MUST NOT DO:
- Implement actual logic yet
- Add unnecessary dependencies

VERIFICATION: cargo build --release should succeed
"""
)
```

#### Task 1.2: Nominatim Geocoding Client
```
delegate_task(
  category="quick",
  skills=[],
  prompt="""
TASK: Implement Nominatim geocoding client

FILES:
- src/api/mod.rs (create, export nominatim)
- src/api/nominatim.rs (create)

REQUIREMENTS:
1. Create geocode_city(city: &str, country: &str) -> Result<(f64, f64)>
2. Use reqwest blocking client
3. Hit Nominatim API: https://nominatim.openstreetmap.org/search
4. Parse JSON response to extract lat/lon
5. Set proper User-Agent header (required by Nominatim ToS)
6. Return anyhow::Result with descriptive errors

EXAMPLE QUERY:
https://nominatim.openstreetmap.org/search?q=San+Francisco,USA&format=json&limit=1

MUST DO:
- Add reqwest = { version = "0.11", features = ["blocking", "json"] }
- Add serde, serde_json to Cargo.toml
- Create proper error messages for "city not found"
- Add 1 second delay for rate limiting

MUST NOT DO:
- Use async (blocking only for v1)
- Cache results yet

VERIFICATION: 
- cargo build
- Unit test that parses sample Nominatim response
"""
)
```

#### Task 1.3: Overpass API Client (Roads)
```
delegate_task(
  category="quick", 
  skills=[],
  prompt="""
TASK: Implement Overpass API client for fetching roads

FILES:
- src/api/overpass.rs (create)
- src/api/mod.rs (update exports)

REQUIREMENTS:
1. Create fetch_roads(center: (f64, f64), radius_m: u32) -> Result<OverpassResponse>
2. Build Overpass QL query for highway ways in bounding box
3. Use POST to https://overpass-api.de/api/interpreter
4. Parse JSON response into structured types

OVERPASS QUERY:
```
[out:json][timeout:60];
(
  way["highway"]({{south}},{{west}},{{north}},{{east}});
);
out body;
>;
out skel qt;
```

TYPES TO CREATE:
- OverpassResponse { elements: Vec<Element> }
- Element { type_: String, id: u64, nodes: Option<Vec<u64>>, tags: Option<HashMap<String, String>>, lat: Option<f64>, lon: Option<f64> }

MUST DO:
- Calculate bounding box from center + radius
- Handle both way and node elements
- Add timeout handling

MUST NOT DO:
- Parse into domain types yet (just raw Overpass types)
- Fetch water/parks yet

VERIFICATION: cargo build, test with small radius
"""
)
```

#### Task 1.4: OSM Parser → Domain Types
```
delegate_task(
  category="quick",
  skills=[],
  prompt="""
TASK: Parse Overpass response into domain road segments

FILES:
- src/domain/mod.rs (create)
- src/domain/road.rs (create)
- src/osm/mod.rs (create)
- src/osm/parser.rs (create)
- src/osm/tags.rs (create)

DOMAIN TYPES:
```rust
pub enum RoadClass {
    Motorway,
    Primary,
    Secondary,
    Tertiary,
    Residential,
}

pub struct RoadSegment {
    pub points: Vec<(f64, f64)>,  // lat/lon
    pub class: RoadClass,
    pub layer: i8,  // for bridges/tunnels (-1, 0, 1)
}
```

CLASSIFICATION (from OSM highway tag):
- motorway, motorway_link → Motorway
- trunk, trunk_link, primary, primary_link → Primary
- secondary, secondary_link → Secondary
- tertiary, tertiary_link → Tertiary
- residential, living_street, unclassified, service → Residential

PARSER FUNCTION:
pub fn parse_roads(response: &OverpassResponse) -> Vec<RoadSegment>

MUST DO:
- Build node_id → (lat, lon) lookup map first
- For each way, resolve node refs to coordinates
- Extract highway tag for classification
- Extract layer tag (default 0)

MUST NOT DO:
- Handle multipolygons yet
- Parse water/parks yet

VERIFICATION: cargo build, unit tests for classification
"""
)
```

#### Task 1.5: Coordinate Projection
```
delegate_task(
  category="quick",
  skills=[],
  prompt="""
TASK: Implement WGS84 to local meters projection

FILES:
- src/geometry/mod.rs (create)
- src/geometry/projection.rs (create)
- src/geometry/scaling.rs (create)

REQUIREMENTS:
1. Project lat/lon to meters using simple Mercator approximation:
   - x = lon * cos(center_lat) * 111320
   - y = lat * 111320
   (Good enough for city-scale, avoids proj crate complexity)

2. Create Projector struct:
```rust
pub struct Projector {
    center_lat: f64,
    center_lon: f64,
}

impl Projector {
    pub fn new(center: (f64, f64)) -> Self;
    pub fn project(&self, lat: f64, lon: f64) -> (f64, f64);  // returns (x_m, y_m)
}
```

3. Create Scaler for physical dimensions:
```rust
pub struct Scaler {
    scale: f64,  // mm per meter
    offset_x: f64,
    offset_y: f64,
}

impl Scaler {
    pub fn from_bounds(bounds: &Bounds, target_mm: f64) -> Self;
    pub fn scale(&self, x: f64, y: f64) -> (f32, f32);  // returns mm coordinates
}
```

MUST DO:
- Use f64 for intermediate calculations
- Return f32 for final STL coordinates
- Center the map in the target area

MUST NOT DO:
- Use proj crate (too complex for v1)
- Handle coordinate edge cases (antimeridian, poles)

VERIFICATION: cargo build, test that 1km projects to ~1000m
"""
)
```

#### Task 1.6: Road Ribbon Extrusion
```
delegate_task(
  category="quick",
  skills=[],
  prompt="""
TASK: Implement road polyline to 3D ribbon mesh extrusion

FILES:
- src/mesh/mod.rs (create)
- src/mesh/ribbon.rs (create)
- src/mesh/builder.rs (create)

REQUIREMENTS:
1. Create Triangle struct for STL output:
```rust
pub struct Triangle {
    pub vertices: [[f32; 3]; 3],
    pub normal: [f32; 3],
}
```

2. Create MeshBuilder:
```rust
pub struct MeshBuilder {
    triangles: Vec<Triangle>,
}

impl MeshBuilder {
    pub fn new() -> Self;
    pub fn add_triangle(&mut self, v0: [f32; 3], v1: [f32; 3], v2: [f32; 3]);
    pub fn finish(self) -> Vec<Triangle>;
}
```

3. Create ribbon extrusion:
```rust
pub fn extrude_ribbon(
    points: &[(f32, f32)],  // 2D points in mm
    width: f32,             // ribbon width in mm
    height: f32,            // extrusion height in mm
    base_z: f32,            // base z level
) -> Vec<Triangle>
```

ALGORITHM:
1. For each segment (p1, p2):
   - direction = normalize(p2 - p1)
   - perpendicular = (-direction.y, direction.x)
   - left = p - perpendicular * width/2
   - right = p + perpendicular * width/2
2. Create quads between consecutive left/right pairs
3. Split quads into triangles
4. Add top face (same as bottom but at z + height)
5. Add side faces

MUST DO:
- Calculate normals correctly (outward facing)
- Handle degenerate segments (zero length)
- Use miter joins at corners (simple version: just connect)

MUST NOT DO:
- Complex miter limit handling
- Rounded caps

VERIFICATION: cargo build, generate simple 2-segment ribbon
"""
)
```

#### Task 1.7: STL File Output
```
delegate_task(
  category="quick",
  skills=[],
  prompt="""
TASK: Implement binary STL file writer

FILES:
- src/mesh/stl.rs (create)
- src/mesh/mod.rs (update exports)

REQUIREMENTS:
1. Write binary STL format:
   - 80 byte header (can be zeros or project name)
   - 4 byte u32 triangle count (little endian)
   - For each triangle:
     - 3 x f32 normal (12 bytes)
     - 3 x 3 x f32 vertices (36 bytes)
     - 2 byte attribute (usually 0)

2. Create write function:
```rust
pub fn write_stl(path: &Path, triangles: &[Triangle]) -> Result<()>
```

MUST DO:
- Use little-endian byte order
- Add stl_io = "0.7" to Cargo.toml as alternative/reference
- Validate triangle count fits in u32

MUST NOT DO:
- Write ASCII STL (binary only for size)
- Compress output

VERIFICATION: 
- cargo build
- Generate test STL, open in 3D viewer/slicer

REFERENCE: https://en.wikipedia.org/wiki/STL_(file_format)#Binary
"""
)
```

#### Task 1.8: Integration
```
delegate_task(
  category="quick",
  skills=[],
  prompt="""
TASK: Integrate all modules into working CLI

FILES:
- src/main.rs (update)
- src/lib.rs (create, re-export modules)
- src/layers/mod.rs (create)
- src/layers/roads.rs (create)

REQUIREMENTS:
1. Wire up the full pipeline in main():
   - Parse CLI args
   - Geocode city
   - Fetch roads from Overpass
   - Parse into RoadSegments
   - Project coordinates
   - Scale to physical dimensions
   - Generate road ribbons
   - Write STL

2. Create road layer processor:
```rust
pub fn generate_road_meshes(
    roads: &[RoadSegment],
    projector: &Projector,
    scaler: &Scaler,
    config: &RoadConfig,
) -> Vec<Triangle>
```

3. RoadConfig with widths/heights per class:
```rust
pub struct RoadConfig {
    pub motorway: (f32, f32),    // (width_mm, height_mm)
    pub primary: (f32, f32),
    pub secondary: (f32, f32),
    pub tertiary: (f32, f32),
    pub residential: (f32, f32),
}
```

DEFAULT VALUES:
- Motorway: 3.0mm wide, 2.0mm high
- Primary: 2.5mm wide, 1.5mm high
- Secondary: 2.0mm wide, 1.0mm high
- Tertiary: 1.5mm wide, 0.7mm high
- Residential: 0.8mm wide, 0.5mm high

MUST DO:
- Add progress output (println! for now)
- Handle errors gracefully with context
- Print summary (triangle count, file size)

MUST NOT DO:
- Add water/parks yet
- Add base plate yet

VERIFICATION:
- cargo run -- -c "Venice" -C "Italy" -r 4000 -o test.stl
- Open test.stl in slicer
"""
)
```

---

### Phase 2: Complete Layers

#### Task 2.1-2.2: Water and Parks Fetching
```
delegate_task(
  category="quick",
  skills=[],
  prompt="""
TASK: Extend Overpass client to fetch water and parks

FILES:
- src/api/overpass.rs (update)
- src/domain/water.rs (create)
- src/domain/park.rs (create)

REQUIREMENTS:
1. Add fetch_water() and fetch_parks() functions
2. Create domain types:

```rust
pub struct WaterPolygon {
    pub outer: Vec<(f64, f64)>,
    pub holes: Vec<Vec<(f64, f64)>>,
}

pub struct ParkPolygon {
    pub outer: Vec<(f64, f64)>,
}
```

OVERPASS QUERIES:
Water:
```
[out:json][timeout:60];
(
  way["natural"="water"](bbox);
  way["waterway"="riverbank"](bbox);
  relation["natural"="water"](bbox);
);
out body;
>;
out skel qt;
```

Parks:
```
[out:json][timeout:60];
(
  way["leisure"="park"](bbox);
  way["landuse"="grass"](bbox);
);
out body;
>;
out skel qt;
```

MUST DO:
- Handle closed ways (polygon where first node == last node)
- Skip unclosed ways

MUST NOT DO:
- Handle multipolygon relations yet (complex, skip for v1)

VERIFICATION: cargo build, test fetch with Venice
"""
)
```

#### Task 2.3-2.4: Polygon Triangulation
```
delegate_task(
  category="quick",
  skills=[],
  prompt="""
TASK: Implement polygon triangulation for water/parks

FILES:
- src/mesh/triangulation.rs (create)
- src/mesh/extrusion.rs (create)

REQUIREMENTS:
1. Add earclip = "0.1" to Cargo.toml
2. Create triangulate function:

```rust
pub fn triangulate_polygon(
    outer: &[(f32, f32)],
    holes: &[Vec<(f32, f32)>],
) -> Vec<[usize; 3]>  // triangle indices
```

3. Create polygon extrusion:
```rust
pub fn extrude_polygon(
    outer: &[(f32, f32)],
    holes: &[Vec<(f32, f32)>],
    z_bottom: f32,
    z_top: f32,
) -> Vec<Triangle>
```

ALGORITHM:
1. Triangulate the 2D polygon using earclip
2. Create bottom face (triangles at z_bottom, reversed winding)
3. Create top face (triangles at z_top, normal winding)
4. Create side walls (quads between consecutive edge points)

MUST DO:
- Ensure consistent winding order (CCW for top, CW for bottom)
- Calculate correct normals
- Handle empty/degenerate polygons gracefully

MUST NOT DO:
- Complex polygon cleaning/repair

VERIFICATION: cargo build, test with simple square polygon
"""
)
```

#### Task 2.5-2.7: Water, Parks, and Base Layers
```
delegate_task(
  category="quick",
  skills=[],
  prompt="""
TASK: Implement water, parks, and base plate layer generation

FILES:
- src/layers/water.rs (create)
- src/layers/parks.rs (create)
- src/layers/base.rs (create)
- src/layers/mod.rs (update)

REQUIREMENTS:

1. Water layer (recessed):
```rust
pub fn generate_water_meshes(
    water: &[WaterPolygon],
    projector: &Projector,
    scaler: &Scaler,
) -> Vec<Triangle>
```
- Extrude from z=-1.0 to z=0.0 (recessed into base)

2. Parks layer (raised):
```rust
pub fn generate_park_meshes(
    parks: &[ParkPolygon],
    projector: &Projector,
    scaler: &Scaler,
) -> Vec<Triangle>
```
- Extrude from z=0.0 to z=0.3

3. Base plate:
```rust
pub fn generate_base_plate(
    size_mm: f32,
    thickness: f32,
) -> Vec<Triangle>
```
- Simple box: size_mm x size_mm x thickness
- Centered at (size/2, size/2)
- z from -thickness to 0

MUST DO:
- Use the extrude_polygon function from mesh module
- Handle empty water/parks gracefully

MUST NOT DO:
- Boolean operations (just overlap layers)

VERIFICATION: cargo build, generate model with all layers
"""
)
```

#### Task 2.8: Layer Assembly
```
delegate_task(
  category="quick",
  skills=[],
  prompt="""
TASK: Assemble all layers into final mesh

FILES:
- src/main.rs (update)
- src/lib.rs (update exports)

REQUIREMENTS:
1. Update main() to:
   - Fetch roads, water, parks in sequence
   - Generate all layer meshes
   - Combine into single triangle list
   - Write to STL

2. Layer order (z-levels):
   - Base plate: z = -2.0 to 0.0
   - Water: z = -1.0 to 0.0 (cut into base)
   - Parks: z = 0.0 to 0.3
   - Roads: z = 0.0 to (height by class)

3. Add progress reporting:
   - "Geocoding city..."
   - "Fetching roads... (X segments)"
   - "Fetching water... (X polygons)"
   - "Fetching parks... (X polygons)"
   - "Generating mesh... (X triangles)"
   - "Writing STL... (X KB)"

MUST DO:
- Handle missing water/parks gracefully (empty vec)
- Print timing for each step
- Report final statistics

MUST NOT DO:
- Mesh boolean operations
- Deduplication of overlapping triangles

VERIFICATION:
- cargo run -- -c "Amsterdam" -C "Netherlands" -r 6000
- Verify STL opens in slicer and looks correct
"""
)
```

---

### Phase 3: Quality & Polish

#### Task 3.1-3.2: Projection & Scaling Improvements
```
delegate_task(
  category="quick",
  skills=[],
  prompt="""
TASK: Improve coordinate projection and road width scaling

FILES:
- src/geometry/projection.rs (update)
- src/config/defaults.rs (create)
- src/config/mod.rs (create)

REQUIREMENTS:
1. Better Mercator projection with scale factor at center latitude

2. Dynamic road width scaling:
   - For small maps (< 5km): use default widths
   - For large maps (> 15km): scale widths up proportionally
   - Ensure minimum width of 0.6mm (FDM printability)

3. Create defaults module:
```rust
pub struct MapConfig {
    pub size_mm: f32,
    pub base_height: f32,
    pub road_scale: f32,
    pub min_feature_width: f32,
    pub min_feature_height: f32,
}

impl Default for MapConfig { ... }
```

MUST DO:
- Apply road_scale CLI argument
- Clamp widths to minimum printable size
- Log when features are scaled up

MUST NOT DO:
- Use proj crate

VERIFICATION: Generate maps at 5km and 20km, verify road proportions
"""
)
```

#### Task 3.3: Geometry Simplification
```
delegate_task(
  category="quick",
  skills=[],
  prompt="""
TASK: Implement Douglas-Peucker line simplification

FILES:
- src/geometry/simplify.rs (create)
- src/geometry/mod.rs (update)

REQUIREMENTS:
1. Implement Douglas-Peucker algorithm:
```rust
pub fn simplify_polyline(
    points: &[(f64, f64)],
    epsilon: f64,  // tolerance in source units (meters)
) -> Vec<(f64, f64)>
```

2. Apply to roads before extrusion to reduce triangle count

3. Apply to polygon outlines before triangulation

ALGORITHM:
1. Find point farthest from line between first and last
2. If distance > epsilon, recursively simplify both halves
3. Otherwise, remove intermediate points

MUST DO:
- Preserve first and last points
- Handle edge cases (< 3 points)
- Make epsilon configurable via CLI

MUST NOT DO:
- Remove too many points (default epsilon = 5m)

VERIFICATION: 
- cargo build
- Compare triangle count with/without simplification
- Visually verify no obvious quality loss
"""
)
```

#### Task 3.4-3.5: Mesh Validation
```
delegate_task(
  category="quick",
  skills=[],
  prompt="""
TASK: Add mesh validation and normal orientation checks

FILES:
- src/mesh/validation.rs (create)
- src/mesh/mod.rs (update)

REQUIREMENTS:
1. Validate triangles:
```rust
pub fn validate_mesh(triangles: &[Triangle]) -> ValidationResult {
    // Check for:
    // - Degenerate triangles (zero area)
    // - NaN/Inf coordinates
    // - Incorrect normal orientation
}

pub struct ValidationResult {
    pub total: usize,
    pub degenerate: usize,
    pub invalid_normal: usize,
    pub warnings: Vec<String>,
}
```

2. Fix normals:
```rust
pub fn fix_normals(triangles: &mut [Triangle]) {
    // Recalculate normal from vertices using cross product
    // Ensure right-hand rule (CCW winding = outward normal)
}
```

3. Filter degenerates:
```rust
pub fn remove_degenerate(triangles: Vec<Triangle>) -> Vec<Triangle> {
    // Remove triangles with area < epsilon
}
```

MUST DO:
- Run validation before STL write
- Log warnings for issues found
- Auto-fix normals

MUST NOT DO:
- Fail on minor issues (just warn)

VERIFICATION: cargo build, test with known good/bad meshes
"""
)
```

#### Task 3.6-3.7: Progress & Error Handling
```
delegate_task(
  category="quick",
  skills=[],
  prompt="""
TASK: Add progress bars and improve error handling

FILES:
- src/main.rs (update)
- Cargo.toml (add indicatif, thiserror)

REQUIREMENTS:
1. Add indicatif progress bars:
   - Spinner for API calls
   - Progress bar for mesh generation

2. Create custom error types:
```rust
#[derive(thiserror::Error, Debug)]
pub enum MapError {
    #[error("City not found: {0}")]
    CityNotFound(String),
    
    #[error("Overpass API error: {0}")]
    OverpassError(String),
    
    #[error("No roads found in area")]
    NoRoads,
    
    #[error("Mesh generation failed: {0}")]
    MeshError(String),
}
```

3. Add context to all errors using anyhow::Context

MUST DO:
- Show elapsed time for each step
- Clear progress on completion
- Provide actionable error messages

MUST NOT DO:
- Panic on errors (always return Result)

VERIFICATION: Test with invalid city, verify error message
"""
)
```

---

### Phase 4: Documentation

#### Task 4.1: README and User Guide
```
delegate_task(
  subagent_type="document-writer",
  skills=[],
  prompt="""
TASK: Write comprehensive README.md for mapto3d

FILE: README.md (in project root, not in maptoposter subdir)

REQUIREMENTS:
1. Project overview and screenshot placeholder
2. Installation instructions (cargo install)
3. Usage examples for different cities
4. CLI reference with all options
5. Distance guide (like maptoposter)
6. 3D printing tips (orientation, supports, materials)
7. Troubleshooting section
8. Contributing guide
9. License (MIT)
10. Credits (maptoposter, OSM)

STYLE:
- Clear, concise
- Code blocks for commands
- Tables for options
- Emoji sparingly (checkmarks, warnings only)

MUST DO:
- Include at least 5 example commands
- Document minimum printable feature sizes
- Mention OSM attribution requirement

MUST NOT DO:
- Include implementation details
- Reference PLAN.md or AGENTS.md
"""
)
```

---

## Delegation Best Practices

### Before Delegating
1. Ensure previous task is complete and tested
2. Verify Cargo.toml has required dependencies
3. Check that module structure exists

### Prompt Structure
Always include:
- **TASK**: What to build
- **FILES**: Exact paths to create/modify
- **REQUIREMENTS**: Detailed specifications
- **MUST DO**: Critical requirements
- **MUST NOT DO**: Boundaries
- **VERIFICATION**: How to test

### After Receiving Results
1. Run `cargo build` to verify compilation
2. Run `cargo clippy` for lint check
3. Test the feature manually
4. Check `lsp_diagnostics` for errors

### When to Escalate to Oracle
- Architecture decisions with tradeoffs
- Debugging failures after 2 attempts
- Performance optimization questions
- Security considerations

---

## Quick Reference: Module Dependencies

```
main.rs
  ├── api/nominatim.rs (geocoding)
  ├── api/overpass.rs (data fetching)
  ├── osm/parser.rs (parsing)
  ├── domain/*.rs (types)
  ├── geometry/projection.rs (coordinate transform)
  ├── geometry/scaling.rs (physical sizing)
  ├── layers/*.rs (mesh generation per layer)
  ├── mesh/ribbon.rs (road extrusion)
  ├── mesh/triangulation.rs (polygon triangulation)
  ├── mesh/extrusion.rs (polygon extrusion)
  ├── mesh/stl.rs (file output)
  └── config/*.rs (defaults, themes)
```

## Estimated Total Effort

| Phase | Tasks | Estimated Time |
|-------|-------|----------------|
| Phase 1 | 8 tasks | 13-16 hours |
| Phase 2 | 8 tasks | 13-16 hours |
| Phase 3 | 7 tasks | 10-12 hours |
| Phase 4 | 1 task | 2-3 hours |
| **Total** | 24 tasks | ~40-47 hours |

---

## Notes

- Each task is designed to be completable in 1-3 hours
- Tasks within a phase can sometimes be parallelized
- Always run `cargo build` and `cargo clippy` after each task
- Keep the maptoposter Python code as reference in `maptoposter/` subdirectory
