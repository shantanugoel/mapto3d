use geo::CoordsIter;

use crate::domain::ParkPolygon;
use crate::geometry::{ClipRect, Projector, Scaler, clip_polygon_to_rect};
use crate::mesh::{Triangle, extrude_polygon_ex};

pub fn generate_park_meshes(
    park_polygons: &[ParkPolygon],
    projector: &Projector,
    scaler: &Scaler,
    clip_rect: &ClipRect,
    z_top: f32,
) -> Vec<Triangle> {
    let mut all_triangles = Vec::new();

    for polygon in park_polygons {
        if !polygon.is_valid() {
            continue;
        }

        let projected: Vec<(f64, f64)> = polygon
            .outer
            .iter()
            .map(|&(lat, lon)| projector.project(lat, lon))
            .collect();

        let scaled: Vec<(f32, f32)> = projected.iter().map(|&(x, y)| scaler.scale(x, y)).collect();

        let clipped = clip_polygon_to_rect(&scaled, &[], clip_rect);
        for polygon in clipped {
            let outer = line_string_to_ring(polygon.exterior(), false);
            if outer.len() < 3 {
                continue;
            }

            let holes: Vec<Vec<(f32, f32)>> = polygon
                .interiors()
                .iter()
                .map(|ring| line_string_to_ring(ring, true))
                .filter(|ring| ring.len() >= 3)
                .collect();

            let triangles = extrude_polygon_ex(&outer, &holes, 0.0, z_top, true);
            all_triangles.extend(triangles);
        }
    }

    all_triangles
}

fn line_string_to_ring(ring: &geo::LineString<f64>, expect_clockwise: bool) -> Vec<(f32, f32)> {
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
    (a.0 - b.0).abs() < 1e-4 && (a.1 - b.1).abs() < 1e-4
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{Bounds, ClipRect, Projector, Scaler};

    #[test]
    fn test_generate_parks_empty() {
        let projector = Projector::new((0.0, 0.0));
        let bounds = Bounds::from_points(&[(0.0, 0.0), (1000.0, 1000.0)]).unwrap();
        let scaler = Scaler::from_bounds_with_margin(&bounds, 220.0, 0.0);

        let clip_rect = ClipRect::from_bounds(&bounds, &scaler);
        let triangles = generate_park_meshes(&[], &projector, &scaler, &clip_rect, 3.2);
        assert!(triangles.is_empty());
    }
}
