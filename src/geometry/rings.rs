use geo::CoordsIter;

pub const RING_POINT_EPSILON: f32 = 1e-4;

pub fn line_string_to_ring(
    ring: &geo::LineString<f64>,
    expect_clockwise: bool,
) -> Vec<(f32, f32)> {
    let mut points: Vec<(f32, f32)> = ring
        .coords_iter()
        .map(|coord| (coord.x as f32, coord.y as f32))
        .collect();

    if points.len() < 3 {
        return Vec::new();
    }

    if points_are_close(*points.first().unwrap(), *points.last().unwrap()) {
        points.pop();
    }

    points = clean_ring(points);
    if points.len() < 3 {
        return Vec::new();
    }

    if is_clockwise(&points) != expect_clockwise {
        points.reverse();
    }

    points
}

fn clean_ring(points: Vec<(f32, f32)>) -> Vec<(f32, f32)> {
    let mut cleaned = Vec::with_capacity(points.len());
    for point in points {
        let is_duplicate = cleaned
            .last()
            .is_some_and(|&previous| points_are_close(previous, point));
        if !is_duplicate {
            cleaned.push(point);
        }
    }

    if cleaned.len() > 2 && points_are_close(*cleaned.first().unwrap(), *cleaned.last().unwrap()) {
        cleaned.pop();
    }

    cleaned
}

fn is_clockwise(points: &[(f32, f32)]) -> bool {
    signed_area(points) < 0.0
}

fn signed_area(points: &[(f32, f32)]) -> f32 {
    let mut area = 0.0;
    for i in 0..points.len() {
        let (x1, y1) = points[i];
        let (x2, y2) = points[(i + 1) % points.len()];
        area += x1 * y2 - x2 * y1;
    }
    area * 0.5
}

fn points_are_close(a: (f32, f32), b: (f32, f32)) -> bool {
    (a.0 - b.0).abs() < RING_POINT_EPSILON && (a.1 - b.1).abs() < RING_POINT_EPSILON
}
