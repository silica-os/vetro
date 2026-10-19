//! The Vulkan backend (primary): one instance, device and Skia context for the process, a
//! `VkSurfaceKHR` and swapchain per window. Ported from silica-desktop's embedder
//! (`ffi/vulkan.rs`), which runs on the dev VM's Venus driver.
//!
//! Frames are pipelined: up to [`FRAMES_IN_FLIGHT`] frames are on the GPU while the CPU
//! records the next. Each frame slot has an acquire semaphore, a render-done semaphore and
//! a fence. No semaphore crosses Skia's flush (its semaphore API is not wrapped safely by
//! skia-safe): the swapchain image is acquired with the slot's semaphore, which an empty
//! batch waits on before Skia's work is submitted (a wait covers the commands submitted
//! after it), and an empty batch after Skia's submits signals the render-done semaphore
//! (a signal covers the commands submitted before it) and the slot's fence. The present
//! waits on render-done; the CPU waits only for the slot's previous frame.
//!
//! Partial repaints: swapchain images do not reliably keep their contents (Venus, other
//! drivers), so each window keeps a back surface on the GPU that only the damaged region is
//! repainted into; the whole back surface is then copied onto the acquired image.

#![allow(unsafe_code)]

use std::ffi::{CStr, c_char, c_void};

use ash::vk;
use ash::vk::Handle;
use skia_safe::gpu::{self, DirectContext, SurfaceOrigin, vk as skvk};
use wayland_client::Proxy;
use wayland_client::protocol::wl_surface::WlSurface;

const INSTANCE_EXTENSIONS: &[&CStr] = &[ash::khr::surface::NAME, ash::khr::wayland_surface::NAME];
const DEVICE_EXTENSIONS: &[&CStr] = &[ash::khr::swapchain::NAME];

/// Frames recorded ahead of the GPU.
pub const FRAMES_IN_FLIGHT: usize = 2;

/// The synchronisation objects of one frame slot.
#[derive(Debug, Clone, Copy)]
struct Slot {
    /// Signalled when the swapchain image is ready to be drawn.
    acquired: vk::Semaphore,
    /// Signalled when the frame's GPU work is done (the present waits on it).
    rendered: vk::Semaphore,
    /// Signalled with `rendered`: the slot can be reused.
    done: vk::Fence,
}

/// The process's Vulkan context.
pub struct VkContext {
    _drivers: Vec<libloading::Library>,
    _entry: ash::Entry,
    instance: ash::Instance,
    physical: vk::PhysicalDevice,
    device: ash::Device,
    queue: vk::Queue,
    queue_family: u32,
    surface_fn: ash::khr::surface::Instance,
    wayland_fn: ash::khr::wayland_surface::Instance,
    swapchain_fn: ash::khr::swapchain::Device,
    skia: DirectContext,
    display: usize,
    name: String,
}

/// One window's surface and swapchain.
pub struct VkWindow {
    surface: vk::SurfaceKHR,
    swapchain: vk::SwapchainKHR,
    images: Vec<vk::Image>,
    format: vk::Format,
    color_type: skia_safe::ColorType,
    extent: (i32, i32),
    slots: Vec<Slot>,
    next: usize,
    /// The retained frame: the damaged region is repainted into it, then it is copied to
    /// the swapchain image.
    back: Option<skia_safe::Surface>,
}

fn err(call: &str) -> impl Fn(vk::Result) -> String + '_ {
    move |e| format!("{call}: {e:?}")
}

