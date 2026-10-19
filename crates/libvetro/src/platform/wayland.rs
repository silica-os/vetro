//! The Wayland client: windows (xdg toplevels), popups, shell surfaces (wlr-layer-shell),
//! the lock screen (ext-session-lock), fractional scale with viewporter, cursor shapes,
//! keyboard (xkbcommon, client-side repeat), pointer and scroll, text-input-v3, the
//! clipboard, and frame pacing by frame callbacks. Built on smithay-client-toolkit; the
//! knowledge of silica-desktop's embedder (`crates/embedder/src/window.rs`) is ported here.
//!
//! The core never calls this layer: after every dispatch the platform drains the core's
//! requests ([`Request`]) and renders the windows that asked for a frame and whose last
//! frame callback has come back.

use std::time::{Duration, Instant};

use smithay_client_toolkit::compositor::{CompositorHandler, CompositorState};
use smithay_client_toolkit::data_device_manager::data_device::{DataDevice, DataDeviceHandler};
use smithay_client_toolkit::data_device_manager::data_offer::{DataOfferHandler, DragOffer};
use smithay_client_toolkit::data_device_manager::data_source::{
    CopyPasteSource, DataSourceHandler,
};
use smithay_client_toolkit::data_device_manager::{DataDeviceManagerState, WritePipe};
use smithay_client_toolkit::output::{OutputHandler, OutputState};
use smithay_client_toolkit::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay_client_toolkit::reexports::calloop::{EventLoop, LoopHandle, RegistrationToken};
use smithay_client_toolkit::reexports::calloop_wayland_source::WaylandSource;
use smithay_client_toolkit::registry::{ProvidesRegistryState, RegistryState};
use smithay_client_toolkit::seat::keyboard::{
    KeyEvent as SctkKey, KeyboardHandler, Keysym, Modifiers, RawModifiers, RepeatInfo,
};
use smithay_client_toolkit::seat::pointer::cursor_shape::CursorShapeManager;
use smithay_client_toolkit::seat::pointer::{PointerEvent, PointerEventKind, PointerHandler};
use smithay_client_toolkit::seat::{Capability, SeatHandler, SeatState};
use smithay_client_toolkit::session_lock::{
    SessionLock, SessionLockHandler, SessionLockState, SessionLockSurface,
    SessionLockSurfaceConfigure,
};
use smithay_client_toolkit::shell::WaylandSurface;
use smithay_client_toolkit::shell::wlr_layer::{
    Anchor, KeyboardInteractivity, Layer as WlrLayer, LayerShell, LayerShellHandler, LayerSurface,
    LayerSurfaceConfigure,
};
use smithay_client_toolkit::shell::xdg::popup::{Popup, PopupConfigure, PopupHandler};
use smithay_client_toolkit::shell::xdg::window::{
    Window as XdgWindow, WindowConfigure, WindowDecorations, WindowHandler,
};
use smithay_client_toolkit::shell::xdg::{XdgPositioner, XdgShell, XdgSurface};
use smithay_client_toolkit::shm::slot::SlotPool;
use smithay_client_toolkit::shm::{Shm, ShmHandler};
use smithay_client_toolkit::{
    delegate_compositor, delegate_data_device, delegate_keyboard, delegate_layer, delegate_output,
    delegate_pointer, delegate_registry, delegate_seat, delegate_session_lock, delegate_shm,
    delegate_xdg_popup, delegate_xdg_shell, delegate_xdg_window, registry_handlers,
};
use wayland_client::globals::{GlobalList, registry_queue_init};
use wayland_client::protocol::{
    wl_data_device::WlDataDevice, wl_data_source::WlDataSource, wl_keyboard, wl_output, wl_pointer,
    wl_seat, wl_shm, wl_surface,
};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_device_v1::{
    Shape as CursorShapeName, WpCursorShapeDeviceV1,
};
use wayland_protocols::wp::fractional_scale::v1::client::wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1;
use wayland_protocols::wp::fractional_scale::v1::client::wp_fractional_scale_v1::{
    self, WpFractionalScaleV1,
};
use wayland_protocols::wp::text_input::zv3::client::zwp_text_input_manager_v3::ZwpTextInputManagerV3;
use wayland_protocols::wp::text_input::zv3::client::zwp_text_input_v3::{self, ZwpTextInputV3};
use wayland_protocols::wp::viewporter::client::wp_viewport::WpViewport;
use wayland_protocols::wp::viewporter::client::wp_viewporter::WpViewporter;
use wayland_protocols::xdg::shell::client::xdg_positioner;

use crate::core::event::{Button, Key, KeyEvent, Mods, Phase};
use crate::core::ui::{Cursor, Request, Ui};
use crate::core::window::{Edges, KeyboardMode, Layer, WindowId, WindowKind};
use crate::geom::{Point, Rect, Size};
use crate::platform::keys::{key_of, mods_of};

/// How a window's role is held.
enum Role {
    Toplevel(XdgWindow),
    Popup(Popup),
    Layer(LayerSurface),
    Lock(SessionLockSurface),
}

impl Role {
    fn wl_surface(&self) -> &wl_surface::WlSurface {
        match self {
            Role::Toplevel(w) => w.wl_surface(),
            Role::Popup(p) => p.wl_surface(),
            Role::Layer(l) => l.wl_surface(),
            Role::Lock(l) => l.wl_surface(),
        }
    }
}

/// A mapped window.
struct Surface {
    id: WindowId,
    role: Role,
    configured: bool,
    /// The size in points from the last configure.
    size: Size,
    /// Device pixels per point: fractional scale (120ths) or the integer buffer scale.
    scale: f32,
    fractional: Option<WpFractionalScaleV1>,
    viewport: Option<WpViewport>,
    frame_pending: bool,
    wants_frame: bool,
    pool: Option<SlotPool>,
    last_frame: Instant,
}

