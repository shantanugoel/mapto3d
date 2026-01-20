use geo::Polygon;

use crate::domain::{RoadClass, RoadSegment};
use crate::geometry::{
    buffer_polyline, calculate_epsilon_meters, calculate_min_segment_length, simplify_for_mesh,
    union_polygons_batched, BufferConfig, Projector, Scaler,
};
use crate::mesh::{extrude_polygon_ex, Triangle};

#[derive(Debug, Clone)]
pub struct RoadConfig {
    pub width_scale: f32,
    pub simplify_level: u8,
    pub z_top: f32,
    pub radius_m: u32,
}

impl Default for RoadConfig {
    fn default() -> Self {
        Self {
            width_scale: 1.0,
            simplify_level: 0,
            z_top: 3.8,
            radius_m: 10000,
        }
    }
}

impl RoadConfig {
    pub fn with_scale(mut self, scale: f32) -> Self {
        self.width_scale = scale;
        self
    }

    pub fn with_map_radius(mut self, radius_m: u32, physical_size_mm: f32) -> Self {
        self.radius_m = radius_m;
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

    fn get_width_meters(&self, class: RoadClass) -> f64 {
        let base_w = match class {
            RoadClass::Motorway => 12.0,
            RoadClass::Primary => 10.0,
            RoadClass::Secondary => 8.0,
            RoadClass::Tertiary => 6.0,
            RoadClass::Residential => 5.0,
        };
        base_w * self.width_scale as f64
    }
}

pub fn generate_road_meshes(
    roads: &[RoadSegment],
    projector: &Projector,
    scaler: &Scaler,
    config: &RoadConfig,
) -> Vec<Triangle> {
    if roads.is_empty() {
        return Vec::new();
    }

    let epsilon = calculate_epsilon_meters(config.radius_m, config.simplify_level);
    let min_seg_len = calculate_min_segment_length(config.radius_m);
    let buffer_config = BufferConfig::for_roads();

    let mut all_road_polygons: Vec<Polygon<f64>> = Vec::new();

    for road in roads {
        if road.points.len() < 2 {
            continue;
        }

        let projected: Vec<(f64, f64)> = road
            .points
            .iter()
            .map(|&(lat, lon)| projector.project(lat, lon))
            .collect();

        let simplified = if epsilon > 0.0 {
            simplify_for_mesh(&projected, epsilon, min_seg_len)
        } else if min_seg_len > 0.0 {
            crate::geometry::filter_short_segments(&projected, min_seg_len)
        } else {
            projected
        };

        if simplified.len() < 2 {
            continue;
        }

        let width_meters = config.get_width_meters(road.class);
        let buffered = buffer_polyline(&simplified, width_meters, &buffer_config);

        all_road_polygons.extend(buffered.0);
    }

    if all_road_polygons.is_empty() {
        return Vec::new();
    }

    let unified = union_polygons_batched(all_road_polygons, 100);

    let mut all_triangles = Vec::new();

    for polygon in unified.0 {
        let outer_scaled: Vec<(f32, f32)> = polygon
            .exterior()
            .coords()
            .map(|c| scaler.scale(c.x, c.y))
            .collect();

        let holes_scaled: Vec<Vec<(f32, f32)>> = polygon
            .interiors()
            .iter()
            .map(|ring| ring.coords().map(|c| scaler.scale(c.x, c.y)).collect())
            .collect();

        let triangles = extrude_polygon_ex(&outer_scaled, &holes_scaled, 0.0, config.z_top, true);
        all_triangles.extend(triangles);
    }

    all_triangles
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_road_config_default() {
        let config = RoadConfig::default();
        assert_eq!(config.width_scale, 1.0);
        assert_eq!(config.z_top, 3.8);
    }

    #[test]
    fn test_road_config_scale() {
        let config = RoadConfig::default().with_scale(1.5);
        assert_eq!(config.width_scale, 1.5);
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
    fn test_get_width_meters() {
        let config = RoadConfig::default();
        assert_eq!(config.get_width_meters(RoadClass::Motorway), 12.0);
        assert_eq!(config.get_width_meters(RoadClass::Residential), 5.0);
    }

    #[test]
    fn test_get_width_meters_scaled() {
        let config = RoadConfig::default().with_scale(2.0);
        assert_eq!(config.get_width_meters(RoadClass::Motorway), 24.0);
    }

    #[test]
    fn test_generate_road_meshes_empty() {
        let roads: Vec<RoadSegment> = vec![];
        let projector = Projector::new((0.0, 0.0));
        let scaler = Scaler::new(1.0, (0.0, 0.0));
        let config = RoadConfig::default();

        let result = generate_road_meshes(&roads, &projector, &scaler, &config);
        assert!(result.is_empty());
    }
}
