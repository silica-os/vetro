# Vetro

Silica's native system toolkit: a shared library (`libvetro.so`, C API `vetro.h`) that draws every
Silica 1.0 Amalfi component, talks directly to Ossido's Wayland protocols and Silica's system
services, and is used by the shell and every app through a safe Rust crate (`vetro-kit`) and a
declarative layer (`vetro`). Rust, Skia.

Design and plan: [silica-os/silica-desktop#254](https://github.com/silica-os/silica-desktop/issues/254).

Licensed under either of Apache-2.0 or MIT, at your option.
