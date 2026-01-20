//! Polygon union operations for merging overlapping road shapes
//!
//! Uses geo-clipper for robust boolean operations to create a single
//! manifold polygon from multiple overlapping road buffers.

use geo::{MultiPolygon, Polygon};
use geo_clipper::Clipper;

/// Union multiple polygons into a single MultiPolygon
///
/// Merges all overlapping polygons into a unified shape, eliminating
/// z-fighting and non-manifold geometry at intersections.
pub fn union_polygons(polygons: Vec<Polygon<f64>>) -> MultiPolygon<f64> {
    if polygons.is_empty() {
        return MultiPolygon::new(vec![]);
    }

    if polygons.len() == 1 {
        return MultiPolygon::new(polygons);
    }

    let precision_factor = 1000.0;

    let mut result = MultiPolygon::new(vec![polygons[0].clone()]);

    for polygon in polygons.into_iter().skip(1) {
        result = result.union(&polygon, precision_factor);
    }

    result
}

/// Union polygons in batches for better performance with large datasets
///
/// Processes polygons in groups to reduce complexity of individual
/// union operations, then merges the batch results.
pub fn union_polygons_batched(polygons: Vec<Polygon<f64>>, batch_size: usize) -> MultiPolygon<f64> {
    if polygons.is_empty() {
        return MultiPolygon::new(vec![]);
    }

    if polygons.len() <= batch_size {
        return union_polygons(polygons);
    }

    let precision_factor = 1000.0;
    let mut batch_results: Vec<MultiPolygon<f64>> = Vec::new();

    for chunk in polygons.chunks(batch_size) {
        let batch_union = union_polygons(chunk.to_vec());
        batch_results.push(batch_union);
    }

    let mut final_result = batch_results.remove(0);
    for batch in batch_results {
        for polygon in batch.0 {
            final_result = final_result.union(&polygon, precision_factor);
        }
    }

    final_result
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