impl VkContext {
    /// Creates the context; `probe` is a window's surface, to pick a GPU that presents on it.
    pub fn new(
        conn: &wayland_client::Connection,
        probe: &WlSurface,
    ) -> Result<(VkContext, VkWindow), String> {
        // SAFETY: loads the system Vulkan loader, which exposes the standard API.
        let entry = unsafe { ash::Entry::load() }.map_err(|e| format!("libvulkan: {e}"))?;
        let app = vk::ApplicationInfo::default()
            .application_name(c"vetro")
            .api_version(vk::API_VERSION_1_1);
        let ext: Vec<*const c_char> = INSTANCE_EXTENSIONS.iter().map(|e| e.as_ptr()).collect();
        let (instance, drivers) = create_instance(&entry, &app, &ext)?;
        let surface_fn = ash::khr::surface::Instance::new(&entry, &instance);
        let wayland_fn = ash::khr::wayland_surface::Instance::new(&entry, &instance);
        let display = conn.backend().display_ptr() as usize;
        let surface = create_surface(&wayland_fn, display, probe)?;
        let Some((physical, queue_family, name)) = pick_device(&instance, &surface_fn, surface)
        else {
            // SAFETY: the surface and instance are ours and unused.
            unsafe {
                surface_fn.destroy_surface(surface, None);
                instance.destroy_instance(None);
            }
            return Err("no Vulkan GPU presents on this surface".into());
        };
        let priorities = [1.0f32];
        let queues = [vk::DeviceQueueCreateInfo::default()
            .queue_family_index(queue_family)
            .queue_priorities(&priorities)];
        let dext: Vec<*const c_char> = DEVICE_EXTENSIONS.iter().map(|e| e.as_ptr()).collect();
        let info = vk::DeviceCreateInfo::default()
            .queue_create_infos(&queues)
            .enabled_extension_names(&dext);
        // SAFETY: physical comes from this instance; info valid for the call.
        let device = unsafe { instance.create_device(physical, &info, None) }
            .map_err(err("vkCreateDevice"))?;
        // SAFETY: the family has one queue.
        let queue = unsafe { device.get_device_queue(queue_family, 0) };
        let swapchain_fn = ash::khr::swapchain::Device::new(&instance, &device);
        let get_instance = entry.static_fn().get_instance_proc_addr;
        let get_device = instance.fp_v1_0().get_device_proc_addr;
        let get_proc = move |of: skvk::GetProcOf| -> skvk::GetProcResult {
            // SAFETY: Skia passes valid handles of this instance/device and NUL-terminated
            // names; the loader functions have these signatures.
            unsafe {
                match of {
                    skvk::GetProcOf::Instance(i, name) => {
                        // Calling the global queries (no instance) makes the loader open every
                        // installed driver: Skia gets this context's answers instead.
                        if i.is_null() {
                            let wanted = CStr::from_ptr(name);
                            tracing::debug!("Vulkan global query {wanted:?}");
                            if wanted == c"vkEnumerateInstanceVersion" {
                                return enumerate_instance_version as *const c_void;
                            }
                            if wanted == c"vkEnumerateInstanceExtensionProperties" {
                                return enumerate_instance_extensions as *const c_void;
                            }
                            if wanted == c"vkEnumerateInstanceLayerProperties" {
                                return enumerate_instance_layers as *const c_void;
                            }
                        }
                        let i = vk::Instance::from_raw(i as u64);
                        get_instance(i, name).map_or(std::ptr::null(), |f| f as *const c_void)
                    }
                    skvk::GetProcOf::Device(d, name) => {
                        let d = vk::Device::from_raw(d as u64);
                        get_device(d, name).map_or(std::ptr::null(), |f| f as *const c_void)
                    }
                }
            }
        };
        let iext: Vec<&str> = INSTANCE_EXTENSIONS
            .iter()
            .filter_map(|e| e.to_str().ok())
            .collect();
        let dexts: Vec<&str> = DEVICE_EXTENSIONS
            .iter()
            .filter_map(|e| e.to_str().ok())
            .collect();
        // SAFETY: the handles outlive the backend context (destroyed in Drop after Skia).
        let backend = unsafe {
            skvk::BackendContext::new_builder(
                instance.handle().as_raw() as _,
                physical.as_raw() as _,
                device.handle().as_raw() as _,
                (queue.as_raw() as _, queue_family as usize),
                &get_proc,
                Some(skvk::Version::new(1, 1, 0)),
            )
            .with_extensions(&iext, &dexts)
            .build()
        };
        // Fewer shader variations: fewer pipelines to compile before the first frames
        // (seconds on slow drivers, the VM's Venus among them).
        let mut options = gpu::ContextOptions::new();
        options.reduced_shader_variations = true;
        let skia =
            gpu::direct_contexts::make_vulkan(&backend, &options).ok_or("Skia Vulkan context")?;
        drop(backend);
        let ctx = VkContext {
            _drivers: drivers,
            _entry: entry,
            instance,
            physical,
            device,
            queue,
            queue_family,
            surface_fn,
            wayland_fn,
            swapchain_fn,
            skia,
            display,
            name,
        };
        let window = ctx.window_from_surface(surface)?;
        Ok((ctx, window))
    }

