pub mod buffer;
pub mod clip;
pub mod projection;
pub mod rings;
pub mod scaling;
pub mod simplify;
pub mod union;

pub use buffer::{BufferConfig, buffer_polyline};
pub use clip::{ClipRect, clip_polygon_to_rect};
pub use projection::Projector;
pub use rings::{RING_POINT_EPSILON, line_string_to_ring};
pub use scaling::{Bounds, Scaler};
pub use union::union_polygons_batched;

pub const CLIPPER_PRECISION_FACTOR: f64 = 1000.0;
