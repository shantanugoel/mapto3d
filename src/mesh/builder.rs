/// A triangle for STL output
#[derive(Debug, Clone)]
pub struct Triangle {
    /// Three vertices: [[x, y, z], [x, y, z], [x, y, z]]
    pub vertices: [[f32; 3]; 3],
    /// Normal vector [nx, ny, nz]
    pub normal: [f32; 3],
}

impl Triangle {
    /// Create a new triangle and calculate its normal
    pub fn new(v0: [f32; 3], v1: [f32; 3], v2: [f32; 3]) -> Self {
        let normal = calculate_normal(v0, v1, v2);
        Self {
            vertices: [v0, v1, v2],
            normal,
        }
    }

    /// Create a triangle with a pre-calculated normal
    #[allow(dead_code)]
    pub fn with_normal(vertices: [[f32; 3]; 3], normal: [f32; 3]) -> Self {
        Self { vertices, normal }
    }
}

/// Calculate the normal vector for a triangle using the cross product
fn calculate_normal(v0: [f32; 3], v1: [f32; 3], v2: [f32; 3]) -> [f32; 3] {
    // Edge vectors
    let u = [v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2]];
    let v = [v2[0] - v0[0], v2[1] - v0[1], v2[2] - v0[2]];

    // Cross product
    let nx = u[1] * v[2] - u[2] * v[1];
    let ny = u[2] * v[0] - u[0] * v[2];
    let nz = u[0] * v[1] - u[1] * v[0];

    // Normalize
    let len = (nx * nx + ny * ny + nz * nz).sqrt();
    if len > 1e-10 {
        [nx / len, ny / len, nz / len]
    } else {
        [0.0, 0.0, 1.0] // Default to up for degenerate triangles
    }
}

/// Accumulator for building triangle meshes
#[allow(dead_code)]
#[derive(Debug, Default)]
pub struct MeshBuilder {
    triangles: Vec<Triangle>,
}

#[allow(dead_code)]
impl MeshBuilder {
    pub fn new() -> Self {
        Self {
            triangles: Vec::new(),
        }
    }

    /// Add a triangle from three vertices
    pub fn add_triangle(&mut self, v0: [f32; 3], v1: [f32; 3], v2: [f32; 3]) {
        self.triangles.push(Triangle::new(v0, v1, v2));
    }

    /// Add a quad (two triangles) from four vertices
    /// Vertices should be in counter-clockwise order when viewed from above
    pub fn add_quad(&mut self, v0: [f32; 3], v1: [f32; 3], v2: [f32; 3], v3: [f32; 3]) {
        self.add_triangle(v0, v1, v2);
        self.add_triangle(v0, v2, v3);
    }

    /// Add triangles from another collection
    pub fn extend(&mut self, triangles: impl IntoIterator<Item = Triangle>) {
        self.triangles.extend(triangles);
    }

    /// Get the number of triangles
    pub fn len(&self) -> usize {
        self.triangles.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.triangles.is_empty()
    }

    /// Consume the builder and return the triangles
    pub fn finish(self) -> Vec<Triangle> {
        self.triangles
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_triangle_normal() {
        // A triangle in the XY plane should have a Z-pointing normal
        let tri = Triangle::new([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);

        // Normal should point in +Z direction
        assert!((tri.normal[0]).abs() < 0.001);
        assert!((tri.normal[1]).abs() < 0.001);
        assert!((tri.normal[2] - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_mesh_builder() {
        let mut builder = MeshBuilder::new();
        builder.add_triangle([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        builder.add_quad(
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        );

        assert_eq!(builder.len(), 3); // 1 triangle + 2 from quad
    }
}