/// Key repeat state (client side: Wayland compositors send rate and delay only).
#[derive(Debug, Clone, Copy)]
struct Repeat {
    rate: u32,
    delay: u32,
}

/// The platform state: the core and every Wayland object.
pub struct Platform {
    /// The core.
    pub ui: Ui,
    conn: Connection,
    qh: QueueHandle<Platform>,
    loop_handle: LoopHandle<'static, Platform>,
    registry: RegistryState,
    seats: SeatState,
    outputs: OutputState,
    compositor: CompositorState,
    shm: Shm,
    xdg: Option<XdgShell>,
    layer_shell: Option<LayerShell>,
    lock_state: SessionLockState,
    lock: Option<SessionLock>,
    fractional: Option<WpFractionalScaleManagerV1>,
    viewporter: Option<WpViewporter>,
    cursor_shapes: Option<CursorShapeManager>,
    text_input_manager: Option<ZwpTextInputManagerV3>,
    data_devices: Option<DataDeviceManagerState>,
    surfaces: Vec<Surface>,
    seat: Option<wl_seat::WlSeat>,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    pointer: Option<wl_pointer::WlPointer>,
    cursor_device: Option<WpCursorShapeDeviceV1>,
    text_input: Option<ZwpTextInputV3>,
    data_device: Option<DataDevice>,
    copy_source: Option<(CopyPasteSource, String)>,
    keyboard_focus: Option<WindowId>,
    pointer_focus: Option<WindowId>,
    pointer_pos: Point,
    pointer_serial: u32,
    enter_serial: u32,
    mods: Mods,
    repeat: Repeat,
    repeat_timer: Option<RegistrationToken>,
    text_input_pending: TextInputPending,
    text_input_enabled: bool,
    cursor: Cursor,
    app_id: String,
    had_windows: bool,
    start: Instant,
    last_tick: Instant,
    exit: bool,
}

#[derive(Debug, Default)]
struct TextInputPending {
    preedit: Option<(String, Option<(usize, usize)>)>,
    commit: Option<String>,
    delete: Option<(usize, usize)>,
}

/// An error of the platform.
#[derive(Debug)]
pub struct PlatformError(pub String);

impl std::fmt::Display for PlatformError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for PlatformError {}

fn err(e: impl std::fmt::Display) -> PlatformError {
    PlatformError(e.to_string())
}

/// Connects to the Wayland display, runs `setup` on the core, and runs the event loop until
/// the app quits (the last window closes or [`Request::Quit`]).
pub fn run(app_id: &str, setup: impl FnOnce(&mut Ui)) -> Result<(), PlatformError> {
    let conn = Connection::connect_to_env().map_err(err)?;
    let (globals, queue) = registry_queue_init::<Platform>(&conn).map_err(err)?;
    let qh = queue.handle();
    let mut event_loop: EventLoop<'static, Platform> = EventLoop::try_new().map_err(err)?;
    WaylandSource::new(conn.clone(), queue)
        .insert(event_loop.handle())
        .map_err(|e| err(format!("Wayland source: {e}")))?;
    let mut platform = Platform::new(conn, &globals, qh, event_loop.handle())?;
    platform.app_id = app_id.to_string();
    setup(&mut platform.ui);
    platform.process();
    while !platform.exit {
        let timeout = platform.timeout();
        event_loop
            .dispatch(timeout, &mut platform)
            .map_err(|e| err(format!("event loop: {e}")))?;
        platform.process();
    }
    Ok(())
}

impl Platform {
    fn new(
        conn: Connection,
        globals: &GlobalList,
        qh: QueueHandle<Platform>,
        loop_handle: LoopHandle<'static, Platform>,
    ) -> Result<Platform, PlatformError> {
        let compositor = CompositorState::bind(globals, &qh).map_err(err)?;
        let shm = Shm::bind(globals, &qh).map_err(err)?;
        let mut ui = Ui::new_headless();
        ui.has_platform_clipboard = true;
        Ok(Platform {
            ui,
            registry: RegistryState::new(globals),
            seats: SeatState::new(globals, &qh),
            outputs: OutputState::new(globals, &qh),
            xdg: XdgShell::bind(globals, &qh).ok(),
            layer_shell: LayerShell::bind(globals, &qh).ok(),
            lock_state: SessionLockState::new(globals, &qh),
            lock: None,
            fractional: globals.bind(&qh, 1..=1, ()).ok(),
            viewporter: globals.bind(&qh, 1..=1, ()).ok(),
            cursor_shapes: CursorShapeManager::bind(globals, &qh).ok(),
            text_input_manager: globals.bind(&qh, 1..=1, ()).ok(),
            data_devices: DataDeviceManagerState::bind(globals, &qh).ok(),
            compositor,
            shm,
            surfaces: Vec::new(),
            seat: None,
            keyboard: None,
            pointer: None,
            cursor_device: None,
            text_input: None,
            data_device: None,
            copy_source: None,
            keyboard_focus: None,
            pointer_focus: None,
            pointer_pos: Point::ZERO,
            pointer_serial: 0,
            enter_serial: 0,
            mods: Mods::empty(),
            repeat: Repeat {
                rate: 30,
                delay: 400,
            },
            repeat_timer: None,
            text_input_pending: TextInputPending::default(),
            text_input_enabled: false,
            cursor: Cursor::Default,
            had_windows: false,
            start: Instant::now(),
            last_tick: Instant::now(),
            exit: false,
            conn,
            qh,
            loop_handle,
            app_id: String::new(),
        })
    }

    fn timeout(&self) -> Option<Duration> {
        if self.ui.is_animating() {
            // Frame callbacks drive animations; wake at least every 50 ms as a safety net.
            return Some(Duration::from_millis(50));
        }
        let now = self.start.elapsed().as_secs_f64();
        self.ui
            .next_deadline()
            .map(|d| Duration::from_secs_f64((d - now).clamp(0.0, 3600.0)))
    }

