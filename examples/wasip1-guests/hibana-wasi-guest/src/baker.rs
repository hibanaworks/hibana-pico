//! Baker Link Dev Rev1 guest helpers.
//!
//! These helpers are intentionally proof-specific. They wrap the narrow WASI
//! Preview 1 surface used by the Baker ChoreoFS LED demo without becoming a
//! generic guest appkit or authority layer.

pub mod device;
pub mod time;

pub use device::Led;
pub use time::sleep_ms;
