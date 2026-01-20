use crate::domain::{RoadClass, RoadSegment};
use crate::geometry::{simplify_polyline, Projector, Scaler};
use crate::mesh::{extrude_ribbon_ex, Triangle};

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

        let width = config.get_width(road.class);

        let triangles = extrude_ribbon_ex(&scaled, width, config.z_top, 0.0, true, true);
        all_triangles.extend(triangles);
    }

    all_triangles
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
