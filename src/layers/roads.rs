use geo::{CoordsIter, LineString};

use crate::domain::{RoadClass, RoadSegment};
use crate::geometry::simplify::simplify_polyline;
use crate::geometry::{BufferConfig, Projector, Scaler, buffer_polyline, union_polygons_batched};
use crate::mesh::{Triangle, extrude_polygon_ex};

#[derive(Debug, Clone)]
pub struct RoadConfig {
    pub motorway_width: f32,
    pub primary_width: f32,
    pub secondary_width: f32,
    pub tertiary_width: f32,
    pub residential_width: f32,
    pub width_scale: f32,
    pub min_width_mm: f32,
    pub simplify_level: u8,
    pub z_top: f32,
}

impl Default for RoadConfig {
    fn default() -> Self {
        Self {
            motorway_width: 1.5,
            primary_width: 1.5,
            secondary_width: 1.0,
            tertiary_width: 0.5,
            residential_width: 0.8,
            width_scale: 1.0,
            min_width_mm: 0.6,
            simplify_level: 0,
            z_top: 3.8,
        }
    }
}

impl RoadConfig {
    pub fn get_width(&self, class: RoadClass) -> f32 {
        let base_w = match class {
            RoadClass::Motorway => self.motorway_width,
            RoadClass::Primary => self.primary_width,
            RoadClass::Secondary => self.secondary_width,
            RoadClass::Tertiary => self.tertiary_width,
            RoadClass::Residential => self.residential_width,
        };

        (base_w * self.width_scale).max(self.min_width_mm)
    }

    pub fn with_scale(mut self, scale: f32) -> Self {
        self.width_scale = scale;
        self
    }

    pub fn with_map_radius(mut self, radius_m: u32, physical_size_mm: f32) -> Self {
        let radius_km = radius_m as f32 / 1000.0;

        let map_scale_factor = if radius_km < 5.0 {
            1.0
        } else if radius_km < 10.0 {
            1.0 + (radius_km - 5.0) * 0.1
        } else if radius_km < 20.0 {
            1.5 + (radius_km - 10.0) * 0.05
        } else {
            2.0
        };

        let mm_per_km = physical_size_mm / (radius_km * 2.0);
        let density_factor = if mm_per_km < 5.0 { 1.5 } else { 1.0 };

        self.width_scale *= map_scale_factor * density_factor;
        self
    }

    pub fn with_simplify_level(mut self, level: u8) -> Self {
        self.simplify_level = level.min(3);
        self
    }

    pub fn with_z_top(mut self, z_top: f32) -> Self {
        self.z_top = z_top;
        self
    }

    fn simplification_epsilon(&self, class: RoadClass) -> Option<f64> {
        if self.simplify_level == 0 {
            return None;
        }

        let base_epsilon = match class {
            RoadClass::Motorway => 0.00015,
            RoadClass::Primary => 0.00012,
            RoadClass::Secondary => 0.00010,
            RoadClass::Tertiary => 0.00008,
            RoadClass::Residential => 0.00005,
        };

        let multiplier = match self.simplify_level {
            1 => 1.0,
            2 => 2.0,
            3 => 4.0,
            _ => 1.0,
        };

        Some(base_epsilon * multiplier)
    }
}

const ROAD_UNION_BATCH_SIZE: usize = 500;
const POINT_EPSILON: f32 = 1e-4;

/// Generate mesh triangles for all road segments
///
/// # Arguments
/// * `roads` - Road segments with lat/lon coordinates
/// * `projector` - Coordinate projector (lat/lon → meters)
/// * `scaler` - Coordinate scaler (meters → mm)
/// * `config` - Road dimension configuration
///
/// # Returns
/// Vector of triangles for all roads
pub fn generate_road_meshes(
    roads: &[RoadSegment],
    projector: &Projector,
    scaler: &Scaler,
    config: &RoadConfig,
) -> Vec<Triangle> {
    let road_polygons = build_road_polygons(roads, projector, scaler, config);
    let mut all_triangles = Vec::new();

    for polygon in road_polygons.0 {
        let outer = line_string_to_ring(polygon.exterior(), false);
        if outer.len() < 3 {
            continue;
        }

        let holes: Vec<Vec<(f32, f32)>> = polygon
            .interiors()
            .iter()
            .map(|ring| line_string_to_ring(ring, true))
            .filter(|ring| ring.len() >= 3)
            .collect();

        let triangles = extrude_polygon_ex(&outer, &holes, 0.0, config.z_top, true);
        all_triangles.extend(triangles);
    }

    all_triangles
}