    fn surface(&self, id: WindowId) -> Option<&Surface> {
        self.surfaces.iter().find(|s| s.id == id)
    }

    fn surface_mut(&mut self, id: WindowId) -> Option<&mut Surface> {
        self.surfaces.iter_mut().find(|s| s.id == id)
    }

    fn window_of(&self, wl: &wl_surface::WlSurface) -> Option<WindowId> {
        self.surfaces
            .iter()
            .find(|s| s.role.wl_surface() == wl)
            .map(|s| s.id)
    }

    /// After every dispatch: time, timers, requests, frames.
    fn process(&mut self) {
        self.ui.now = self.start.elapsed().as_secs_f64();
        self.ui.fire_timers();
        for _ in 0..8 {
            let requests = self.ui.take_requests();
            if requests.is_empty() {
                break;
            }
            for r in requests {
                self.handle_request(r);
            }
        }
        let ids: Vec<WindowId> = self.surfaces.iter().map(|s| s.id).collect();
        for id in ids {
            let ready = self
                .surface(id)
                .is_some_and(|s| s.configured && s.wants_frame && !s.frame_pending);
            if ready {
                self.render(id);
            }
        }
        let _ = self.conn.flush();
        if self.surfaces.is_empty() && self.ui.windows().next().is_none() && self.had_windows {
            self.exit = true;
        }
    }

    fn handle_request(&mut self, r: Request) {
        match r {
            Request::ShowWindow(id) => self.map_window(id),
            Request::CloseWindow(id) => self.unmap_window(id),
            Request::SetTitle(id, title) => {
                if let Some(Role::Toplevel(w)) = self.surface(id).map(|s| &s.role) {
                    w.set_title(title);
                }
            }
            Request::Resize(id, size) => {
                if let Some(s) = self.surface_mut(id) {
                    s.size = size;
                    s.wants_frame = true;
                }
                let scale = self.surface(id).map(|s| s.scale).unwrap_or(1.0);
                self.ui.resize_window(id, size, scale);
            }
            Request::SetSizeLimits(id, min, max) => {
                if let Some(Role::Toplevel(w)) = self.surface(id).map(|s| &s.role) {
                    let px = |s: Size| (s.width.max(0.0) as u32, s.height.max(0.0) as u32);
                    w.set_min_size(min.map(px));
                    w.set_max_size(max.map(px));
                    w.commit();
                }
            }
            Request::Redraw(id) => {
                if let Some(s) = self.surface_mut(id) {
                    s.wants_frame = true;
                }
            }
            Request::SetCursor(_, cursor) => {
                self.cursor = cursor;
                self.apply_cursor();
            }
            Request::SetClipboard(text) => self.set_clipboard(text),
            Request::ReadClipboard(view) => self.read_clipboard(view),
            Request::TextInput(_, rect) => self.update_text_input(rect),
            Request::TextInputSurrounding(_, text, cursor, anchor) => {
                if let Some(ti) = &self.text_input {
                    // The protocol limits the text to 4000 bytes: send a window around the caret.
                    let (text, c, a) = clip_surrounding(&text, cursor, anchor);
                    ti.set_surrounding_text(text, c as i32, a as i32);
                    ti.commit();
                }
            }
            Request::OpenUri(uri) => {
                let _ = std::process::Command::new("xdg-open").arg(uri).spawn();
            }
            Request::Beep => {}
            Request::Quit => self.exit = true,
            Request::StartMove(id) => {
                if let (Some(Role::Toplevel(w)), Some(seat)) =
                    (self.surface(id).map(|s| &s.role), &self.seat)
                {
                    w.move_(seat, self.pointer_serial);
                }
            }
            Request::AccessibilityChanged(_) => {}
        }
    }

