//! The GLES backend: EGL on the Wayland display (one context shared by every window), a
//! `wl_egl_window` per window, Skia's GL `DirectContext` drawing into the default
//! framebuffer. Partial repaint with `EGL_EXT_buffer_age` and
//! `EGL_KHR_swap_buffers_with_damage` (silica-desktop#233: correct buffer age, or a full
//! repaint when the age is unknown). The swap interval is 0: frames are paced by
//! `wl_surface.frame` callbacks. Ported from silica-desktop's embedder (`ffi/gl.rs`).

#![allow(unsafe_code)]

use std::ffi::c_void;

use khronos_egl as egl;
use skia_safe::gpu::{self, DirectContext, SurfaceOrigin, gl as skgl};
use wayland_client::Proxy;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_egl::WlEglSurface;

use crate::geom::Rect;

const PLATFORM_WAYLAND: egl::Enum = 0x31D8;
const BUFFER_AGE: egl::Int = 0x313D;
const GL_RGBA8: u32 = 0x8058;

type SwapWithDamageFn =
    unsafe extern "C" fn(*mut c_void, *mut c_void, *const egl::Int, egl::Int) -> egl::Boolean;

/// The shared EGL display, context and Skia context.
pub struct GlContext {
    egl: egl::DynamicInstance<egl::EGL1_5>,
    display: egl::Display,
    config: egl::Config,
    context: egl::Context,
    skia: DirectContext,
    swap_with_damage: Option<SwapWithDamageFn>,
    buffer_age: bool,
    renderer: String,
}

/// One window's EGL surface.
pub struct GlWindow {
    window: WlEglSurface,
    surface: egl::Surface,
    size: (i32, i32),
    /// Damage of the previous frames (newest last), for buffer age.
    history: Vec<Rect>,
}

impl GlContext {
    /// Creates the context on the connection's display.
    pub fn new(display_ptr: *mut c_void, probe: &WlSurface) -> Result<GlContext, String> {
        // SAFETY: loading libEGL runs its initialisers; nothing else runs meanwhile.
        let egl = unsafe { egl::DynamicInstance::<egl::EGL1_5>::load_required() }
            .map_err(|e| format!("libEGL: {e}"))?;
        // SAFETY: `display_ptr` is the live `wl_display` of the connection, which outlives
        // the EGL display (the platform drops the context before the connection).
        let display = unsafe {
            egl.get_platform_display(PLATFORM_WAYLAND, display_ptr, &[egl::ATTRIB_NONE])
        }
        .map_err(|e| format!("eglGetPlatformDisplay: {e}"))?;
        egl.initialize(display)
            .map_err(|e| format!("eglInitialize: {e}"))?;
        egl.bind_api(egl::OPENGL_ES_API)
            .map_err(|e| format!("eglBindAPI: {e}"))?;
        let config = egl
            .choose_first_config(
                display,
                &[
                    egl::SURFACE_TYPE,
                    egl::WINDOW_BIT,
                    egl::RENDERABLE_TYPE,
                    egl::OPENGL_ES2_BIT,
                    egl::RED_SIZE,
                    8,
                    egl::GREEN_SIZE,
                    8,
                    egl::BLUE_SIZE,
                    8,
                    egl::ALPHA_SIZE,
                    8,
                    egl::STENCIL_SIZE,
                    8,
                    egl::NONE,
                ],
            )
            .map_err(|e| format!("eglChooseConfig: {e}"))?
            .ok_or("eglChooseConfig: no RGBA8 configuration")?;
        let context = egl
            .create_context(display, config, None, &[egl::CONTEXT_CLIENT_VERSION, 2, egl::NONE])
            .map_err(|e| format!("eglCreateContext: {e}"))?;
        // A probe surface makes the context current to create Skia's interface.
        let probe_window = WlEglSurface::new(probe.id(), 1, 1).map_err(|e| e.to_string())?;
        // SAFETY: the wl_egl_window is alive for the duration of this function.
        let probe_surface = unsafe {
            egl.create_platform_window_surface(
                display,
                config,
                probe_window.ptr().cast_mut(),
                &[egl::ATTRIB_NONE],
            )
        }
        .map_err(|e| format!("eglCreatePlatformWindowSurface: {e}"))?;
        egl.make_current(display, Some(probe_surface), Some(probe_surface), Some(context))
            .map_err(|e| format!("eglMakeCurrent: {e}"))?;
        let _ = egl.swap_interval(display, 0);
        let interface = skgl::Interface::new_load_with(|name| {
            egl.get_proc_address(name)
                .map_or(std::ptr::null(), |f| f as *const c_void)
        })
        .ok_or("Skia GL interface")?;
        let skia = gpu::direct_contexts::make_gl(interface, None).ok_or("Skia GL context")?;
        let renderer = gl_renderer(&egl);
        let extensions = egl
            .query_string(Some(display), egl::EXTENSIONS)
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let swap_with_damage = if extensions.contains("EGL_KHR_swap_buffers_with_damage") {
            egl.get_proc_address("eglSwapBuffersWithDamageKHR")
                // SAFETY: the extension's entry point has this signature.
                .map(|f| unsafe { std::mem::transmute::<extern "system" fn(), SwapWithDamageFn>(f) })
        } else {
            None
        };
        let buffer_age = extensions.contains("EGL_EXT_buffer_age");
        let _ = egl.make_current(display, None, None, None);
        let _ = egl.destroy_surface(display, probe_surface);
        drop(probe_window);
        Ok(GlContext {
            egl,
            display,
            config,
            context,
            skia,
            swap_with_damage,
            buffer_age,
            renderer,
        })
    }

