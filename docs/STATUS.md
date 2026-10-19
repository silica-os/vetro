# Vetro: status

Living document: what is done, the decisions taken, and what comes next. A successor picks up
from "Next" below. Plan: [silica-os/silica-desktop#254](https://github.com/silica-os/silica-desktop/issues/254).

Gate (real exit codes): `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`.

## Done

- Workspace (`crates/*`), workspace lints copied from silica-desktop (no-panic clippy lints;
  `clippy.toml` allows them in tests).
- `libvetro` (cdylib + rlib, library name `vetro_lib`):
  - `geom`, `color`, `shape` (smooth corners, port of silica_ui `AmalfiSmoothBorder`),
    `style` (kit layer styles: fills incl. gradients, borders inside/centre/outside and
    dashed, CSS-blur drop and inner shadows, backdrop blur), `render::glass` (nested glass:
    SkSL refraction ported from Ossido's material pass, specular rim).
  - `amalfi::tokens`: generated at build time from `data/amalfi-tokens.json` and
    `data/amalfi-motion.md` (copies of silica-desktop `docs/design/amalfi/`), port of
    `gen_amalfi_tokens.dart`; `amalfi::theme` (palette, Increase Contrast port, materials as
    styles, Reduce Transparency fallback).
  - `motion`: cubic Bézier, springs from response/ζ, interruptible `Animated<T>` with
    velocity carried over.
  - `text`: bundled Inter, JetBrains Mono, Silica Symbols (embedded); fontconfig fallback;
    SkParagraph layout with even leading (kit), attributed strings, attachments, UTF-8 offsets
    everywhere (UTF-16 conversion inside).
  - `core`: generational view arena, generic modifiers (padding, frame, priority, opacity,
    background, offset, scale, hidden, disabled), proposal-based layout (Stack with SwiftUI
    space sharing, Spacer, Overlay, Grid), display lists of named kit layers (render +
    inspection), events with responder chain, hover/press/focus tracking with activation
    (AmalfiInteraction semantics), key view loop with Full Keyboard Access, default/cancel
    buttons, app shortcuts, signals with deferred re-entrant-safe handlers, timers,
    animations, damage tracking, headless snapshots, accessibility tree.

## Services

`libvetro::services` ([docs/services.md](services.md) has the interface tables and gaps):

- Typed zbus proxies for all 13 daemon interfaces plus `org.freedesktop.Notifications` and the
  FileChooser, OpenURI, Settings and Request portals; plain structures in `services::types`.
- `Observable<T>` (single-threaded, callbacks and `Changes` async stream), `AppearanceWatcher`
  (`Appearance` + `SystemLook` from System1 `State`/`Changed`), `SettingsWatcher` (all keys, key
  repeat, renderer choice), `Services` facade (lazy session bus connection, proxy getters,
  `run()` drives the watchers on the caller's executor). Defaults when the daemon or bus is
  absent; typed `Error` (`is_service_absent`).
- 17 tests (observable, mappings, proxies against mock servers on a private `dbus-daemon`,
  live watchers incl. daemon appearing and disappearing).
- Gaps: no renderer-backend key and no appearance keys in Settings1; JSON replies are raw
  strings; portal request helper and Trust1 agent callbacks missing.

## Decisions

- **Library name.** The cdylib is built as `libvetro_lib.so` (the crate `vetro` is the
  declarative layer); packaging installs it as `libvetro.so.1` with soname `libvetro.so.1`.
- **Kit layers as display lists.** Components draw named layers (`Surface`, `Label`, `Focus
  Halo` …) with kit styles; the same list renders and is read back by kit-parity tests and
  the inspector. Geometry parity is exact by construction.
- **Fonts embedded** in the library (Inter, JetBrains Mono, Silica Symbols): identical text
  everywhere; tests run with no system fallback (`VETRO_DETERMINISTIC`, always in `cfg(test)`).
- **Skia prebuilt binaries**: the feature set must match a published combination
  (`egl, gl, vulkan, wayland, x11, textlayout, svg, webp, pdf, skottie`).
- **No runtime JSON**: tokens, motion and symbol codepoints are generated Rust.
- **Core never calls the platform**: it queues `Request`s; signal handlers run after
  dispatch with nothing borrowed.
- **Window content** is offered the window size and centred at the size it takes
  (SwiftUI); fill with an infinite max width/height.
- **Stale handles are safe**: generational ids; every operation on a destroyed view is a
  no-op or `Error::NoSuchObject`.

## Next

1. Text editing engine (selection, movement, undo, IME preedit) and text field widgets.
2. `vetro-test`: kit fixtures (synced from silica-design) and the parity harness.
3. Amalfi components family by family with parity tests.
4. Platform (Wayland, SCTK) with shm raster first, then EGL/GL and Vulkan.
5. Declarative layer, C API, gallery in the VM.
