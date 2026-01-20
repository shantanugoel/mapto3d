use crate::domain::ParkPolygon;
use crate::geometry::{Projector, Scaler};
use crate::mesh::{extrude_polygon_ex, Triangle};

pub fn generate_park_meshes(
    park_polygons: &[ParkPolygon],
    projector: &Projector,
    scaler: &Scaler,
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

        let triangles = extrude_polygon_ex(&scaled, &[], 0.0, z_top, true);
        all_triangles.extend(triangles);
    }

    all_triangles
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{Bounds, Projector, Scaler};

    #[test]
    fn test_generate_parks_empty() {
        let projector = Projector::new((0.0, 0.0));
        let bounds = Bounds::from_points(&[(0.0, 0.0), (1000.0, 1000.0)]).unwrap();
        let scaler = Scaler::from_bounds(&bounds, 220.0);

        let triangles = generate_park_meshes(&[], &projector, &scaler, 3.2);
        assert!(triangles.is_empty());
    }
}
