use super::Triangle;
use super::triangulation::triangulate_polygon;

pub fn extrude_polygon(
    outer: &[(f32, f32)],
    holes: &[Vec<(f32, f32)>],
    z_bottom: f32,
    z_top: f32,
) -> Vec<Triangle> {
    if outer.len() < 3 {
        return Vec::new();
    }

    let mut triangles = Vec::new();

    let mut all_points: Vec<(f32, f32)> = outer.to_vec();
    for hole in holes {
        all_points.extend(hole.iter().copied());
    }

    let indices = triangulate_polygon(outer, holes);

    if indices.is_empty() {
        return Vec::new();
    }

    for tri in indices.chunks(3) {
        if tri.len() != 3 {
            continue;
        }
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

    add_side_walls(&mut triangles, outer, z_bottom, z_top);

    for hole in holes {
        add_side_walls_reversed(&mut triangles, hole, z_bottom, z_top);
    }

    triangles
}

fn add_side_walls(triangles: &mut Vec<Triangle>, ring: &[(f32, f32)], z_bottom: f32, z_top: f32) {
    let n = ring.len();
    if n < 3 {
        return;
    }

    for i in 0..n {
        let p1 = ring[i];
        let p2 = ring[(i + 1) % n];

        triangles.push(Triangle::new(
            [p1.0, p1.1, z_bottom],
            [p2.0, p2.1, z_bottom],
            [p2.0, p2.1, z_top],
        ));

        triangles.push(Triangle::new(
            [p1.0, p1.1, z_bottom],
            [p2.0, p2.1, z_top],
            [p1.0, p1.1, z_top],
        ));
    }
}

fn add_side_walls_reversed(
    triangles: &mut Vec<Triangle>,
    ring: &[(f32, f32)],
    z_bottom: f32,
    z_top: f32,
) {
    let n = ring.len();
    if n < 3 {
        return;
    }

    for i in 0..n {
        let p1 = ring[i];
        let p2 = ring[(i + 1) % n];

        triangles.push(Triangle::new(
            [p1.0, p1.1, z_bottom],
            [p2.0, p2.1, z_top],
            [p2.0, p2.1, z_bottom],
        ));

        triangles.push(Triangle::new(
            [p1.0, p1.1, z_bottom],
            [p1.0, p1.1, z_top],
            [p2.0, p2.1, z_top],
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extrude_square() {
        let square = vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        let triangles = extrude_polygon(&square, &[], 0.0, 1.0);
        assert!(!triangles.is_empty());
    }

    #[test]
    fn test_extrude_empty() {
        let empty: Vec<(f32, f32)> = vec![];
        let triangles = extrude_polygon(&empty, &[], 0.0, 1.0);
        assert!(triangles.is_empty());
    }
}
