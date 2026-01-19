# mapto3d - City Map STL Generator

> Transform any city into a 3D-printable STL file, inspired by [maptoposter](https://github.com/originalankur/maptoposter)

## Project Overview

**Goal**: A Rust CLI that generates 3D-printable STL city maps fitting within 22cm x 22cm (220mm x 220mm).

**Input**: City name, country, radius in meters
**Output**: Binary STL file ready for 3D printing

---

## Reference Analysis (maptoposter)

The Python maptoposter project provides the baseline for map generation:

### Data Sources
- **Geocoding**: Nominatim API (city name → lat/lon)
- **Map Data**: OSMnx (wraps Overpass API for OpenStreetMap)
- **Features Fetched**:
  - Street network: `network_type='all'`
  - Water: `{'natural': 'water', 'waterway': 'riverbank'}`
  - Parks: `{'leisure': 'park', 'landuse': 'grass'}`

### Road Hierarchy
| OSM Tag | Width | Color (Feature-Based) |
|---------|-------|----------------------|
| motorway, motorway_link | 1.2 | #0A0A0A |
| trunk, primary | 1.0 | #1A1A1A |
| secondary | 0.8 | #2A2A2A |
| tertiary | 0.6 | #3A3A3A |
| residential, living_street | 0.4 | #4A4A4A |

### Rendering Layers (z-order)
1. Background
2. Water polygons
3. Parks polygons
4. Roads (via graph edges)
5. Gradient fades
6. Text labels

---

## 3D Translation Strategy

### Layer Mapping (2D → 3D)

| 2D Layer | 3D Representation | Height (mm) |
|----------|-------------------|-------------|
| Base | Solid rectangular plate | 2.0 |
| Water | Recessed area (cut into base) | -1.0 (below surface) |
| Parks | Slight raised area | +0.4 |
| Roads (motorway) | Extruded ribbon | +2.0, width 3.0mm |
| Roads (primary) | Extruded ribbon | +1.6, width 2.5mm |
| Roads (secondary) | Extruded ribbon | +1.0, width 2.0mm |
| Roads (tertiary) | Extruded ribbon | +0.6, width 1.5mm |
| Roads (residential) | Extruded ribbon | +0.6, width 0.8mm |

### Physical Constraints (3D Printability)
- **Minimum feature width**: 0.6mm (FDM printers)
- **Minimum feature height**: 0.4mm
- **Maximum relief height**: 5mm (to avoid excessive print time)
- **Base thickness**: 2mm minimum for structural integrity
- **Edge bevel**: Optional 0.5mm chamfer to prevent warping

---

## Architecture

### Data Flow
```
┌─────────────────────────────────────────────────────────────────┐
│                         CLI Input                                │
│  --city "San Francisco" --country "USA" --radius 10000          │
└─────────────────────────────────────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│                      Geocoding Module                            │
│  Nominatim API: "San Francisco, USA" → (37.7749, -122.4194)    │
└─────────────────────────────────────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│                    OSM Data Fetching                             │
│  Overpass API: Bounding box query for roads, water, parks       │
└─────────────────────────────────────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│                     Domain Parsing                               │
│  OSM JSON → RoadSegment[], WaterPolygon[], ParkPolygon[]        │
└─────────────────────────────────────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│                  Coordinate Projection                           │
│  WGS84 (lat/lon) → UTM meters → Scale to 220mm × 220mm         │
└─────────────────────────────────────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│                   Geometry Generation                            │
│  Roads: Polyline → Ribbon extrusion                             │
│  Water: Polygon → Recessed triangulated mesh                    │
│  Parks: Polygon → Raised triangulated mesh                      │
│  Base: Rectangle → Box mesh                                     │
└─────────────────────────────────────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Mesh Assembly                                 │
│  Combine all triangles, validate normals, check manifold        │
└─────────────────────────────────────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│                     STL Output                                   │
│  Binary STL file: {city}.stl                                    │
└─────────────────────────────────────────────────────────────────┘
```

### Module Structure
```
src/
├── main.rs                 # CLI entry point (clap)
├── lib.rs                  # Library re-exports
│
├── api/                    # External API clients
│   ├── mod.rs
│   ├── nominatim.rs        # Geocoding: city → coordinates
│   └── overpass.rs         # OSM data fetching
│
├── domain/                 # Domain types (clean, normalized)
│   ├── mod.rs
│   ├── road.rs             # RoadSegment { polyline, class, layer }
│   ├── water.rs            # WaterPolygon { polygon, holes }
│   ├── park.rs             # ParkPolygon { polygon }
│   └── map_data.rs         # MapData { roads, water, parks, bounds }
│
├── osm/                    # OSM-specific parsing
│   ├── mod.rs
│   ├── parser.rs           # JSON → domain types
│   ├── tags.rs             # Highway classification, feature detection
│   └── types.rs            # Raw OSM types (Node, Way, Relation)
│
├── geometry/               # Coordinate & geometry processing
│   ├── mod.rs
│   ├── projection.rs       # WGS84 → UTM projection
│   ├── scaling.rs          # Normalize to physical dimensions
│   ├── simplify.rs         # Douglas-Peucker simplification
│   └── bounds.rs           # Bounding box calculations
│
├── mesh/                   # 3D mesh generation
│   ├── mod.rs
│   ├── triangulation.rs    # Polygon → triangles (earclip)
│   ├── extrusion.rs        # 2D → 3D extrusion
│   ├── ribbon.rs           # Polyline → ribbon mesh (roads)
│   ├── builder.rs          # Triangle mesh accumulator
│   └── stl.rs              # STL file writer (stl_io)
│
├── layers/                 # Layer-specific mesh generation
│   ├── mod.rs
│   ├── base.rs             # Base plate generation
│   ├── roads.rs            # Road layer → ribbon meshes
│   ├── water.rs            # Water layer → recessed meshes
│   └── parks.rs            # Parks layer → raised meshes
│
└── config/                 # Configuration
    ├── mod.rs
    ├── defaults.rs         # Default heights, widths, sizes
    └── theme.rs            # Optional: height/width themes
```

---

## Dependencies (Cargo.toml)

```toml
[package]
name = "mapto3d"
version = "0.1.0"
edition = "2021"
description = "Generate 3D-printable STL city maps"
license = "MIT"

[dependencies]
# CLI
clap = { version = "4", features = ["derive"] }

# HTTP & Serialization
reqwest = { version = "0.11", features = ["blocking", "json"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Geometry
geo = "0.28"
proj = "0.27"              # Coordinate projection

# Mesh & STL
stl_io = "0.7"             # STL file I/O
earclip = "0.1"            # Polygon triangulation

# Error Handling
anyhow = "1"
thiserror = "1"

# Progress & Logging
indicatif = "0.17"         # Progress bars
tracing = "0.1"
tracing-subscriber = "0.3"
```

---

## CLI Interface

```
mapto3d - Generate 3D-printable city map STL files

USAGE:
    mapto3d [OPTIONS] --city <CITY> --country <COUNTRY>

OPTIONS:
    -c, --city <CITY>           City name (required)
    -C, --country <COUNTRY>     Country name (required)
    -r, --radius <METERS>       Map radius in meters [default: 10000]
    -o, --output <FILE>         Output STL file [default: {city}.stl]
    -s, --size <MM>             Physical size in mm [default: 220]
    -b, --base-height <MM>      Base plate thickness [default: 2.0]
    -R, --road-scale <FACTOR>   Road height multiplier [default: 1.0]
    -v, --verbose               Enable verbose logging
    -h, --help                  Print help
    -V, --version               Print version

EXAMPLES:
    # Generate San Francisco map with default settings
    mapto3d -c "San Francisco" -C "USA"
    
    # Generate Tokyo with larger radius
    mapto3d -c "Tokyo" -C "Japan" -r 15000 -o tokyo.stl
    
    # Generate Venice (small, detailed)
    mapto3d -c "Venice" -C "Italy" -r 4000 --road-scale 1.5
```

---

## Implementation Phases

### Phase 1: Foundation (MVP)
**Goal**: End-to-end pipeline producing basic STL

| Task | Description | Effort |
|------|-------------|--------|
| 1.1 | CLI setup with clap | 1h |
| 1.2 | Nominatim geocoding client | 1h |
| 1.3 | Overpass API client (roads only) | 2h |
| 1.4 | OSM JSON parser → RoadSegment | 2h |
| 1.5 | Basic coordinate projection (WGS84 → meters) | 1h |
| 1.6 | Simple road ribbon extrusion (rectangular) | 3h |
| 1.7 | STL file output | 1h |
| 1.8 | Integration & testing | 2h |

**Deliverable**: CLI that generates roads-only STL

---

### Phase 2: Complete Layers
**Goal**: Add water, parks, base plate

| Task | Description | Effort |
|------|-------------|--------|
| 2.1 | Overpass query for water features | 1h |
| 2.2 | Overpass query for parks | 1h |
| 2.3 | Polygon parsing (closed ways, multipolygons) | 3h |
| 2.4 | Polygon triangulation with earclip | 2h |
| 2.5 | Water layer (recessed mesh) | 2h |
| 2.6 | Parks layer (raised mesh) | 1h |
| 2.7 | Base plate generation | 1h |
| 2.8 | Layer assembly & z-ordering | 2h |
| 2.9 | By default only motorways and primary roads are queried/printed. Make it configurably for people to go deeper via CLI
| 2.10 | City/Country name can be overridden with a primary and secondary text on the stl. 
| 2.11 | People can directly enter a lat/long if they want. If they dont give any primary/secondary text in this case, then those are not in the stl.

**Deliverable**: Complete city model with all layers

---

### Phase 3: Quality & Polish
**Goal**: Production-ready output

| Task | Description | Effort |
|------|-------------|--------|
| 3.1 | Proper UTM zone selection | 1h |
| 3.2 | Road width scaling based on map size | 1h |
| 3.3 | Geometry simplification (Douglas-Peucker) | 2h |
| 3.4 | Mesh validation (manifold check, and also ensuring there are no floating layers or parts and there are no holes) | 2h |
| 3.5 | Normal orientation verification | 1h |
| 3.6 | Progress bar integration | 1h |
| 3.7 | Error handling improvements | 2h |
| 3.8 | Configuration file support (themes) | 2h |
| 3.9 | Alterate overpass api url configuration support, also enable option to choose only the alternate or configure mirrors to try in case one fails after retries etc. By default we can add "https://overpass.private.coffee/api/interpreter" as the primary, and also add another mirror which we try to use if default fails
| 3.10 | Recheck sane defaults for all options etc so people dont have to configure too much to just use

**Deliverable**: Robust, configurable tool

---

### Phase 4: Advanced Features (Optional)
**Goal**: Extended functionality

| Task | Description | Effort |
|------|-------------|--------|
| 4.1 | Bridge/tunnel support (layer tags) | 3h |
| 4.2 | Building footprints | 4h |
| 4.3 | Railway lines | 2h |
| 4.4 | Custom height profiles | 2h |
| 4.5 | Multi-city tiling | 4h |
| 4.6 | STL optimization (merge vertices) | 2h |
| 4.7 | Caching

---

## Key Algorithms

### Road Ribbon Extrusion

```
Input: Polyline [(x1,y1), (x2,y2), ...], width W, height H

For each segment (P1, P2):
1. Compute direction vector D = normalize(P2 - P1)
2. Compute perpendicular N = (-D.y, D.x)
3. Left points: P1 - N*W/2, P2 - N*W/2
4. Right points: P1 + N*W/2, P2 + N*W/2

For joints between segments:
1. Use miter join (extend edges until intersection)
2. Apply miter limit (fallback to bevel if angle too sharp)

Create ribbon polygon from left/right point sequences
Triangulate ribbon polygon
Extrude to height H (add top face + side faces)
```

### Polygon Triangulation

```
Input: Polygon with optional holes

1. Ensure outer ring is clockwise
2. Ensure hole rings are counter-clockwise
3. Apply earclip algorithm
4. Output: List of triangle indices

For extrusion:
- Create bottom face (reversed winding)
- Create top face (normal winding, offset by height)
- Create side faces (quads split into triangles)
```

### Coordinate Projection

```
Input: (lat, lon) in WGS84

1. Determine UTM zone from longitude: zone = floor((lon + 180) / 6) + 1
2. Project using proj crate with EPSG code
3. Result: (easting, northing) in meters

Scaling to physical size:
1. Compute bounding box in meters
2. Scale factor = target_size_mm / max(width, height)
3. Apply scale and center in target area
```

---

## Overpass API Queries

### Roads Query
```
[out:json][timeout:60];
(
  way["highway"]({{bbox}});
);
out body;
>;
out skel qt;
```

### Water Query
```
[out:json][timeout:60];
(
  way["natural"="water"]({{bbox}});
  way["waterway"="riverbank"]({{bbox}});
  relation["natural"="water"]({{bbox}});
);
out body;
>;
out skel qt;
```

### Parks Query
```
[out:json][timeout:60];
(
  way["leisure"="park"]({{bbox}});
  way["landuse"="grass"]({{bbox}});
  relation["leisure"="park"]({{bbox}});
);
out body;
>;
out skel qt;
```

---

## Testing Strategy

### Unit Tests
- Coordinate projection accuracy
- Road classification logic
- Polygon winding detection
- Triangle normal calculation

### Integration Tests
- Nominatim API response parsing
- Overpass API response parsing
- End-to-end small city generation

### Validation Tests
- STL file validity (load in slicer)
- Manifold mesh verification
- Physical dimension accuracy

---

## Success Criteria

1. **Functional**: Generates valid STL for any city worldwide
2. **Accurate**: Physical dimensions match specified size (220mm default)
3. **Printable**: Output passes 3D slicer validation
4. **Performant**: Large cities (15km radius) complete in < 60 seconds
5. **Robust**: Graceful handling of missing/malformed OSM data

---

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| OSM data quality varies | Validate/clean polygons, skip invalid features |
| API rate limits | Add caching, respect usage policies, user-agent |
| Complex multipolygons | Handle relations properly, test with Venice/Amsterdam |
| Memory with large cities | Stream processing, limit triangle count |
| Non-manifold meshes | Validation pass, overlap strategy for v1 |

---

## References

- [maptoposter](https://github.com/originalankur/maptoposter) - Python reference implementation
- [Overpass API](https://wiki.openstreetmap.org/wiki/Overpass_API) - OSM query language
- [Nominatim](https://nominatim.org/) - Geocoding service
- [stl_io](https://docs.rs/stl_io) - Rust STL library
- [earclip](https://docs.rs/earclip) - Polygon triangulation
- [geo](https://docs.rs/geo) - Geometry primitives
- [proj](https://docs.rs/proj) - Coordinate projections
