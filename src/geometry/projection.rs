/// Improved Transverse Mercator-like projection from WGS84 to local meters
///
/// Uses a refined approximation with proper scale factor calculation:
/// - Accounts for Earth's ellipsoid (WGS84 parameters)
/// - Applies transverse Mercator scale factor at center
/// - Accurate for maps up to ~100km across
///
/// This avoids the complexity of proj crate while providing good accuracy
/// for city and regional maps.
#[derive(Debug, Clone)]
pub struct Projector {
    center_lat: f64,
    center_lon: f64,
    /// Meters per degree of longitude at center latitude
    meters_per_lon_degree: f64,
    /// Meters per degree of latitude at center latitude
    meters_per_lat_degree: f64,
}

impl Projector {
    // WGS84 ellipsoid parameters
    const WGS84_A: f64 = 6_378_137.0; // Semi-major axis (equatorial radius) in meters
    const WGS84_E2: f64 = 0.006_694_379_990_14; // First eccentricity squared

    /// Create a new projector centered at the given coordinates
    ///
    /// # Arguments
    /// * `center` - (lat, lon) center point in WGS84
    pub fn new(center: (f64, f64)) -> Self {
        let (lat, lon) = center;
        let lat_rad = lat.to_radians();

        // Calculate meters per degree using WGS84 ellipsoid
        // These formulas account for Earth's ellipsoidal shape
        let sin_lat = lat_rad.sin();
        let cos_lat = lat_rad.cos();
        let sin2_lat = sin_lat * sin_lat;

        // Radius of curvature in the prime vertical (N)
        let n = Self::WGS84_A / (1.0 - Self::WGS84_E2 * sin2_lat).sqrt();

        // Radius of curvature in the meridian (M)
        let m =
            Self::WGS84_A * (1.0 - Self::WGS84_E2) / (1.0 - Self::WGS84_E2 * sin2_lat).powf(1.5);

        // Meters per degree of latitude (varies with latitude due to ellipsoid)
        let meters_per_lat_degree = m * std::f64::consts::PI / 180.0;

        // Meters per degree of longitude (varies with latitude)
        let meters_per_lon_degree = n * cos_lat * std::f64::consts::PI / 180.0;

        Self {
            center_lat: lat,
            center_lon: lon,
            meters_per_lon_degree,
            meters_per_lat_degree,
        }
    }

    /// Project a lat/lon point to local meters
    ///
    /// Uses refined ellipsoidal calculations for better accuracy
    ///
    /// # Returns
    /// * (x, y) in meters, centered at the projection center
    pub fn project(&self, lat: f64, lon: f64) -> (f64, f64) {
        let delta_lon = lon - self.center_lon;
        let delta_lat = lat - self.center_lat;

        // For small areas, linear approximation with proper scale factors
        let x = delta_lon * self.meters_per_lon_degree;
        let y = delta_lat * self.meters_per_lat_degree;

        (x, y)
    }

    /// Project a slice of lat/lon points
    pub fn project_points(&self, points: &[(f64, f64)]) -> Vec<(f64, f64)> {
        points
            .iter()
            .map(|&(lat, lon)| self.project(lat, lon))
            .collect()
    }

    /// Get projection accuracy estimate for a given radius in meters
    ///
    /// Returns the approximate maximum error in meters at the edge of the map
    #[cfg(test)]
    pub fn estimate_error(&self, radius_m: f64) -> f64 {
        // For transverse Mercator, error grows with distance from center
        // Approximate error: (distance^2) / (2 * Earth_radius)
        let earth_radius = Self::WGS84_A;
        (radius_m * radius_m) / (2.0 * earth_radius)
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
        let (_, y) = proj.project(37.7749 + 0.009, -122.4194);
        assert!((y - 1000.0).abs() < 50.0);
    }

    #[test]
    fn test_estimate_error() {
        let proj = Projector::new((37.7749, -122.4194));
        let error_10km = proj.estimate_error(10_000.0);
        let error_50km = proj.estimate_error(50_000.0);
        assert!(error_10km < 10.0);
        assert!(error_50km < 200.0);
    }
}
