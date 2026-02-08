//! Polyline buffering utilities for road mesh generation
//!
//! Converts road centerlines into polygons with proper width using
//! clipper-based offsetting with configurable join and end cap styles.

use geo::{LineString, MultiLineString, MultiPolygon};
use geo_clipper::{ClipperOpen, EndType, JoinType};

/// Buffer join style for road corners
#[derive(Debug, Clone, Copy, Default)]
#[allow(dead_code)]
pub enum BufferJoinStyle {
    /// Round joins - smooth corners, best for roads
    #[default]
    Round,
    /// Square/bevel joins - flat corners
    Square,
    /// Miter joins with limit - sharp corners up to limit
    Miter,
}

/// Buffer end cap style for road endpoints
#[derive(Debug, Clone, Copy, Default)]
#[allow(dead_code)]
pub enum BufferCapStyle {
    /// Round caps - semicircular ends
    #[default]
    Round,
    /// Square caps - flat ends extending by half width
    Square,
    /// Butt caps - flat ends at exact endpoint
    Butt,
}

/// Configuration for polyline buffering
#[derive(Debug, Clone)]
pub struct BufferConfig {
    /// Join style for corners
    pub join_style: BufferJoinStyle,
    /// End cap style for line endpoints
    pub cap_style: BufferCapStyle,
    /// Miter limit (only used with Miter join style)
    /// Higher values allow sharper miters before falling back to square
    pub miter_limit: f64,
    /// Scaling factor for clipper precision (typically 1000.0 or higher)
    pub precision_factor: f64,
}

impl Default for BufferConfig {
    fn default() -> Self {
        Self {
            join_style: BufferJoinStyle::Round,
            cap_style: BufferCapStyle::Round,
            miter_limit: 2.0,
            precision_factor: 1000.0,
        }
    }
}

impl BufferConfig {
    /// Create config optimized for smooth roads
    pub fn for_roads() -> Self {
        Self {
            join_style: BufferJoinStyle::Round,
            cap_style: BufferCapStyle::Round,
            miter_limit: 2.0,
            precision_factor: 1000.0,
        }
    }

    fn to_clipper_join_type(&self) -> JoinType {
        match self.join_style {
            BufferJoinStyle::Round => JoinType::Round(self.miter_limit),
            BufferJoinStyle::Square => JoinType::Square,
            BufferJoinStyle::Miter => JoinType::Miter(self.miter_limit),
        }
    }

    fn to_clipper_end_type(&self) -> EndType {
        match self.cap_style {
            BufferCapStyle::Round => EndType::OpenRound(self.miter_limit),
            BufferCapStyle::Square => EndType::OpenSquare,
            BufferCapStyle::Butt => EndType::OpenButt,
        }
    }
}

/// Buffer a polyline (road centerline) into a polygon with the specified width
///
/// # Arguments
/// * `points` - Road centerline points in local coordinates (meters or mm)
/// * `width` - Total road width (buffer distance will be width/2)
/// * `config` - Buffer configuration for join and cap styles
///
/// # Returns
/// A MultiPolygon representing the buffered road shape
pub fn buffer_polyline(
    points: &[(f64, f64)],
    width: f64,
    config: &BufferConfig,
) -> MultiPolygon<f64> {
    if points.len() < 2 {
        return MultiPolygon::new(vec![]);
    }

    let closed = is_closed_loop(points);
    let normalized_points = if closed {
        points[..points.len() - 1].to_vec()
    } else {
        points.to_vec()
    };

    if normalized_points.len() < 2 {
        return MultiPolygon::new(vec![]);
    }

    let line_string: LineString<f64> = normalized_points
        .iter()
        .map(|&(x, y)| geo::coord! { x: x, y: y })
        .collect();

    // geo-clipper only implements ClipperOpen for MultiLineString, not LineString
    let multi_line_string = MultiLineString::new(vec![line_string]);

    let delta = width / 2.0;

    let end_type = if closed {
        EndType::ClosedLine
    } else {
        config.to_clipper_end_type()
    };

    multi_line_string.offset(
        delta,
        config.to_clipper_join_type(),
        end_type,
        config.precision_factor,
    )
}

fn is_closed_loop(points: &[(f64, f64)]) -> bool {
    if points.len() < 3 {
        return false;
    }

    let first = points.first().unwrap();
    let last = points.last().unwrap();
    let dx = first.0 - last.0;
    let dy = first.1 - last.1;
    (dx * dx + dy * dy) < 1e-10
}

/// Buffer multiple polylines and return all resulting polygons
#[allow(dead_code)]
pub fn buffer_polylines<'a, I>(polylines: I, config: &BufferConfig) -> Vec<geo::Polygon<f64>>
where
    I: Iterator<Item = (&'a [(f64, f64)], f64)>,
{
    let mut all_polygons = Vec::new();

    for (points, width) in polylines {
        let multi = buffer_polyline(points, width, config);
        all_polygons.extend(multi.0);
    }

    all_polygons
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo::{Contains, Point};

    #[test]
    fn test_buffer_diagonal_line() {
        let points = vec![(0.0, 0.0), (10.0, 10.0)];
        let config = BufferConfig::for_roads();
        let result = buffer_polyline(&points, 2.0, &config);

        assert!(!result.0.is_empty(), "Should produce at least one polygon");
    }

    #[test]
    fn test_buffer_empty_line() {
        let points: Vec<(f64, f64)> = vec![];
        let config = BufferConfig::for_roads();
        let result = buffer_polyline(&points, 2.0, &config);

        assert!(
            result.0.is_empty(),
            "Empty input should produce empty output"
        );
    }

    #[test]
    fn test_buffer_single_point() {
        let points = vec![(0.0, 0.0)];
        let config = BufferConfig::for_roads();
        let result = buffer_polyline(&points, 2.0, &config);

        assert!(
            result.0.is_empty(),
            "Single point should produce empty output"
        );
    }

    #[test]
    fn test_buffer_curved_line() {
        let points = vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)];
        let config = BufferConfig::for_roads();
        let result = buffer_polyline(&points, 2.0, &config);

        assert!(!result.0.is_empty(), "Should produce polygon for L-shape");
    }

    #[test]
    fn test_buffer_different_styles() {
        let points = vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)];

        for join_style in [
            BufferJoinStyle::Round,
            BufferJoinStyle::Square,
            BufferJoinStyle::Miter,
        ] {
            let config = BufferConfig {
                join_style,
                ..Default::default()
            };
            let result = buffer_polyline(&points, 2.0, &config);
            assert!(
                !result.0.is_empty(),
                "Join style {:?} should work",
                join_style
            );
        }
    }

    #[test]
    fn test_buffer_closed_loop_keeps_hole() {
        let points = vec![
            (0.0, 0.0),
            (10.0, 0.0),
            (10.0, 10.0),
            (0.0, 10.0),
            (0.0, 0.0),
        ];
        let config = BufferConfig::for_roads();
        let result = buffer_polyline(&points, 2.0, &config);

        assert!(
            !result.0.is_empty(),
            "Closed loop should produce buffered polygon"
        );
        let center = Point::new(5.0, 5.0);
        assert!(
            !result.contains(&center),
            "Buffered closed loop should not fill interior region"
        );
    }
}