    fn map_window(&mut self, id: WindowId) {
        if self.surface(id).is_some() {
            return;
        }
        let Some(win) = self.ui.window(id).cloned() else {
            return;
        };
        self.had_windows = true;
        let wl = self.compositor.create_surface(&self.qh);
        let fractional = self
            .fractional
            .as_ref()
            .map(|m| m.get_fractional_scale(&wl, &self.qh, id.to_raw()));
        let viewport = self
            .viewporter
            .as_ref()
            .map(|v| v.get_viewport(&wl, &self.qh, ()));
        let size = win.size;
        let role = match &win.kind {
            WindowKind::Normal | WindowKind::Panel | WindowKind::Sheet { .. } => {
                let Some(xdg) = &self.xdg else {
                    tracing::error!("no xdg_wm_base: cannot show a window");
                    return;
                };
                let w = xdg.create_window(wl, WindowDecorations::RequestServer, &self.qh);
                w.set_title(win.title.clone());
                w.set_app_id(self.app_id.clone());
                if let Some(min) = win.min_size {
                    w.set_min_size(Some((min.width as u32, min.height as u32)));
                }
                if let Some(max) = win.max_size {
                    w.set_max_size(Some((max.width as u32, max.height as u32)));
                }
                if let WindowKind::Sheet { parent } = win.kind
                    && let Some(Role::Toplevel(p)) = self.surface(parent).map(|s| &s.role)
                {
                    w.set_parent(Some(p));
                }
                w.commit();
                Role::Toplevel(w)
            }
            WindowKind::Popup { parent, anchor, .. } => {
                let Some(xdg) = &self.xdg else {
                    return;
                };
                let Some(parent_xdg) = self.surface(*parent).and_then(|s| match &s.role {
                    Role::Toplevel(w) => Some(w.xdg_surface().clone()),
                    Role::Popup(p) => Some(p.xdg_surface().clone()),
                    _ => None,
                }) else {
                    tracing::warn!("popup without an xdg parent");
                    return;
                };
                let Ok(pos) = XdgPositioner::new(xdg) else {
                    return;
                };
                pos.set_size(size.width.max(1.0) as i32, size.height.max(1.0) as i32);
                pos.set_anchor_rect(
                    anchor.x as i32,
                    anchor.y as i32,
                    anchor.width.max(1.0) as i32,
                    anchor.height.max(1.0) as i32,
                );
                pos.set_anchor(xdg_positioner::Anchor::BottomLeft);
                pos.set_gravity(xdg_positioner::Gravity::BottomRight);
                pos.set_constraint_adjustment(
                    xdg_positioner::ConstraintAdjustment::FlipY
                        | xdg_positioner::ConstraintAdjustment::SlideX
                        | xdg_positioner::ConstraintAdjustment::SlideY,
                );
                match Popup::from_surface(Some(&parent_xdg), &pos, &self.qh, wl, xdg) {
                    Ok(p) => {
                        p.wl_surface().commit();
                        Role::Popup(p)
                    }
                    Err(e) => {
                        tracing::error!("popup: {e}");
                        return;
                    }
                }
            }
            WindowKind::Shell {
                layer,
                anchor,
                exclusive,
                margin,
                keyboard,
                namespace,
                ..
            } => {
                let Some(shell) = &self.layer_shell else {
                    tracing::error!("no wlr-layer-shell: cannot show a shell surface");
                    return;
                };
                let l = shell.create_layer_surface(
                    &self.qh,
                    wl,
                    match layer {
                        Layer::Background => WlrLayer::Background,
                        Layer::Bottom => WlrLayer::Bottom,
                        Layer::Top => WlrLayer::Top,
                        Layer::Overlay => WlrLayer::Overlay,
                    },
                    Some(namespace.clone()),
                    None,
                );
                let mut a = Anchor::empty();
                a.set(Anchor::TOP, anchor.contains(Edges::TOP));
                a.set(Anchor::BOTTOM, anchor.contains(Edges::BOTTOM));
                a.set(Anchor::LEFT, anchor.contains(Edges::LEFT));
                a.set(Anchor::RIGHT, anchor.contains(Edges::RIGHT));
                l.set_anchor(a);
                l.set_exclusive_zone(*exclusive);
                l.set_margin(margin[0], margin[1], margin[2], margin[3]);
                l.set_keyboard_interactivity(match keyboard {
                    KeyboardMode::None => KeyboardInteractivity::None,
                    KeyboardMode::Exclusive => KeyboardInteractivity::Exclusive,
                    KeyboardMode::OnDemand => KeyboardInteractivity::OnDemand,
                });
                l.set_size(size.width.max(0.0) as u32, size.height.max(0.0) as u32);
                l.commit();
                Role::Layer(l)
            }
            WindowKind::Lock { .. } => {
                if self.lock.is_none() {
                    match self.lock_state.lock(&self.qh) {
                        Ok(lock) => self.lock = Some(lock),
                        Err(e) => {
                            tracing::error!("session lock: {e}");
                            return;
                        }
                    }
                }
                let output = self.outputs.outputs().next();
                let (Some(lock), Some(output)) = (&self.lock, output) else {
                    return;
                };
                Role::Lock(lock.create_lock_surface(wl, &output, &self.qh))
            }
        };
        self.surfaces.push(Surface {
            id,
            role,
            configured: false,
            size,
            scale: 1.0,
            fractional,
            viewport,
            frame_pending: false,
            wants_frame: true,
            pool: None,
            last_frame: Instant::now(),
        });
    }

    fn unmap_window(&mut self, id: WindowId) {
        if let Some(pos) = self.surfaces.iter().position(|s| s.id == id) {
            let s = self.surfaces.remove(pos);
            if let Some(f) = s.fractional {
                f.destroy();
            }
            if let Some(v) = s.viewport {
                v.destroy();
            }
            drop(s.role);
        }
        if self.keyboard_focus == Some(id) {
            self.keyboard_focus = None;
        }
        if self.pointer_focus == Some(id) {
            self.pointer_focus = None;
        }
    }

    fn configured(&mut self, id: WindowId, size: Option<Size>) {
        let (scale, size) = match self.surface_mut(id) {
            Some(s) => {
                if let Some(sz) = size.filter(|z| !z.is_empty()) {
                    s.size = sz;
                }
                s.configured = true;
                s.wants_frame = true;
                (s.scale, s.size)
            }
            None => return,
        };
        self.ui.resize_window(id, size, scale);
        if self.ui.window(id).is_some_and(|w| !w.active) {
            // Toplevels get activated by the keyboard enter; shell surfaces stay inactive.
        }
    }

