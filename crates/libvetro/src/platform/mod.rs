//! The platform: the Wayland client and the input mapping.

pub mod a11y;
pub mod keys;
pub mod registry;
pub mod wayland;

pub use wayland::{PlatformError, post, run, run_with, services, spawn, trim_memory};
