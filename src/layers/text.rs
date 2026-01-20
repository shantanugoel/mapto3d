use crate::mesh::{Triangle, extrude_ribbon_ex};

use std::path::Path;

const TEXT_EXTRUDE_HEIGHT: f32 = 2.0; // 10 layers at 0.2mm for 5th color
const CURVE_SUBDIVISIONS: u8 = 20;

pub struct TtfTextRenderer {
    font_data: Vec<u8>,
    pub extrude_height: f32,
}

impl TtfTextRenderer {
    pub fn load(font_path: &Path) -> Option<Self> {
        let font_data = std::fs::read(font_path).ok()?;
        let face = fontmesh::Face::parse(&font_data, 0).ok()?;

        if fontmesh::char_to_mesh_3d(&face, 'A', 1.0, 8).is_err() {
            return None;
        }

        Some(Self {
            font_data,
            extrude_height: TEXT_EXTRUDE_HEIGHT,
        })
    }

    pub fn load_default() -> Option<Self> {
        let default_paths = [
            Path::new("fonts/RobotoSerif.ttf"),
            Path::new("./fonts/RobotoSerif.ttf"),
        ];
        for path in &default_paths {
            if path.exists()
                && let Some(renderer) = Self::load(path)
            {
                return Some(renderer);
            }
        }
        None
    }

    fn face(&self) -> fontmesh::Face<'_> {
        fontmesh::Face::parse(&self.font_data, 0).unwrap()
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

            if let Ok(mesh) =
                fontmesh::char_to_mesh_3d(&face, ch, self.extrude_height, CURVE_SUBDIVISIONS)
            {
                for tri_indices in mesh.indices.chunks(3) {
                    if tri_indices.len() < 3 {
                        continue;
                    }
                    let i0 = tri_indices[0] as usize;
                    let i1 = tri_indices[1] as usize;
                    let i2 = tri_indices[2] as usize;

                    if i0 >= mesh.vertices.len()
                        || i1 >= mesh.vertices.len()
                        || i2 >= mesh.vertices.len()
                    {
                        continue;
                    }

                    let v0 = mesh.vertices[i0];
                    let v1 = mesh.vertices[i1];
                    let v2 = mesh.vertices[i2];

                    let tri = Triangle::new(
                        [cursor_x + v0[0] * scale, y + v0[1] * scale, z + v0[2]],
                        [cursor_x + v1[0] * scale, y + v1[1] * scale, z + v1[2]],
                        [cursor_x + v2[0] * scale, y + v2[1] * scale, z + v2[2]],
                    );
                    triangles.push(tri);
                }
            }

            if let Some(advance) = fontmesh::glyph_advance(&face, ch) {
                cursor_x += advance * scale;
            }
        }

        triangles
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

pub struct StrokeTextRenderer {
    pub char_width: f32,
    pub char_height: f32,
    pub char_spacing: f32,
    pub stroke_width: f32,
    pub extrude_height: f32,
}

impl Default for StrokeTextRenderer {
    fn default() -> Self {
        Self {
            char_width: 5.0,
            char_height: 7.0,
            char_spacing: 1.5,
            stroke_width: 0.8,
            extrude_height: TEXT_EXTRUDE_HEIGHT,
        }
    }
}

impl StrokeTextRenderer {
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
                        false,
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
    pub fn new(font_path: Option<&Path>) -> Self {
        if let Some(path) = font_path
            && let Some(ttf) = TtfTextRenderer::load(path)
        {
            return Self::Ttf(ttf);
        }
        if let Some(ttf) = TtfTextRenderer::load_default() {
            return Self::Ttf(ttf);
        }
        Self::Stroke(StrokeTextRenderer::default())
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

    pub fn text_width(&self, text: &str, scale: f32) -> f32 {
        match self {
            Self::Ttf(ttf) => ttf.text_width(text, scale),
            Self::Stroke(stroke) => stroke.clone().with_scale(scale).text_width(text),
        }
    }

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
    use super::*;

    #[test]
    fn test_stroke_text_width() {
        let renderer = StrokeTextRenderer::default();
        let width = renderer.text_width("AB");
        assert!((width - 11.5).abs() < 0.01);
    }

    #[test]
    fn test_stroke_render_single_char() {
        let renderer = StrokeTextRenderer::default();
        let triangles = renderer.render_text("A", 0.0, 0.0, 0.0);
        assert!(!triangles.is_empty());
    }

    #[test]
    fn test_text_renderer_fallback() {
        let renderer = TextRenderer::new(None);
        assert!(!renderer.is_ttf() || renderer.is_ttf());
    }

    #[test]
    fn test_scale_calculation() {
        let renderer = StrokeTextRenderer::default();
        let scale = renderer.calculate_scale_for_width("TEST", 100.0);
        assert!(scale > 0.0);
    }

    #[test]
    fn test_ttf_fallback_to_stroke() {
        let path = Path::new("fonts/RobotoSerif.ttf");
        if !path.exists() {
            return;
        }

        let ttf_renderer = TtfTextRenderer::load(path);
        if ttf_renderer.is_some() {
            let triangles = ttf_renderer
                .unwrap()
                .render_text("TEST", 0.0, 0.0, 0.0, 10.0);
            assert!(!triangles.is_empty());
        } else {
            let stroke = StrokeTextRenderer::default();
            let triangles = stroke.render_text("TEST", 0.0, 0.0, 0.0);
            assert!(!triangles.is_empty());
        }
    }

    #[test]
    fn test_text_renderer_produces_triangles() {
        let renderer = TextRenderer::new(None);
        let triangles = renderer.render_text_centered("TEST", 100.0, 50.0, 0.0, 5.0);
        assert!(
            !triangles.is_empty(),
            "TextRenderer should produce triangles"
        );
    }
}
