pub mod builder;
pub mod extrusion;
pub mod ribbon;
pub mod stl;
pub mod triangulation;

pub use builder::{MeshBuilder, Triangle};
pub use extrusion::extrude_polygon;
pub use ribbon::extrude_ribbon;
pub use stl::write_stl;
pub use triangulation::{triangulate_polygon, triangulate_polygon_f64};
