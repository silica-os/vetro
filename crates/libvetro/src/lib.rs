//! Vetro, Silica's native system toolkit: the shared library (`libvetro.so`).
//!
//! The modules follow the plan (silica-os/silica-desktop#254): `core` (object model, layout,
//! events), `amalfi` (tokens and components), `text`, `render`, `platform` (Wayland) and
//! `services` (system clients), with the C ABI in `ffi`. Every module is usable headless:
//! a window can render into a Skia raster surface for tests.

pub mod amalfi;
pub mod color;
pub mod core;
pub mod geom;
pub mod motion;
pub mod render;
pub mod shape;
pub mod style;
pub mod text;
