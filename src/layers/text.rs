use crate::mesh::{Triangle, extrude_ribbon};

/// Simple stroke font for 3D text rendering
/// Each character is defined as a series of strokes (line segments)
/// Characters are on a 5x7 grid for consistency

pub struct TextRenderer {
    pub char_width: f32,
    pub char_height: f32,
    pub char_spacing: f32,
    pub stroke_width: f32,
    pub extrude_height: f32,
}

impl Default for TextRenderer {
    fn default() -> Self {
        Self {
            char_width: 5.0,
            char_height: 7.0,
            char_spacing: 1.5,
            stroke_width: 0.8,
            extrude_height: 0.6,
        }
    }
}

impl TextRenderer {
    pub fn with_scale(mut self, scale: f32) -> Self {
        self.char_width *= scale;
        self.char_height *= scale;
        self.char_spacing *= scale;
        self.stroke_width *= scale;
        self.extrude_height *= scale;
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
                    let ribbon = extrude_ribbon(&points, self.stroke_width, self.extrude_height, z);
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
}

/// Get stroke definitions for a character
/// Each stroke is a polyline of (x, y) points on a 5x7 grid
/// Origin (0,0) is bottom-left
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
    fn test_text_width() {
        let renderer = TextRenderer::default();
        let width = renderer.text_width("AB");
        assert!((width - 11.5).abs() < 0.01); // 5 + 1.5 + 5
    }

    #[test]
    fn test_render_single_char() {
        let renderer = TextRenderer::default();
        let triangles = renderer.render_text("A", 0.0, 0.0, 0.0);
        assert!(!triangles.is_empty());
    }
}
