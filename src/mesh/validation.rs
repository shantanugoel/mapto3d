//! Mesh validation and repair utilities
//!
//! Provides functions to validate triangle meshes for 3D printing compatibility:
//! - Detect degenerate triangles (zero area)
//! - Check for NaN/Inf coordinates
//! - Verify and fix normal orientation
//! - Remove invalid geometry

use super::Triangle;

/// Result of mesh validation
#[derive(Debug, Default)]
pub struct ValidationResult {
    /// Total number of triangles validated
    #[allow(dead_code)]
    pub total: usize,
    /// Number of degenerate triangles (zero or near-zero area)
    pub degenerate: usize,
    /// Number of triangles with invalid coordinates (NaN/Inf)
    pub invalid_coords: usize,
    /// Number of triangles with incorrect normals (fixed during validation)
    pub invalid_normal: usize,
    /// Warning messages for issues found
    pub warnings: Vec<String>,
}

impl ValidationResult {
    /// Check if the mesh passed validation without critical issues
    #[allow(dead_code)]
    pub fn is_valid(&self) -> bool {
        self.invalid_coords == 0
    }

    /// Check if the mesh has any issues at all
    #[allow(dead_code)]
    pub fn has_issues(&self) -> bool {
        self.degenerate > 0 || self.invalid_coords > 0 || self.invalid_normal > 0
    }

    /// Get a summary string
    #[allow(dead_code)]
    pub fn summary(&self) -> String {
        if !self.has_issues() {
            format!("Mesh valid: {} triangles, no issues", self.total)
        } else {
            format!(
                "Mesh issues: {} total, {} degenerate, {} invalid coords, {} bad normals",
                self.total, self.degenerate, self.invalid_coords, self.invalid_normal
            )
        }
    }
}

/// Minimum area threshold for non-degenerate triangles (in square mm)
const MIN_TRIANGLE_AREA: f32 = 1e-10;

/// Validate a mesh and return a detailed report
///
/// Checks for:
/// - Degenerate triangles (zero or near-zero area)
/// - Invalid coordinates (NaN, Inf)
/// - Normal vector validity
pub fn validate_mesh(triangles: &[Triangle]) -> ValidationResult {
    let mut result = ValidationResult {
        total: triangles.len(),
        ..Default::default()
    };

    for (i, tri) in triangles.iter().enumerate() {
        if has_invalid_coords(tri) {
            result.invalid_coords += 1;
            result
                .warnings
                .push(format!("Triangle {} has NaN/Inf coordinates", i));
            continue;
        }

        if is_degenerate(tri) {
            result.degenerate += 1;
        }

        if !is_normal_valid(&tri.normal) {
            result.invalid_normal += 1;
        }
    }

    if result.degenerate > 0 {
        result.warnings.push(format!(
            "{} degenerate triangles detected (will be removed)",
            result.degenerate
        ));
    }
    if result.invalid_normal > 0 {
        result.warnings.push(format!(
            "{} triangles had invalid normals (will be recalculated)",
            result.invalid_normal
        ));
    }

    result
}

/// Check if a triangle has any invalid (NaN/Inf) coordinates
fn has_invalid_coords(tri: &Triangle) -> bool {
    for vertex in &tri.vertices {
        for coord in vertex {
            if !coord.is_finite() {
                return true;
            }
        }
    }
    for coord in &tri.normal {
        if !coord.is_finite() {
            return true;
        }
    }
    false
}

/// Check if a triangle is degenerate (zero or near-zero area)
fn is_degenerate(tri: &Triangle) -> bool {
    let area = triangle_area(&tri.vertices);
    area < MIN_TRIANGLE_AREA
}

/// Calculate the area of a triangle from its vertices
fn triangle_area(vertices: &[[f32; 3]; 3]) -> f32 {
    let v0 = vertices[0];
    let v1 = vertices[1];
    let v2 = vertices[2];

    let edge_a = [v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2]];
    let edge_b = [v2[0] - v0[0], v2[1] - v0[1], v2[2] - v0[2]];

    let cx = edge_a[1] * edge_b[2] - edge_a[2] * edge_b[1];
    let cy = edge_a[2] * edge_b[0] - edge_a[0] * edge_b[2];
    let cz = edge_a[0] * edge_b[1] - edge_a[1] * edge_b[0];

    0.5 * (cx * cx + cy * cy + cz * cz).sqrt()
}

/// Check if a normal vector is valid (unit length, not zero/NaN)
fn is_normal_valid(normal: &[f32; 3]) -> bool {
    let len_sq = normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2];
    len_sq.is_finite() && (0.99..=1.01).contains(&len_sq)
}

/// Recalculate normals for all triangles in the mesh
///
/// Uses the right-hand rule: CCW winding = outward normal
pub fn fix_normals(triangles: &mut [Triangle]) {
    for tri in triangles.iter_mut() {
        tri.normal = calculate_normal(&tri.vertices);
    }
}