fn build_road_polygons(
    roads: &[RoadSegment],
    projector: &Projector,
    scaler: &Scaler,
    config: &RoadConfig,
) -> geo::MultiPolygon<f64> {
    let buffer_config = BufferConfig::for_roads();
    let mut road_polygons = Vec::new();

    for road in roads {
        let points_to_use = if let Some(epsilon) = config.simplification_epsilon(road.class) {
            let simplified = simplify_polyline(&road.points, epsilon);
            if simplified.len() < 2 {
                continue;
            }
            simplified
        } else {
            if road.points.len() < 2 {
                continue;
            }
            road.points.clone()
        };

        let projected: Vec<(f64, f64)> = points_to_use
            .iter()
            .map(|&(lat, lon)| projector.project(lat, lon))
            .collect();

        let scaled: Vec<(f32, f32)> = projected.iter().map(|&(x, y)| scaler.scale(x, y)).collect();
        let scaled_polyline = clean_polyline(&scaled);
        if scaled_polyline.len() < 2 {
            continue;
        }

        let width = config.get_width(road.class);
        let buffered = buffer_polyline(&scaled_polyline, width as f64, &buffer_config);
        road_polygons.extend(buffered.0);
    }

    if road_polygons.is_empty() {
        return geo::MultiPolygon::new(vec![]);
    }

    union_polygons_batched(road_polygons, ROAD_UNION_BATCH_SIZE)
}

fn clean_polyline(points: &[(f32, f32)]) -> Vec<(f64, f64)> {
    let mut cleaned: Vec<(f64, f64)> = Vec::with_capacity(points.len());

    for &(x, y) in points {
        let current = (x as f64, y as f64);
        let is_duplicate = cleaned.last().is_some_and(|&(px, py)| {
            (px - current.0).abs() < POINT_EPSILON as f64
                && (py - current.1).abs() < POINT_EPSILON as f64
        });

        if !is_duplicate {
            cleaned.push(current);
        }
    }

    cleaned
}

fn line_string_to_ring(ring: &LineString<f64>, expect_clockwise: bool) -> Vec<(f32, f32)> {
    let mut points: Vec<(f32, f32)> = ring
        .coords_iter()
        .map(|coord| (coord.x as f32, coord.y as f32))
        .collect();

    if points.len() < 3 {
        return Vec::new();
    }

    if points_are_close(*points.first().unwrap(), *points.last().unwrap()) {
        points.pop();
    }

    points = clean_ring(points);
    if points.len() < 3 {
        return Vec::new();
    }

    if is_clockwise(&points) != expect_clockwise {
        points.reverse();
    }

    points
}

fn clean_ring(points: Vec<(f32, f32)>) -> Vec<(f32, f32)> {
    let mut cleaned = Vec::with_capacity(points.len());
    for point in points {
        let is_duplicate = cleaned
            .last()
            .is_some_and(|&previous| points_are_close(previous, point));
        if !is_duplicate {
            cleaned.push(point);
        }
    }

    if cleaned.len() > 2 && points_are_close(*cleaned.first().unwrap(), *cleaned.last().unwrap()) {
        cleaned.pop();
    }

    cleaned
}

fn is_clockwise(points: &[(f32, f32)]) -> bool {
    signed_area(points) < 0.0
}

fn signed_area(points: &[(f32, f32)]) -> f32 {
    let mut area = 0.0;
    for i in 0..points.len() {
        let (x1, y1) = points[i];
        let (x2, y2) = points[(i + 1) % points.len()];
        area += x1 * y2 - x2 * y1;
    }
    area * 0.5
}

