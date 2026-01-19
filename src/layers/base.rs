use crate::mesh::Triangle;

/// Generate a base plate mesh (rectangular box)
///
/// The base plate sits below the map (z = -thickness to 0)
/// and provides a solid foundation for 3D printing
pub fn generate_base_plate(size_mm: f32, thickness: f32) -> Vec<Triangle> {
    let mut triangles = Vec::new();

    let x_min = 0.0;
    let x_max = size_mm;
    let y_min = 0.0;
    let y_max = size_mm;
    let z_bottom = -thickness;
    let z_top = 0.0;

    // Bottom face (z = -thickness, normal pointing down)
    triangles.push(Triangle::new(
        [x_min, y_min, z_bottom],
        [x_max, y_min, z_bottom],
        [x_max, y_max, z_bottom],
    ));
    triangles.push(Triangle::new(
        [x_min, y_min, z_bottom],
        [x_max, y_max, z_bottom],
        [x_min, y_max, z_bottom],
    ));

    // Top face (z = 0, normal pointing up)
    triangles.push(Triangle::new(
        [x_min, y_min, z_top],
        [x_max, y_max, z_top],
        [x_max, y_min, z_top],
    ));
    triangles.push(Triangle::new(
        [x_min, y_min, z_top],
        [x_min, y_max, z_top],
        [x_max, y_max, z_top],
    ));

    // Front face (y = 0)
    triangles.push(Triangle::new(
        [x_min, y_min, z_bottom],
        [x_max, y_min, z_top],
        [x_max, y_min, z_bottom],
    ));
    triangles.push(Triangle::new(
        [x_min, y_min, z_bottom],
        [x_min, y_min, z_top],
        [x_max, y_min, z_top],
    ));

    // Back face (y = size)
    triangles.push(Triangle::new(
        [x_min, y_max, z_bottom],
        [x_max, y_max, z_bottom],
        [x_max, y_max, z_top],
    ));
    triangles.push(Triangle::new(
        [x_min, y_max, z_bottom],
        [x_max, y_max, z_top],
        [x_min, y_max, z_top],
    ));

    // Left face (x = 0)
    triangles.push(Triangle::new(
        [x_min, y_min, z_bottom],
        [x_min, y_max, z_bottom],
        [x_min, y_max, z_top],
    ));
    triangles.push(Triangle::new(
        [x_min, y_min, z_bottom],
        [x_min, y_max, z_top],
        [x_min, y_min, z_top],
    ));

    // Right face (x = size)
    triangles.push(Triangle::new(
        [x_max, y_min, z_bottom],
        [x_max, y_max, z_top],
        [x_max, y_max, z_bottom],
    ));
    triangles.push(Triangle::new(
        [x_max, y_min, z_bottom],
        [x_max, y_min, z_top],
        [x_max, y_max, z_top],
    ));

    triangles
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_plate_triangle_count() {
        let triangles = generate_base_plate(100.0, 2.0);
        // 6 faces * 2 triangles each = 12 triangles
        assert_eq!(triangles.len(), 12);
    }
}
