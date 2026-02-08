use geo::{LineString, Polygon};
use geo_clipper::Clipper;

use super::scaling::{Bounds, Scaler};

const CLIPPER_PRECISION_FACTOR: f64 = 1000.0;

#[derive(Debug, Clone, Copy)]
pub struct ClipRect {
    pub min_x: f32,
    pub max_x: f32,
    pub min_y: f32,
    pub max_y: f32,
}

impl ClipRect {
    pub fn from_bounds(bounds: &Bounds, scaler: &Scaler) -> Self {
        let corners = [
            scaler.scale(bounds.min_x, bounds.min_y),
            scaler.scale(bounds.min_x, bounds.max_y),
            scaler.scale(bounds.max_x, bounds.min_y),
            scaler.scale(bounds.max_x, bounds.max_y),
        ];

        let mut min_x = f32::MAX;
        let mut max_x = f32::MIN;
        let mut min_y = f32::MAX;
        let mut max_y = f32::MIN;

        for (x, y) in corners {
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_y = min_y.min(y);
            max_y = max_y.max(y);
        }

        Self {
            min_x,
            max_x,
            min_y,
            max_y,
        }
    }

    fn to_polygon(&self) -> Polygon<f64> {
        let coords = vec![
            (self.min_x as f64, self.min_y as f64),
            (self.max_x as f64, self.min_y as f64),
            (self.max_x as f64, self.max_y as f64),
            (self.min_x as f64, self.max_y as f64),
            (self.min_x as f64, self.min_y as f64),
        ];

        Polygon::new(LineString::from(coords), vec![])
    }
}

pub fn clip_polygon_to_rect(
    outer: &[(f32, f32)],
    holes: &[Vec<(f32, f32)>],
    rect: &ClipRect,
) -> Vec<Polygon<f64>> {
    if outer.len() < 3 {
        return Vec::new();
    }

    let subject = polygon_from_rings(outer, holes);
    let clip = rect.to_polygon();
    let clipped = subject.intersection(&clip, CLIPPER_PRECISION_FACTOR);
    clipped.0
}

fn polygon_from_rings(outer: &[(f32, f32)], holes: &[Vec<(f32, f32)>]) -> Polygon<f64> {
    let exterior = ring_to_linestring(outer);
    let interiors = holes
        .iter()
        .filter(|ring| ring.len() >= 3)
        .map(|ring| ring_to_linestring(ring))
        .collect();

    Polygon::new(exterior, interiors)
}

fn ring_to_linestring(ring: &[(f32, f32)]) -> LineString<f64> {
    let mut coords: Vec<(f64, f64)> = ring.iter().map(|&(x, y)| (x as f64, y as f64)).collect();
    if coords.len() >= 3 {
        if coords.first() != coords.last() {
            if let Some(first) = coords.first().copied() {
                coords.push(first);
            }
        }
    }
    LineString::from(coords)
}