    /// The GL renderer string (driver).
    pub fn renderer(&self) -> &str {
        &self.renderer
    }

    /// Whether the driver renders in software (llvmpipe).
    pub fn is_software(&self) -> bool {
        let r = self.renderer.to_ascii_lowercase();
        r.contains("llvmpipe") || r.contains("softpipe") || r.contains("swrast")
    }

    /// An EGL surface for a window.
    pub fn create_window(&self, surface: &WlSurface, size: (i32, i32)) -> Result<GlWindow, String> {
        let window = WlEglSurface::new(surface.id(), size.0.max(1), size.1.max(1))
            .map_err(|e| format!("wl_egl_window: {e}"))?;
        // SAFETY: the wl_egl_window outlives the EGL surface (both in GlWindow, the surface
        // destroyed first in `destroy_window`).
        let egl_surface = unsafe {
            self.egl.create_platform_window_surface(
                self.display,
                self.config,
                window.ptr().cast_mut(),
                &[egl::ATTRIB_NONE],
            )
        }
        .map_err(|e| format!("eglCreatePlatformWindowSurface: {e}"))?;
        Ok(GlWindow {
            window,
            surface: egl_surface,
            size,
            history: Vec::new(),
        })
    }

    /// Destroys a window's EGL surface.
    pub fn destroy_window(&mut self, w: GlWindow) {
        let _ = self.egl.make_current(self.display, None, None, None);
        let _ = self.egl.destroy_surface(self.display, w.surface);
        drop(w.window);
    }

    /// Draws a frame: `draw` gets a canvas in device pixels and the region to repaint
    /// (device pixels; `None` = everything), then the buffer is swapped with `damage`
    /// (device pixels, top-left origin). The caller has requested the frame callback.
    pub fn render(
        &mut self,
        w: &mut GlWindow,
        size: (i32, i32),
        damage: Rect,
        draw: impl FnOnce(&skia_safe::Canvas, Option<Rect>),
    ) -> Result<(), String> {
        if w.size != size {
            w.window.resize(size.0.max(1), size.1.max(1), 0, 0);
            w.size = size;
            w.history.clear();
        }
        self.egl
            .make_current(self.display, Some(w.surface), Some(w.surface), Some(self.context))
            .map_err(|e| format!("eglMakeCurrent: {e}"))?;
        let age = if self.buffer_age {
            self.egl
                .query_surface(self.display, w.surface, BUFFER_AGE)
                .unwrap_or(0)
        } else {
            0
        };
        let full = Rect::new(0.0, 0.0, size.0 as f32, size.1 as f32);
        // The repaint region: this frame's damage plus the damage of the frames since the
        // buffer was last drawn; unknown age (0) or an old buffer repaints everything.
        let repaint = match usize::try_from(age) {
            Ok(a) if a >= 1 && a <= w.history.len() + 1 => {
                let mut r = damage;
                for d in w.history.iter().rev().take(a - 1) {
                    r = r.union(*d);
                }
                Some(r.intersect(full))
            }
            _ => None,
        };
        let target = gpu::backend_render_targets::make_gl(
            size,
            None,
            8,
            skgl::FramebufferInfo {
                fboid: 0,
                format: GL_RGBA8,
                ..Default::default()
            },
        );
        let mut surface = gpu::surfaces::wrap_backend_render_target(
            &mut self.skia,
            &target,
            SurfaceOrigin::BottomLeft,
            skia_safe::ColorType::RGBA8888,
            None,
            None,
        )
        .ok_or("Skia: cannot wrap the framebuffer")?;
        draw(surface.canvas(), repaint);
        self.skia.flush_and_submit();
        drop(surface);
        w.history.push(damage);
        if w.history.len() > 4 {
            w.history.remove(0);
        }
        let swapped = match (self.swap_with_damage, repaint.is_some()) {
            (Some(swap), _) if !damage.is_empty() => {
                // Bottom-left origin for EGL.
                let r = damage.round_out(1.0);
                let rect = [
                    r.x as egl::Int,
                    (size.1 as f32 - r.bottom()) as egl::Int,
                    r.width as egl::Int,
                    r.height as egl::Int,
                ];
                // SAFETY: display and surface are live; `rect` holds one rectangle.
                unsafe {
                    swap(
                        self.display.as_ptr(),
                        w.surface.as_ptr(),
                        rect.as_ptr(),
                        1,
                    ) == egl::TRUE
                }
            }
            _ => self.egl.swap_buffers(self.display, w.surface).is_ok(),
        };
        if swapped {
            Ok(())
        } else {
            Err("eglSwapBuffers failed".into())
        }
    }
}

fn gl_renderer(egl: &egl::DynamicInstance<egl::EGL1_5>) -> String {
    type GetString = unsafe extern "C" fn(u32) -> *const std::ffi::c_char;
    let Some(f) = egl.get_proc_address("glGetString") else {
        return String::new();
    };
    // SAFETY: glGetString has this signature.
    let get: GetString = unsafe { std::mem::transmute::<extern "system" fn(), GetString>(f) };
    // SAFETY: a current context exists (the caller made it current).
    let p = unsafe { get(0x1F01) };
    if p.is_null() {
        return String::new();
    }
    // SAFETY: glGetString returns a NUL-terminated static string.
    unsafe { std::ffi::CStr::from_ptr(p) }
        .to_string_lossy()
        .into_owned()
}
