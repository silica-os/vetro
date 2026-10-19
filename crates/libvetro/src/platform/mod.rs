//! The platform: the Wayland client and the input mapping.

pub mod keys;
pub mod wayland;

pub use wayland::{PlatformError, run};