    fn render(&mut self, id: WindowId) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_tick).as_secs_f64();
        self.last_tick = now;
        self.ui.now = self.start.elapsed().as_secs_f64();
        if self.ui.is_animating() {
            self.ui.advance(dt.min(0.1));
        }
        let damage = self.ui.prepare_frame(id);
        let Some(s) = self.surfaces.iter_mut().find(|s| s.id == id) else {
            return;
        };
        let scale = s.scale.max(0.5);
        let w = (s.size.width * scale).round().max(1.0) as i32;
        let h = (s.size.height * scale).round().max(1.0) as i32;
        let stride = w * 4;
        if s.pool.is_none() {
            match SlotPool::new((stride * h * 2) as usize, &self.shm) {
                Ok(p) => s.pool = Some(p),
                Err(e) => {
                    tracing::error!("shm pool: {e}");
                    return;
                }
            }
        }
        let Some(pool) = s.pool.as_mut() else { return };
        let (buffer, canvas) = match pool.create_buffer(w, h, stride, wl_shm::Format::Argb8888) {
            Ok(b) => b,
            Err(e) => {
                tracing::error!("shm buffer: {e}");
                return;
            }
        };
        let info = skia_safe::ImageInfo::new(
            (w, h),
            skia_safe::ColorType::BGRA8888,
            skia_safe::AlphaType::Premul,
            None,
        );
        if let Some(mut surface) =
            skia_safe::surfaces::wrap_pixels(&info, canvas, stride as usize, None)
        {
            let c = surface.canvas();
            c.clear(skia_safe::Color::TRANSPARENT);
            c.scale((scale, scale));
            self.ui.paint_window(id, c, None);
        }
        let wl = s.role.wl_surface().clone();
        if let Some(vp) = &s.viewport {
            vp.set_destination(s.size.width.round() as i32, s.size.height.round() as i32);
        } else {
            wl.set_buffer_scale(scale.round().max(1.0) as i32);
        }
        // The whole buffer is repainted (shm buffers are not reused in place), so the damage
        // only tells the compositor what changed since the previous frame.
        let d = if damage.is_empty() {
            Rect::from_size(s.size)
        } else {
            damage
        }
        .round_out(scale);
        wl.damage_buffer(
            (d.x * scale) as i32,
            (d.y * scale) as i32,
            (d.width * scale).ceil() as i32,
            (d.height * scale).ceil() as i32,
        );
        wl.frame(&self.qh, wl.clone());
        if buffer.attach_to(&wl).is_ok() {
            wl.commit();
        }
        s.frame_pending = true;
        s.wants_frame = self.ui.is_animating();
        s.last_frame = now;
    }

    fn apply_cursor(&self) {
        let (Some(dev), true) = (&self.cursor_device, self.pointer_focus.is_some()) else {
            return;
        };
        let shape = match self.cursor {
            Cursor::Default => CursorShapeName::Default,
            Cursor::Pointer => CursorShapeName::Pointer,
            Cursor::Text => CursorShapeName::Text,
            Cursor::Wait => CursorShapeName::Wait,
            Cursor::Crosshair => CursorShapeName::Crosshair,
            Cursor::ColResize => CursorShapeName::ColResize,
            Cursor::RowResize => CursorShapeName::RowResize,
            Cursor::Grab => CursorShapeName::Grab,
            Cursor::Grabbing => CursorShapeName::Grabbing,
            Cursor::NotAllowed => CursorShapeName::NotAllowed,
        };
        dev.set_shape(self.enter_serial, shape);
    }

    fn set_clipboard(&mut self, text: String) {
        let (Some(mgr), Some(dev)) = (&self.data_devices, &self.data_device) else {
            return;
        };
        let source = mgr.create_copy_paste_source(
            &self.qh,
            ["text/plain;charset=utf-8", "text/plain", "UTF8_STRING"],
        );
        source.set_selection(dev, self.enter_serial.max(self.pointer_serial));
        self.copy_source = Some((source, text));
    }

    #[allow(unsafe_code)]
    fn read_clipboard(&mut self, view: crate::core::tree::ViewId) {
        let Some(dev) = &self.data_device else {
            return;
        };
        let Some(offer) = dev.data().selection_offer() else {
            return;
        };
        let mime = offer.with_mime_types(|m| {
            ["text/plain;charset=utf-8", "text/plain", "UTF8_STRING"]
                .into_iter()
                .find(|want| m.iter().any(|x| x == want))
                .map(str::to_string)
        });
        let Some(mime) = mime else { return };
        let Ok(pipe) = offer.receive(mime) else {
            return;
        };
        let _ = self.conn.flush();
        // Read on the loop: the pipe is a calloop source.
        let mut buf = Vec::new();
        let res = self
            .loop_handle
            .insert_source(pipe, move |_, f, p: &mut Platform| {
                use std::io::Read;
                let mut chunk = [0u8; 8192];
                // SAFETY: calloop gives us the pipe's file; reading from it does not invalidate it.
                let file = unsafe { f.get_mut() };
                loop {
                    match file.read(&mut chunk) {
                        Ok(0) => {
                            let text = String::from_utf8_lossy(&buf).into_owned();
                            p.ui.deliver_paste(view, text);
                            return smithay_client_toolkit::reexports::calloop::PostAction::Remove;
                        }
                        Ok(n) => buf.extend_from_slice(chunk.get(..n).unwrap_or_default()),
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            return smithay_client_toolkit::reexports::calloop::PostAction::Continue;
                        }
                        Err(_) => {
                            return smithay_client_toolkit::reexports::calloop::PostAction::Remove;
                        }
                    }
                }
            });
        if let Err(e) = res {
            tracing::warn!("clipboard read: {e}");
        }
    }

    fn update_text_input(&mut self, rect: Option<Rect>) {
        let Some(ti) = &self.text_input else {
            return;
        };
        match rect {
            Some(r) => {
                if !self.text_input_enabled {
                    ti.enable();
                    self.text_input_enabled = true;
                }
                ti.set_cursor_rectangle(
                    r.x as i32,
                    r.y as i32,
                    r.width.max(1.0) as i32,
                    r.height as i32,
                );
                ti.commit();
            }
            None => {
                if self.text_input_enabled {
                    ti.disable();
                    ti.commit();
                    self.text_input_enabled = false;
                }
            }
        }
    }

    fn start_repeat(&mut self, event: KeyEvent) {
        self.stop_repeat();
        if self.repeat.rate == 0 || event.key == Key::Modifier {
            return;
        }
        let delay = Duration::from_millis(u64::from(self.repeat.delay));
        let interval = Duration::from_millis(1000 / u64::from(self.repeat.rate.max(1)));
        let token = self.loop_handle.insert_source(
            Timer::from_duration(delay),
            move |_, _, p: &mut Platform| {
                if let Some(w) = p.keyboard_focus {
                    let mut e = event.clone();
                    e.repeat = true;
                    e.mods = p.mods;
                    p.ui.now = p.start.elapsed().as_secs_f64();
                    p.ui.key(w, e);
                }
                TimeoutAction::ToDuration(interval)
            },
        );
        self.repeat_timer = token.ok();
    }

    fn stop_repeat(&mut self) {
        if let Some(t) = self.repeat_timer.take() {
            self.loop_handle.remove(t);
        }
    }
}

