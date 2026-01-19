use geo::{LineString, Simplify};

pub fn simplify_polyline(points: &[(f64, f64)], epsilon: f64) -> Vec<(f64, f64)> {
    if points.len() < 4 {
        return points.to_vec();
    }

    let line: LineString<f64> = points
        .iter()
        .map(|&(lat, lon)| geo::coord! { x: lon, y: lat })
        .collect();

    let simplified = line.simplify(&epsilon);

    simplified.0.into_iter().map(|c| (c.y, c.x)).collect()
}

pub fn calculate_epsilon(radius_m: u32) -> f64 {
    let radius_km = radius_m as f64 / 1000.0;

    if radius_km < 3.0 {
        2.0
    } else if radius_km < 5.0 {
        5.0
    } else if radius_km < 10.0 {
        8.0
    } else if radius_km < 20.0 {
        15.0
    } else {
        25.0
    }
}

pub fn simplify_polygon(outer: &[(f64, f64)], epsilon: f64) -> Vec<(f64, f64)> {
    if outer.len() < 5 {
        return outer.to_vec();
    }

    let simplified = simplify_polyline(outer, epsilon);

    if simplified.len() < 4 {
        return outer.to_vec();
    }

    simplified
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
    fn test_calculate_epsilon() {
        assert_eq!(calculate_epsilon(2000), 2.0);
        assert_eq!(calculate_epsilon(4000), 5.0);
        assert_eq!(calculate_epsilon(8000), 8.0);
        assert_eq!(calculate_epsilon(15000), 15.0);
        assert_eq!(calculate_epsilon(30000), 25.0);
    }

    #[test]
    fn test_simplify_polygon_preserves_minimum() {
        let square = vec![(0.0, 0.0), (0.0, 1.0), (1.0, 1.0), (1.0, 0.0), (0.0, 0.0)];
        let result = simplify_polygon(&square, 0.1);
        assert!(result.len() >= 4);
    }
}
