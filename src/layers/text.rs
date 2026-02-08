use crate::mesh::{Triangle, extrude_polygon_ex, extrude_ribbon_ex};

use geo::{CoordsIter, LineString, MultiPolygon, Polygon};
use geo_clipper::Clipper;

use std::path::Path;

const CURVE_SUBDIVISIONS: u8 = 20;
const EMBEDDED_ROBOTO_SERIF: &[u8] = include_bytes!("../../fonts/RobotoSerif.ttf");
const CONTOUR_POINT_EPSILON: f32 = 1e-5;
const CLIPPER_PRECISION_FACTOR: f64 = 1000.0;

pub struct TtfTextRenderer {
    font_data: Vec<u8>,
    pub extrude_height: f32,
}

impl TtfTextRenderer {
    pub fn load(font_path: &Path, extrude_height: f32) -> Option<Self> {
        let font_data = std::fs::read(font_path).ok()?;
        Self::load_from_bytes(font_data, extrude_height)
    }

    pub fn load_default(extrude_height: f32) -> Option<Self> {
        let default_paths = [
            Path::new("fonts/RobotoSerif.ttf"),
            Path::new("./fonts/RobotoSerif.ttf"),
        ];
        for path in &default_paths {
            if path.exists()
                && let Some(renderer) = Self::load(path, extrude_height)
            {
                return Some(renderer);
            }
        }
        Self::load_from_bytes(EMBEDDED_ROBOTO_SERIF.to_vec(), extrude_height)
    }

    fn face(&self) -> fontmesh::Face<'_> {
        fontmesh::Face::parse(&self.font_data, 0).unwrap()
    }

    fn load_from_bytes(font_data: Vec<u8>, extrude_height: f32) -> Option<Self> {
        let face = fontmesh::Face::parse(&font_data, 0).ok()?;
        if fontmesh::char_to_mesh_3d(&face, 'A', 1.0, 8).is_err() {
            return None;
        }

        Some(Self {
            font_data,
            extrude_height,
        })
    }

    pub fn text_width(&self, text: &str, scale: f32) -> f32 {
        let face = self.face();
        let mut width = 0.0;
        for ch in text.chars() {
            if let Some(advance) = fontmesh::glyph_advance(&face, ch) {
                width += advance * scale;
            }
        }
        width
    }

    pub fn render_text(&self, text: &str, x: f32, y: f32, z: f32, scale: f32) -> Vec<Triangle> {
        let face = self.face();
        let mut triangles = Vec::new();
        let mut cursor_x = x;

        for ch in text.chars() {
            if ch == ' ' {
                if let Some(advance) = fontmesh::glyph_advance(&face, ch) {
                    cursor_x += advance * scale;
                } else {
                    cursor_x += 0.3 * scale;
                }
                continue;
            }

            if let Some(glyph_triangles) = self.render_glyph(&face, ch, cursor_x, y, z, scale) {
                triangles.extend(glyph_triangles);
            }

            if let Some(advance) = fontmesh::glyph_advance(&face, ch) {
                cursor_x += advance * scale;
            }
        }

        triangles
    }

    fn render_glyph(
        &self,
        face: &fontmesh::Face<'_>,
        ch: char,
        cursor_x: f32,
        y: f32,
        z: f32,
        scale: f32,
    ) -> Option<Vec<Triangle>> {
        let glyph = fontmesh::Glyph::new(face, ch).ok()?;
        let outline = glyph
            .with_subdivisions(CURVE_SUBDIVISIONS)
            .to_outline()
            .ok()?;

        let mut rings: Vec<GlyphRing> = outline
            .contours
            .iter()
            .filter_map(|contour| contour_to_ring(contour, cursor_x, y, scale))
            .filter_map(|points| {
                let polygon = ring_to_polygon(&points)?;
                Some(GlyphRing {
                    area_abs: signed_area(&points).abs() as f64,
                    polygon,
                    parent: None,
                    depth: 0,
                })
            })
            .collect();

        if rings.is_empty() {
            return None;
        }

        assign_ring_hierarchy(&mut rings);

        let max_depth = rings.iter().map(|ring| ring.depth).max().unwrap_or(0);
        let mut merged = MultiPolygon::new(vec![]);
        for depth in 0..=max_depth {
            let depth_polygons: Vec<Polygon<f64>> = rings
                .iter()
                .filter(|ring| ring.depth == depth)
                .map(|ring| ring.polygon.clone())
                .collect();

            let Some(layer_union) = union_polygon_layer(depth_polygons) else {
                continue;
            };

            if depth % 2 == 0 {
                merged = union_multi_polygon(merged, layer_union);
            } else {
                merged = difference_multi_polygon(merged, layer_union);
            }
        }

        if merged.0.is_empty() {
            return None;
        }

        let z_top = z + self.extrude_height;
        let mut triangles = Vec::new();
        for polygon in merged.0 {
            let outer = line_string_to_ring(polygon.exterior(), false);
            if outer.len() < 3 {
                continue;
            }

            let holes: Vec<Vec<(f32, f32)>> = polygon
                .interiors()
                .iter()
                .map(|ring| line_string_to_ring(ring, true))
                .filter(|ring| ring.len() >= 3)
                .collect();

            triangles.extend(extrude_polygon_ex(&outer, &holes, z, z_top, true));
        }

        Some(triangles)
    }

    pub fn render_text_centered(
        &self,
        text: &str,
        center_x: f32,
        y: f32,
        z: f32,
        scale: f32,
    ) -> Vec<Triangle> {
        let width = self.text_width(text, scale);
        let start_x = center_x - width / 2.0;
        self.render_text(text, start_x, y, z, scale)
    }

    pub fn calculate_scale_for_width(&self, text: &str, target_width: f32) -> f32 {
        let face = self.face();
        let mut raw_width = 0.0;
        for ch in text.chars() {
            if let Some(advance) = fontmesh::glyph_advance(&face, ch) {
                raw_width += advance;
            }
        }
        if raw_width > 0.0 {
            target_width / raw_width
        } else {
            1.0
        }
    }
}