    /// The GPU's name.
    /// Compiles the pipelines a typical frame needs (rounded rectangles with antialiasing,
    /// strokes, gradients, images, text, layers with opacity, rounded clips) by drawing them
    /// once offscreen, after the first frame: skia-safe has no binding for Skia's persistent
    /// shader cache, so the first menu or popup would otherwise compile them mid-animation.
    pub fn warm_up(&mut self) {
        let ii = skia_safe::ImageInfo::new_n32_premul((96, 96), None);
        let Some(mut surface) = gpu::surfaces::render_target(
            &mut self.skia,
            gpu::Budgeted::Yes,
            &ii,
            None,
            SurfaceOrigin::TopLeft,
            None,
            false,
            None,
        ) else {
            return;
        };
        let canvas = surface.canvas();
        canvas.clear(skia_safe::Color::TRANSPARENT);
        let mut p = skia_safe::Paint::default();
        p.set_anti_alias(true);
        p.set_color(skia_safe::Color::from_argb(200, 30, 120, 220));
        let r = skia_safe::Rect::from_xywh(4.0, 4.0, 60.0, 30.0);
        canvas.draw_rrect(skia_safe::RRect::new_rect_xy(r, 8.0, 8.0), &p);
        canvas.draw_circle((70.0, 20.0), 10.0, &p);
        p.set_style(skia_safe::paint::Style::Stroke);
        p.set_stroke_width(1.5);
        canvas.draw_rrect(skia_safe::RRect::new_rect_xy(r, 8.0, 8.0), &p);
        p.set_style(skia_safe::paint::Style::Fill);
        {
            use skia_safe::gradient::{Colors, Gradient, Interpolation, shaders};
            let colors = [
                skia_safe::Color4f::from(skia_safe::Color::WHITE),
                skia_safe::Color4f::from(skia_safe::Color::BLACK),
            ];
            let gradient = Gradient::new(
                Colors::new(&colors, None, skia_safe::TileMode::Clamp, None),
                Interpolation::default(),
            );
            if let Some(shader) =
                shaders::linear_gradient(((0.0, 0.0), (0.0, 96.0)), &gradient, None)
            {
                p.set_shader(shader);
                canvas.draw_rect(skia_safe::Rect::from_xywh(0.0, 40.0, 40.0, 20.0), &p);
                p.set_shader(None);
            }
        }
        canvas.save();
        canvas.clip_rrect(
            skia_safe::RRect::new_rect_xy(
                skia_safe::Rect::from_xywh(40.0, 40.0, 50.0, 50.0),
                12.0,
                12.0,
            ),
            skia_safe::ClipOp::Intersect,
            true,
        );
        canvas.save_layer_alpha_f(None, 0.5);
        canvas.draw_paint(&p);
        canvas.restore();
        canvas.restore();
        let snapshot = surface.image_snapshot();
        let canvas = surface.canvas();
        canvas.draw_image_rect_with_sampling_options(
            &snapshot,
            None,
            skia_safe::Rect::from_xywh(10.0, 60.0, 30.0, 30.0),
            skia_safe::SamplingOptions::new(
                skia_safe::FilterMode::Linear,
                skia_safe::MipmapMode::None,
            ),
            &p,
        );
        let font = skia_safe::Font::default();
        canvas.draw_str("Aa", (10.0, 90.0), &font, &p);
        self.skia.flush_and_submit();
    }

    pub fn renderer(&self) -> &str {
        &self.name
    }

    /// A window for another surface.
    pub fn create_window(&self, wl: &WlSurface) -> Result<VkWindow, String> {
        let surface = create_surface(&self.wayland_fn, self.display, wl)?;
        // SAFETY: valid handles of this instance.
        let supported = unsafe {
            self.surface_fn.get_physical_device_surface_support(
                self.physical,
                self.queue_family,
                surface,
            )
        }
        .unwrap_or(false);
        if !supported {
            // SAFETY: our surface, unused.
            unsafe { self.surface_fn.destroy_surface(surface, None) };
            return Err("the GPU does not present on this surface".into());
        }
        self.window_from_surface(surface)
    }

