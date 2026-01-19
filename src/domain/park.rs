#[derive(Debug, Clone)]
pub struct ParkPolygon {
    pub outer: Vec<(f64, f64)>,
}

impl ParkPolygon {
    pub fn new(outer: Vec<(f64, f64)>) -> Self {
        Self { outer }
    }

    pub fn is_valid(&self) -> bool {
        self.outer.len() >= 3
    }
}