#[derive(Debug, Clone)]
struct GlyphRing {
    polygon: Polygon<f64>,
    area_abs: f64,
    parent: Option<usize>,
    depth: usize,
}

fn contour_to_ring(
    contour: &fontmesh::types::Contour,
    cursor_x: f32,
    y: f32,
    scale: f32,
) -> Option<Vec<(f32, f32)>> {
    if !contour.closed || contour.points.len() < 3 {
        return None;
    }

    let mut points = Vec::with_capacity(contour.points.len());
    for contour_point in &contour.points {
        let px = cursor_x + contour_point.point[0] * scale;
        let py = y + contour_point.point[1] * scale;
        let current = (px, py);

        if points
            .last()
            .is_some_and(|&last| points_are_close(last, current))
        {
            continue;
        }
        points.push(current);
    }

    if points.len() < 3 {
        return None;
    }

    if points_are_close(*points.first().unwrap(), *points.last().unwrap()) {
        points.pop();
    }
    points = clean_ring(points);
    if points.len() < 3 {
        return None;
    }

    Some(points)
}

fn ring_to_polygon(points: &[(f32, f32)]) -> Option<Polygon<f64>> {
    if points.len() < 3 {
        return None;
    }

    let mut coords: Vec<geo::Coord<f64>> = points
        .iter()
        .map(|&(x, y)| geo::coord! { x: x as f64, y: y as f64 })
        .collect();
    coords.push(*coords.first().unwrap());
    Some(Polygon::new(LineString::from(coords), vec![]))
}

fn assign_ring_hierarchy(rings: &mut [GlyphRing]) {
    let ring_count = rings.len();
    let mut parents = vec![None; ring_count];

    for i in 0..ring_count {
        let mut best_parent: Option<(usize, f64)> = None;

        for j in 0..ring_count {
            if i == j {
                continue;
            }

            if rings[j].area_abs <= rings[i].area_abs {
                continue;
            }

            if polygon_fully_contains(&rings[j].polygon, &rings[i].polygon) {
                match best_parent {
                    Some((_, best_area)) if rings[j].area_abs >= best_area => {}
                    _ => best_parent = Some((j, rings[j].area_abs)),
                }
            }
        }

        parents[i] = best_parent.map(|(index, _)| index);
    }

    let mut depths = vec![0usize; ring_count];
    for i in 0..ring_count {
        let mut depth = 0usize;
        let mut current = parents[i];
        let mut guard = 0usize;
        while let Some(parent_index) = current {
            depth += 1;
            current = parents[parent_index];
            guard += 1;
            if guard > ring_count {
                break;
            }
        }
        depths[i] = depth;
    }

    for i in 0..ring_count {
        rings[i].parent = parents[i];
        rings[i].depth = depths[i];
    }
}

fn polygon_fully_contains(container: &Polygon<f64>, candidate: &Polygon<f64>) -> bool {
    let residual = candidate.difference(container, CLIPPER_PRECISION_FACTOR);
    residual.0.is_empty()
}

fn line_string_to_ring(ring: &LineString<f64>, expect_clockwise: bool) -> Vec<(f32, f32)> {
    let mut points: Vec<(f32, f32)> = ring
        .coords_iter()
        .map(|coord| (coord.x as f32, coord.y as f32))
        .collect();

    if points.len() < 3 {
        return Vec::new();
    }

    if points_are_close(*points.first().unwrap(), *points.last().unwrap()) {
        points.pop();
    }

    points = clean_ring(points);
    if points.len() < 3 {
        return Vec::new();
    }

    orient_ring(points, expect_clockwise)
}

