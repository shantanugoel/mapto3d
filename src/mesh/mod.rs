pub mod builder;
pub mod extrusion;
pub mod stl;
pub mod triangulation;
pub mod validation;

pub use builder::Triangle;
pub use extrusion::{extrude_polygon, extrude_polygon_ex};
pub use stl::write_stl;
pub use validation::validate_and_fix;
