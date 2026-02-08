# AGENTS.md - mapto3d Agent Guide

Project goal: generate a single, printable, manifold STL city map from OSM data.

This guide is intentionally project-specific. It documents the invariants and workflows that prevent regressions in this codebase.

## What "done" means in this repo

A change is complete when all of the following hold:

- STL output remains manifold (no boundary edges, no non-manifold edges).
- No floating components are introduced.
- Solid-column layer model is preserved (features extrude from `z=0`).
- CLI and config behavior remain consistent with `src/main.rs`.
- Relevant topology regression tests still pass.

## Ground Truth Files

- `src/main.rs`: orchestration, CLI parsing, config merge, text fallback decision, final validation/write.
- `src/config/mod.rs`: feature height model, config schema/search order, Overpass defaults.
- `src/api/overpass.rs`: road/water/park queries, retry/backoff, mirror failover.
- `src/api/nominatim.rs`: city geocoding and required rate-limit delay.
- `src/osm/parser.rs`: conversion from raw Overpass elements to domain objects.
- `src/layers/roads.rs`: simplification, buffering, polygon union, road extrusion.
- `src/layers/text.rs`: TTF and stroke renderers, contour hierarchy, hole handling.
- `src/mesh/extrusion.rs`: polygon extrusion and side wall winding rules.
- `src/mesh/validation.rs`: mesh cleanup before STL write.
- `src/bin/mesh_check.rs`: strict post-generation manifold/floating check.

## End-to-End Pipeline

1. Resolve center via `--lat/--lon` or Nominatim geocoding.
2. Fetch roads from Overpass (mandatory), optionally water and parks.
3. Parse OSM nodes/ways into domain structs.
4. Project WGS84 to local meters (`Projector`), then scale to mm (`Scaler`).
5. Build layer meshes:
   - base plate
   - optional water
   - optional parks
   - roads (buffer + union + extrusion)
   - text (TTF, with optional stroke fallback)
6. Run `validate_and_fix`.
7. Write binary STL.
8. Print layer/color change guidance.

## Critical Geometry Invariants

- Coordinate tuples are `(lat, lon)` until projection.
- Projected coordinates are meters (`f64`), scaled coordinates are mm (`f32`).
- All feature geometry is currently extruded from `z=0` to a feature-specific top height.
- Outer rings must be CCW and holes CW before extrusion.
- Road intersections rely on polygon union before extrusion; skipping union tends to create non-manifold overlaps.
- Closed-loop roads should keep interior voids (do not accidentally fill them).
- TTF glyph holes (for `O`, `0`, etc.) must remain open on top faces.

## Layer Height Model (Dynamic)

`FeatureHeights::new(base_height, water_enabled, parks_enabled)` controls absolute top heights.

- Step increment is `0.6mm`.
- Water and parks consume increments only if enabled.
- Roads and text always consume increments.

With default base `2.0mm` and water+parks enabled:
- water top `2.6`
- parks top `3.2`
- roads top `3.8`
- text top `4.4`

If you change this model, also verify `print_color_change_guide()` in `src/main.rs` stays correct.

## Config/CLI Behavior You Must Preserve

- Auto config search order is implemented in `get_config_paths()`.
- `--config <path>` is strict: missing file is an error.
- Merge logic uses sentinel defaults for many numeric args in `src/main.rs`.
- Passing a value equal to the built-in default may not override config.
- Config schema currently does not include `water`, `parks`, `font`, or `no_text_fallback`.

## API and Parsing Assumptions

- Overpass requests are POST form data (`data=<query>`), not raw body.
- Overpass retries only on `429`/`504`, with wait `30 * attempt` seconds.
- Overpass query timeout in query text is `180`; HTTP client timeout is config-driven (`timeout_secs`, default `200`).
- Nominatim geocoding sleeps 1 second before request (rate-limit compliance).
- Water/park parsing currently handles closed `way` geometries; relations/multipolygons are not parsed.

## Text Rendering Contracts

- Renderer selection:
  - custom font path if valid
  - default/embedded Roboto Serif
  - stroke fallback as last resort
- `generate_text_layer()` computes edge topology counts.
- If fallback is allowed and stroke topology is better/equal, stroke output replaces TTF.
- `--no-text-fallback` disables that replacement.

## High-Value Regression Tests

Use targeted tests matching your change surface:

- Roads/manifold intersections:
  - `cargo test layers::roads::tests::test_intersection_roads_are_manifold`
  - `cargo test layers::roads::tests::test_closed_loop_roads_keep_hole`
- Text topology/holes:
  - `cargo test layers::text::tests::test_ttf_text_topology_monaco`
  - `cargo test layers::text::tests::test_ttf_monaco_word_o_holes_not_filled`
- Extrusion and triangulation:
  - `cargo test mesh::extrusion::tests::`
  - `cargo test mesh::triangulation::tests::`
- Full local baseline:
  - `cargo test`

Note: tests are compiled/run for both `lib` and `main` targets, so totals appear duplicated.

## Mesh QA Workflow

Recommended smoke run after geometry changes:

```bash
cargo run -- -c "Monaco" -C "Monaco" -r 2000 --water --parks -o /tmp/monaco.stl
cargo run --bin mesh_check -- /tmp/monaco.stl
```

`mesh_check` is the quickest way to catch boundary edges, non-manifold edges, and floating components.

## Change Map (Edit X -> Also Check Y)

- Road class/filter changes:
  - Update `src/domain/road.rs`, `src/api/overpass.rs`, and width logic in `src/layers/roads.rs`.
- Feature height or layer ordering changes:
  - Update `src/config/mod.rs` and `print_color_change_guide()` in `src/main.rs`.
- Text layout/renderer changes:
  - Update `src/layers/text.rs` and rerun text topology tests.
- OSM query changes:
  - Keep parser expectations in `src/osm/parser.rs` aligned.
- CLI option changes:
  - Keep docs in `README.md` and clap docs in `src/main.rs` synchronized.

## Performance Hotspots

- Road polygon union is expensive on large maps.
- `ROAD_UNION_BATCH_SIZE` in `src/layers/roads.rs` is a key tuning knob.
- `simplify` levels materially reduce triangle count; validate detail loss before changing epsilon constants.
- TTF contour subdivision (`CURVE_SUBDIVISIONS`) affects text quality and triangle count.
