use crate::api::OverpassResponse;
use crate::api::overpass::Member;
use crate::domain::{ParkPolygon, RoadClass, RoadSegment, WaterPolygon};
use geo::{Contains, LineString, Point, Polygon};
use std::collections::{HashMap, HashSet};

/// Parse Overpass response into domain road segments
///
/// # Algorithm
/// 1. Build node_id → (lat, lon) lookup map from all node elements
/// 2. For each way element with highway tag:
///    - Resolve node refs to coordinates
///    - Classify road type from highway tag
pub fn parse_roads(response: &OverpassResponse) -> Vec<RoadSegment> {
    // Step 1: Build node lookup map
    let nodes: HashMap<u64, (f64, f64)> = response
        .elements
        .iter()
        .filter(|e| e.type_ == "node")
        .filter_map(|e| {
            let lat = e.lat?;
            let lon = e.lon?;
            Some((e.id, (lat, lon)))
        })
        .collect();

    // Step 2: Process ways into road segments
    let mut roads = Vec::new();

    for element in &response.elements {
        if element.type_ != "way" {
            continue;
        }

        // Get highway tag
        let tags = match &element.tags {
            Some(t) => t,
            None => continue,
        };

        let highway = match tags.get("highway") {
            Some(h) => h,
            None => continue,
        };

        // Classify road type
        let class = match RoadClass::from_highway_tag(highway) {
            Some(c) => c,
            None => continue, // Skip unknown road types
        };

        // Resolve node refs to coordinates
        let node_refs = match &element.nodes {
            Some(n) => n,
            None => continue,
        };

        let points: Vec<(f64, f64)> = node_refs
            .iter()
            .filter_map(|id| nodes.get(id).copied())
            .collect();

        // Skip segments with less than 2 points
        if points.len() < 2 {
            continue;
        }

        roads.push(RoadSegment::new(points, class));
    }

    roads
}

fn build_node_lookup(response: &OverpassResponse) -> HashMap<u64, (f64, f64)> {
    response
        .elements
        .iter()
        .filter(|e| e.type_ == "node")
        .filter_map(|e| {
            let lat = e.lat?;
            let lon = e.lon?;
            Some((e.id, (lat, lon)))
        })
        .collect()
}

fn build_way_lookup(response: &OverpassResponse) -> HashMap<u64, Vec<u64>> {
    response
        .elements
        .iter()
        .filter(|e| e.type_ == "way")
        .filter_map(|e| e.nodes.as_ref().map(|nodes| (e.id, nodes.clone())))
        .collect()
}

fn resolve_way_to_points(node_refs: &[u64], nodes: &HashMap<u64, (f64, f64)>) -> Vec<(f64, f64)> {
    node_refs
        .iter()
        .filter_map(|id| nodes.get(id).copied())
        .collect()
}

fn is_closed_way(points: &[(f64, f64)]) -> bool {
    if points.len() < 3 {
        return false;
    }
    let first = points.first().unwrap();
    let last = points.last().unwrap();
    (first.0 - last.0).abs() < 1e-9 && (first.1 - last.1).abs() < 1e-9
}

pub fn parse_water(response: &OverpassResponse) -> Vec<WaterPolygon> {
    let nodes = build_node_lookup(response);
    let ways = build_way_lookup(response);
    let mut water_polygons = Vec::new();
    let mut used_way_ids = HashSet::new();

    for element in &response.elements {
        if element.type_ != "relation" {
            continue;
        }

        let members = match &element.members {
            Some(m) => m,
            None => continue,
        };

        let relation_rings = build_relation_rings(members, &ways, &nodes);
        if relation_rings.outers.is_empty() {
            continue;
        }

        for way_id in &relation_rings.used_way_ids {
            used_way_ids.insert(*way_id);
        }

        water_polygons.extend(build_water_polygons_from_relation(&relation_rings));
    }

    for element in &response.elements {
        if element.type_ != "way" {
            continue;
        }

        if used_way_ids.contains(&element.id) {
            continue;
        }

        let node_refs = match &element.nodes {
            Some(n) => n,
            None => continue,
        };

        let points = resolve_way_to_points(node_refs, &nodes);

        if !is_closed_way(&points) {
            continue;
        }

        if points.len() < 4 {
            continue;
        }

        water_polygons.push(WaterPolygon::new(points));
    }

    water_polygons
}