    fn new_slot(&self) -> Result<Slot, String> {
        // SAFETY: valid device; the objects are destroyed with the window.
        unsafe {
            let acquired = self
                .device
                .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)
                .map_err(err("vkCreateSemaphore"))?;
            let rendered = self
                .device
                .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)
                .map_err(err("vkCreateSemaphore"))?;
            let done = self
                .device
                .create_fence(
                    &vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),
                    None,
                )
                .map_err(err("vkCreateFence"))?;
            Ok(Slot {
                acquired,
                rendered,
                done,
            })
        }
    }

    fn destroy_slot(&self, slot: Slot) {
        // SAFETY: the queue is idle (callers wait) and the objects are ours.
        unsafe {
            self.device.destroy_semaphore(slot.acquired, None);
            self.device.destroy_semaphore(slot.rendered, None);
            self.device.destroy_fence(slot.done, None);
        }
    }

    fn window_from_surface(&self, surface: vk::SurfaceKHR) -> Result<VkWindow, String> {
        let mut slots = Vec::with_capacity(FRAMES_IN_FLIGHT);
        for _ in 0..FRAMES_IN_FLIGHT {
            match self.new_slot() {
                Ok(s) => slots.push(s),
                Err(e) => {
                    for s in slots {
                        self.destroy_slot(s);
                    }
                    return Err(e);
                }
            }
        }
        Ok(VkWindow {
            surface,
            swapchain: vk::SwapchainKHR::null(),
            images: Vec::new(),
            format: vk::Format::B8G8R8A8_UNORM,
            color_type: skia_safe::ColorType::BGRA8888,
            extent: (0, 0),
            slots,
            next: 0,
            back: None,
        })
    }

    /// Destroys a window's swapchain and surface.
    pub fn destroy_window(&mut self, w: VkWindow) {
        // SAFETY: the queue is idle after the wait; the handles are ours.
        unsafe {
            let _ = self.device.queue_wait_idle(self.queue);
            if w.swapchain != vk::SwapchainKHR::null() {
                self.swapchain_fn.destroy_swapchain(w.swapchain, None);
            }
            self.surface_fn.destroy_surface(w.surface, None);
        }
        for s in w.slots {
            self.destroy_slot(s);
        }
    }

    /// Replaces a slot whose semaphores may be left signalled (a present that failed).
    fn reset_slot(&self, w: &mut VkWindow, i: usize) {
        // SAFETY: valid queue.
        unsafe {
            let _ = self.device.queue_wait_idle(self.queue);
        }
        if let Ok(fresh) = self.new_slot()
            && let Some(slot) = w.slots.get_mut(i)
        {
            let old = std::mem::replace(slot, fresh);
            self.destroy_slot(old);
        }
    }

    fn rebuild(&self, w: &mut VkWindow, size: (i32, i32)) -> Result<(), String> {
        // SAFETY: valid handles; the queue is idle before the old swapchain is retired.
        unsafe {
            let caps = self
                .surface_fn
                .get_physical_device_surface_capabilities(self.physical, w.surface)
                .map_err(err("surface capabilities"))?;
            let formats = self
                .surface_fn
                .get_physical_device_surface_formats(self.physical, w.surface)
                .map_err(err("surface formats"))?;
            let modes = self
                .surface_fn
                .get_physical_device_surface_present_modes(self.physical, w.surface)
                .map_err(err("present modes"))?;
            let (format, color_type) = if let Some(f) = formats
                .iter()
                .find(|f| f.format == vk::Format::B8G8R8A8_UNORM)
            {
                (*f, skia_safe::ColorType::BGRA8888)
            } else if let Some(f) = formats
                .iter()
                .find(|f| f.format == vk::Format::R8G8B8A8_UNORM)
            {
                (*f, skia_safe::ColorType::RGBA8888)
            } else {
                return Err("no 8-bit BGRA or RGBA surface format".into());
            };
            // Frames are paced by wl_surface.frame callbacks: no waiting in the present.
            let mode = [vk::PresentModeKHR::MAILBOX, vk::PresentModeKHR::IMMEDIATE]
                .into_iter()
                .find(|m| modes.contains(m))
                .unwrap_or(vk::PresentModeKHR::FIFO);
            let alpha = [
                vk::CompositeAlphaFlagsKHR::PRE_MULTIPLIED,
                vk::CompositeAlphaFlagsKHR::INHERIT,
                vk::CompositeAlphaFlagsKHR::OPAQUE,
            ]
            .into_iter()
            .find(|a| caps.supported_composite_alpha.contains(*a))
            .unwrap_or(vk::CompositeAlphaFlagsKHR::OPAQUE);
            let extent = if caps.current_extent.width != u32::MAX {
                caps.current_extent
            } else {
                vk::Extent2D {
                    width: (size.0.max(1) as u32).clamp(
                        caps.min_image_extent.width.max(1),
                        caps.max_image_extent.width.max(1),
                    ),
                    height: (size.1.max(1) as u32).clamp(
                        caps.min_image_extent.height.max(1),
                        caps.max_image_extent.height.max(1),
                    ),
                }
            };
            let mut count = (caps.min_image_count + 1).max(3);
            if caps.max_image_count > 0 {
                count = count.min(caps.max_image_count);
            }
            let usage = vk::ImageUsageFlags::COLOR_ATTACHMENT
                | (caps.supported_usage_flags
                    & (vk::ImageUsageFlags::TRANSFER_SRC
                        | vk::ImageUsageFlags::TRANSFER_DST
                        | vk::ImageUsageFlags::SAMPLED));
            let info = vk::SwapchainCreateInfoKHR::default()
                .surface(w.surface)
                .min_image_count(count)
                .image_format(format.format)
                .image_color_space(format.color_space)
                .image_extent(extent)
                .image_array_layers(1)
                .image_usage(usage)
                .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
                .pre_transform(vk::SurfaceTransformFlagsKHR::IDENTITY)
                .composite_alpha(alpha)
                .present_mode(mode)
                .clipped(true)
                .old_swapchain(w.swapchain);
            let _ = self.device.queue_wait_idle(self.queue);
            let created = self.swapchain_fn.create_swapchain(&info, None);
            if w.swapchain != vk::SwapchainKHR::null() {
                self.swapchain_fn.destroy_swapchain(w.swapchain, None);
                w.swapchain = vk::SwapchainKHR::null();
            }
            w.swapchain = created.map_err(err("vkCreateSwapchainKHR"))?;
            w.images = self
                .swapchain_fn
                .get_swapchain_images(w.swapchain)
                .map_err(err("swapchain images"))?;
            w.format = format.format;
            w.color_type = color_type;
            w.extent = (extent.width as i32, extent.height as i32);
        }
        Ok(())
    }

    /// Draws and presents a frame of `size` device pixels: `draw` gets the back surface's
    /// canvas and the region to repaint (`damage`, device pixels; `None` = everything, as
    /// after a resize). The caller has requested the frame callback (the present commits the
    /// surface).
    pub fn render(
        &mut self,
        w: &mut VkWindow,
        size: (i32, i32),
        damage: crate::geom::Rect,
        draw: impl FnOnce(&skia_safe::Canvas, Option<crate::geom::Rect>),
    ) -> Result<(), String> {
        if w.swapchain == vk::SwapchainKHR::null() || w.extent != size {
            self.rebuild(w, size)?;
            w.back = None;
        }
        let slot_index = w.next % w.slots.len().max(1);
        let Some(slot) = w.slots.get(slot_index).copied() else {
            return Err("no frame slot".into());
        };
        // The slot's previous frame must be done before its objects are reused.
        // SAFETY: the slot's fence, signalled by its last frame (or created signalled).
        unsafe {
            let _ = self
                .device
                .wait_for_fences(&[slot.done], true, 1_000_000_000);
        }
        // SAFETY: valid swapchain; the slot's acquire semaphore is unsignalled (its last
        // wait has completed with the fence above).
        let acquire = |this: &Self, w: &VkWindow| unsafe {
            this.swapchain_fn.acquire_next_image(
                w.swapchain,
                1_000_000_000,
                slot.acquired,
                vk::Fence::null(),
            )
        };
        let index = match acquire(self, w) {
            Ok((i, _)) => i,
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                self.rebuild(w, size)?;
                acquire(self, w).map_err(err("vkAcquireNextImageKHR"))?.0
            }
            Err(e) => return Err(format!("vkAcquireNextImageKHR: {e:?}")),
        };
        let Some(image) = w.images.get(index as usize).copied() else {
            return Err("swapchain image index out of range".into());
        };
        // The GPU work submitted from here on waits for the image.
        let wait_stage = [vk::PipelineStageFlags::ALL_COMMANDS];
        let waits = [slot.acquired];
        let wait = vk::SubmitInfo::default()
            .wait_semaphores(&waits)
            .wait_dst_stage_mask(&wait_stage);
        // SAFETY: valid queue and semaphore; the batch has no command buffers.
        unsafe {
            self.device
                .queue_submit(self.queue, &[wait], vk::Fence::null())
        }
        .map_err(err("vkQueueSubmit (acquire)"))?;
        // SAFETY: the image belongs to the live swapchain; the info lives for the call.
        let info = unsafe {
            skvk::ImageInfo::new(
                image.as_raw() as _,
                skvk::Alloc::default(),
                skvk::ImageTiling::OPTIMAL,
                skvk::ImageLayout::UNDEFINED,
                std::mem::transmute::<i32, skvk::Format>(w.format.as_raw()),
                1,
                self.queue_family,
                None,
                None,
                None,
            )
        };
        // The retained frame: repaint the damage only.
        let full = w.back.is_none();
        if w.back.is_none() {
            let ii = skia_safe::ImageInfo::new(
                w.extent,
                w.color_type,
                skia_safe::AlphaType::Premul,
                None,
            );
            w.back = gpu::surfaces::render_target(
                &mut self.skia,
                gpu::Budgeted::Yes,
                &ii,
                None,
                SurfaceOrigin::TopLeft,
                None,
                false,
                None,
            );
        }
        let target = gpu::backend_render_targets::make_vk(w.extent, &info);
        let mut surface = gpu::surfaces::wrap_backend_render_target(
            &mut self.skia,
            &target,
            SurfaceOrigin::TopLeft,
            w.color_type,
            None,
            None,
        )
        .ok_or("Skia: cannot wrap the swapchain image")?;
        match w.back.as_mut() {
            Some(back) => {
                let repaint = (!full && !damage.is_empty()).then_some(damage);
                draw(back.canvas(), repaint);
                let mut copy = skia_safe::Paint::default();
                copy.set_blend_mode(skia_safe::BlendMode::Src);
                back.draw(
                    surface.canvas(),
                    (0, 0),
                    skia_safe::SamplingOptions::default(),
                    Some(&copy),
                );
            }
            // No offscreen surface: the whole frame straight onto the image.
            None => draw(surface.canvas(), None),
        }
        self.skia.flush_surface_with_access(
            &mut surface,
            skia_safe::surface::BackendSurfaceAccess::Present,
            &gpu::FlushInfo::default(),
        );
        self.skia.submit(gpu::SyncCpu::No);
        drop(surface);
        // Signalled once everything submitted before is done.
        let signals = [slot.rendered];
        let signal = vk::SubmitInfo::default().signal_semaphores(&signals);
        // SAFETY: valid queue, semaphore and fence (waited on above, reset here).
        unsafe {
            let _ = self.device.reset_fences(&[slot.done]);
            self.device
                .queue_submit(self.queue, &[signal], slot.done)
                .map_err(err("vkQueueSubmit (done)"))?;
        }
        w.next = w.next.wrapping_add(1);
        let swapchains = [w.swapchain];
        let indices = [index];
        let present = vk::PresentInfoKHR::default()
            .wait_semaphores(&signals)
            .swapchains(&swapchains)
            .image_indices(&indices);
        // SAFETY: the present waits for the frame's work; the image is in the present
        // layout (Skia's Present access).
        match unsafe { self.swapchain_fn.queue_present(self.queue, &present) } {
            Ok(_) => Ok(()),
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                // The wait may not have happened: the slot gets fresh semaphores.
                self.reset_slot(w, slot_index);
                Ok(())
            }
            Err(e) => {
                self.reset_slot(w, slot_index);
                Err(format!("vkQueuePresentKHR: {e:?}"))
            }
        }
    }
}

