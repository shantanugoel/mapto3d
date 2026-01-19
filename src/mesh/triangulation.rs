use earcutr::earcut;

pub fn triangulate_polygon(outer: &[(f32, f32)], holes: &[Vec<(f32, f32)>]) -> Vec<usize> {
    if outer.len() < 3 {
        return Vec::new();
    }

    let mut vertices: Vec<f64> =
        Vec::with_capacity((outer.len() + holes.iter().map(|h| h.len()).sum::<usize>()) * 2);
    let mut hole_indices: Vec<usize> = Vec::with_capacity(holes.len());

    for &(x, y) in outer {
        vertices.push(x as f64);
        vertices.push(y as f64);
    }

    for hole in holes {
        hole_indices.push(vertices.len() / 2);
        for &(x, y) in hole {
            vertices.push(x as f64);
            vertices.push(y as f64);
        }
    }

    earcut(&vertices, &hole_indices, 2).unwrap_or_default()
}

pub fn triangulate_polygon_f64(outer: &[(f64, f64)], holes: &[Vec<(f64, f64)>]) -> Vec<usize> {
    if outer.len() < 3 {
        return Vec::new();
    }

    let mut vertices: Vec<f64> =
        Vec::with_capacity((outer.len() + holes.iter().map(|h| h.len()).sum::<usize>()) * 2);
    let mut hole_indices: Vec<usize> = Vec::with_capacity(holes.len());

    for &(x, y) in outer {
        vertices.push(x);
        vertices.push(y);
    }

    for hole in holes {
        hole_indices.push(vertices.len() / 2);
        for &(x, y) in hole {
            vertices.push(x);
            vertices.push(y);
        }
    }

    earcut(&vertices, &hole_indices, 2).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_triangulate_square() {
        let square = vec![(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
        let indices = triangulate_polygon(&square, &[]);
        assert_eq!(indices.len(), 6);
    }

    #[test]
    fn test_triangulate_empty() {
        let empty: Vec<(f32, f32)> = vec![];
        let indices = triangulate_polygon(&empty, &[]);
        assert!(indices.is_empty());
    }

    #[test]
    fn test_triangulate_with_hole() {
        let outer = vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        let hole = vec![(2.0, 2.0), (8.0, 2.0), (8.0, 8.0), (2.0, 8.0)];
        let indices = triangulate_polygon(&outer, &[hole]);
        assert!(!indices.is_empty());
        assert_eq!(indices.len() % 3, 0);
    }
}
