pub mod buffer;
pub mod projection;
pub mod scaling;
pub mod simplify;
pub mod union;

pub use buffer::{buffer_polyline, BufferConfig};
pub use projection::Projector;
pub use scaling::{Bounds, Scaler};
pub use simplify::{
    calculate_epsilon_meters, calculate_min_segment_length, filter_short_segments,
    simplify_for_mesh,
};
pub use union::union_polygons_batched;
