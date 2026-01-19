use crate::domain::{RoadClass, RoadSegment};
use crate::geometry::{Projector, Scaler, simplify_polyline};
use crate::mesh::{Triangle, extrude_ribbon};

#[derive(Debug, Clone)]
pub struct RoadConfig {
    pub motorway: (f32, f32),
    pub primary: (f32, f32),
    pub secondary: (f32, f32),
    pub tertiary: (f32, f32),
    pub residential: (f32, f32),
    pub road_scale: f32,
    pub map_scale_factor: f32,
    pub min_width_mm: f32,
    pub min_height_mm: f32,
    pub simplify_level: u8,
}

impl Default for RoadConfig {
    fn default() -> Self {
        Self {
            motorway: (3.0, 2.0),
            primary: (2.5, 1.5),
            secondary: (2.0, 1.0),
            tertiary: (1.5, 0.7),
            residential: (0.8, 0.5),
            road_scale: 1.0,
            map_scale_factor: 1.0,
            min_width_mm: 0.6,
            min_height_mm: 0.4,
            simplify_level: 0,
        }
    }
}

impl RoadConfig {
    pub fn get_dimensions(&self, class: RoadClass) -> (f32, f32) {
        let (base_w, base_h) = match class {
            RoadClass::Motorway => self.motorway,
            RoadClass::Primary => self.primary,
            RoadClass::Secondary => self.secondary,
            RoadClass::Tertiary => self.tertiary,
            RoadClass::Residential => self.residential,
        };

        let scaled_w = (base_w * self.map_scale_factor).max(self.min_width_mm);
        let scaled_h = (base_h * self.road_scale * self.map_scale_factor).max(self.min_height_mm);

        (scaled_w, scaled_h)
    }

    pub fn with_scale(mut self, scale: f32) -> Self {
        self.road_scale = scale;
        self
    }

    pub fn with_map_radius(mut self, radius_m: u32, physical_size_mm: f32) -> Self {
        let radius_km = radius_m as f32 / 1000.0;

        self.map_scale_factor = if radius_km < 5.0 {
            1.0
        } else if radius_km < 10.0 {
            1.0 + (radius_km - 5.0) * 0.1
        } else if radius_km < 20.0 {
            1.5 + (radius_km - 10.0) * 0.05
        } else {
            2.0
        };

        let mm_per_km = physical_size_mm / (radius_km * 2.0);
        if mm_per_km < 5.0 {
            self.map_scale_factor *= 1.5;
        }

        self
    }

    pub fn with_simplify_level(mut self, level: u8) -> Self {
        self.simplify_level = level.min(3);
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
    let mut all_triangles = Vec::new();

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

        let (width, height) = config.get_dimensions(road.class);

        let base_z = road.layer as f32 * 0.5;

        let triangles = extrude_ribbon(&scaled, width, height, base_z);
        all_triangles.extend(triangles);
    }

    all_triangles
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_road_config_dimensions() {
        let config = RoadConfig::default();
        let (w, h) = config.get_dimensions(RoadClass::Motorway);
        assert_eq!(w, 3.0);
        assert_eq!(h, 2.0);
    }

    #[test]
    fn test_road_config_scale() {
        let config = RoadConfig::default().with_scale(1.5);
        let (_, h) = config.get_dimensions(RoadClass::Motorway);
        assert_eq!(h, 3.0);
    }

    #[test]
    fn test_road_config_map_radius_small() {
        let config = RoadConfig::default().with_map_radius(3000, 220.0);
        assert!((config.map_scale_factor - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_road_config_map_radius_large() {
        let config = RoadConfig::default().with_map_radius(15000, 220.0);
        assert!(config.map_scale_factor > 1.5);
    }

    #[test]
    fn test_road_config_min_width() {
        let config = RoadConfig::default();
        let (w, h) = config.get_dimensions(RoadClass::Residential);
        assert!(w >= 0.6);
        assert!(h >= 0.4);
    }
}