pub fn parse_parks(response: &OverpassResponse) -> Vec<ParkPolygon> {
    let nodes = build_node_lookup(response);
    let ways = build_way_lookup(response);
    let mut park_polygons = Vec::new();
    let mut used_way_ids = HashSet::new();

    for element in &response.elements {
        if element.type_ != "relation" {
            continue;
        }

        let members = match &element.members {
            Some(m) => m,
            None => continue,
        };

        let relation_rings = build_relation_rings(members, &ways, &nodes);
        if relation_rings.outers.is_empty() {
            continue;
        }

        for way_id in &relation_rings.used_way_ids {
            used_way_ids.insert(*way_id);
        }

        for outer in relation_rings.outers {
            park_polygons.push(ParkPolygon::new(outer));
        }
    }

    for element in &response.elements {
        if element.type_ != "way" {
            continue;
        }

        if used_way_ids.contains(&element.id) {
            continue;
        }

        let node_refs = match &element.nodes {
            Some(n) => n,
            None => continue,
        };

        let points = resolve_way_to_points(node_refs, &nodes);

        if !is_closed_way(&points) {
            continue;
        }

        if points.len() < 4 {
            continue;
        }

        park_polygons.push(ParkPolygon::new(points));
    }

    park_polygons
}

struct RelationRings {
    outers: Vec<Vec<(f64, f64)>>,
    inners: Vec<Vec<(f64, f64)>>,
    used_way_ids: Vec<u64>,
}

struct StitchedRing {
    node_ids: Vec<u64>,
}

fn build_relation_rings(
    members: &[Member],
    ways: &HashMap<u64, Vec<u64>>,
    nodes: &HashMap<u64, (f64, f64)>,
) -> RelationRings {
    let outer_fragments = collect_way_fragments(members, ways, true);
    let inner_fragments = collect_way_fragments(members, ways, false);

    let (outer_rings, outer_used) = stitch_fragments(outer_fragments);
    let (inner_rings, inner_used) = stitch_fragments(inner_fragments);

    let mut outers = Vec::new();
    let mut inners = Vec::new();

    for ring in outer_rings {
        if let Some(points) = resolve_relation_ring(&ring.node_ids, nodes) {
            outers.push(orient_ring(points, false));
        }
    }

    for ring in inner_rings {
        if let Some(points) = resolve_relation_ring(&ring.node_ids, nodes) {
            inners.push(orient_ring(points, true));
        }
    }

    let mut used_way_ids = outer_used;
    used_way_ids.extend(inner_used);

    RelationRings {
        outers,
        inners,
        used_way_ids,
    }
}

fn collect_way_fragments(
    members: &[Member],
    ways: &HashMap<u64, Vec<u64>>,
    outer: bool,
) -> Vec<(u64, Vec<u64>)> {
    members
        .iter()
        .filter(|member| member.type_ == "way")
        .filter(|member| {
            if outer {
                member.role == "outer" || member.role.is_empty()
            } else {
                member.role == "inner"
            }
        })
        .filter_map(|member| {
            ways.get(&member.ref_id)
                .map(|nodes| (member.ref_id, nodes.clone()))
        })
        .filter(|(_, nodes)| nodes.len() >= 2)
        .collect()
}

fn stitch_fragments(mut fragments: Vec<(u64, Vec<u64>)>) -> (Vec<StitchedRing>, Vec<u64>) {
    let mut rings = Vec::new();
    let mut used_way_ids = Vec::new();

    while let Some((way_id, mut ring_nodes)) = fragments.pop() {
        if ring_nodes.len() < 2 {
            continue;
        }

        let mut ring_way_ids = vec![way_id];

        loop {
            if ring_nodes.first() == ring_nodes.last() {
                break;
            }

            let head = *ring_nodes.first().unwrap();
            let tail = *ring_nodes.last().unwrap();

            let mut match_idx = None;
            let mut attach_at_end = true;
            let mut reverse = false;

            for (idx, (_, fragment)) in fragments.iter().enumerate() {
                if fragment.is_empty() {
                    continue;
                }
                let frag_head = fragment[0];
                let frag_tail = *fragment.last().unwrap();

                if frag_head == tail {
                    match_idx = Some(idx);
                    attach_at_end = true;
                    reverse = false;
                    break;
                }
                if frag_tail == tail {
                    match_idx = Some(idx);
                    attach_at_end = true;
                    reverse = true;
                    break;
                }
                if frag_tail == head {
                    match_idx = Some(idx);
                    attach_at_end = false;
                    reverse = false;
                    break;
                }
                if frag_head == head {
                    match_idx = Some(idx);
                    attach_at_end = false;
                    reverse = true;
                    break;
                }
            }

            let Some(idx) = match_idx else {
                break;
            };

            let (frag_id, fragment) = fragments.swap_remove(idx);
            ring_way_ids.push(frag_id);

            if attach_at_end {
                append_fragment(&mut ring_nodes, fragment, reverse);
            } else {
                prepend_fragment(&mut ring_nodes, fragment, reverse);
            }
        }

        if ring_nodes.len() >= 4 && ring_nodes.first() == ring_nodes.last() {
            used_way_ids.extend(&ring_way_ids);
            rings.push(StitchedRing {
                node_ids: ring_nodes,
            });
        }
    }

    (rings, used_way_ids)
}

