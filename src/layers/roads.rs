use crate::domain::{RoadClass, RoadSegment};
use crate::geometry::{Projector, Scaler};
use crate::mesh::{Triangle, extrude_ribbon};

/// Configuration for road mesh generation
#[derive(Debug, Clone)]
pub struct RoadConfig {
    /// (width_mm, height_mm) for each road class
    pub motorway: (f32, f32),
    pub primary: (f32, f32),
    pub secondary: (f32, f32),
    pub tertiary: (f32, f32),
    pub residential: (f32, f32),
    /// Road scale multiplier from CLI
    pub road_scale: f32,
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
        }
    }
}

impl RoadConfig {
    /// Get width and height for a road class
    pub fn get_dimensions(&self, class: RoadClass) -> (f32, f32) {
        let (w, h) = match class {
            RoadClass::Motorway => self.motorway,
            RoadClass::Primary => self.primary,
            RoadClass::Secondary => self.secondary,
            RoadClass::Tertiary => self.tertiary,
            RoadClass::Residential => self.residential,
        };
        (w, h * self.road_scale)
    }

    /// Create a config with a road scale multiplier
    pub fn with_scale(mut self, scale: f32) -> Self {
        self.road_scale = scale;
        self
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
        // Project lat/lon to meters
        let projected: Vec<(f64, f64)> = road
            .points
            .iter()
            .map(|&(lat, lon)| projector.project(lat, lon))
            .collect();

        // Scale to mm
        let scaled: Vec<(f32, f32)> = projected.iter().map(|&(x, y)| scaler.scale(x, y)).collect();

        // Get dimensions for this road class
        let (width, height) = config.get_dimensions(road.class);

        // Base Z level (layer 0 = ground level)
        // Bridges go higher, tunnels go lower
        let base_z = road.layer as f32 * 0.5;

        // Generate ribbon mesh
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
        assert_eq!(h, 3.0); // 2.0 * 1.5
    }
}
