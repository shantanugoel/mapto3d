/// Simple Mercator-like projection from WGS84 to local meters
///
/// Uses approximation suitable for city-scale maps:
/// - x = (lon - center_lon) * cos(center_lat) * 111320
/// - y = (lat - center_lat) * 111320
///
/// This avoids the complexity of proj crate while being accurate
/// enough for maps up to ~50km across.
#[derive(Debug, Clone)]
pub struct Projector {
    center_lat: f64,
    center_lon: f64,
    cos_lat: f64,
}

impl Projector {
    /// Create a new projector centered at the given coordinates
    ///
    /// # Arguments
    /// * `center` - (lat, lon) center point in WGS84
    pub fn new(center: (f64, f64)) -> Self {
        let (lat, lon) = center;
        Self {
            center_lat: lat,
            center_lon: lon,
            cos_lat: lat.to_radians().cos(),
        }
    }

    /// Project a lat/lon point to local meters
    ///
    /// # Returns
    /// * (x, y) in meters, centered at the projection center
    pub fn project(&self, lat: f64, lon: f64) -> (f64, f64) {
        // Meters per degree at equator
        const METERS_PER_DEGREE: f64 = 111320.0;

        let x = (lon - self.center_lon) * self.cos_lat * METERS_PER_DEGREE;
        let y = (lat - self.center_lat) * METERS_PER_DEGREE;

        (x, y)
    }

    /// Project a slice of lat/lon points
    pub fn project_points(&self, points: &[(f64, f64)]) -> Vec<(f64, f64)> {
        points
            .iter()
            .map(|&(lat, lon)| self.project(lat, lon))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_projector_center() {
        let proj = Projector::new((37.7749, -122.4194));
        let (x, y) = proj.project(37.7749, -122.4194);
        assert!((x).abs() < 0.01);
        assert!((y).abs() < 0.01);
    }

    #[test]
    fn test_projector_1km() {
        let proj = Projector::new((37.7749, -122.4194));

        // 1 degree latitude ≈ 111.32 km
        // So 0.009 degrees ≈ 1 km
        let (_, y) = proj.project(37.7749 + 0.009, -122.4194);
        assert!((y - 1000.0).abs() < 50.0); // Within 50m tolerance
    }
}
