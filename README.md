# mapto3d

Generate 3D-printable STL city maps from OpenStreetMap data.

## Installation

```bash
git clone https://github.com/shantanugoel/mapto3d.git
cd mapto3d
cargo build --release
# Binary at target/release/mapto3d
```

Requires Rust 1.92.0+.

## Quick Start & Examples

Just a city and country is enough to get started — everything else is optional:

```bash
# Simplest usage - defaults work great
mapto3d -c "San Francisco" -C "USA"

# Smaller area with all road detail
mapto3d -c "Monaco" -C "Monaco" -r 2000 --road-depth all

# Use coordinates directly
mapto3d --lat 48.8566 --lon 2.3522 -r 5000 -o paris.stl

# Dense city with simplification for smaller file size
mapto3d -c "Manhattan" -C "USA" -r 4000 --road-depth all --simplify 2

# Include water and parks for multi-color printing
mapto3d -c "Venice" -C "Italy" -r 3000 --water --parks

# Large region with only highways
mapto3d -c "Los Angeles" -C "USA" -r 25000 --road-depth motorway

# Custom text labels
mapto3d -c "Paris" -C "France" --primary-text "PARIS" --secondary-text "CITY OF LIGHT"

# Scaled-up road height for visibility
mapto3d -c "Tokyo" -C "Japan" -r 8000 --road-scale 1.5
```

Output is a binary STL file ready for slicing and 3D printing.

## Usage

```
mapto3d [OPTIONS]

Location (one required):
  -c, --city <CITY>           City name (requires --country)
  -C, --country <COUNTRY>     Country name
      --lat <LAT>             Latitude (use with --lon)
      --lon <LON>             Longitude (use with --lat)

Output:
  -r, --radius <RADIUS>       Map radius in meters [default: 10000]
  -o, --output <OUTPUT>       Output STL file [default: {city}.stl]
  -s, --size <SIZE>           Physical size in mm [default: 220.0]

Features:
      --road-depth <DEPTH>    Road detail level [default: primary]
      --water                 Include water features (rivers, lakes)
      --parks                 Include park features (parks, forests)

Customization:
      --base-height <HEIGHT>  Base plate thickness in mm [default: 2.0]
      --road-scale <SCALE>    Road height multiplier [default: 1.0]
      --primary-text <TEXT>   Large text label [default: city name]
      --secondary-text <TEXT> Small text label [default: coordinates]
      --simplify <LEVEL>      0=off, 1=light, 2=medium, 3=aggressive [default: 0]
      --font <PATH>           Custom TTF font file

Other:
  -v, --verbose               Show detailed progress
      --config <PATH>         Path to config file (optional)
```

### Road Depth Levels

| Level | Included Roads |
|-------|----------------|
| motorway | Highways only |
| primary | + Trunk roads, primary roads |
| secondary | + Secondary roads |
| tertiary | + Tertiary roads |
| all | All mapped roads including residential |

### Config File (Optional)

Create `mapto3d.toml` in the current directory or `~/.config/mapto3d/config.toml` to save defaults:

```toml
city = "Tokyo"
country = "Japan"
radius = 15000
road_depth = "secondary"

[overpass]
urls = ["https://overpass-api.de/api/interpreter"]
timeout_secs = 300
```

CLI arguments override config values.

## Printing Tips

- Default 220mm size fits most printer beds
- 0.2mm layer height works well for detail
- Use `--water --parks` flags for multi-color prints
- PLA with matte finish gives nice results

---

## Development

```bash
cargo build              # Debug build
cargo build --release    # Release build
cargo test               # Run all tests
cargo clippy             # Lint
cargo fmt                # Format
```

See `AGENTS.md` for coding guidelines.

## License

MIT