impl Drop for VkContext {
    fn drop(&mut self) {
        // SAFETY: everything is idle; Skia's context is abandoned before the device goes.
        unsafe {
            let _ = self.device.device_wait_idle();
            self.skia.abandon();
            self.device.destroy_device(None);
            self.instance.destroy_instance(None);
        }
    }
}

/// The instance version Skia asks for (the one this context's instance was made with).
unsafe extern "system" fn enumerate_instance_version(version: *mut u32) -> vk::Result {
    if version.is_null() {
        return vk::Result::ERROR_INITIALIZATION_FAILED;
    }
    // SAFETY: Vulkan's contract: a valid pointer to write the version into.
    unsafe { *version = vk::API_VERSION_1_1 };
    vk::Result::SUCCESS
}

/// The instance extensions Skia asks for: the ones this context's instance enabled.
unsafe extern "system" fn enumerate_instance_extensions(
    layer: *const c_char,
    count: *mut u32,
    props: *mut vk::ExtensionProperties,
) -> vk::Result {
    if count.is_null() {
        return vk::Result::ERROR_INITIALIZATION_FAILED;
    }
    let list: &[&CStr] = if layer.is_null() {
        INSTANCE_EXTENSIONS
    } else {
        &[]
    };
    // SAFETY: Vulkan's two-call contract: `count` is valid; `props`, when not null, holds
    // `*count` elements.
    unsafe {
        if props.is_null() {
            *count = list.len() as u32;
            return vk::Result::SUCCESS;
        }
        let n = (*count as usize).min(list.len());
        for (i, name) in list.iter().take(n).enumerate() {
            let mut p = vk::ExtensionProperties::default();
            for (d, b) in p.extension_name.iter_mut().zip(name.to_bytes()) {
                *d = *b as c_char;
            }
            p.spec_version = 1;
            *props.add(i) = p;
        }
        *count = n as u32;
        if n < list.len() {
            vk::Result::INCOMPLETE
        } else {
            vk::Result::SUCCESS
        }
    }
}

