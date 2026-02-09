//! Polygon union operations for merging overlapping road shapes
//!
//! Uses geo-clipper for robust boolean operations to create a single
//! manifold polygon from multiple overlapping road buffers.

use geo::{MultiPolygon, Polygon};
use geo_clipper::Clipper;
use crate::geometry::CLIPPER_PRECISION_FACTOR;

pub fn union_polygons(polygons: Vec<Polygon<f64>>) -> MultiPolygon<f64> {
    union_polygons_batched(polygons, 64)
}

/// Union polygons in batches for better performance with large datasets
///
/// Processes polygons using a binary tree merge strategy to keep individual
/// operations as simple as possible.
pub fn union_polygons_batched(polygons: Vec<Polygon<f64>>, batch_size: usize) -> MultiPolygon<f64> {
    if polygons.is_empty() {
        return MultiPolygon::new(vec![]);
    }

    let precision_factor = CLIPPER_PRECISION_FACTOR;

    // First, process into initial MultiPolygons in chunks
    let mut current_level: Vec<MultiPolygon<f64>> = polygons
        .chunks(batch_size)
        .map(|chunk| {
            let mut batch_union = MultiPolygon::new(vec![chunk[0].clone()]);
            for poly in chunk.iter().skip(1) {
                batch_union = batch_union.union(poly, precision_factor);
            }
            batch_union
        })
        .collect();

    // Binary tree merge of the MultiPolygons
    while current_level.len() > 1 {
        let mut next_level = Vec::with_capacity((current_level.len() + 1) / 2);
        for chunk in current_level.chunks(2) {
            if chunk.len() == 2 {
                let mut merged = chunk[0].clone();
                for poly in &chunk[1].0 {
                    merged = merged.union(poly, precision_factor);
                }
                next_level.push(merged);
            } else {
                next_level.push(chunk[0].clone());
            }
        }
        current_level = next_level;
    }

    current_level.remove(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo::polygon;

    #[test]
    fn test_union_empty() {
        let result = union_polygons(vec![]);
        assert!(result.0.is_empty());
    }

    #[test]
    fn test_union_single_polygon() {
        let poly = polygon![
            (x: 0.0, y: 0.0),
            (x: 10.0, y: 0.0),
            (x: 10.0, y: 10.0),
            (x: 0.0, y: 10.0),
        ];
        let result = union_polygons(vec![poly.clone()]);
        assert_eq!(result.0.len(), 1);
    }

    #[test]
    fn test_union_overlapping_polygons() {
        let poly1 = polygon![
            (x: 0.0, y: 0.0),
            (x: 10.0, y: 0.0),
            (x: 10.0, y: 10.0),
            (x: 0.0, y: 10.0),
        ];
        let poly2 = polygon![
            (x: 5.0, y: 5.0),
            (x: 15.0, y: 5.0),
            (x: 15.0, y: 15.0),
            (x: 5.0, y: 15.0),
        ];

        let result = union_polygons(vec![poly1, poly2]);
        assert!(!result.0.is_empty(), "Should produce merged polygon");
    }

    #[test]
    fn test_union_non_overlapping_polygons() {
        let poly1 = polygon![
            (x: 0.0, y: 0.0),
            (x: 5.0, y: 0.0),
            (x: 5.0, y: 5.0),
            (x: 0.0, y: 5.0),
        ];
        let poly2 = polygon![
            (x: 10.0, y: 10.0),
            (x: 15.0, y: 10.0),
            (x: 15.0, y: 15.0),
            (x: 10.0, y: 15.0),
        ];

        let result = union_polygons(vec![poly1, poly2]);
        assert_eq!(
            result.0.len(),
            2,
            "Non-overlapping should remain as 2 polygons"
        );
    }

    #[test]
    fn test_union_batched() {
        let polygons: Vec<Polygon<f64>> = (0..10)
            .map(|i| {
                let offset = i as f64 * 5.0;
                polygon![
                    (x: offset, y: 0.0),
                    (x: offset + 10.0, y: 0.0),
                    (x: offset + 10.0, y: 10.0),
                    (x: offset, y: 10.0),
                ]
            })
            .collect();

        let result = union_polygons_batched(polygons, 3);
        assert!(!result.0.is_empty());
    }
}
