use geo::{LineString, Simplify};

/// Simplify a polyline in lat/lon coordinates using Ramer-Douglas-Peucker
pub fn simplify_polyline(points: &[(f64, f64)], epsilon: f64) -> Vec<(f64, f64)> {
    if points.len() < 4 {
        return points.to_vec();
    }

    let line: LineString<f64> = points
        .iter()
        .map(|&(lat, lon)| geo::coord! { x: lon, y: lat })
        .collect();

    let simplified = line.simplify(epsilon);

    simplified.0.into_iter().map(|c| (c.y, c.x)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simplify_polyline_short() {
        let points = vec![(0.0, 0.0), (1.0, 1.0), (2.0, 2.0)];
        let result = simplify_polyline(&points, 1.0);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_simplify_polyline_reduces_points() {
        let points: Vec<(f64, f64)> = (0..100)
            .map(|i| {
                let x = i as f64;
                let y = if i % 2 == 0 { 0.0 } else { 0.0001 };
                (y, x)
            })
            .collect();

        let result = simplify_polyline(&points, 0.001);
        assert!(result.len() < points.len());
    }

    #[test]
    fn test_simplify_projected() {
        let points = vec![(0.0, 0.0), (5.0, 0.0), (10.0, 0.0)];
        let result = simplify_polyline(&points, 1.0);
        assert!(result.len() <= points.len());
    }
}
