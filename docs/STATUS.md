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
| `vetro-sys` | the C ABI: types, ids, one function list (`for_each_function!`) | done (94 functions, VETRO_1.0) |
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
- **Components**: every kit family (table under "Components"), plus `TextView` (documents:
  line index with Fenwick sums, layout per visible line, 0.8 ms per key on a 10 MB document in
  release) and the collections (`amalfi::collections`, one provider model: items are made
  when they come into sight and dropped when they leave; a million rows cost what is shown):
  `ListView` (inset, plain and bordered list-box looks), `TableView` (columns, header with
  sort and column resizing, cells per row and column, in-place editing of editable columns,
  alternating rows; outline mode over a `TreeSource` with chevrons, indentation, ←/→ and ⌥
  deep expansion), `GridView` (reflowing tiles, 2-D keyboard), `TableHeader`; shared
  selection (⌘/⇧, anchor and cursor), keyboard, double click/Return, overlay scroller;
  selected rows are emphasized (`NodeFlags::EMPHASIZED`, `Theme::emphasize`) while focused
  in an active window. `TabView` (window tabs on the kit's tab bar), `Toolbar` (kit unified
  bar pieces after the window controls), form pieces as kinds (value, footer, box surface).
- **Windows**: size limits honoured (#245: sizes clamped from the compositor, the app and
  restoration); sheets are xdg-dialog modal dialogs with client-side decorations (Ossido on
  the silica-desktop `vetro` branch centres them, veils the parent, keeps the focus and
  moves them with the parent) and their parent takes no input; state restoration
  (`core::restore`: autosave name → size and identified views' column widths, sort,
  expanded nodes, split positions, selected tabs, collapsed panes, scroll offsets in
  `$XDG_STATE_HOME/<app id>/windows/<name>.state`); full-size content with a toolbar
  (`Ui::set_toolbar_height`, also on shown windows); frame alignment applied
  (`.align()`: content at its own size aligned in a larger frame); input regions
  (`Ui::set_input_region`, for shell surfaces with transparent margins); undecorated
  windows (`Window::decorated`: alert panels of their own have no title bar and are modal
  xdg dialogs without a parent, which Ossido centres on the screen).
- **App**: lifecycle (`core::app`), single instance and reopen through silica-desktop's app
  registry socket (`platform::registry`), drag and drop, xdg-activation, pinch gestures, idle
  inhibition, the AccessKit AT-SPI bridge (`platform::a11y`, verified in the VM when
  `IsEnabled`), the inspector socket (`vetro-inspector` CLI dumps trees and display lists).
- **Platform** (`platform::wayland`, SCTK 0.20): xdg toplevels, popups, layer-shell, session
  lock, fractional scale + viewporter, cursor-shape, keyboard (xkb, client-side repeat with
  Settings1 rate/delay), pointer + axis (value120), text-input-v3 (preedit/commit/delete,
  surrounding text clipped to 4000 bytes), clipboard (copy/paste text), silica-window-v1
  (full-size content, appearance), silica-glass-v1 (compositor glass regions with profiles),
  silica-menu-v1 (menu bar JSON model in a sealed memfd, pop-up menus), frame pacing by frame
  callbacks.
- **Render backends**: Vulkan (primary; ash + Skia Ganesh VK, direct driver loading as the
  embedder, two frames in flight with per-slot semaphores and fences around Skia's submit,
  reduced shader variations; partial repaints into a retained GPU back surface copied onto
  each swapchain image, since Venus does not keep image contents; a pipeline warm-up draw
  after the first frame, as skia-safe has no persistent shader cache binding), GLES/EGL (buffer-age partial repaint,
  swap-with-damage), software (shm raster). Choice: `VETRO_RENDERER=vulkan|gl|software`,
  falling back in that order. Raster caches of static subtrees (`core::raster`: stable for
  3 frames, ≥ 24 items, no backdrop/glass/custom drawing; drawn at whole device pixels;
  96 MB cap; `VETRO_RASTER_CACHE=0` turns it off; off in headless tests).
