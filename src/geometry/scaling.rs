/// Bounding box in projected coordinates (meters)
#[derive(Debug, Clone)]
pub struct Bounds {
    pub min_x: f64,
    pub max_x: f64,
    pub min_y: f64,
    pub max_y: f64,
}

impl Bounds {
    /// Create bounds from a set of points
    pub fn from_points(points: &[(f64, f64)]) -> Option<Self> {
        if points.is_empty() {
            return None;
        }

        let mut min_x = f64::MAX;
        let mut max_x = f64::MIN;
        let mut min_y = f64::MAX;
        let mut max_y = f64::MIN;

        for &(x, y) in points {
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_y = min_y.min(y);
            max_y = max_y.max(y);
        }

        Some(Self {
            min_x,
            max_x,
            min_y,
            max_y,
        })
    }

    /// Expand bounds to include another set of points
    #[allow(dead_code)]
    pub fn expand(&mut self, points: &[(f64, f64)]) {
        for &(x, y) in points {
            self.min_x = self.min_x.min(x);
            self.max_x = self.max_x.max(x);
            self.min_y = self.min_y.min(y);
            self.max_y = self.max_y.max(y);
        }
    }

    pub fn width(&self) -> f64 {
        self.max_x - self.min_x
    }

    pub fn height(&self) -> f64 {
        self.max_y - self.min_y
    }
}

/// Scales projected coordinates (meters) to physical dimensions (mm)
#[derive(Debug, Clone)]
pub struct Scaler {
    /// Scale factor: mm per meter
    scale: f64,
    /// Offset to center the map
    offset_x: f64,
    offset_y: f64,
    /// Target size in mm
    #[allow(dead_code)]
    target_mm: f64,
}

impl Scaler {
    /// Create a scaler from bounds and target physical size
    ///
    /// # Arguments
    /// * `bounds` - Bounding box in meters
    /// * `target_mm` - Target size in mm (will fit the larger dimension)
    #[allow(dead_code)]
    pub fn from_bounds(bounds: &Bounds, target_mm: f64) -> Self {
        Self::from_bounds_with_margin(bounds, target_mm, 0.0)
    }

    /// Create a scaler with a bottom margin reserved for text labels
    pub fn from_bounds_with_margin(bounds: &Bounds, target_mm: f64, bottom_margin_mm: f64) -> Self {
        let width = bounds.width();
        let height = bounds.height();

        let usable_height = target_mm - bottom_margin_mm;
        let max_dim = width.max(height);

        let scale = if max_dim > 0.0 {
            usable_height / max_dim
        } else {
            1.0
        };

        let scaled_width = width * scale;
        let scaled_height = height * scale;

        let offset_x = (target_mm - scaled_width) / 2.0 - bounds.min_x * scale;
        let offset_y =
            bottom_margin_mm + (usable_height - scaled_height) / 2.0 - bounds.min_y * scale;

        Self {
            scale,
            offset_x,
            offset_y,
            target_mm,
        }
    }

    /// Scale a point from meters to mm
    ///
    /// # Returns
    /// * (x, y) in mm as f32 for STL output
    pub fn scale(&self, x: f64, y: f64) -> (f32, f32) {
        let scaled_x = x * self.scale + self.offset_x;
        let scaled_y = y * self.scale + self.offset_y;
        (scaled_x as f32, scaled_y as f32)
    }

    /// Scale a slice of points
    #[allow(dead_code)]
    pub fn scale_points(&self, points: &[(f64, f64)]) -> Vec<(f32, f32)> {
        points.iter().map(|&(x, y)| self.scale(x, y)).collect()
    }

    /// Get the scale factor (mm per meter)
    #[allow(dead_code)]
    pub fn scale_factor(&self) -> f64 {
        self.scale
    }

    /// Get the target size in mm
    #[allow(dead_code)]
    pub fn target_size(&self) -> f64 {
        self.target_mm
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bounds_from_points() {
        let points = vec![(0.0, 0.0), (1000.0, 2000.0), (500.0, 1000.0)];
        let bounds = Bounds::from_points(&points).unwrap();

        assert_eq!(bounds.min_x, 0.0);
        assert_eq!(bounds.max_x, 1000.0);
        assert_eq!(bounds.min_y, 0.0);
        assert_eq!(bounds.max_y, 2000.0);
    }

    #[test]
    fn test_scaler() {
        let bounds = Bounds {
            min_x: 0.0,
            max_x: 10000.0,
            min_y: 0.0,
            max_y: 10000.0,
        };

        let scaler = Scaler::from_bounds(&bounds, 220.0);

        // 10km should scale to 220mm
        // scale = 220 / 10000 = 0.022 mm/m
        assert!((scaler.scale_factor() - 0.022).abs() < 0.001);

        // Center point should be at 110mm
        let (x, y) = scaler.scale(5000.0, 5000.0);
        assert!((x - 110.0).abs() < 1.0);
        assert!((y - 110.0).abs() < 1.0);
    }
}
