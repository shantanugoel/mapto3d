use crate::mesh::{triangulation::triangulate_polygon, Triangle};
use geo::{CoordsIter, LineString, MultiPolygon, Polygon};
use geo_clipper::Clipper;
use crate::geometry::CLIPPER_PRECISION_FACTOR;

/// Generate a base plate mesh (rectangular box from z=0 to z=thickness)
///
/// If `footprint` is provided, it is subtracted from the top and bottom faces
/// to avoid overlapping geometry with other layers, which helps slicers.
pub fn generate_base_plate(
    size_mm: f32,
    thickness: f32,
    footprint: Option<&MultiPolygon<f64>>,
) -> Vec<Triangle> {
    let mut triangles = Vec::new();

    let x_min = 0.0;
    let x_max = size_mm;
    let y_min = 0.0;
    let y_max = size_mm;
    let z_bottom = 0.0;
    let z_top = thickness;

    if let Some(fp) = footprint {
        let top_rect = Polygon::new(
            LineString::from(vec![
                (x_min as f64, y_min as f64),
                (x_max as f64, y_min as f64),
                (x_max as f64, y_max as f64),
                (x_min as f64, y_max as f64),
                (x_min as f64, y_min as f64),
            ]),
            vec![],
        );

        let precision_factor = CLIPPER_PRECISION_FACTOR;
        let mut diff = MultiPolygon::new(vec![top_rect]);
        for poly in &fp.0 {
            diff = diff.difference(poly, precision_factor);
        }

        for poly in &diff.0 {
            let outer: Vec<(f32, f32)> = poly
                .exterior()
                .coords_iter()
                .map(|c| (c.x as f32, c.y as f32))
                .collect();

            // Add side walls for exterior
            add_side_walls(&mut triangles, &outer, z_bottom, z_top);

            let mut outer_unique = outer.clone();
            if outer_unique.len() > 1 && outer_unique.first() == outer_unique.last() {
                outer_unique.pop();
            }

            let holes: Vec<Vec<(f32, f32)>> = poly
                .interiors()
                .iter()
                .map(|h| {
                    let pts: Vec<(f32, f32)> =
                        h.coords_iter().map(|c| (c.x as f32, c.y as f32)).collect();

                    // Add side walls for hole
                    add_side_walls(&mut triangles, &pts, z_bottom, z_top);

                    let mut pts_unique = pts;
                    if pts_unique.len() > 1 && pts_unique.first() == pts_unique.last() {
                        pts_unique.pop();
                    }
                    pts_unique
                })
                .collect();

            let indices = triangulate_polygon(&outer_unique, &holes);
            let mut all_points = outer_unique;
            for hole in &holes {
                all_points.extend(hole.iter().copied());
            }

            for tri in indices.chunks(3) {
                if tri.len() == 3 {
                    let p0 = all_points[tri[0]];
                    let p1 = all_points[tri[1]];
                    let p2 = all_points[tri[2]];
                    triangles.push(Triangle::new(
                        [p0.0, p0.1, z_top],
                        [p1.0, p1.1, z_top],
                        [p2.0, p2.1, z_top],
                    ));
                    triangles.push(Triangle::new(
                        [p0.0, p0.1, z_bottom],
                        [p2.0, p2.1, z_bottom],
                        [p1.0, p1.1, z_bottom],
                    ));
                }
            }
        }
    } else {
        // Bottom face (z = 0, normal pointing down)
        triangles.push(Triangle::new(
            [x_min, y_min, z_bottom],
            [x_max, y_max, z_bottom],
            [x_max, y_min, z_bottom],
        ));
        triangles.push(Triangle::new(
            [x_min, y_min, z_bottom],
            [x_min, y_max, z_bottom],
            [x_max, y_max, z_bottom],
        ));

        // Fallback for no footprint (standard solid plate)
        triangles.push(Triangle::new(
            [x_min, y_min, z_top],
            [x_max, y_min, z_top],
            [x_max, y_max, z_top],
        ));
        triangles.push(Triangle::new(
            [x_min, y_min, z_top],
            [x_max, y_max, z_top],
            [x_min, y_max, z_top],
        ));

        // Add standard outer side walls
        triangles.push(Triangle::new(
            [x_min, y_min, z_bottom],
            [x_max, y_min, z_bottom],
            [x_max, y_min, z_top],
        ));
        triangles.push(Triangle::new(
            [x_min, y_min, z_bottom],
            [x_max, y_min, z_top],
            [x_min, y_min, z_top],
        ));
        triangles.push(Triangle::new(
            [x_min, y_max, z_bottom],
            [x_max, y_max, z_top],
            [x_max, y_max, z_bottom],
        ));
        triangles.push(Triangle::new(
            [x_min, y_max, z_bottom],
            [x_min, y_max, z_top],
            [x_max, y_max, z_top],
        ));
        triangles.push(Triangle::new(
            [x_min, y_min, z_bottom],
            [x_min, y_max, z_top],
            [x_min, y_max, z_bottom],
        ));
        triangles.push(Triangle::new(
            [x_min, y_min, z_bottom],
            [x_min, y_min, z_top],
            [x_min, y_max, z_top],
        ));
        triangles.push(Triangle::new(
            [x_max, y_min, z_bottom],
            [x_max, y_max, z_bottom],
            [x_max, y_max, z_top],
        ));
        triangles.push(Triangle::new(
            [x_max, y_min, z_bottom],
            [x_max, y_max, z_top],
            [x_max, y_min, z_top],
        ));
    }

    triangles
}

fn add_side_walls(triangles: &mut Vec<Triangle>, points: &[(f32, f32)], z_bottom: f32, z_top: f32) {
    if points.len() < 2 {
        return;
    }
    for i in 0..points.len() - 1 {
        let p1 = points[i];
        let p2 = points[i + 1];

        // Triangle 1
        triangles.push(Triangle::new(
            [p1.0, p1.1, z_bottom],
            [p2.0, p2.1, z_bottom],
            [p2.0, p2.1, z_top],
        ));
        // Triangle 2
        triangles.push(Triangle::new(
            [p1.0, p1.1, z_bottom],
            [p2.0, p2.1, z_top],
            [p1.0, p1.1, z_top],
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_plate_normals() {
        let size = 100.0;
        let thickness = 2.0;
        let triangles = generate_base_plate(size, thickness, None);

        for tri in triangles {
            let center = [
                (tri.vertices[0][0] + tri.vertices[1][0] + tri.vertices[2][0]) / 3.0,
                (tri.vertices[0][1] + tri.vertices[1][1] + tri.vertices[2][1]) / 3.0,
                (tri.vertices[0][2] + tri.vertices[1][2] + tri.vertices[2][2]) / 3.0,
            ];

            // For each triangle, the dot product of the normal and (center - box_center) should be positive
            let box_center = [size / 2.0, size / 2.0, thickness / 2.0];
            let dir = [
                center[0] - box_center[0],
                center[1] - box_center[1],
                center[2] - box_center[2],
            ];

            let dot = tri.normal[0] * dir[0] + tri.normal[1] * dir[1] + tri.normal[2] * dir[2];
            assert!(
                dot > 0.0,
                "Normal {:?} at center {:?} points inward (box center {:?})",
                tri.normal,
                center,
                box_center
            );
        }
    }
}
