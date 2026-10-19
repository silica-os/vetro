# Vetro: status

Living document: what is done, the decisions taken, and what comes next. A successor picks up
from "Next" below. Plan: [silica-os/silica-desktop#254](https://github.com/silica-os/silica-desktop/issues/254).

Gate (real exit codes): `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`.
ABI: `cargo xtask header` (regenerates `include/vetro.h`, `abi/vetro.map`), `cargo xtask abi-check [--debug]`.
VM: `cargo xtask vm run [example]`, `vm shot PATH`, `vm log`, `vm stop` (hold no lock yourself: the
task takes `/dev/shm/silica-vm.lock` per command). Local e2e: Ossido headless (see "Testing").

## Crates

| Crate | What | State |
|---|---|---|
| `libvetro` (lib `vetro_lib`, cdylib + rlib + staticlib for packaging) | the toolkit | see below |
| `vetro-sys` | the C ABI: types, ids, one function list (`for_each_function!`) | done (79 functions, VETRO_1.0) |
| `vetro-kit` | safe Rust objects over the ABI | done for the current ABI |
| `vetro` | declarative layer | done (v1), see below |
| `vetro-test` | kit-parity harness | done; families added as components land |
| `examples/gallery` | demo app for the VM | push buttons, menus, context menu; grows with components |
| `xtask` | sync-kit, check-sources, header, package, abi-check, vm | done |
| `vetro-terminal`, `vetro-inspector` | planned | not started |

## libvetro: done

- **Foundations**: `geom`, `color`, `shape` (smooth corners, port of silica_ui), `style` (kit
  layer styles: fills/gradients, inside/centre/outside/dashed borders, CSS-blur shadows, inner
  shadows, backdrop blur), `render::glass` (SkSL refraction from Ossido's material pass),
  `motion` (Bézier, springs from response/ζ, interruptible `Animated<T>`).
- **Tokens** generated at build time from `data/amalfi-tokens.json` + `data/amalfi-motion.md`
  (copies of silica-desktop `docs/design/amalfi/`; `cargo xtask check-sources` compares), Silica
  Symbols codepoints from `data/silica-symbols-codepoints.json`. `amalfi::theme`: palette,
  Increase Contrast, accent, materials as styles, Reduce Transparency fallback.
- **Text**: embedded Inter / JetBrains Mono / Silica Symbols, fontconfig fallback (off in
  tests: `VETRO_DETERMINISTIC`), SkParagraph layout (even leading as the kit), attributed
  strings, attachments, UTF-8 offsets; `text::edit::Editor` (selection, grapheme/word/line/
  paragraph/document movement, macOS bindings, undo groups, IME preedit, surrounding delete).
- **Core**: generational view arena; generic modifiers (padding, frame, priority, opacity,
  background, corner radius, offset, scale, hidden, disabled, appearance override); proposal
  layout (Stack with SwiftUI space sharing and first-baseline alignment, Spacer, Overlay,
  Grid); display lists of named kit layers (render + inspection) incl. compositor glass
  items; events with responder chain, hover/press/focus and activation (silica_ui
  AmalfiInteraction semantics), `state_changed` notifications, key view loop with Full
  Keyboard Access, default/cancel buttons, menu key equivalents, app shortcuts; signals with
  deferred re-entrant-safe handlers; timers; animations; damage tracking; snapshots;
  accessibility descriptions/tree and actions (`Ui::perform`); menus (`core::menu`: menu bar,
  pop-up menus through the menu service).
- **Components**: push buttons (all 200 kit masters pass geometry + pixels), labels, symbol
  images. The other families are being implemented on branch `components` (agent, worktree
  `~/build/wt-components-vetro`); merge it, then extend `ffi/kinds.rs`, `vetro-sys` KINDS and
  the declarative builders for every new widget.
- **Platform** (`platform::wayland`, SCTK 0.20): xdg toplevels, popups, layer-shell, session
  lock, fractional scale + viewporter, cursor-shape, keyboard (xkb, client-side repeat with
  Settings1 rate/delay), pointer + axis (value120), text-input-v3 (preedit/commit/delete,
  surrounding text clipped to 4000 bytes), clipboard (copy/paste text), silica-window-v1
  (full-size content, appearance), silica-glass-v1 (compositor glass regions with profiles),
  silica-menu-v1 (menu bar JSON model in a sealed memfd, pop-up menus), frame pacing by frame
  callbacks.
- **Render backends**: Vulkan (primary; ash + Skia Ganesh VK, direct driver loading as the
  embedder, CPU-synchronised frames, full repaint), GLES/EGL (buffer-age partial repaint,
  swap-with-damage), software (shm raster). Choice: `VETRO_RENDERER=vulkan|gl|software`,
  falling back in that order.
- **Services** (`services`, zbus): typed proxies for every daemon interface and the portals,
  `AppearanceWatcher` (System1 look → `Appearance`, live), `SettingsWatcher` (key repeat,
  renderer key), `Observable`, `Services` facade. The platform runs them on the calloop
  executor; `platform::spawn` / `platform::services` for apps. Details: `docs/services.md`.
- **C ABI** (`ffi`): every function of `vetro_sys::for_each_function!` defined and
  signature-checked at compile time; runtime reached through `with_ui` (the platform's `Ui`
  inside `vetro_app_run` and its callbacks, or a per-thread headless one); panics caught at
  the boundary; callback user data freed through `destroy` exactly once.

## Declarative layer (`vetro`)

Components (`body(&self, cx) -> Element`), positional `cx.state`, `State` / `Binding` /
`Model` (observable, `cx.watch`), builders (`column`, `row`, `overlay`, `grid`, `spacer`,
`group`, `label`, `symbol`, `button`, `view(kind)`, `custom`, `host`, `component`,
`for_each`), modifiers (padding, frame, min/max, fill, align, priority, background, corner
radius, clip, opacity, hidden, disabled, id, tooltip, accessibility label, dark, spacing,
alignment, style, control size, text style/colour, symbol, default/cancel, handlers). Keyed
reconciler with property diffing and handler swapping; one batched re-render per loop turn;
`mount` (declarative inside imperative), `host` (imperative inside declarative), `preview`
(headless PNG). Not yet: implicit animations (`.animation`, `.transition`), `.sheet`,
`.popover`, `.toolbar`, `.context_menu` modifiers, per-component invalidation, `List`/`Table`/
`Form` mapping (needs the components branch).

## Decisions

- **Library name.** The crate `libvetro` builds `vetro_lib` (the crate `vetro` is the
  declarative layer). Distribution: `cargo xtask package` links `libvetro.so.1` (soname) from
  the staticlib with `abi/vetro.map` (`VETRO_1.0`), plus `libvetro.so` and `vetro.pc`.
- **Header generator instead of cbindgen.** The ABI is one macro list in vetro-sys (so that
  libvetro's definitions are checked against the declarations Rust users link); cbindgen does
  not expand macros on stable, so `xtask header` generates `vetro.h` from the same list. The
  ABI check compiles a C layout probe and a C smoke test against the header and the packaged
  library, and verifies the exported symbol set and versions.
- **Property-based ABI.** Views are made by kind (`VETRO_KIND_*`) and configured by property
  ids (typed setters) and signals: new components and properties are new numbers, never new
  signatures, so the ABI stays stable while the kit grows.
- **Kit layers as display lists.** Components draw named layers (`Surface`, `Label`, `Focus
  Halo` …) with kit styles; the same list renders and is read back by kit-parity tests and the
  inspector.
- **Kit defects** go to `docs/kit-issues.md` and become named deviations in the tests (the
  design docs win over the kit).
- **Pixel tolerances**: `Tolerance::SHAPES` (mean 1, max 48), `CONTROLS` (mean 1.5, max 84:
  Sketch antialiases small controls ~¼ pt wider, measured by silica_ui on the same masters),
  `WITH_TEXT` (mean 2.5); glass refraction bands are masked when a comparison fails (as
  silica_ui).
- **Fonts embedded**; **no runtime JSON** except the silica-menu-v1 model (a format owned by
  silica-desktop; candidate for a typed protocol there).
- **Core never calls the platform**: it queues `Request`s; signal handlers run after dispatch.
- **Window content** is offered the window size and centred at the size it takes; fill with
  an infinite max width/height. Normal windows are opaque (`Surface/Window`), popups and shell
  surfaces transparent with compositor glass.
- **Skia prebuilt binaries**: the feature set matches a published combination (`egl, gl,
  vulkan, wayland, x11, textlayout, svg, webp, pdf, skottie`).
- **One app loop per process** (`vetro_app_run`); headless runtimes are per thread.
- **Precise closure capture pitfall**: a `move` closure that reads only `data.user` captures
  only that field; the FFI forces whole capture (`let d = &data;`) where a guard must live.

## Testing

- `cargo test`: unit tests, kit parity (`crates/vetro-test/tests/*.rs`), FFI round trips,
  vetro-kit, declarative reconciler.
- Ossido headless e2e (manual for now): `/dev/shm/headless.sh BIN OUT.png [WxH] [secs]` runs
  `~/silica/silica/target/release/ossido --backend headless` in a private runtime dir, starts
  the client, screenshots with `silica-cli`; `HEADLESS_CMDS` runs `input.click`/`input.key`.
  Make it a cargo test in `vetro-test` gated on the binary's presence.
- VM: gallery verified with Vulkan (Venus), GL (virgl) and software; dark appearance follows
  System1; menu bar shows the app's menus.

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

1. Merge `components` (agent); register kinds; gallery page per family; VM screenshots.
2. Text fields → platform: `Request::TextInput` with the caret rectangle when a field gets
   focus, `TextInputSurrounding`; clipboard paste into fields (done in core via
   `request_paste`).
3. AccessKit / AT-SPI adapter in the platform (tree from `Ui::accessibility_tree`, actions via
   `Ui::perform`).
4. App lifecycle: single instance through silica-desktop's app registry socket protocol
   (`silica_control::apps`), windowless apps and reopen, standard App/Window/Help menus,
   About panel, document model.
5. Window features: sheets attached to windows, tabbing, restoration state, toolbar styles.
6. Declarative: animations/transitions, sheet/popover/toolbar/context-menu modifiers, List/
   Table/Form.
7. `vetro-terminal` (port `crates/embedder/src/termgrid.rs` + `pty.rs`), `vetro-inspector`.
8. Performance: raster caches for static subtrees, Vulkan frame pipelining, damage-based
   Vulkan repaint (incremental present), budgets in CI benchmarks.
9. Then the shell (§2) and apps (§3) — not before the toolkit milestone tests pass.
