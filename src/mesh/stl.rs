use super::Triangle;
use anyhow::{Context, Result};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

/// Write triangles to a binary STL file
///
/// Binary STL format:
/// - 80 byte header
/// - 4 byte u32 triangle count (little endian)
/// - For each triangle:
///   - 3 x f32 normal (12 bytes)
///   - 3 x 3 x f32 vertices (36 bytes)
///   - 2 byte attribute (usually 0)
///
/// # Arguments
/// * `path` - Output file path
/// * `triangles` - Triangles to write
pub fn write_stl(path: &Path, triangles: &[Triangle]) -> Result<()> {
    let file = File::create(path)
        .with_context(|| format!("Failed to create STL file: {}", path.display()))?;
    let mut writer = BufWriter::new(file);

    let header: [u8; 80] =
        *b"mapto3d - City Map STL Generator                                                ";
    writer.write_all(&header)?;

    // Triangle count (u32, little endian)
    let count = triangles.len() as u32;
    writer.write_all(&count.to_le_bytes())?;

    // Write each triangle
    for tri in triangles {
        // Normal (3 x f32)
        for &n in &tri.normal {
            writer.write_all(&n.to_le_bytes())?;
        }

        // Vertices (3 vertices x 3 coords x f32)
        for vertex in &tri.vertices {
            for &coord in vertex {
                writer.write_all(&coord.to_le_bytes())?;
            }
        }

        // Attribute byte count (2 bytes, usually 0)
        writer.write_all(&[0u8, 0u8])?;
    }

    writer.flush()?;

    Ok(())
}

/// Get the file size of an STL with the given number of triangles
pub fn estimate_stl_size(triangle_count: usize) -> usize {
    // 80 (header) + 4 (count) + triangles * (12 normal + 36 vertices + 2 attribute)
    80 + 4 + triangle_count * 50
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_write_stl() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.stl");

        let triangles = vec![
            Triangle::new([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
            Triangle::new([0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
        ];

        write_stl(&path, &triangles).unwrap();

        // Check file exists and has correct size
        let metadata = fs::metadata(&path).unwrap();
        assert_eq!(metadata.len(), estimate_stl_size(2) as u64);
    }

    #[test]
    fn test_estimate_size() {
        // Empty STL: 80 + 4 = 84 bytes
        assert_eq!(estimate_stl_size(0), 84);
        // 1 triangle: 84 + 50 = 134 bytes
        assert_eq!(estimate_stl_size(1), 134);
    }
}