/// At most 4000 bytes of text around the caret (text-input-v3 limit), with the caret and
/// anchor offsets moved into it.
fn clip_surrounding(text: &str, cursor: usize, anchor: usize) -> (String, usize, usize) {
    const MAX: usize = 4000;
    if text.len() <= MAX {
        return (
            text.to_string(),
            cursor.min(text.len()),
            anchor.min(text.len()),
        );
    }
    let mut start = cursor.saturating_sub(MAX / 2);
    while start > 0 && !text.is_char_boundary(start) {
        start -= 1;
    }
    let mut end = (start + MAX).min(text.len());
    while end > start && !text.is_char_boundary(end) {
        end -= 1;
    }
    let slice = text.get(start..end).unwrap_or_default().to_string();
    let clamp = |x: usize| x.saturating_sub(start).min(slice.len());
    (slice.clone(), clamp(cursor), clamp(anchor))
}

// ----- SCTK handlers ------------------------------------------------------------------------

impl CompositorHandler for Platform {
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        factor: i32,
    ) {
        let Some(id) = self.window_of(surface) else {
            return;
        };
        let size = match self.surface_mut(id) {
            Some(s) if s.fractional.is_none() => {
                s.scale = factor.max(1) as f32;
                s.wants_frame = true;
                s.size
            }
            _ => return,
        };
        self.ui.resize_window(id, size, factor.max(1) as f32);
    }

    fn transform_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: wl_output::Transform,
    ) {
    }

    fn frame(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        _: u32,
    ) {
        let Some(id) = self.window_of(surface) else {
            return;
        };
        let animating = self.ui.is_animating();
        if let Some(s) = self.surface_mut(id) {
            s.frame_pending = false;
            if animating {
                s.wants_frame = true;
            }
        }
    }

    fn surface_enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
}

impl OutputHandler for Platform {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.outputs
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl WindowHandler for Platform {
    fn request_close(&mut self, _: &Connection, _: &QueueHandle<Self>, window: &XdgWindow) {
        let Some(id) = self.window_of(window.wl_surface()) else {
            return;
        };
        if let Some(root) = self.ui.window(id).map(|w| w.root) {
            self.ui.emit(
                root,
                crate::core::value::Signal::CloseRequested,
                crate::core::value::Value::None,
            );
            self.ui.flush();
        }
        // The app closes the window from the handler; with no handler it closes now.
        if !self.ui.window(id).is_some_and(|w| w.keep_on_close_request) {
            self.ui.close_window(id);
        }
    }

    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        window: &XdgWindow,
        configure: WindowConfigure,
        _serial: u32,
    ) {
        let Some(id) = self.window_of(window.wl_surface()) else {
            return;
        };
        let size = match configure.new_size {
            (Some(w), Some(h)) => Some(Size::new(w.get() as f32, h.get() as f32)),
            _ => None,
        };
        let active = configure.is_activated();
        self.ui.set_window_active(id, active);
        self.configured(id, size);
    }
}

impl PopupHandler for Platform {
    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        popup: &Popup,
        config: PopupConfigure,
    ) {
        let Some(id) = self.window_of(popup.wl_surface()) else {
            return;
        };
        let size = Size::new(config.width as f32, config.height as f32);
        self.configured(id, Some(size));
    }

    fn done(&mut self, _: &Connection, _: &QueueHandle<Self>, popup: &Popup) {
        if let Some(id) = self.window_of(popup.wl_surface()) {
            if let Some(root) = self.ui.window(id).map(|w| w.root) {
                self.ui.emit(
                    root,
                    crate::core::value::Signal::Closed,
                    crate::core::value::Value::None,
                );
            }
            self.ui.close_window(id);
        }
    }
}

impl LayerShellHandler for Platform {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, layer: &LayerSurface) {
        if let Some(id) = self.window_of(layer.wl_surface()) {
            self.ui.close_window(id);
        }
    }

    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        let Some(id) = self.window_of(layer.wl_surface()) else {
            return;
        };
        let (w, h) = configure.new_size;
        let size = (w > 0 && h > 0).then(|| Size::new(w as f32, h as f32));
        self.configured(id, size);
    }
}

impl SessionLockHandler for Platform {
    fn locked(&mut self, _: &Connection, _: &QueueHandle<Self>, _: SessionLock) {}

    fn finished(&mut self, _: &Connection, _: &QueueHandle<Self>, _: SessionLock) {
        tracing::warn!("the session lock was refused or ended");
        self.lock = None;
    }

    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        surface: SessionLockSurface,
        configure: SessionLockSurfaceConfigure,
        _serial: u32,
    ) {
        let Some(id) = self.window_of(surface.wl_surface()) else {
            return;
        };
        let (w, h) = configure.new_size;
        self.configured(id, Some(Size::new(w as f32, h as f32)));
    }
}

