#[derive(Debug, Clone)]
pub struct WaterPolygon {
    pub outer: Vec<(f64, f64)>,
    pub holes: Vec<Vec<(f64, f64)>>,
}

impl WaterPolygon {
    pub fn new(outer: Vec<(f64, f64)>) -> Self {
        Self {
            outer,
            holes: Vec::new(),
        }
    }

    pub fn with_holes(outer: Vec<(f64, f64)>, holes: Vec<Vec<(f64, f64)>>) -> Self {
        Self { outer, holes }
    }

    pub fn is_valid(&self) -> bool {
        self.outer.len() >= 3
    }
}