/// No layers are enabled on this context's instance.
unsafe extern "system" fn enumerate_instance_layers(
    count: *mut u32,
    _props: *mut vk::LayerProperties,
) -> vk::Result {
    if count.is_null() {
        return vk::Result::ERROR_INITIALIZATION_FAILED;
    }
    // SAFETY: Vulkan's contract: a valid pointer to the count.
    unsafe { *count = 0 };
    vk::Result::SUCCESS
}

/// `vk_icdGetInstanceProcAddr`, the loader-driver interface's entry point.
type DriverEntry = unsafe extern "system" fn(vk::Instance, *const c_char) -> vk::PFN_vkVoidFunction;

/// The instance, with only the drivers of this machine's GPUs when they are known
/// (silica-desktop#191: the loader would otherwise open every installed driver, seconds on
/// the VM). The drivers loaded here stay loaded as long as the instance.
fn create_instance(
    entry: &ash::Entry,
    app: &vk::ApplicationInfo<'_>,
    extensions: &[*const c_char],
) -> Result<(ash::Instance, Vec<libloading::Library>), String> {
    let mut libraries = Vec::new();
    let mut entries: Vec<DriverEntry> = Vec::new();
    let chosen = super::vulkan_drivers::for_this_machine();
    tracing::info!(
        "Vulkan drivers for this machine: {:?}",
        chosen.as_ref().map(|d| d
            .iter()
            .map(|x| x.library.display().to_string())
            .collect::<Vec<_>>())
    );
    for d in chosen.unwrap_or_default() {
        // SAFETY: a Vulkan driver named by its installed manifest, the library the loader
        // itself would open.
        let Ok(lib) = (unsafe { libloading::Library::new(&d.library) }) else {
            continue;
        };
        // SAFETY: the loader-driver interface symbol has this signature.
        if let Ok(get) = unsafe { lib.get::<DriverEntry>(b"vk_icdGetInstanceProcAddr\0") } {
            entries.push(*get);
            libraries.push(lib);
        }
    }
    if !entries.is_empty() {
        let infos: Vec<vk::DirectDriverLoadingInfoLUNARG<'_>> = entries
            .iter()
            .map(|&g| {
                vk::DirectDriverLoadingInfoLUNARG::default().pfn_get_instance_proc_addr(Some(g))
            })
            .collect();
        let mut list = vk::DirectDriverLoadingListLUNARG::default()
            .mode(vk::DirectDriverLoadingModeLUNARG::EXCLUSIVE)
            .drivers(&infos);
        let mut with_direct = extensions.to_vec();
        with_direct.push(ash::lunarg::direct_driver_loading::NAME.as_ptr());
        let create = vk::InstanceCreateInfo::default()
            .application_info(app)
            .enabled_extension_names(&with_direct)
            .push_next(&mut list);
        // SAFETY: valid structure; the drivers stay loaded with the instance.
        match unsafe { entry.create_instance(&create, None) } {
            Ok(i) => {
                tracing::info!("Vulkan: {} driver(s) loaded directly", libraries.len());
                return Ok((i, libraries));
            }
            Err(e) => tracing::info!("Vulkan with this machine's drivers only: {e:?}, loading all"),
        }
    }
    let create = vk::InstanceCreateInfo::default()
        .application_info(app)
        .enabled_extension_names(extensions);
    // SAFETY: valid structure, pointers alive for the call.
    let instance =
        unsafe { entry.create_instance(&create, None) }.map_err(err("vkCreateInstance"))?;
    Ok((instance, Vec::new()))
}

