use crate::domain::WaterPolygon;
use crate::geometry::{ClipRect, Projector, Scaler, clip_polygon_to_rect, line_string_to_ring};
use crate::mesh::{extrude_polygon, Triangle};

pub fn generate_water_meshes(
    water_polygons: &[WaterPolygon],
    projector: &Projector,
    scaler: &Scaler,
    clip_rect: &ClipRect,
    z_top: f32,
) -> Vec<Triangle> {
    let polygons = build_water_polygons(water_polygons, projector, scaler, clip_rect);
    generate_water_meshes_from_polygons(&polygons, z_top)
}

pub fn generate_water_meshes_from_polygons(
    polygons: &geo::MultiPolygon<f64>,
    z_top: f32,
) -> Vec<Triangle> {
    let mut all_triangles = Vec::new();

    for polygon in &polygons.0 {
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

        let triangles = extrude_polygon(&outer, &holes, 0.0, z_top);
        all_triangles.extend(triangles);
    }

    all_triangles
}

pub fn build_water_polygons(
    water_polygons: &[WaterPolygon],
    projector: &Projector,
    scaler: &Scaler,
    clip_rect: &ClipRect,
) -> geo::MultiPolygon<f64> {
    let mut all_polygons = Vec::new();

    for polygon in water_polygons {
        if !polygon.is_valid() {
            continue;
        }

        let projected: Vec<(f64, f64)> = polygon
            .outer
            .iter()
            .map(|&(lat, lon)| projector.project(lat, lon))
            .collect();

        let scaled: Vec<(f32, f32)> = projected.iter().map(|&(x, y)| scaler.scale(x, y)).collect();

        let holes_scaled: Vec<Vec<(f32, f32)>> = polygon
            .holes
            .iter()
            .map(|hole| {
                hole.iter()
                    .map(|&(lat, lon)| {
                        let (x, y) = projector.project(lat, lon);
                        scaler.scale(x, y)
                    })
                    .collect()
            })
            .collect();

        let clipped = clip_polygon_to_rect(&scaled, &holes_scaled, clip_rect);
        all_polygons.extend(clipped);
    }

    geo::MultiPolygon::new(all_polygons)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{Bounds, ClipRect, Projector, Scaler};

    #[test]
    fn test_generate_water_empty() {
        let projector = Projector::new((0.0, 0.0));
        let bounds = Bounds::from_points(&[(0.0, 0.0), (1000.0, 1000.0)]).unwrap();
        let scaler = Scaler::from_bounds_with_margin(&bounds, 220.0, 0.0);

        let clip_rect = ClipRect::from_bounds(&bounds, &scaler);
        let triangles = generate_water_meshes(&[], &projector, &scaler, &clip_rect, 2.6);
        assert!(triangles.is_empty());
    }
}
