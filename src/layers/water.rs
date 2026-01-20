use crate::domain::WaterPolygon;
use crate::geometry::{Projector, Scaler};
use crate::mesh::{Triangle, extrude_polygon};

/// Water features are recessed into the base plate.
/// Depth of 0.6mm = 3 layers at 0.2mm layer height for solid color.
const WATER_Z_BOTTOM: f32 = -0.6;
const WATER_Z_TOP: f32 = 0.0;

pub fn generate_water_meshes(
    water_polygons: &[WaterPolygon],
    projector: &Projector,
    scaler: &Scaler,
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

        let triangles = extrude_polygon(&scaled, &holes_scaled, WATER_Z_BOTTOM, WATER_Z_TOP);
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

        let triangles = generate_water_meshes(&[], &projector, &scaler);
        assert!(triangles.is_empty());
    }
}