fn orient_ring(mut points: Vec<(f32, f32)>, expect_clockwise: bool) -> Vec<(f32, f32)> {
    if is_clockwise(&points) != expect_clockwise {
        points.reverse();
    }
    points
}

fn union_polygon_layer(polygons: Vec<Polygon<f64>>) -> Option<MultiPolygon<f64>> {
    if polygons.is_empty() {
        return None;
    }

    let mut merged = MultiPolygon::new(vec![polygons[0].clone()]);
    for polygon in polygons.into_iter().skip(1) {
        merged = merged.union(&polygon, CLIPPER_PRECISION_FACTOR);
    }
    Some(merged)
}

fn union_multi_polygon(mut base: MultiPolygon<f64>, layer: MultiPolygon<f64>) -> MultiPolygon<f64> {
    if base.0.is_empty() {
        return layer;
    }

    for polygon in layer.0 {
        base = base.union(&polygon, CLIPPER_PRECISION_FACTOR);
    }
    base
}

fn difference_multi_polygon(
    mut base: MultiPolygon<f64>,
    subtract: MultiPolygon<f64>,
) -> MultiPolygon<f64> {
    if base.0.is_empty() || subtract.0.is_empty() {
        return base;
    }

    for polygon in subtract.0 {
        base = base.difference(&polygon, CLIPPER_PRECISION_FACTOR);
    }
    base
}

fn clean_ring(points: Vec<(f32, f32)>) -> Vec<(f32, f32)> {
    let mut cleaned = Vec::with_capacity(points.len());
    for point in points {
        if cleaned
            .last()
            .is_some_and(|&previous| points_are_close(previous, point))
        {
            continue;
        }
        cleaned.push(point);
    }

    if cleaned.len() > 2 && points_are_close(*cleaned.first().unwrap(), *cleaned.last().unwrap()) {
        cleaned.pop();
    }

    cleaned
}

fn is_clockwise(points: &[(f32, f32)]) -> bool {
    signed_area(points) < 0.0
}

fn signed_area(points: &[(f32, f32)]) -> f32 {
    let mut area = 0.0;
    for i in 0..points.len() {
        let (x1, y1) = points[i];
        let (x2, y2) = points[(i + 1) % points.len()];
        area += x1 * y2 - x2 * y1;
    }
    area * 0.5
}

fn points_are_close(a: (f32, f32), b: (f32, f32)) -> bool {
    (a.0 - b.0).abs() < CONTOUR_POINT_EPSILON && (a.1 - b.1).abs() < CONTOUR_POINT_EPSILON
}

pub struct StrokeTextRenderer {
    pub char_width: f32,
    pub char_height: f32,
    pub char_spacing: f32,
    pub stroke_width: f32,
    pub extrude_height: f32,
}

impl StrokeTextRenderer {
    pub fn new(extrude_height: f32) -> Self {
        Self {
            char_width: 5.0,
            char_height: 7.0,
            char_spacing: 1.5,
            stroke_width: 0.8,
            extrude_height,
        }
    }

    pub fn with_scale(mut self, scale: f32) -> Self {
        self.char_width *= scale;
        self.char_height *= scale;
        self.char_spacing *= scale;
        self.stroke_width *= scale;
        self
    }

    pub fn text_width(&self, text: &str) -> f32 {
        let char_count = text.chars().count();
        if char_count == 0 {
            return 0.0;
        }
        (char_count as f32 * self.char_width) + ((char_count - 1) as f32 * self.char_spacing)
    }

    pub fn render_text(&self, text: &str, x: f32, y: f32, z: f32) -> Vec<Triangle> {
        let mut triangles = Vec::new();
        let mut cursor_x = x;

        for ch in text.chars() {
            let strokes = get_char_strokes(ch);
            for stroke in strokes {
                let points: Vec<(f32, f32)> = stroke
                    .iter()
                    .map(|&(sx, sy)| {
                        (
                            cursor_x + sx * (self.char_width / 5.0),
                            y + sy * (self.char_height / 7.0),
                        )
                    })
                    .collect();

                if points.len() >= 2 {
                    let ribbon = extrude_ribbon_ex(
                        &points,
                        self.stroke_width,
                        self.extrude_height,
                        z,
                        true,
                        true,
                    );
                    triangles.extend(ribbon);
                }
            }
            cursor_x += self.char_width + self.char_spacing;
        }

        triangles
    }

