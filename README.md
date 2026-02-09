# mapto3d

Generate 3D-printable STL city maps from OpenStreetMap data.

`mapto3d` builds a single STL using a solid-column layer model:
- Base plate
- Optional water layer
- Optional park layer
- Roads
- Text

Every feature extrudes from `z=0` to its own top height. This avoids floating geometry and keeps slicer behavior predictable for multi-color prints.

## Build

```bash
git clone https://github.com/shantanugoel/mapto3d.git
cd mapto3d
cargo build --release
```

Requires Rust `1.92.0+`.

## Quick Start

```bash
# city + country
cargo run -- -c "Monaco" -C "Monaco" -r 2000

# direct coordinates
cargo run -- --lat 48.8566 --lon 2.3522 -r 5000 -o paris.stl

# full-detail roads + optional layers
cargo run -- -c "Venice" -C "Italy" -r 3000 --road-depth all --water --parks
```

## CLI (Current)

```text
mapto3d [OPTIONS]

Location:
  -c, --city <CITY>
  -C, --country <COUNTRY>
      --lat <LAT>
      --lon <LON>

Geometry:
  -r, --radius <RADIUS>          Map radius in meters (default: 10000)
  -s, --size <SIZE>              Output square size in mm (default: 220.0)
      --base-height <HEIGHT>     Base thickness in mm (default: 2.0)
      --road-scale <SCALE>       Road width multiplier (default: 1.0)
      --road-depth <DEPTH>       motorway|primary|secondary|tertiary|all
      --simplify <LEVEL>         0..3 (default: 0)
      --edge-margin-mm <MM>      Edge margin for map features in mm (default: 0.0)

Layers:
      --water                    Enable water features
      --parks                    Enable park features

Text:
      --primary-text <TEXT>
      --secondary-text <TEXT>
      --font <PATH>              Custom TTF
      --no-text-fallback         Disable topology-based stroke fallback

I/O:
      --config <PATH>
      --no-cache
      --refresh
      --cache-dir <PATH>
      --cache-ttl-hours <HOURS>  Cache TTL in hours (default: 24)
  -o, --output <OUTPUT>
  -v, --verbose
```

### Road Depth Mapping

| Level | OSM highway tags included |
|---|---|
| `motorway` | `motorway`, `motorway_link` |
| `primary` | motorway + trunk + primary (+ link variants) |
| `secondary` | primary + `secondary`, `secondary_link` |
| `tertiary` | secondary + `tertiary`, `tertiary_link` |
| `all` | any `highway=*` |

## Config File

If `--config` is not provided, mapto3d auto-searches:

1. `mapto3d.toml`
2. `.mapto3d.toml`
3. `$XDG_CONFIG_HOME/mapto3d/config.toml`
4. `$XDG_CONFIG_HOME/mapto3d.toml`
5. `~/.mapto3d.toml`
6. `~/.config/mapto3d/config.toml`

If `--config <PATH>` is provided and the file does not exist, execution fails.

Example:

```toml
city = "Tokyo"
country = "Japan"
radius = 15000
size = 220.0
base_height = 2.0
road_scale = 1.2
road_depth = "secondary"
simplify = 1
edge_margin_mm = 0.0
verbose = true
cache_enabled = true
cache_ttl_hours = 24
cache_dir = ".mapto3d-cache"
primary_text = "TOKYO"
secondary_text = "35.6764N / 139.6500E"
output = "tokyo.stl"

[overpass]
urls = [
  "https://overpass.private.coffee/api/interpreter",
  "https://overpass-api.de/api/interpreter"
]
timeout_secs = 200
max_retries = 3
```

Supported config keys are exactly the fields in `src/config/mod.rs::FileConfig`. Notably, `water`, `parks`, `font`, and `no_text_fallback` are CLI-only right now.

### HTTP Cache Defaults

- Cache is enabled by default.
- Default TTL is `24` hours.
- Default cache dir is `$XDG_CACHE_HOME/mapto3d` when `XDG_CACHE_HOME` is set.
- Otherwise cache dir falls back to `.mapto3d-cache` in the current working directory.

### Config vs CLI precedence

Location fields (`city/country/lat/lon`) are straightforward CLI-over-config. Numeric options use sentinel-default merging in `src/main.rs`, so passing a CLI value equal to the built-in default may still keep the config value.

## Layer Heights

`FeatureHeights::new(base_height, water_enabled, parks_enabled)` increments by `0.6mm` per enabled feature stage.

- Base top: `base_height`
- Water top (if enabled): `base_height + 0.6`
- Parks top (if enabled): next `+0.6`
- Roads top: next `+0.6`
- Text top: next `+0.6`

So with both optional layers enabled and default base (`2.0mm`), tops are:
- Water `2.6mm`
- Parks `3.2mm`
- Roads `3.8mm`
- Text `4.4mm`

The program prints a layer-by-layer color-change guide after STL generation.

## Text Rendering Behavior

- Primary path: TTF-based glyph extrusion (`fontmesh`) using custom font or embedded/default Roboto Serif.
- Fallback path: stroke text (`extrude_ribbon_ex`) if TTF output has worse topology (boundary/non-manifold edges).
- Use `--no-text-fallback` to force TTF-only output.

## Mesh Quality Checks

`mapto3d` already runs `validate_and_fix()` before writing STL (normal fixes + invalid/degenerate removal).

For strict manifold checks and floating-component detection:

```bash
cargo run --bin mesh_check -- path/to/model.stl
```

`mesh_check` fails with a non-zero exit code on boundary edges, non-manifold edges, or floating components.

## Troubleshooting

- `No roads found in the specified area`:
  - increase `--radius`
  - try `--road-depth all`
- Overpass instability/timeouts:
  - tune `[overpass] timeout_secs/max_retries`
  - provide multiple mirrors in `[overpass].urls`
- Broken/missing text geometry:
  - remove `--no-text-fallback` to allow stroke fallback
  - test with another `--font`

## Development

```bash
cargo test
cargo clippy -- -D warnings
cargo fmt -- --check
```

Project-specific regression checks used heavily in this repo:

```bash
cargo test layers::roads::tests::test_intersection_roads_are_manifold
cargo test layers::text::tests::test_ttf_monaco_word_o_holes_not_filled
```

## License

MIT