fn append_fragment(ring: &mut Vec<u64>, fragment: Vec<u64>, reverse: bool) {
    if fragment.len() < 2 {
        return;
    }

    if reverse {
        let mut iter = fragment.into_iter().rev();
        iter.next();
        ring.extend(iter);
    } else {
        ring.extend(fragment.into_iter().skip(1));
    }
}

fn prepend_fragment(ring: &mut Vec<u64>, fragment: Vec<u64>, reverse: bool) {
    if fragment.len() < 2 {
        return;
    }

    let mut prefix: Vec<u64> = if reverse {
        fragment.into_iter().rev().skip(1).collect()
    } else {
        let mut nodes = fragment;
        nodes.pop();
        nodes
    };

    prefix.extend(ring.iter().copied());
    *ring = prefix;
}

fn resolve_relation_ring(
    node_ids: &[u64],
    nodes: &HashMap<u64, (f64, f64)>,
) -> Option<Vec<(f64, f64)>> {
    let mut points = Vec::with_capacity(node_ids.len());
    for node_id in node_ids {
        let point = nodes.get(node_id)?;
        points.push(*point);
    }

    if !is_closed_way(&points) {
        return None;
    }

    if points.len() < 4 {
        return None;
    }

    Some(points)
}

fn build_water_polygons_from_relation(relation_rings: &RelationRings) -> Vec<WaterPolygon> {
    if relation_rings.outers.is_empty() {
        return Vec::new();
    }

    let mut polygons: Vec<WaterPolygon> = relation_rings
        .outers
        .iter()
        .map(|outer| WaterPolygon {
            outer: outer.clone(),
            holes: Vec::new(),
        })
        .collect();

    if polygons.len() == 1 {
        polygons[0].holes = relation_rings.inners.clone();
        return polygons;
    }

    let outer_polys: Vec<Polygon<f64>> = relation_rings
        .outers
        .iter()
        .map(|outer| polygon_from_ring(outer))
        .collect();

    for hole in &relation_rings.inners {
        let centroid = match ring_centroid(hole) {
            Some(point) => point,
            None => continue,
        };
        let test_point = Point::new(centroid.1, centroid.0);

        if let Some((index, _)) = outer_polys
            .iter()
            .enumerate()
            .find(|(_, poly)| poly.contains(&test_point))
        {
            if let Some(target) = polygons.get_mut(index) {
                target.holes.push(hole.clone());
            }
        }
    }

    polygons
}

fn polygon_from_ring(ring: &[(f64, f64)]) -> Polygon<f64> {
    let coords: Vec<(f64, f64)> = ring.iter().map(|(lat, lon)| (*lon, *lat)).collect();
    Polygon::new(LineString::from(coords), vec![])
}

fn ring_centroid(ring: &[(f64, f64)]) -> Option<(f64, f64)> {
    if ring.is_empty() {
        return None;
    }

    let mut lat_sum = 0.0;
    let mut lon_sum = 0.0;
    let mut count = 0.0;

    for (lat, lon) in ring.iter() {
        lat_sum += lat;
        lon_sum += lon;
        count += 1.0;
    }

    if count == 0.0 {
        return None;
    }

    Some((lat_sum / count, lon_sum / count))
}

fn orient_ring(mut points: Vec<(f64, f64)>, clockwise: bool) -> Vec<(f64, f64)> {
    if is_clockwise(&points) != clockwise {
        points.reverse();
    }
    points
}

fn is_clockwise(points: &[(f64, f64)]) -> bool {
    ring_signed_area(points) < 0.0
}