    pub fn render_text_centered(&self, text: &str, center_x: f32, y: f32, z: f32) -> Vec<Triangle> {
        let width = self.text_width(text);
        let start_x = center_x - width / 2.0;
        self.render_text(text, start_x, y, z)
    }

    pub fn calculate_scale_for_width(&self, text: &str, target_width: f32) -> f32 {
        let char_count = text.chars().count();
        if char_count == 0 {
            return 1.0;
        }
        let base_width = (char_count as f32 * 5.0) + ((char_count - 1) as f32 * 1.5);
        if base_width > 0.0 {
            target_width / base_width
        } else {
            1.0
        }
    }
}

pub enum TextRenderer {
    Ttf(TtfTextRenderer),
    Stroke(StrokeTextRenderer),
}

impl TextRenderer {
    pub fn new(font_path: Option<&Path>, extrude_height: f32) -> Self {
        if let Some(path) = font_path
            && let Some(ttf) = TtfTextRenderer::load(path, extrude_height)
        {
            return Self::Ttf(ttf);
        }
        if let Some(ttf) = TtfTextRenderer::load_default(extrude_height) {
            return Self::Ttf(ttf);
        }
        Self::Stroke(StrokeTextRenderer::new(extrude_height))
    }

    pub fn render_text_centered(
        &self,
        text: &str,
        center_x: f32,
        y: f32,
        z: f32,
        scale: f32,
    ) -> Vec<Triangle> {
        match self {
            Self::Ttf(ttf) => ttf.render_text_centered(text, center_x, y, z, scale),
            Self::Stroke(stroke) => {
                let scaled = stroke.clone().with_scale(scale);
                scaled.render_text_centered(text, center_x, y, z)
            }
        }
    }

    pub fn calculate_scale_for_width(&self, text: &str, target_width: f32) -> f32 {
        match self {
            Self::Ttf(ttf) => ttf.calculate_scale_for_width(text, target_width),
            Self::Stroke(stroke) => stroke.calculate_scale_for_width(text, target_width),
        }
    }

    #[cfg(test)]
    pub fn is_ttf(&self) -> bool {
        matches!(self, Self::Ttf(_))
    }
}

impl Clone for StrokeTextRenderer {
    fn clone(&self) -> Self {
        Self {
            char_width: self.char_width,
            char_height: self.char_height,
            char_spacing: self.char_spacing,
            stroke_width: self.stroke_width,
            extrude_height: self.extrude_height,
        }
    }
}

