use super::Triangle;

/// Extrude a 2D polyline into a 3D ribbon mesh
///
/// Creates a ribbon of the specified width and height from a series of 2D points.
/// The ribbon has top, bottom, and side faces.
///
/// # Arguments
/// * `points` - 2D points in mm [(x, y), ...]
/// * `width` - Ribbon width in mm
/// * `height` - Ribbon height in mm
/// * `base_z` - Base Z level in mm
///
/// # Returns
/// Vector of triangles forming the ribbon mesh
pub fn extrude_ribbon(
    points: &[(f32, f32)],
    width: f32,
    height: f32,
    base_z: f32,
) -> Vec<Triangle> {
    if points.len() < 2 {
        return Vec::new();
    }

    let mut triangles = Vec::new();
    let half_width = width / 2.0;
    let top_z = base_z + height;

    // Generate left and right edge points for each input point
    let edges: Vec<([f32; 2], [f32; 2])> = points
        .iter()
        .enumerate()
        .map(|(i, &(x, y))| {
            // Calculate direction at this point
            let (dx, dy) = if i == 0 {
                // First point: use direction to next point
                direction(points[0], points[1])
            } else if i == points.len() - 1 {
                // Last point: use direction from previous point
                direction(points[i - 1], points[i])
            } else {
                // Middle point: average directions for miter join
                let d1 = direction(points[i - 1], points[i]);
                let d2 = direction(points[i], points[i + 1]);
                let avg = ((d1.0 + d2.0) / 2.0, (d1.1 + d2.1) / 2.0);
                normalize(avg)
            };

            // Perpendicular vector (rotate 90 degrees)
            let (px, py) = (-dy, dx);

            // Left and right points
            let left = [x - px * half_width, y - py * half_width];
            let right = [x + px * half_width, y + py * half_width];

            (left, right)
        })
        .collect();

    // Generate mesh for each segment
    for i in 0..edges.len() - 1 {
        let (l0, r0) = edges[i];
        let (l1, r1) = edges[i + 1];

        // Top face (CCW when viewed from above)
        let tl0 = [l0[0], l0[1], top_z];
        let tr0 = [r0[0], r0[1], top_z];
        let tl1 = [l1[0], l1[1], top_z];
        let tr1 = [r1[0], r1[1], top_z];

        triangles.push(Triangle::new(tl0, tr0, tr1));
        triangles.push(Triangle::new(tl0, tr1, tl1));

        // Bottom face (CW when viewed from above = CCW from below)
        let bl0 = [l0[0], l0[1], base_z];
        let br0 = [r0[0], r0[1], base_z];
        let bl1 = [l1[0], l1[1], base_z];
        let br1 = [r1[0], r1[1], base_z];

        triangles.push(Triangle::new(bl0, br1, br0));
        triangles.push(Triangle::new(bl0, bl1, br1));

        // Left side face
        triangles.push(Triangle::new(bl0, tl0, tl1));
        triangles.push(Triangle::new(bl0, tl1, bl1));

        // Right side face
        triangles.push(Triangle::new(br0, tr1, tr0));
        triangles.push(Triangle::new(br0, br1, tr1));
    }

    // End caps
    if !edges.is_empty() {
        // Start cap
        let (l0, r0) = edges[0];
        let bl = [l0[0], l0[1], base_z];
        let br = [r0[0], r0[1], base_z];
        let tl = [l0[0], l0[1], top_z];
        let tr = [r0[0], r0[1], top_z];
        triangles.push(Triangle::new(bl, tl, tr));
        triangles.push(Triangle::new(bl, tr, br));

        // End cap
        let (l1, r1) = edges[edges.len() - 1];
        let bl = [l1[0], l1[1], base_z];
        let br = [r1[0], r1[1], base_z];
        let tl = [l1[0], l1[1], top_z];
        let tr = [r1[0], r1[1], top_z];
        triangles.push(Triangle::new(bl, tr, tl));
        triangles.push(Triangle::new(bl, br, tr));
    }

    triangles
}

/// Calculate normalized direction vector between two points
fn direction(p1: (f32, f32), p2: (f32, f32)) -> (f32, f32) {
    let dx = p2.0 - p1.0;
    let dy = p2.1 - p1.1;
    normalize((dx, dy))
}

/// Normalize a 2D vector
fn normalize((x, y): (f32, f32)) -> (f32, f32) {
    let len = (x * x + y * y).sqrt();
    if len > 1e-10 {
        (x / len, y / len)
    } else {
        (1.0, 0.0) // Default direction for zero-length vectors
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extrude_simple_segment() {
        let points = vec![(0.0, 0.0), (10.0, 0.0)];
        let triangles = extrude_ribbon(&points, 2.0, 1.0, 0.0);

        // Should have: 8 triangles for the segment (2 top, 2 bottom, 4 sides)
        // + 4 triangles for end caps
        assert_eq!(triangles.len(), 12);
    }

    #[test]
    fn test_extrude_empty() {
        let points: Vec<(f32, f32)> = vec![];
        let triangles = extrude_ribbon(&points, 2.0, 1.0, 0.0);
        assert!(triangles.is_empty());
    }

    #[test]
    fn test_extrude_single_point() {
        let points = vec![(0.0, 0.0)];
        let triangles = extrude_ribbon(&points, 2.0, 1.0, 0.0);
        assert!(triangles.is_empty());
    }
}
