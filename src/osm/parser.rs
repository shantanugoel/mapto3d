use crate::api::OverpassResponse;
use crate::domain::{ParkPolygon, RoadClass, RoadSegment, WaterPolygon};
use std::collections::HashMap;

/// Parse Overpass response into domain road segments
///
/// # Algorithm
/// 1. Build node_id → (lat, lon) lookup map from all node elements
/// 2. For each way element with highway tag:
///    - Resolve node refs to coordinates
///    - Classify road type from highway tag
///    - Extract layer tag (default 0)
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

        // Get layer (for bridges/tunnels)
        let layer: i8 = tags.get("layer").and_then(|l| l.parse().ok()).unwrap_or(0);

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

        roads.push(RoadSegment::new(points, class, layer));
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
    let mut water_polygons = Vec::new();

    for element in &response.elements {
        if element.type_ != "way" {
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
    let mut park_polygons = Vec::new();

    for element in &response.elements {
        if element.type_ != "way" {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::overpass::Element;

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
                    tags: None,
                },
                Element {
                    type_: "node".to_string(),
                    id: 2,
                    lat: Some(37.78),
                    lon: Some(-122.43),
                    nodes: None,
                    tags: None,
                },
                Element {
                    type_: "way".to_string(),
                    id: 100,
                    lat: None,
                    lon: None,
                    nodes: Some(vec![1, 2]),
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
}
