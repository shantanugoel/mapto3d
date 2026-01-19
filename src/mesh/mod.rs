pub mod builder;
pub mod extrusion;
pub mod ribbon;
pub mod stl;
pub mod triangulation;
pub mod validation;

pub use builder::{MeshBuilder, Triangle};
pub use extrusion::{extrude_polygon, extrude_polygon_ex};
pub use ribbon::{extrude_ribbon, extrude_ribbon_ex};
pub use stl::write_stl;
pub use triangulation::{triangulate_polygon, triangulate_polygon_f64};
pub use validation::{
    ValidationResult, fix_normals, remove_degenerate, validate_and_fix, validate_mesh,
};
