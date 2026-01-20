use crate::domain::WaterPolygon;
use crate::geometry::{Projector, Scaler};
use crate::mesh::{extrude_polygon, Triangle};

pub fn generate_water_meshes(
    water_polygons: &[WaterPolygon],
    projector: &Projector,
    scaler: &Scaler,
    z_top: f32,
) -> Vec<Triangle> {
    let mut all_triangles = Vec::new();

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

        let triangles = extrude_polygon(&scaled, &holes_scaled, 0.0, z_top);
        all_triangles.extend(triangles);
    }

    all_triangles
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{Bounds, Projector, Scaler};

    #[test]
    fn test_generate_water_empty() {
        let projector = Projector::new((0.0, 0.0));
        let bounds = Bounds::from_points(&[(0.0, 0.0), (1000.0, 1000.0)]).unwrap();
        let scaler = Scaler::from_bounds(&bounds, 220.0);

        let triangles = generate_water_meshes(&[], &projector, &scaler, 2.6);
        assert!(triangles.is_empty());
    }
}