impl SeatHandler for Platform {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seats
    }

    fn new_seat(&mut self, _: &Connection, qh: &QueueHandle<Self>, seat: wl_seat::WlSeat) {
        if self.seat.is_none() {
            if let Some(mgr) = &self.data_devices {
                self.data_device = Some(mgr.get_data_device(qh, &seat));
            }
            if let Some(m) = &self.text_input_manager {
                self.text_input = Some(m.get_text_input(&seat, qh, ()));
            }
            self.seat = Some(seat);
        }
    }

    fn new_capability(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        cap: Capability,
    ) {
        match cap {
            Capability::Keyboard if self.keyboard.is_none() => {
                match self.seats.get_keyboard(qh, &seat, None) {
                    Ok(k) => self.keyboard = Some(k),
                    Err(e) => tracing::warn!("keyboard: {e}"),
                }
            }
            Capability::Pointer if self.pointer.is_none() => {
                match self.seats.get_pointer(qh, &seat) {
                    Ok(p) => {
                        self.cursor_device = self
                            .cursor_shapes
                            .as_ref()
                            .map(|m| m.get_shape_device(&p, qh));
                        self.pointer = Some(p);
                    }
                    Err(e) => tracing::warn!("pointer: {e}"),
                }
            }
            _ => {}
        }
    }

    fn remove_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        cap: Capability,
    ) {
        match cap {
            Capability::Keyboard => {
                if let Some(k) = self.keyboard.take() {
                    k.release();
                }
                self.stop_repeat();
            }
            Capability::Pointer => {
                if let Some(d) = self.cursor_device.take() {
                    d.destroy();
                }
                if let Some(p) = self.pointer.take() {
                    p.release();
                }
            }
            _ => {}
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl KeyboardHandler for Platform {
    fn enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        surface: &wl_surface::WlSurface,
        serial: u32,
        _: &[u32],
        _: &[Keysym],
    ) {
        self.enter_serial = serial;
        if let Some(id) = self.window_of(surface) {
            self.keyboard_focus = Some(id);
            self.ui.set_window_active(id, true);
        }
    }

    fn leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        surface: &wl_surface::WlSurface,
        _: u32,
    ) {
        self.stop_repeat();
        if let Some(id) = self.window_of(surface) {
            if self.keyboard_focus == Some(id) {
                self.keyboard_focus = None;
            }
            let toplevel = matches!(self.surface(id).map(|s| &s.role), Some(Role::Toplevel(_)));
            if toplevel {
                self.ui.set_window_active(id, false);
            }
        }
    }

    fn press_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        serial: u32,
        event: SctkKey,
    ) {
        self.enter_serial = serial;
        let Some(w) = self.keyboard_focus else { return };
        let key = key_of(event.keysym);
        let text = event
            .utf8
            .filter(|t| !t.chars().any(char::is_control))
            .filter(|_| !self.mods.intersects(Mods::CTRL | Mods::LOGO));
        let e = KeyEvent {
            key,
            text,
            mods: self.mods,
            pressed: true,
            repeat: false,
        };
        self.ui.now = self.start.elapsed().as_secs_f64();
        self.ui.key(w, e.clone());
        self.start_repeat(e);
    }

    fn repeat_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        _: SctkKey,
    ) {
        // Repeat is client-side (start_repeat): the compositor's repeat events are not used.
    }

    fn release_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        event: SctkKey,
    ) {
        self.stop_repeat();
        let Some(w) = self.keyboard_focus else { return };
        let e = KeyEvent {
            key: key_of(event.keysym),
            text: None,
            mods: self.mods,
            pressed: false,
            repeat: false,
        };
        self.ui.key(w, e);
    }

    fn update_modifiers(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        modifiers: Modifiers,
        _: RawModifiers,
        _: u32,
    ) {
        self.mods = mods_of(&modifiers);
    }

    fn update_repeat_info(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        info: RepeatInfo,
    ) {
        self.repeat = match info {
            RepeatInfo::Repeat { rate, delay } => Repeat {
                rate: rate.get(),
                delay,
            },
            RepeatInfo::Disable => Repeat { rate: 0, delay: 0 },
        };
    }
}

fn button_of(code: u32) -> Button {
    match code {
        0x110 => Button::Left,
        0x111 => Button::Right,
        0x112 => Button::Middle,
        other => Button::Other(other),
    }
}

impl PointerHandler for Platform {
    fn pointer_frame(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        self.ui.now = self.start.elapsed().as_secs_f64();
        for e in events {
            let Some(id) = self.window_of(&e.surface) else {
                continue;
            };
            let pos = Point::new(e.position.0 as f32, e.position.1 as f32);
            self.pointer_pos = pos;
            match e.kind {
                PointerEventKind::Enter { serial } => {
                    self.enter_serial = serial;
                    self.pointer_focus = Some(id);
                    self.apply_cursor();
                    self.ui.pointer_move(id, pos, self.mods);
                }
                PointerEventKind::Leave { .. } => {
                    if self.pointer_focus == Some(id) {
                        self.pointer_focus = None;
                    }
                    self.ui.pointer_leave(id);
                }
                PointerEventKind::Motion { .. } => self.ui.pointer_move(id, pos, self.mods),
                PointerEventKind::Press { button, serial, .. } => {
                    self.pointer_serial = serial;
                    self.ui.pointer_down(id, pos, button_of(button), self.mods);
                }
                PointerEventKind::Release { button, serial, .. } => {
                    self.pointer_serial = serial;
                    self.ui.pointer_up(id, pos, button_of(button), self.mods);
                }
                PointerEventKind::Axis {
                    horizontal,
                    vertical,
                    source,
                    ..
                } => {
                    let precise = matches!(
                        source,
                        Some(wl_pointer::AxisSource::Finger | wl_pointer::AxisSource::Continuous)
                    );
                    // Wheel clicks: one line of 3 × 13 pt per step.
                    let step = |a: &smithay_client_toolkit::seat::pointer::AxisScroll| -> f32 {
                        if precise || a.value120 == 0 && a.discrete == 0 {
                            a.absolute as f32
                        } else if a.value120 != 0 {
                            a.value120 as f32 / 120.0 * 39.0
                        } else {
                            a.discrete as f32 * 39.0
                        }
                    };
                    let (dx, dy) = (step(&horizontal), step(&vertical));
                    let phase = if horizontal.stop || vertical.stop {
                        Phase::Ended
                    } else if precise {
                        Phase::Changed
                    } else {
                        Phase::None
                    };
                    self.ui.scroll(id, pos, dx, dy, precise, phase);
                }
            }
        }
    }
}