fn create_surface(
    wayland_fn: &ash::khr::wayland_surface::Instance,
    display: usize,
    surface: &WlSurface,
) -> Result<vk::SurfaceKHR, String> {
    let info = vk::WaylandSurfaceCreateInfoKHR::default()
        .display(display as *mut vk::wl_display)
        .surface(surface.id().as_ptr().cast::<vk::wl_surface>());
    // SAFETY: display and surface are the connection's live libwayland objects, which outlive
    // the Vulkan surface (destroyed in destroy_window before the wl_surface).
    unsafe { wayland_fn.create_wayland_surface(&info, None) }
        .map_err(err("vkCreateWaylandSurfaceKHR"))
}

/// The GPU that presents on this surface: integrated, then discrete, never the CPU when
/// there is another (the embedder's choice).
fn pick_device(
    instance: &ash::Instance,
    surface_fn: &ash::khr::surface::Instance,
    surface: vk::SurfaceKHR,
) -> Option<(vk::PhysicalDevice, u32, String)> {
    // SAFETY: valid instance.
    let devices = unsafe { instance.enumerate_physical_devices() }.ok()?;
    let mut best: Option<(u32, vk::PhysicalDevice, u32, String)> = None;
    for device in devices {
        // SAFETY: device of this instance.
        let props = unsafe { instance.get_physical_device_properties(device) };
        if props.api_version < vk::API_VERSION_1_1 {
            continue;
        }
        // SAFETY: as above.
        let families = unsafe { instance.get_physical_device_queue_family_properties(device) };
        let family = families.iter().enumerate().find_map(|(i, f)| {
            let i = u32::try_from(i).ok()?;
            // SAFETY: family of this device, surface of this instance.
            let present =
                unsafe { surface_fn.get_physical_device_surface_support(device, i, surface) }
                    .unwrap_or(false);
            (f.queue_flags.contains(vk::QueueFlags::GRAPHICS) && present).then_some(i)
        });
        let Some(family) = family else { continue };
        let score = match props.device_type {
            vk::PhysicalDeviceType::INTEGRATED_GPU => 3,
            vk::PhysicalDeviceType::DISCRETE_GPU => 2,
            vk::PhysicalDeviceType::CPU => 0,
            _ => 1,
        };
        let name = props
            .device_name_as_c_str()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if best.as_ref().is_none_or(|(s, ..)| score > *s) {
            best = Some((score, device, family, name));
        }
    }
    best.map(|(_, d, f, n)| (d, f, n))
}