fn ring_signed_area(points: &[(f64, f64)]) -> f64 {
    if points.len() < 3 {
        return 0.0;
    }

    let end = if points.first() == points.last() {
        points.len() - 1
    } else {
        points.len()
    };

    let mut area = 0.0;
    for i in 0..end {
        let (lat1, lon1) = points[i];
        let (lat2, lon2) = points[(i + 1) % end];
        area += lon1 * lat2 - lon2 * lat1;
    }

    area * 0.5
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::overpass::{Element, Member};

    #[test]
    fn test_parse_roads() {
        let response = OverpassResponse {
            elements: vec![
                Element {
                    type_: "node".to_string(),
                    id: 1,
                    lat: Some(37.77),
                    lon: Some(-122.42),
                    nodes: None,
                    members: None,
                    tags: None,
                },
                Element {
                    type_: "node".to_string(),
                    id: 2,
                    lat: Some(37.78),
                    lon: Some(-122.43),
                    nodes: None,
                    members: None,
                    tags: None,
                },
                Element {
                    type_: "way".to_string(),
                    id: 100,
                    lat: None,
                    lon: None,
                    nodes: Some(vec![1, 2]),
                    members: None,
                    tags: Some({
                        let mut m = HashMap::new();
                        m.insert("highway".to_string(), "primary".to_string());
                        m
                    }),
                },
            ],
        };

        let roads = parse_roads(&response);
        assert_eq!(roads.len(), 1);
        assert_eq!(roads[0].class, RoadClass::Primary);
        assert_eq!(roads[0].points.len(), 2);
    }

    #[test]
    fn test_parse_water_relation_with_hole() {
        let response = OverpassResponse {
            elements: vec![
                Element {
                    type_: "node".to_string(),
                    id: 1,
                    lat: Some(0.0),
                    lon: Some(0.0),
                    nodes: None,
                    members: None,
                    tags: None,
                },
                Element {
                    type_: "node".to_string(),
                    id: 2,
                    lat: Some(0.0),
                    lon: Some(10.0),
                    nodes: None,
                    members: None,
                    tags: None,
                },
                Element {
                    type_: "node".to_string(),
                    id: 3,
                    lat: Some(10.0),
                    lon: Some(10.0),
                    nodes: None,
                    members: None,
                    tags: None,
                },
                Element {
                    type_: "node".to_string(),
                    id: 4,
                    lat: Some(10.0),
                    lon: Some(0.0),
                    nodes: None,
                    members: None,
                    tags: None,
                },
                Element {
                    type_: "node".to_string(),
                    id: 5,
                    lat: Some(2.0),
                    lon: Some(2.0),
                    nodes: None,
                    members: None,
                    tags: None,
                },
                Element {
                    type_: "node".to_string(),
                    id: 6,
                    lat: Some(2.0),
                    lon: Some(4.0),
                    nodes: None,
                    members: None,
                    tags: None,
                },
                Element {
                    type_: "node".to_string(),
                    id: 7,
                    lat: Some(4.0),
                    lon: Some(4.0),
                    nodes: None,
                    members: None,
                    tags: None,
                },
                Element {
                    type_: "node".to_string(),
                    id: 8,
                    lat: Some(4.0),
                    lon: Some(2.0),
                    nodes: None,
                    members: None,
                    tags: None,
                },
                Element {
                    type_: "way".to_string(),
                    id: 100,
                    lat: None,
                    lon: None,
                    nodes: Some(vec![1, 2, 3, 4, 1]),
                    members: None,
                    tags: None,
                },
                Element {
                    type_: "way".to_string(),
                    id: 200,
                    lat: None,
                    lon: None,
                    nodes: Some(vec![5, 6, 7, 8, 5]),
                    members: None,
                    tags: None,
                },
                Element {
                    type_: "relation".to_string(),
                    id: 300,
                    lat: None,
                    lon: None,
                    nodes: None,
                    members: Some(vec![
                        Member {
                            type_: "way".to_string(),
                            ref_id: 100,
                            role: "outer".to_string(),
                        },
                        Member {
                            type_: "way".to_string(),
                            ref_id: 200,
                            role: "inner".to_string(),
                        },
                    ]),
                    tags: None,
                },
            ],
        };

        let water = parse_water(&response);
        assert_eq!(water.len(), 1);
        assert_eq!(water[0].holes.len(), 1);
        assert_eq!(water[0].outer.len(), 5);
    }

    #[test]
    fn test_parse_water_relation_stitches_fragments() {
        let response = OverpassResponse {
            elements: vec![
                Element {
                    type_: "node".to_string(),
                    id: 1,
                    lat: Some(0.0),
                    lon: Some(0.0),
                    nodes: None,
                    members: None,
                    tags: None,
                },
                Element {
                    type_: "node".to_string(),
                    id: 2,
                    lat: Some(0.0),
                    lon: Some(10.0),
                    nodes: None,
                    members: None,
                    tags: None,
                },
                Element {
                    type_: "node".to_string(),
                    id: 3,
                    lat: Some(10.0),
                    lon: Some(10.0),
                    nodes: None,
                    members: None,
                    tags: None,
                },
                Element {
                    type_: "node".to_string(),
                    id: 4,
                    lat: Some(10.0),
                    lon: Some(0.0),
                    nodes: None,
                    members: None,
                    tags: None,
                },
                Element {
                    type_: "way".to_string(),
                    id: 101,
                    lat: None,
                    lon: None,
                    nodes: Some(vec![1, 2, 3]),
                    members: None,
                    tags: None,
                },
                Element {
                    type_: "way".to_string(),
                    id: 102,
                    lat: None,
                    lon: None,
                    nodes: Some(vec![3, 4, 1]),
                    members: None,
                    tags: None,
                },
                Element {
                    type_: "relation".to_string(),
                    id: 400,
                    lat: None,
                    lon: None,
                    nodes: None,
                    members: Some(vec![
                        Member {
                            type_: "way".to_string(),
                            ref_id: 101,
                            role: "outer".to_string(),
                        },
                        Member {
                            type_: "way".to_string(),
                            ref_id: 102,
                            role: "outer".to_string(),
                        },
                    ]),
                    tags: None,
                },
            ],
        };

        let water = parse_water(&response);
        assert_eq!(water.len(), 1);
        assert_eq!(water[0].outer.len(), 5);
    }

    #[test]
    fn test_parse_water_relation_malformed_skips() {
        let response = OverpassResponse {
            elements: vec![
                Element {
                    type_: "node".to_string(),
                    id: 1,
                    lat: Some(0.0),
                    lon: Some(0.0),
                    nodes: None,
                    members: None,
                    tags: None,
                },
                Element {
                    type_: "node".to_string(),
                    id: 2,
                    lat: Some(0.0),
                    lon: Some(1.0),
                    nodes: None,
                    members: None,
                    tags: None,
                },
                Element {
                    type_: "node".to_string(),
                    id: 3,
                    lat: Some(1.0),
                    lon: Some(1.0),
                    nodes: None,
                    members: None,
                    tags: None,
                },
                Element {
                    type_: "node".to_string(),
                    id: 4,
                    lat: Some(1.0),
                    lon: Some(0.0),
                    nodes: None,
                    members: None,
                    tags: None,
                },
                Element {
                    type_: "way".to_string(),
                    id: 500,
                    lat: None,
                    lon: None,
                    nodes: Some(vec![1, 2, 3, 4, 1]),
                    members: None,
                    tags: None,
                },
                Element {
                    type_: "relation".to_string(),
                    id: 600,
                    lat: None,
                    lon: None,
                    nodes: None,
                    members: Some(vec![Member {
                        type_: "way".to_string(),
                        ref_id: 999,
                        role: "outer".to_string(),
                    }]),
                    tags: None,
                },
            ],
        };

        let water = parse_water(&response);
        assert_eq!(water.len(), 1);
        assert_eq!(water[0].outer.len(), 5);
    }

    #[test]
    fn test_parse_parks_relation() {
        let response = OverpassResponse {
            elements: vec![
                Element {
                    type_: "node".to_string(),
                    id: 1,
                    lat: Some(0.0),
                    lon: Some(0.0),
                    nodes: None,
                    members: None,
                    tags: None,
                },
                Element {
                    type_: "node".to_string(),
                    id: 2,
                    lat: Some(0.0),
                    lon: Some(5.0),
                    nodes: None,
                    members: None,
                    tags: None,
                },
                Element {
                    type_: "node".to_string(),
                    id: 3,
                    lat: Some(5.0),
                    lon: Some(5.0),
                    nodes: None,
                    members: None,
                    tags: None,
                },
                Element {
                    type_: "node".to_string(),
                    id: 4,
                    lat: Some(5.0),
                    lon: Some(0.0),
                    nodes: None,
                    members: None,
                    tags: None,
                },
                Element {
                    type_: "way".to_string(),
                    id: 700,
                    lat: None,
                    lon: None,
                    nodes: Some(vec![1, 2, 3, 4, 1]),
                    members: None,
                    tags: None,
                },
                Element {
                    type_: "relation".to_string(),
                    id: 701,
                    lat: None,
                    lon: None,
                    nodes: None,
                    members: Some(vec![Member {
                        type_: "way".to_string(),
                        ref_id: 700,
                        role: "outer".to_string(),
                    }]),
                    tags: None,
                },
            ],
        };

        let parks = parse_parks(&response);
        assert_eq!(parks.len(), 1);
        assert_eq!(parks[0].outer.len(), 5);
    }
}