fn points_are_close(a: (f32, f32), b: (f32, f32)) -> bool {
    (a.0 - b.0).abs() < POINT_EPSILON && (a.1 - b.1).abs() < POINT_EPSILON
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use geo::{Contains, Point};

    use super::*;
    use crate::domain::RoadSegment;
    use crate::geometry::Bounds;

    #[test]
    fn test_road_config_width() {
        let config = RoadConfig::default();
        let w = config.get_width(RoadClass::Motorway);
        assert_eq!(w, 1.5);
    }

    #[test]
    fn test_road_config_scale() {
        let config = RoadConfig::default().with_scale(1.5);
        let w = config.get_width(RoadClass::Motorway);
        assert_eq!(w, 2.25);
    }

    #[test]
    fn test_road_config_map_radius_small() {
        let config = RoadConfig::default().with_map_radius(3000, 220.0);
        assert!((config.width_scale - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_road_config_map_radius_large() {
        let config = RoadConfig::default().with_map_radius(15000, 220.0);
        assert!(config.width_scale > 1.5);
    }

    #[test]
    fn test_road_config_min_width() {
        let config = RoadConfig::default();
        let w = config.get_width(RoadClass::Residential);
        assert!(w >= 0.6);
    }

    #[test]
    fn test_closed_loop_roads_keep_hole() {
        let roads = vec![RoadSegment::new(
            vec![
                (0.0, 0.0),
                (0.0, 0.003),
                (0.003, 0.003),
                (0.003, 0.0),
                (0.0, 0.0),
            ],
            RoadClass::Tertiary,
        )];

        let projector = Projector::new((0.0015, 0.0015));
        let projected_points: Vec<(f64, f64)> = roads
            .iter()
            .flat_map(|road| {
                road.points
                    .iter()
                    .map(|&(lat, lon)| projector.project(lat, lon))
            })
            .collect();
        let bounds = Bounds::from_points(&projected_points).unwrap();
        let scaler = Scaler::from_bounds_with_margin(&bounds, 220.0, 0.0);

        let polygons = build_road_polygons(&roads, &projector, &scaler, &RoadConfig::default());
        assert!(!polygons.0.is_empty());

        let center_projected = projector.project(0.0015, 0.0015);
        let center_scaled = scaler.scale(center_projected.0, center_projected.1);
        let center = Point::new(center_scaled.0 as f64, center_scaled.1 as f64);

        assert!(
            !polygons.contains(&center),
            "Closed loop road should keep interior void"
        );
    }

    #[test]
    fn test_intersection_roads_are_manifold() {
        let roads = vec![
            RoadSegment::new(vec![(0.0, -0.003), (0.0, 0.003)], RoadClass::Primary),
            RoadSegment::new(vec![(-0.003, 0.0), (0.003, 0.0)], RoadClass::Primary),
        ];

        let projector = Projector::new((0.0, 0.0));
        let projected_points: Vec<(f64, f64)> = roads
            .iter()
            .flat_map(|road| {
                road.points
                    .iter()
                    .map(|&(lat, lon)| projector.project(lat, lon))
            })
            .collect();
        let bounds = Bounds::from_points(&projected_points).unwrap();
        let scaler = Scaler::from_bounds_with_margin(&bounds, 220.0, 0.0);

        let triangles = generate_road_meshes(&roads, &projector, &scaler, &RoadConfig::default());
        let (boundary_edges, non_manifold_edges) = edge_counts(&triangles);

        assert!(!triangles.is_empty());
        assert_eq!(boundary_edges, 0);
        assert_eq!(non_manifold_edges, 0);
    }

    type QuantizedVertex = (i64, i64, i64);
    type QuantizedEdge = (QuantizedVertex, QuantizedVertex);

    fn edge_counts(triangles: &[Triangle]) -> (usize, usize) {
        let mut counts: HashMap<QuantizedEdge, usize> = HashMap::new();

        for triangle in triangles {
            let vertices = triangle.vertices.map(quantize);
            for (a, b) in [(0, 1), (1, 2), (2, 0)] {
                let edge = ordered_edge(vertices[a], vertices[b]);
                *counts.entry(edge).or_insert(0) += 1;
            }
        }

        let boundary_edges = counts.values().filter(|&&count| count == 1).count();
        let non_manifold_edges = counts.values().filter(|&&count| count > 2).count();
        (boundary_edges, non_manifold_edges)
    }

    fn quantize(vertex: [f32; 3]) -> QuantizedVertex {
        const SCALE: f32 = 10_000.0;
        (
            (vertex[0] * SCALE).round() as i64,
            (vertex[1] * SCALE).round() as i64,
            (vertex[2] * SCALE).round() as i64,
        )
    }

    fn ordered_edge(a: QuantizedVertex, b: QuantizedVertex) -> QuantizedEdge {
        if a <= b { (a, b) } else { (b, a) }
    }
}
