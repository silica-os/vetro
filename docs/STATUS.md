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

## Components

Each family: widgets in `crates/libvetro/src/amalfi/<family>.rs`, parity test in
`crates/vetro-test/tests/<family>.rs` (every master of the family, geometry exact, pixels with
a named tolerance, text masked), behaviour unit tests in the module. Kit defects and the
deviations they cause are in [kit-issues.md](kit-issues.md).

| Family | Widgets | Masters | State |
| --- | --- | --- | --- |
| Toggles | `Toggle` (checkbox, radio), `Switch`, `toggle_group` | 296 | done |
| Tooltips | `Tooltip`, `place`, hover presenter in `Ui` | 14 | done (placement by position; pixels of `Placement` wait for the round icon button) |
| Segments | `SegmentedControl`, `ActionSegments`, `PaneControl`, `SegmentItem` | 530 | done (Dark Icon + Label: geometry only) |
| Sliders | `Slider`, `Dial`, `SliderThumb`, `LensThumb` | 64 | done |
| Steppers | `Stepper`, `stepper_with_field` | 68 | done |
| Progress | `ProgressBar`, `CircularProgress`, `Spinner` | 84 | done |
| Disclosure | `Disclosure`, `DisclosureGroup` | 44 | done |
| Fields | `TextField` (text, secure, area, search, tokens, combo, value), `Token`, `ClearButton`, `ComboButton` | 126 of 128 | partial: `Combo Box/Open` (the list is a menu) waits for Menus |

Shared changes made on the way: the harness expands nested symbol masters (path-addressed
layers, instance text overrides applied in order) and walks groups; `Scene` has
`auto_geometry_except`, `extra_mask`, `png_margin`, `hover_at`, `press_at`; `Recorder` has
`glyph_sized`; `Widget::attached` lets a view start animations on its own; `Ui::set` restarts
ticking; the tooltip presenter is part of `Ui`; `Text` has a wrapping paragraph mode.

## Next

1. Text editing engine (selection, movement, undo, IME preedit) and text field widgets.
2. `vetro-test`: kit fixtures (synced from silica-design) and the parity harness.
3. Amalfi components family by family with parity tests.
4. Platform (Wayland, SCTK) with shm raster first, then EGL/GL and Vulkan.
5. Declarative layer, services, C API, gallery in the VM.
