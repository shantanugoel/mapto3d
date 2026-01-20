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

    let simplified = line.simplify(&epsilon);

    simplified.0.into_iter().map(|c| (c.y, c.x)).collect()
}

/// Simplify a polyline in projected (x, y) coordinates using Ramer-Douglas-Peucker
pub fn simplify_projected(points: &[(f64, f64)], epsilon: f64) -> Vec<(f64, f64)> {
    if points.len() < 3 {
        return points.to_vec();
    }

    let line: LineString<f64> = points
        .iter()
        .map(|&(x, y)| geo::coord! { x: x, y: y })
        .collect();

    let simplified = line.simplify(&epsilon);

    simplified.0.into_iter().map(|c| (c.x, c.y)).collect()
}

pub fn filter_short_segments(points: &[(f64, f64)], min_length: f64) -> Vec<(f64, f64)> {
    if points.len() < 2 {
        return points.to_vec();
    }

    let min_length_sq = min_length * min_length;
    let mut result = Vec::with_capacity(points.len());
    result.push(points[0]);

    for &(x, y) in points.iter().skip(1) {
        let (last_x, last_y) = *result.last().unwrap();
        let dx = x - last_x;
        let dy = y - last_y;
        let dist_sq = dx * dx + dy * dy;

        if dist_sq >= min_length_sq {
            result.push((x, y));
        }
    }

    if result.len() < 2 && points.len() >= 2 {
        return vec![points[0], *points.last().unwrap()];
    }

    result
}

pub fn simplify_for_mesh(
    points: &[(f64, f64)],
    epsilon: f64,
    min_segment_length: f64,
) -> Vec<(f64, f64)> {
    let simplified = simplify_projected(points, epsilon);
    filter_short_segments(&simplified, min_segment_length)
}

pub fn calculate_epsilon_meters(radius_m: u32, simplify_level: u8) -> f64 {
    let base_epsilon = match radius_m {
        0..=3000 => 0.5,
        3001..=5000 => 1.0,
        5001..=10000 => 2.0,
        10001..=20000 => 4.0,
        _ => 8.0,
    };

    let multiplier = match simplify_level {
        0 => 0.0,
        1 => 1.0,
        2 => 2.0,
        3 => 4.0,
        _ => 1.0,
    };

    base_epsilon * multiplier
}

pub fn calculate_min_segment_length(radius_m: u32) -> f64 {
    match radius_m {
        0..=5000 => 0.5,
        5001..=10000 => 1.0,
        10001..=20000 => 2.0,
        _ => 5.0,
    }
}

#[allow(dead_code)]
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

#[allow(dead_code)]
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

    #[test]
    fn test_simplify_projected() {
        let points = vec![(0.0, 0.0), (5.0, 0.0), (10.0, 0.0)];
        let result = simplify_projected(&points, 1.0);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_filter_short_segments() {
        let points = vec![(0.0, 0.0), (0.1, 0.0), (10.0, 0.0)];
        let result = filter_short_segments(&points, 1.0);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], (0.0, 0.0));
        assert_eq!(result[1], (10.0, 0.0));
    }

    #[test]
    fn test_filter_preserves_endpoints() {
        let points = vec![(0.0, 0.0), (0.01, 0.0)];
        let result = filter_short_segments(&points, 1.0);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_simplify_for_mesh() {
        let points: Vec<(f64, f64)> = (0..20).map(|i| (i as f64 * 0.5, 0.0)).collect();
        let result = simplify_for_mesh(&points, 0.5, 0.5);
        assert!(result.len() <= points.len());
        assert!(result.len() >= 2);
    }

    #[test]
    fn test_calculate_epsilon_meters() {
        assert_eq!(calculate_epsilon_meters(2000, 0), 0.0);
        assert_eq!(calculate_epsilon_meters(2000, 1), 0.5);
        assert_eq!(calculate_epsilon_meters(2000, 2), 1.0);
        assert_eq!(calculate_epsilon_meters(15000, 1), 4.0);
    }
}