fn get_char_strokes(ch: char) -> Vec<Vec<(f32, f32)>> {
    match ch.to_ascii_uppercase() {
        'A' => vec![
            vec![(0.0, 0.0), (2.5, 7.0), (5.0, 0.0)],
            vec![(1.0, 3.0), (4.0, 3.0)],
        ],
        'B' => vec![
            vec![
                (0.0, 0.0),
                (0.0, 7.0),
                (3.5, 7.0),
                (5.0, 6.0),
                (5.0, 4.5),
                (3.5, 3.5),
                (0.0, 3.5),
            ],
            vec![(3.5, 3.5), (5.0, 2.5), (5.0, 1.0), (3.5, 0.0), (0.0, 0.0)],
        ],
        'C' => vec![vec![
            (5.0, 1.0),
            (4.0, 0.0),
            (1.0, 0.0),
            (0.0, 1.0),
            (0.0, 6.0),
            (1.0, 7.0),
            (4.0, 7.0),
            (5.0, 6.0),
        ]],
        'D' => vec![vec![
            (0.0, 0.0),
            (0.0, 7.0),
            (3.0, 7.0),
            (5.0, 5.5),
            (5.0, 1.5),
            (3.0, 0.0),
            (0.0, 0.0),
        ]],
        'E' => vec![
            vec![(5.0, 0.0), (0.0, 0.0), (0.0, 7.0), (5.0, 7.0)],
            vec![(0.0, 3.5), (4.0, 3.5)],
        ],
        'F' => vec![
            vec![(0.0, 0.0), (0.0, 7.0), (5.0, 7.0)],
            vec![(0.0, 3.5), (4.0, 3.5)],
        ],
        'G' => vec![vec![
            (5.0, 6.0),
            (4.0, 7.0),
            (1.0, 7.0),
            (0.0, 6.0),
            (0.0, 1.0),
            (1.0, 0.0),
            (4.0, 0.0),
            (5.0, 1.0),
            (5.0, 3.5),
            (2.5, 3.5),
        ]],
        'H' => vec![
            vec![(0.0, 0.0), (0.0, 7.0)],
            vec![(5.0, 0.0), (5.0, 7.0)],
            vec![(0.0, 3.5), (5.0, 3.5)],
        ],
        'I' => vec![
            vec![(1.0, 0.0), (4.0, 0.0)],
            vec![(2.5, 0.0), (2.5, 7.0)],
            vec![(1.0, 7.0), (4.0, 7.0)],
        ],
        'J' => vec![
            vec![(0.0, 1.0), (1.0, 0.0), (3.0, 0.0), (4.0, 1.0), (4.0, 7.0)],
            vec![(2.0, 7.0), (5.0, 7.0)],
        ],
        'K' => vec![
            vec![(0.0, 0.0), (0.0, 7.0)],
            vec![(5.0, 7.0), (0.0, 3.5), (5.0, 0.0)],
        ],
        'L' => vec![vec![(0.0, 7.0), (0.0, 0.0), (5.0, 0.0)]],
        'M' => vec![vec![
            (0.0, 0.0),
            (0.0, 7.0),
            (2.5, 4.0),
            (5.0, 7.0),
            (5.0, 0.0),
        ]],
        'N' => vec![vec![(0.0, 0.0), (0.0, 7.0), (5.0, 0.0), (5.0, 7.0)]],
        'O' => vec![vec![
            (1.0, 0.0),
            (0.0, 1.0),
            (0.0, 6.0),
            (1.0, 7.0),
            (4.0, 7.0),
            (5.0, 6.0),
            (5.0, 1.0),
            (4.0, 0.0),
            (1.0, 0.0),
        ]],
        'P' => vec![vec![
            (0.0, 0.0),
            (0.0, 7.0),
            (4.0, 7.0),
            (5.0, 6.0),
            (5.0, 4.0),
            (4.0, 3.0),
            (0.0, 3.0),
        ]],
        'Q' => vec![
            vec![
                (1.0, 0.0),
                (0.0, 1.0),
                (0.0, 6.0),
                (1.0, 7.0),
                (4.0, 7.0),
                (5.0, 6.0),
                (5.0, 1.0),
                (4.0, 0.0),
                (1.0, 0.0),
            ],
            vec![(3.0, 2.0), (5.5, -0.5)],
        ],
        'R' => vec![
            vec![
                (0.0, 0.0),
                (0.0, 7.0),
                (4.0, 7.0),
                (5.0, 6.0),
                (5.0, 4.0),
                (4.0, 3.0),
                (0.0, 3.0),
            ],
            vec![(2.5, 3.0), (5.0, 0.0)],
        ],
        'S' => vec![vec![
            (5.0, 6.0),
            (4.0, 7.0),
            (1.0, 7.0),
            (0.0, 6.0),
            (0.0, 4.5),
            (1.0, 3.5),
            (4.0, 3.5),
            (5.0, 2.5),
            (5.0, 1.0),
            (4.0, 0.0),
            (1.0, 0.0),
            (0.0, 1.0),
        ]],
        'T' => vec![vec![(0.0, 7.0), (5.0, 7.0)], vec![(2.5, 7.0), (2.5, 0.0)]],
        'U' => vec![vec![
            (0.0, 7.0),
            (0.0, 1.0),
            (1.0, 0.0),
            (4.0, 0.0),
            (5.0, 1.0),
            (5.0, 7.0),
        ]],
        'V' => vec![vec![(0.0, 7.0), (2.5, 0.0), (5.0, 7.0)]],
        'W' => vec![vec![
            (0.0, 7.0),
            (1.0, 0.0),
            (2.5, 4.0),
            (4.0, 0.0),
            (5.0, 7.0),
        ]],
        'X' => vec![vec![(0.0, 0.0), (5.0, 7.0)], vec![(0.0, 7.0), (5.0, 0.0)]],
        'Y' => vec![
            vec![(0.0, 7.0), (2.5, 3.5), (5.0, 7.0)],
            vec![(2.5, 3.5), (2.5, 0.0)],
        ],
        'Z' => vec![vec![(0.0, 7.0), (5.0, 7.0), (0.0, 0.0), (5.0, 0.0)]],
        '0' => vec![
            vec![
                (1.0, 0.0),
                (0.0, 1.0),
                (0.0, 6.0),
                (1.0, 7.0),
                (4.0, 7.0),
                (5.0, 6.0),
                (5.0, 1.0),
                (4.0, 0.0),
                (1.0, 0.0),
            ],
            vec![(1.0, 1.0), (4.0, 6.0)],
        ],
        '1' => vec![
            vec![(1.0, 5.0), (2.5, 7.0), (2.5, 0.0)],
            vec![(1.0, 0.0), (4.0, 0.0)],
        ],
        '2' => vec![vec![
            (0.0, 6.0),
            (1.0, 7.0),
            (4.0, 7.0),
            (5.0, 6.0),
            (5.0, 4.5),
            (0.0, 0.0),
            (5.0, 0.0),
        ]],
        '3' => vec![
            vec![
                (0.0, 6.0),
                (1.0, 7.0),
                (4.0, 7.0),
                (5.0, 6.0),
                (5.0, 4.5),
                (4.0, 3.5),
                (2.0, 3.5),
            ],
            vec![
                (4.0, 3.5),
                (5.0, 2.5),
                (5.0, 1.0),
                (4.0, 0.0),
                (1.0, 0.0),
                (0.0, 1.0),
            ],
        ],
        '4' => vec![vec![(4.0, 0.0), (4.0, 7.0), (0.0, 2.5), (5.0, 2.5)]],
        '5' => vec![vec![
            (5.0, 7.0),
            (0.0, 7.0),
            (0.0, 4.0),
            (4.0, 4.0),
            (5.0, 3.0),
            (5.0, 1.0),
            (4.0, 0.0),
            (1.0, 0.0),
            (0.0, 1.0),
        ]],
        '6' => vec![vec![
            (4.0, 7.0),
            (1.0, 7.0),
            (0.0, 6.0),
            (0.0, 1.0),
            (1.0, 0.0),
            (4.0, 0.0),
            (5.0, 1.0),
            (5.0, 3.0),
            (4.0, 4.0),
            (0.0, 4.0),
        ]],
        '7' => vec![vec![(0.0, 7.0), (5.0, 7.0), (2.0, 0.0)]],
        '8' => vec![
            vec![
                (1.0, 3.5),
                (0.0, 4.5),
                (0.0, 6.0),
                (1.0, 7.0),
                (4.0, 7.0),
                (5.0, 6.0),
                (5.0, 4.5),
                (4.0, 3.5),
                (1.0, 3.5),
            ],
            vec![
                (1.0, 3.5),
                (0.0, 2.5),
                (0.0, 1.0),
                (1.0, 0.0),
                (4.0, 0.0),
                (5.0, 1.0),
                (5.0, 2.5),
                (4.0, 3.5),
            ],
        ],
        '9' => vec![vec![
            (1.0, 0.0),
            (4.0, 0.0),
            (5.0, 1.0),
            (5.0, 6.0),
            (4.0, 7.0),
            (1.0, 7.0),
            (0.0, 6.0),
            (0.0, 4.0),
            (1.0, 3.0),
            (5.0, 3.0),
        ]],
        '.' => vec![vec![
            (2.0, 0.0),
            (3.0, 0.0),
            (3.0, 1.0),
            (2.0, 1.0),
            (2.0, 0.0),
        ]],
        ',' => vec![vec![(2.5, 1.0), (2.5, 0.0), (1.5, -1.0)]],
        '-' => vec![vec![(1.0, 3.5), (4.0, 3.5)]],
        '/' => vec![vec![(0.0, 0.0), (5.0, 7.0)]],
        ':' => vec![
            vec![(2.0, 2.0), (3.0, 2.0), (3.0, 3.0), (2.0, 3.0), (2.0, 2.0)],
            vec![(2.0, 5.0), (3.0, 5.0), (3.0, 6.0), (2.0, 6.0), (2.0, 5.0)],
        ],
        '°' => vec![vec![
            (1.5, 6.0),
            (1.0, 6.5),
            (1.0, 7.0),
            (1.5, 7.5),
            (2.5, 7.5),
            (3.0, 7.0),
            (3.0, 6.5),
            (2.5, 6.0),
            (1.5, 6.0),
        ]],
        ' ' => vec![],
        _ => vec![vec![
            (0.0, 0.0),
            (5.0, 0.0),
            (5.0, 7.0),
            (0.0, 7.0),
            (0.0, 0.0),
        ]],
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn test_stroke_text_width() {
        let renderer = StrokeTextRenderer::new(4.4);
        let width = renderer.text_width("AB");
        assert!((width - 11.5).abs() < 0.01);
    }

    #[test]
    fn test_stroke_render_single_char() {
        let renderer = StrokeTextRenderer::new(4.4);
        let triangles = renderer.render_text("A", 0.0, 0.0, 0.0);
        assert!(!triangles.is_empty());
    }

    #[test]
    fn test_text_renderer_fallback() {
        let renderer = TextRenderer::new(None, 4.4);
        assert!(!renderer.is_ttf() || renderer.is_ttf());
    }

    #[test]
    fn test_scale_calculation() {
        let renderer = StrokeTextRenderer::new(4.4);
        let scale = renderer.calculate_scale_for_width("TEST", 100.0);
        assert!(scale > 0.0);
    }

    #[test]
    fn test_ttf_fallback_to_stroke() {
        let path = Path::new("fonts/RobotoSerif.ttf");
        if !path.exists() {
            return;
        }

        let ttf_renderer = TtfTextRenderer::load(path, 4.4);
        if let Some(ttf) = ttf_renderer {
            let triangles = ttf.render_text("TEST", 0.0, 0.0, 0.0, 10.0);
            assert!(!triangles.is_empty());
        } else {
            let stroke = StrokeTextRenderer::new(4.4);
            let triangles = stroke.render_text("TEST", 0.0, 0.0, 0.0);
            assert!(!triangles.is_empty());
        }
    }

    #[test]
    fn test_text_renderer_produces_triangles() {
        let renderer = TextRenderer::new(None, 4.4);
        let triangles = renderer.render_text_centered("TEST", 100.0, 50.0, 0.0, 5.0);
        assert!(
            !triangles.is_empty(),
            "TextRenderer should produce triangles"
        );
    }

    #[test]
    fn test_ttf_text_topology_monaco() {
        let renderer = TtfTextRenderer::load_default(4.4)
            .expect("Embedded/default TTF should load for topology test");
        let triangles = renderer.render_text_centered("MONACO", 100.0, 50.0, 0.0, 10.0);
        let (boundary_edges, non_manifold_edges) = edge_topology_counts(&triangles);
        assert_eq!(boundary_edges, 0);
        assert_eq!(non_manifold_edges, 0);
    }

    #[test]
    fn test_ttf_text_topology_coordinates() {
        let renderer = TtfTextRenderer::load_default(4.4)
            .expect("Embedded/default TTF should load for topology test");
        let triangles = renderer.render_text_centered("43.7323N / 7.4277E", 100.0, 50.0, 0.0, 6.0);
        let (boundary_edges, non_manifold_edges) = edge_topology_counts(&triangles);
        assert_eq!(boundary_edges, 0);
        assert_eq!(non_manifold_edges, 0);
    }

    #[test]
    fn test_ttf_o_hole_not_filled() {
        let renderer =
            TtfTextRenderer::load_default(4.4).expect("Embedded/default TTF should load for test");
        let triangles = renderer.render_text_centered("O", 0.0, 0.0, 0.0, 10.0);
        assert!(!triangles.is_empty());

        let mut min_x = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        for triangle in &triangles {
            for vertex in &triangle.vertices {
                min_x = min_x.min(vertex[0]);
                max_x = max_x.max(vertex[0]);
                min_y = min_y.min(vertex[1]);
                max_y = max_y.max(vertex[1]);
            }
        }

        let center = ((min_x + max_x) * 0.5, (min_y + max_y) * 0.5);
        let mut center_covered_on_top = false;
        for triangle in &triangles {
            let is_top_face = triangle
                .vertices
                .iter()
                .all(|vertex| (vertex[2] - 4.4).abs() < 1e-3);
            if !is_top_face {
                continue;
            }

            let a = (triangle.vertices[0][0], triangle.vertices[0][1]);
            let b = (triangle.vertices[1][0], triangle.vertices[1][1]);
            let c = (triangle.vertices[2][0], triangle.vertices[2][1]);
            if point_in_triangle_2d(center, a, b, c) {
                center_covered_on_top = true;
                break;
            }
        }

        assert!(
            !center_covered_on_top,
            "Center of O should remain a hole on top surface"
        );
    }

    #[test]
    fn test_ttf_monaco_word_o_holes_not_filled() {
        let text = "MONACO";
        let text_z_top = 3.2;
        let renderer = TtfTextRenderer::load_default(text_z_top)
            .expect("Embedded/default TTF should load for MONACO hole regression");
        let face = renderer.face();

        let size_mm = 220.0;
        let target_width = size_mm * 0.75;
        let scale = renderer.calculate_scale_for_width(text, target_width);
        let center_x = size_mm * 0.5;
        let y = 12.0 * (size_mm / 220.0);
        let text_width = renderer.text_width(text, scale);
        let start_x = center_x - text_width * 0.5;
        let all_triangles = renderer.render_text(text, start_x, y, 0.0, scale);
        assert!(!all_triangles.is_empty());

        let mut cursor_x = start_x;
        let mut o_centers = Vec::new();
        for ch in text.chars() {
            if ch == 'O'
                && let Some(glyph_triangles) =
                    renderer.render_glyph(&face, ch, cursor_x, y, 0.0, scale)
            {
                let mut min_x = f32::INFINITY;
                let mut max_x = f32::NEG_INFINITY;
                let mut min_y = f32::INFINITY;
                let mut max_y = f32::NEG_INFINITY;
                for triangle in &glyph_triangles {
                    for vertex in &triangle.vertices {
                        min_x = min_x.min(vertex[0]);
                        max_x = max_x.max(vertex[0]);
                        min_y = min_y.min(vertex[1]);
                        max_y = max_y.max(vertex[1]);
                    }
                }
                o_centers.push(((min_x + max_x) * 0.5, (min_y + max_y) * 0.5));
            }

            if let Some(advance) = fontmesh::glyph_advance(&face, ch) {
                cursor_x += advance * scale;
            }
        }

        assert_eq!(o_centers.len(), 2, "MONACO should produce two O centers");

        for center in o_centers {
            assert!(
                !top_face_contains_point(&all_triangles, center, text_z_top),
                "MONACO O center should remain a hole on the top surface"
            );
        }
    }

    #[test]
    fn test_ring_hierarchy_detects_true_containment_only() {
        let outer = make_glyph_ring(&[(0.0, 0.0), (12.0, 0.0), (12.0, 12.0), (0.0, 12.0)]);
        let hole = make_glyph_ring(&[(3.0, 3.0), (9.0, 3.0), (9.0, 9.0), (3.0, 9.0)]);

        let mut rings = vec![outer, hole];
        assign_ring_hierarchy(&mut rings);

        assert_eq!(rings[0].parent, None);
        assert_eq!(rings[0].depth, 0);
        assert_eq!(rings[1].parent, Some(0));
        assert_eq!(rings[1].depth, 1);
    }

    #[test]
    fn test_ring_hierarchy_ignores_partial_overlap() {
        let stem = make_glyph_ring(&[(0.0, 0.0), (2.0, 0.0), (2.0, 8.0), (0.0, 8.0)]);
        let crossbar = make_glyph_ring(&[(1.0, 3.0), (7.0, 3.0), (7.0, 5.0), (1.0, 5.0)]);

        let mut rings = vec![stem, crossbar];
        assign_ring_hierarchy(&mut rings);

        assert_eq!(rings[0].parent, None);
        assert_eq!(rings[1].parent, None);
        assert_eq!(rings[0].depth, 0);
        assert_eq!(rings[1].depth, 0);
    }

    type QuantizedVertex = (i64, i64, i64);
    type QuantizedEdge = (QuantizedVertex, QuantizedVertex);

    fn edge_topology_counts(triangles: &[Triangle]) -> (usize, usize) {
        let mut counts: HashMap<QuantizedEdge, usize> = HashMap::new();

        for triangle in triangles {
            let vertices = triangle.vertices.map(quantize_vertex);
            for (a, b) in [(0, 1), (1, 2), (2, 0)] {
                let edge = ordered_edge(vertices[a], vertices[b]);
                *counts.entry(edge).or_insert(0) += 1;
            }
        }

        let boundary_edges = counts.values().filter(|&&count| count == 1).count();
        let non_manifold_edges = counts.values().filter(|&&count| count > 2).count();
        (boundary_edges, non_manifold_edges)
    }

    fn quantize_vertex(vertex: [f32; 3]) -> QuantizedVertex {
        const SCALE: f32 = 10_000.0;
        (
            (vertex[0] * SCALE).round() as i64,
            (vertex[1] * SCALE).round() as i64,
            (vertex[2] * SCALE).round() as i64,
        )
    }

    fn ordered_edge(a: QuantizedVertex, b: QuantizedVertex) -> QuantizedEdge {
        if a <= b { (a, b) } else { (b, a) }
    }

    fn point_in_triangle_2d(p: (f32, f32), a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> bool {
        fn sign(p1: (f32, f32), p2: (f32, f32), p3: (f32, f32)) -> f32 {
            (p1.0 - p3.0) * (p2.1 - p3.1) - (p2.0 - p3.0) * (p1.1 - p3.1)
        }

        let d1 = sign(p, a, b);
        let d2 = sign(p, b, c);
        let d3 = sign(p, c, a);
        let has_neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
        let has_pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
        !(has_neg && has_pos)
    }

    fn top_face_contains_point(triangles: &[Triangle], p: (f32, f32), top_z: f32) -> bool {
        triangles.iter().any(|triangle| {
            let is_top_face = triangle
                .vertices
                .iter()
                .all(|vertex| (vertex[2] - top_z).abs() < 1e-3);
            if !is_top_face {
                return false;
            }

            let a = (triangle.vertices[0][0], triangle.vertices[0][1]);
            let b = (triangle.vertices[1][0], triangle.vertices[1][1]);
            let c = (triangle.vertices[2][0], triangle.vertices[2][1]);
            point_in_triangle_2d(p, a, b, c)
        })
    }

    fn make_glyph_ring(points: &[(f32, f32)]) -> GlyphRing {
        let points_vec = points.to_vec();
        GlyphRing {
            area_abs: signed_area(&points_vec).abs() as f64,
            polygon: ring_to_polygon(&points_vec).expect("valid ring"),
            parent: None,
            depth: 0,
        }
    }
}