- **Start-up**: bundled fonts through a custom empty font manager, fontconfig loaded on a
  thread and added to the fallback when ready (text laid out again); `VETRO_STARTUP_TIMING`
  prints the start-up stages and the first frame.
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
(headless PNG). Implicit animations (`.animation`, `.transition` with FLIP moves and leaving
views), `.sheet`, `.popover`, `.context_menu`; control builders bound to state
(`controls.rs`); `list(count, row_height, |i| element)` mounts a tree per visible row and
renders the live rows again with their component; `table`, `outline` and `collection` on the
same model (`on_sort`, `sorted`, `on_edit`, `on_select`, `selection`, `on_open`, `on_expand`,
`alternating`); `tabs` (`on_new_tab`, `on_close_tab`); `.alert(&Binding, AlertSpec)`; the
kit's popover for `.popover`; `toolbar` (`toolbar_button`, `toolbar_group`, `toolbar_title`,
`toolbar_search`) with `Element::toolbar` (full-size content); `form`, `form_section`,
`form_row`, `form_row_with`, `form_footer`; `WindowSpec` (size limits, autosave).

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
- Ossido headless e2e: `examples/gallery/tests/e2e.rs` runs
  `~/silica/silica/target/release/ossido --backend headless` in a private runtime dir, starts
  the gallery, clicks Quit through the control socket and checks the exit (skipped when the
  binary is missing). `examples/bench`: frame budgets (500 controls, hover, 10 MB typing).
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
| Fields | `TextField` (text, secure, area, search, tokens, combo, value), `Token`, `ClearButton`, `ComboButton` | 128 | done (the combo list is a menu popup, `TextField::choose_in`; fields send `TextInput` and `TextInputSurrounding`) |
| Buttons | `PushButton`, `IconButton`, `HelpButton`, `LinkButton`, `GradientButtons`, `ToolbarButton`, `MenuButton` | 214 of 220 | partial: pop-up/pull-down `Open` masters geometry only (the kit's glass hides the button), `App` masters need app icons |
| Group Boxes | `GroupBox` | 8 | done |
| Wells | `ColorWell`, `ImageWell`, swatch grid, colour panel popup | 74 | done |
| Forms | `form_row`, `form_section`, `form_footer`, `form_layout` | 22 | done |
| Scrollbars | `Scrollbar`, `ScrollbarCorner`, `ScrollView` | 34 | done |
| Menus | `MenuItem`, `Menu`, `menus::present` (pop-up, pull-down, list, at a point) | 48 of 52 (+4 open buttons) | partial: `Item/App` needs app icons; the chosen-row blink is not timed |
| Sidebars | `Row`, `Badge`, `EjectButton`, `Header`, `group`, `Footer`, `Panel` | 44 | done |
| Titlebars | `Dot`, `WindowControls`, `TitleBlock`, `SearchPiece`, `SwitcherPiece`, `SendPiece`, `Strip`, `Spacer`, `TitleBar`, `Tab`, `TabBar` | 181 | done |
| Popovers | `PopoverShape`, `Popover`, `popovers::present` (flips sides) | 84 | done; in the C ABI (`vetro_view_present_popover`) |
| Alerts | `Alert`, `alert()` (title, message, accessories, buttons, badge) | 26 | done (icons are bitmaps: masked); in the C ABI (`vetro_alert_*`, presented as a sheet or a panel) |
| Dialogs | `Dialog`, `Pane`, `browser`, `dialog_footer` | 12 | done |
| Status Bar | `StatusItem`, `StatusBar` (plates of Increase Contrast and Reduce Transparency) | 22 | done; `STATUS_ITEM` kind (content kind, glyph, text) |
| Windows | `WindowFrame`, `Block` | 40 | done for the window's own layers; nested components of other families (group boxes, segments, checkbox group, search field, menu bar) are placeholders there |
| Collections (`Apps/Mustache` file list) | `TableHeader`, `TableView` rows, `ListView` rows, `GridView` tile | 16 | done (two kit quirks in kit-issues.md) |

Shared changes made on the way: the harness expands nested symbol masters (path-addressed
layers, instance text overrides applied in order) and walks groups; `Scene` has
`auto_geometry_except`, `extra_mask`, `png_margin`, `hover_at`, `press_at`; `Recorder` has
`glyph_sized`; `Widget::attached` lets a view start animations on its own; `Ui::set` restarts
ticking; the tooltip presenter is part of `Ui`; `Text` has a wrapping paragraph mode. The harness
also has `ignore_aspects` (a kit defect on every layer, e.g. the dark glass blur) and
`skip_trees` (nested components replaced by placeholders); kit-driven builders shared by the
tests live in `tests/common/mod.rs`. Open problems: app icons (Item/App, Pop-up/App, alert
icons), the `Open` masters of buttons, the menu bar family, Windows with
the real nested components. Window buttons are at the 23 pt pitch (windows.md and every kit
render since silica-design fb5f9f10, compared unmasked); alerts follow the binding
anatomy of components.md (280 wide, centred text, buttons one per row 240 × 28, default
first, Escape last).

## Performance (#254 budgets)

`cargo run --release -p vetro-bench` (headless, CPU raster) and the e2e test:
- typing in a 10 MB document: 0.6 ms per key (budget 8 ms);
- a spinner beside 500 controls at 2×: 50 ms per frame without the raster cache, 3.5 ms with
  it (120 Hz budget: 8.3 ms); hover change 0.14 ms;
- cold start to the first frame: 125 ms in software on a local headless Ossido (budget
  150 ms; the e2e test reports it). On the dev VM the Vulkan (Venus) first frame takes
  seconds (driver load and pipeline compiles: Skia's persistent pipeline cache is not
  reachable through skia-safe) and the VM's timings jitter by whole seconds;
- shell memory: the Vetro shell in the VM (Vulkan, Venus) was 166 MB resident with full-size
  icon and wallpaper decodes; icons are now kept at 256 px and wallpapers at the output's
  size (to be measured again). The Flutter shell's embedder starts at 73 MB.

## Next

1. Shell on Vetro (§2): silica-desktop branch `vetro` (worktree ~/build/wt-vetro-shell,
   crate `crates/shell`, binary `silica-shell`; libvetro from the sibling `../vetro`
   checkout, the workspace patches vetro's git dependency on silica-desktop's protocols to
   the local crates). `cargo xtask os deploy --toolkit=vetro` runs it as the dev session's
   kept shell (verified in the VM with Vulkan/Venus: wallpaper from silica.assets, menu bar,
   status items, Control Center, Dock with magnification and bounce, Faro, Launcher,
   banners, the screenshot thumbnail). Done:
   - lock screen (session lock per output, wallpaper, clock, user block, password states and
     shake, unlock through System1, Wi-Fi and battery status items);
   - desktop wallpaper (Background layer; images, colour, the Amalfi set or silica.assets);
   - menu bar (system menu, the focused app's menus, submenus, status items over System1:
     Bluetooth, Wi-Fi, Sound, Battery, Search, Clock, Control Center);
   - Control Center (Wi-Fi, Bluetooth, Do Not Disturb, Dark Mode, Screen Capture, Display
     and Sound sliders through silica-daemon);
   - Dock (pinned and running apps, Silica icons for Silica's apps, the kit's Missing tile,
     magnification with the measured profile and curve, grow/shrink of unpinned apps,
     launch bounce, context menus with Keep in Dock/Remove, Show All Windows, Hide, Quit,
     the separator's magnification toggle, Trash full/empty with the Empty Trash alert, the
     Downloads stack as a pile and a grid, `dock.icons` for the minimise effect);
   - Faro and the Launcher (one morphing panel: apps, home files, calculations, web search;
     grid with categories, recents, search, scroll bar);
   - notifications (banners with the measured timings, Notification Center with stacks,
     Clear All, the empty state, Do Not Disturb);
   - app switcher (switcher.* from Ossido), screenshot thumbnail, selection and bar;
   - Gatekeeper, administrator authentication and Force Quit dialogs.
   Ossido on that branch: xdg-dialog sheets, parentless modal dialogs centred on screen.
   Next: Dock dragging, auto-hide, badges, minimised windows; stack fan and list views;
   notification actions and replies; the screenshot editor (#154) and recording (#153);
   Notification Center widgets; measure the shell's memory again.
2. Performance: a real Skia persistent shader cache needs a skia-safe binding.
3. Collections: type-select, column reordering, drag and drop of rows, rubber-band
   selection in grids.
4. Open kit problems: app icons (bitmaps in the kit), `Open` button masters, menu bar family.
5. `vetro-terminal` (port `crates/embedder/src/termgrid.rs` + `pty.rs`) with the apps phase.
6. Keep Vetro free of distribution-specific assumptions (Silica OS moves to a
   BuildStream/freedesktop-sdk image): fonts are bundled, fontconfig and the services are
   optional at run time.