impl ShmHandler for Platform {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl DataDeviceHandler for Platform {
    fn enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlDataDevice,
        _: f64,
        _: f64,
        _: &wl_surface::WlSurface,
    ) {
    }
    fn leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice) {}
    fn motion(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice, _: f64, _: f64) {}
    fn selection(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice) {}
    fn drop_performed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice) {}
}

impl DataOfferHandler for Platform {
    fn source_actions(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &mut DragOffer,
        _: wayland_client::protocol::wl_data_device_manager::DndAction,
    ) {
    }
    fn selected_action(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &mut DragOffer,
        _: wayland_client::protocol::wl_data_device_manager::DndAction,
    ) {
    }
}

impl DataSourceHandler for Platform {
    fn accept_mime(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlDataSource,
        _: Option<String>,
    ) {
    }

    fn send_request(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        source: &WlDataSource,
        _mime: String,
        mut fd: WritePipe,
    ) {
        use std::io::Write;
        if let Some((s, text)) = &self.copy_source
            && s.inner() == source
        {
            let _ = fd.write_all(text.as_bytes());
        }
    }

    fn cancelled(&mut self, _: &Connection, _: &QueueHandle<Self>, source: &WlDataSource) {
        if self
            .copy_source
            .as_ref()
            .is_some_and(|(s, _)| s.inner() == source)
        {
            self.copy_source = None;
        }
    }

    fn dnd_dropped(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource) {}
    fn dnd_finished(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource) {}
    fn action(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlDataSource,
        _: wayland_client::protocol::wl_data_device_manager::DndAction,
    ) {
    }
}

impl ProvidesRegistryState for Platform {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }
    registry_handlers![OutputState, SeatState];
}

// ----- Protocols without SCTK helpers --------------------------------------------------------

impl Dispatch<WpFractionalScaleManagerV1, ()> for Platform {
    fn event(
        _: &mut Self,
        _: &WpFractionalScaleManagerV1,
        _: <WpFractionalScaleManagerV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WpFractionalScaleV1, u64> for Platform {
    fn event(
        state: &mut Self,
        _: &WpFractionalScaleV1,
        event: wp_fractional_scale_v1::Event,
        data: &u64,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let wp_fractional_scale_v1::Event::PreferredScale { scale } = event else {
            return;
        };
        let Some(id) = WindowId::from_raw(*data) else {
            return;
        };
        let factor = scale as f32 / 120.0;
        let size = match state.surface_mut(id) {
            Some(s) => {
                s.scale = factor;
                s.wants_frame = true;
                s.size
            }
            None => return,
        };
        state.ui.resize_window(id, size, factor);
    }
}

impl Dispatch<WpViewporter, ()> for Platform {
    fn event(
        _: &mut Self,
        _: &WpViewporter,
        _: <WpViewporter as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WpViewport, ()> for Platform {
    fn event(
        _: &mut Self,
        _: &WpViewport,
        _: <WpViewport as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwpTextInputManagerV3, ()> for Platform {
    fn event(
        _: &mut Self,
        _: &ZwpTextInputManagerV3,
        _: <ZwpTextInputManagerV3 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwpTextInputV3, ()> for Platform {
    fn event(
        state: &mut Self,
        _: &ZwpTextInputV3,
        event: zwp_text_input_v3::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use zwp_text_input_v3::Event as E;
        match event {
            E::PreeditString {
                text,
                cursor_begin,
                cursor_end,
            } => {
                let cursor = (cursor_begin >= 0 && cursor_end >= 0)
                    .then_some((cursor_begin as usize, cursor_end as usize));
                state.text_input_pending.preedit = Some((text.unwrap_or_default(), cursor));
            }
            E::CommitString { text } => state.text_input_pending.commit = text,
            E::DeleteSurroundingText {
                before_length,
                after_length,
            } => {
                state.text_input_pending.delete =
                    Some((before_length as usize, after_length as usize));
            }
            E::Done { .. } => {
                let Some(w) = state.keyboard_focus else {
                    state.text_input_pending = TextInputPending::default();
                    return;
                };
                let p = std::mem::take(&mut state.text_input_pending);
                // Order of text-input-v3 `done`: delete, commit, then the new pre-edit.
                if let Some((b, a)) = p.delete {
                    state.ui.delete_surrounding(w, b, a);
                }
                if let Some(t) = p.commit {
                    state.ui.preedit(w, "", None);
                    state.ui.text_input(w, &t);
                }
                let (text, cursor) = p.preedit.unwrap_or_default();
                state.ui.preedit(w, &text, cursor);
            }
            _ => {}
        }
    }
}

delegate_compositor!(Platform);
delegate_output!(Platform);
delegate_shm!(Platform);
delegate_seat!(Platform);
delegate_keyboard!(Platform);
delegate_pointer!(Platform);
delegate_xdg_shell!(Platform);
delegate_xdg_window!(Platform);
delegate_xdg_popup!(Platform);
delegate_layer!(Platform);
delegate_session_lock!(Platform);
delegate_data_device!(Platform);
delegate_registry!(Platform);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surrounding_text_is_clipped_on_boundaries() {
        let long: String = "é".repeat(3000);
        let (t, c, a) = clip_surrounding(&long, 3000, 2990);
        assert!(t.len() <= 4000);
        assert!(c <= t.len() && a <= t.len());
        assert_eq!(clip_surrounding("abc", 9, 1), ("abc".to_string(), 3, 1));
    }
}
