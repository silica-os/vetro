//! Vetro's kit-parity harness: the Silica 1.0 "Amalfi" kit masters (silica-design JSON and
//! 2× PNG, copied into `kit/` by `cargo xtask sync-kit`) drive geometry and pixel tests, as
//! silica_ui's `test/amalfi` does: geometry exactly, pixels with a tolerance (Sketch
//! rasterises text with CoreText).

// A test harness: its `expect_*` functions fail the calling test by panicking, and
// serde_json's `value["key"]` returns Null for a missing key (it never panics).
#![allow(clippy::panic, clippy::indexing_slicing)]

pub mod kit;
pub mod scene;

pub use kit::{KitLayer, KitMaster};
pub use scene::{Scene, Tolerance, center_of};
