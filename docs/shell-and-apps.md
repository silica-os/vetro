# The shell and the apps on Vetro (plan)

Plan sections §2 and §3 of [silica-desktop#254](https://github.com/silica-os/silica-desktop/issues/254).
The shell is under way on silica-desktop's `vetro` branch (`crates/shell`): lock screen,
wallpaper, menu bar and Dock first versions (state in [STATUS.md](STATUS.md) "Next"). This
file says how each piece maps onto Vetro, so the work can be split.

## Shell (`silica-desktop/crates/shell`, one process)

All surfaces are `WindowKind::Shell` (wlr-layer-shell) or `WindowKind::Lock`
(ext-session-lock); glass is the compositor's (`silica-glass-v1`, `Recorder::glass` with the
right `GlassProfile`), so Ossido draws the materials and shadows; the shell draws content.
Curves: the measured ones in `silica-desktop/docs/spec/*.md` (Dock, Faro, Launcher, menus,
switcher, Control Center, notifications, HUD, lock), as `motion::Cubic` and `SpringToken`
values written next to each component with the spec section in a comment.

| Module | Vetro pieces | Notes |
|---|---|---|
| Menu bar | Shell window (Top, anchored top/left/right, exclusive zone), `silica_shell_manager_v1` events `app_menu` (the JSON model, parsed with silica-protocols' types) | the shared Status Bar component (agent family `Status Bar`), auto-hide (#242) |
| Dock | Shell window (Bottom anchor, Top layer), custom-drawn tiles (`Recorder::image` for icons from `Thumbnails1`/silica-assets), `GlassProfile::Dock` | magnification and bounce from `dock.md`; stacks (#244) as popups; context menu through the menu service (#248); drag and drop (`Ui::drop_items`) |
| Faro and Launcher | one Overlay shell window that morphs (Animated<Rect> on a single container), text field, results from the daemon (`services::actions`, `apps`) | `faro.md`, `launcher.md` |
| Switcher | Overlay window, `GlassProfile::Panel` | `app-switcher.md` |
| Notifications | Overlay windows, `services::notifications`, banners `GlassProfile::Banner` | `notifications.md` |
| Control Center | Overlay window, modules on `GlassProfile::Module`, System1 (`services::system`) | `controlcenter.md` |
| Screenshot UI | Overlay window, toolbar | `screenshot-ui.md` |
| Lock screen | `WindowKind::Lock` per output, one shared backdrop for the glass clock (#253), takeover after restart (#252: the lock state is a zbus/Wayland client of its own, no relay) | `lock-screen.md`, L-01 timings |
| Force Quit, polkit agent | panels; Authentication1 / polkit `AuthenticationAgent` served by the shell (the services layer has the client side only) | |
| Spaces bar | Overlay window; Ossido keeps Mission Control | `spaces.md`, `panorama.md` |

What the embedder's `ShellChannel` and `LockController` did becomes direct
`silica-shell-v1` requests/events and zbus calls from the shell process.

## Apps

Each app repository gets a `vetro` branch with a Cargo crate on `vetro` (declarative) or
`vetro-kit` (imperative), keeping its data formats and tests (ported to Rust).

| App | On Vetro |
|---|---|
| Mustache | browser views (icon/list/columns/gallery) on lazy lists (to write: virtualised list/table/collection components on the kit's `Sidebars`/`Forms` rows), sidebar, Get Info panel, Open/Save panels for the portal (`--dialog=open|save`), desktop layer as a Background shell window |
| Mail | `silica-mail-engine` stays; UI on `vetro`; composer on `TextView` (rich text: attributed runs) |
| Preview | images via Skia codecs, PDF (pdfium or mupdf, licence check), thumbnails sidebar |
| Settings | panes on `Forms` rows and the bound controls; Settings1/System1 through `services` |
| Clock, Calculator | always dark (`Window::set_dark(Some(true))`); fixed size (#245: size limits) |
| Terminal | `vetro-terminal`: the grid engine of `crates/embedder/src/termgrid.rs` (alacritty_terminal) and `pty.rs`, drawn through a custom view by text runs; tabs, splits, profiles, settings window; performance gate 110 MB/s, p99 frame < 8 ms |
| Notes, Text Editor | `TextView` (10 MB documents measured: 0.8 ms per key in release headless), find and replace, encodings |

New apps (calendar, reminders, photos, disks, fonts, passwords) start on `vetro` directly.

## Order

1. Missing toolkit pieces the shell needs: virtualised lists/tables/collections, image views
   with the icon cache, Status Bar and menu-bar pieces (agent families), window sheets placed by
   Ossido.
2. Shell modules in the order Lock, Menu bar, Dock, Faro/Launcher, Notifications, Control
   Center, Switcher, Screenshot UI.
3. Apps in parallel once 1 is done; Terminal after `vetro-terminal`.
4. `xtask --toolkit=vetro` in silica-desktop to deploy the Vetro session in the VM; parity
   checklist; cutover.
