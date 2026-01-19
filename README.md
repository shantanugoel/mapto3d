# mapto3d

Generate 3D-printable STL city maps from OpenStreetMap data.

## Installation

```bash
# Clone and build
git clone https://github.com/shantanugoel/mapto3d.git
cd mapto3d
cargo build --release

# Binary will be at target/release/mapto3d
```

Requires Rust 1.92.0 or later.

## Quick Start

```bash
# Generate a map of San Francisco (10km radius)
mapto3d -c "San Francisco" -C "USA"

# Smaller area with more road detail
mapto3d -c "Monaco" -C "Monaco" -r 2000 --road-depth all

# Use coordinates directly
mapto3d --lat 48.8566 --lon 2.3522 -r 5000 -o paris.stl
```

Output is a binary STL file ready for slicing and 3D printing.

## Usage

```
mapto3d [OPTIONS]

Options:
  -c, --city <CITY>           City name
  -C, --country <COUNTRY>     Country name (required with --city)
      --lat <LAT>             Latitude (use with --lon instead of city/country)
      --lon <LON>             Longitude (use with --lat)
  -r, --radius <RADIUS>       Map radius in meters [default: 10000]
  -o, --output <OUTPUT>       Output STL file [default: {city}.stl]
  -s, --size <SIZE>           Physical size in mm [default: 220.0]
      --base-height <HEIGHT>  Base plate thickness in mm [default: 2.0]
      --road-scale <SCALE>    Road height multiplier [default: 1.0]
      --road-depth <DEPTH>    Road detail: motorway, primary, secondary, tertiary, all [default: primary]
      --primary-text <TEXT>   Large text label (defaults to city name)
      --secondary-text <TEXT> Small text label (defaults to coordinates)
      --simplify <LEVEL>      Simplification: 0=off, 1=light, 2=medium, 3=aggressive [default: 0]
  -v, --verbose               Show detailed progress
      --config <PATH>         Path to config file
```

### Road Depth Levels

| Level | Included Roads |
|-------|----------------|
| motorway | Highways only |
| primary | + Trunk roads, primary roads |
| secondary | + Secondary roads |
| tertiary | + Tertiary roads |
| all | All mapped roads including residential |

### Config File

Create `mapto3d.toml` in the current directory or `~/.config/mapto3d/config.toml`:

```toml
city = "Tokyo"
country = "Japan"
radius = 15000
size = 200.0
road_depth = "secondary"
road_scale = 1.2

[overpass]
urls = [
    "https://overpass-api.de/api/interpreter",
    "https://overpass.private.coffee/api/interpreter"
]
timeout_secs = 300
```

CLI arguments override config file values.

## Examples

```bash
# Dense urban area - include all roads, use simplification
mapto3d -c "Manhattan" -C "USA" -r 4000 --road-depth all --simplify 2

# Large region - fewer roads, scaled up height
mapto3d -c "Los Angeles" -C "USA" -r 25000 --road-depth motorway --road-scale 2.0

# Custom labels
mapto3d -c "Paris" -C "France" --primary-text "PARIS" --secondary-text "CITY OF LIGHT"
```

## Printing Tips

- Default size (220mm) fits most printer beds
- Use 0.2mm layer height for good detail
- Roads print best with 2-3 walls
- PLA works well; consider matte filament for better appearance

---

## Development

### Build and Test

```bash
cargo build              # Debug build
cargo build --release    # Release build
cargo test               # Run all tests
cargo test test_name     # Run specific test
cargo clippy             # Lint
cargo fmt                # Format
```

### Project Structure

```
src/
  main.rs         # CLI entry point
  api/            # Nominatim geocoding, Overpass OSM data
  config/         # TOML config parsing
  domain/         # Road, Water, Park types
  geometry/       # Coordinate projection and scaling
  layers/         # Mesh generation (base, roads, water, parks, text)
  mesh/           # Triangle primitives, STL output, validation
  osm/            # OSM response parsing
```

### For AI Agents

See `AGENTS.md` for coding guidelines, conventions, and detailed command reference.

## License

MIT