/// Calculate the normal vector for a triangle using the cross product
fn calculate_normal(vertices: &[[f32; 3]; 3]) -> [f32; 3] {
    let v0 = vertices[0];
    let v1 = vertices[1];
    let v2 = vertices[2];

    let edge_a = [v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2]];
    let edge_b = [v2[0] - v0[0], v2[1] - v0[1], v2[2] - v0[2]];

    let nx = edge_a[1] * edge_b[2] - edge_a[2] * edge_b[1];
    let ny = edge_a[2] * edge_b[0] - edge_a[0] * edge_b[2];
    let nz = edge_a[0] * edge_b[1] - edge_a[1] * edge_b[0];

    let len = (nx * nx + ny * ny + nz * nz).sqrt();
    if len > 1e-10 {
        [nx / len, ny / len, nz / len]
    } else {
        [0.0, 0.0, 1.0]
    }
}

/// Remove degenerate and invalid triangles from a mesh
///
/// Returns a new vector containing only valid triangles
pub fn remove_degenerate(triangles: Vec<Triangle>) -> Vec<Triangle> {
    triangles
        .into_iter()
        .filter(|tri| !has_invalid_coords(tri) && !is_degenerate(tri))
        .collect()
}

/// Validate, fix, and clean a mesh in one pass
///
/// 1. Validates the mesh and reports issues
/// 2. Fixes normals on all triangles
/// 3. Removes degenerate/invalid triangles
///
/// Returns the cleaned mesh and validation report
pub fn validate_and_fix(mut triangles: Vec<Triangle>) -> (Vec<Triangle>, ValidationResult) {
    let report = validate_mesh(&triangles);
    fix_normals(&mut triangles);
    let cleaned = remove_degenerate(triangles);
    (cleaned, report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_triangle(v0: [f32; 3], v1: [f32; 3], v2: [f32; 3]) -> Triangle {
        Triangle {
            vertices: [v0, v1, v2],
            normal: calculate_normal(&[v0, v1, v2]),
        }
    }

    #[test]
    fn test_valid_triangle() {
        let tri = make_triangle([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);

        assert!(!has_invalid_coords(&tri));
        assert!(!is_degenerate(&tri));
        assert!(is_normal_valid(&tri.normal));
    }

    #[test]
    fn test_degenerate_triangle_collinear() {
        let tri = make_triangle([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]);

        assert!(is_degenerate(&tri));
    }

    #[test]
    fn test_degenerate_triangle_coincident() {
        let tri = make_triangle([0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [1.0, 1.0, 0.0]);

        assert!(is_degenerate(&tri));
    }

    #[test]
    fn test_invalid_coords_nan() {
        let tri = Triangle {
            vertices: [[f32::NAN, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normal: [0.0, 0.0, 1.0],
        };

        assert!(has_invalid_coords(&tri));
    }

    #[test]
    fn test_invalid_coords_inf() {
        let tri = Triangle {
            vertices: [[f32::INFINITY, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normal: [0.0, 0.0, 1.0],
        };

        assert!(has_invalid_coords(&tri));
    }

    #[test]
    fn test_validate_mesh() {
        let valid_tri = make_triangle([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        let degenerate_tri = make_triangle([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]);
        let another_valid = make_triangle([0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 2.0, 0.0]);
        let triangles = vec![valid_tri, degenerate_tri, another_valid];

        let result = validate_mesh(&triangles);

        assert_eq!(result.total, 3);
        assert_eq!(result.degenerate, 1);
        assert_eq!(result.invalid_coords, 0);
        assert!(result.is_valid());
    }

    #[test]
    fn test_remove_degenerate() {
        let valid_tri = make_triangle([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        let degenerate_tri = make_triangle([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]);
        let another_valid = make_triangle([0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 2.0, 0.0]);
        let triangles = vec![valid_tri, degenerate_tri, another_valid];

        let cleaned = remove_degenerate(triangles);

        assert_eq!(cleaned.len(), 2);
    }

    #[test]
    fn test_fix_normals() {
        let mut triangles = vec![Triangle {
            vertices: [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normal: [1.0, 0.0, 0.0],
        }];

        fix_normals(&mut triangles);

        assert!((triangles[0].normal[2] - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_validate_and_fix() {
        let valid_tri = make_triangle([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        let degenerate_tri = make_triangle([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]);
        let triangles = vec![valid_tri, degenerate_tri];

        let (cleaned, report) = validate_and_fix(triangles);

        assert_eq!(report.total, 2);
        assert_eq!(report.degenerate, 1);
        assert_eq!(cleaned.len(), 1);
    }

    #[test]
    fn test_triangle_area() {
        let vertices = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let area = triangle_area(&vertices);
        assert!((area - 0.5).abs() < 0.001);
    }
}
