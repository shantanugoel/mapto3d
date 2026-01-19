pub mod projection;
pub mod scaling;
pub mod simplify;

pub use projection::Projector;
pub use scaling::{Bounds, Scaler};
pub use simplify::{calculate_epsilon, simplify_polygon, simplify_polyline};
