use crate::config::heights::{PARK_Z_BOTTOM, PARK_Z_TOP};
use crate::domain::ParkPolygon;
use crate::geometry::{Projector, Scaler};
use crate::mesh::{Triangle, extrude_polygon_ex};

pub fn generate_park_meshes(
    park_polygons: &[ParkPolygon],
    projector: &Projector,
    scaler: &Scaler,
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

        let triangles = extrude_polygon_ex(&scaled, &[], PARK_Z_BOTTOM, PARK_Z_TOP, true);
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

        let triangles = generate_park_meshes(&[], &projector, &scaler);
        assert!(triangles.is_empty());
    }
}
