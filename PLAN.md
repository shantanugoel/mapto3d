# PLAN.md

## Goal
Implement two high-impact upgrades:
1. HTTP caching for geocoding and Overpass data.
2. OSM relation/multipolygon parsing for water and parks.

The objective is faster repeat runs, lower API pressure, and more complete feature geometry.

## Task 1: HTTP Caching

### Scope
Add an on-disk cache for:
- Nominatim geocoding responses.
- Overpass query responses (roads/water/parks).

### Approach (Concise)
1. Add cache module:
- `src/api/cache.rs`
- Cache by deterministic key (`sha256` of normalized request payload).
- Store JSON envelope with:
  - `schema_version`
  - `created_at_unix`
  - `payload` (raw response JSON string)

2. Add runtime cache policy:
- Create a `CachePolicy` struct passed to API functions.
- Fields:
  - `enabled`
  - `refresh`
  - `ttl_secs`
  - `cache_dir`

3. Add CLI/config controls:
- CLI flags:
  - `--no-cache`
  - `--refresh`
  - `--cache-dir <PATH>`
  - `--cache-ttl-hours <HOURS>`
- Config additions in `FileConfig`:
  - `cache_enabled`
  - `cache_ttl_hours`
  - `cache_dir`

4. Integrate into API call flow:
- In `src/api/nominatim.rs` and `src/api/overpass.rs`:
  - read cache before network (unless `refresh`)
  - if fresh hit, return cached payload
  - otherwise perform network call and write cache
- On network failure, if stale cache exists, return stale cache with warning.

5. Defaults:
- Cache enabled by default.
- TTL default: 24h.
- Default cache dir:
  - `$XDG_CACHE_HOME/mapto3d` if available
  - fallback to `.mapto3d-cache` in cwd.

### Acceptance Criteria
- Re-running identical command within TTL avoids network calls.
- `--refresh` forces fetch and updates cache.
- `--no-cache` bypasses read/write cache.
- Failover to stale cache works when network is unavailable.
- Existing tests pass, plus new cache unit tests.

### Test Plan
- Unit tests for key generation and TTL checks.
- Unit tests for stale/fresh resolution logic.
- API tests with mocked payloads for cache-hit and cache-miss paths.

---

## Task 2: OSM Relation/Multipolygon Support (Water + Parks)

### Scope
Support relation-based polygons for:
- water features
- park features

Keep current closed-way parsing as fallback.

### Approach (Concise)
1. Extend Overpass response model:
- In `src/api/overpass.rs`, add relation member parsing to `Element`:
  - `members: Option<Vec<Member>>`
  - `Member { type_, ref, role }`

2. Ensure query coverage:
- Update water/park Overpass queries to include matching `relation[...]` selectors.
- Keep `>; out skel qt;` to fetch dependent ways/nodes.

3. Build relation geometry in parser:
- In `src/osm/parser.rs`:
  - build maps for nodes and ways
  - for each target relation:
    - collect `outer` and `inner` way fragments by member role
    - stitch fragments into closed rings
    - reject invalid/unclosed rings gracefully

4. Construct polygons with holes:
- Water: create `WaterPolygon { outer, holes }`.
- Parks: keep current `ParkPolygon` output model (outer only) for now:
  - use relation outers as separate park polygons
  - optionally defer park holes unless `ParkPolygon` is extended.

5. Robust fallback behavior:
- If relation assembly fails, continue with existing way-based parsing.
- Never hard-fail whole parse because one relation is bad.

### Acceptance Criteria
- Relation-based water bodies with islands produce holes correctly.
- Park relations are included instead of being silently dropped.
- Existing way-only behavior remains intact.
- Existing tests pass, plus relation-focused parser tests.

### Test Plan
- Add parser fixtures for:
  - multipolygon water relation with one hole
  - fragmented outer ring requiring stitching
  - malformed relation (should be skipped, not panic)
- Validate generated geometry count and ring closure.

---

## Delivery Sequence
1. Implement Task 1 (cache) first to reduce iteration time and API dependency.
2. Implement Task 2 (relations) next with fixture-driven parser tests.
3. Run full suite: `cargo test`.
4. Smoke check with real map generation + `mesh_check`.
