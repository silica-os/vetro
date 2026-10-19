//! The Vulkan backend (primary): one instance, device and Skia context for the process, a
//! `VkSurfaceKHR` and swapchain per window. Ported from silica-desktop's embedder
//! (`ffi/vulkan.rs`), which runs on the dev VM's Venus driver.
//!
//! Synchronisation is deliberately simple: the swapchain image is acquired with a fence and
//! the frame is submitted with `SyncCpu::Yes`, so no semaphore crosses Skia's flush (its
//! semaphore API is not wrapped safely by skia-safe) and nothing is reused while the GPU
//! works. A UI frame is small; pipelining is a later optimisation. Every frame repaints the
//! whole image (swapchain images do not keep their contents); the damage is the core's.

#![allow(unsafe_code)]

use std::ffi::{CStr, c_char, c_void};

use ash::vk;
use ash::vk::Handle;
use skia_safe::gpu::{self, DirectContext, SurfaceOrigin, vk as skvk};
use wayland_client::Proxy;
use wayland_client::protocol::wl_surface::WlSurface;

const INSTANCE_EXTENSIONS: &[&CStr] = &[ash::khr::surface::NAME, ash::khr::wayland_surface::NAME];
const DEVICE_EXTENSIONS: &[&CStr] = &[ash::khr::swapchain::NAME];

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
    acquired: vk::Fence,
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
        let skia =
            gpu::direct_contexts::make_vulkan(&backend, None).ok_or("Skia Vulkan context")?;
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

    fn window_from_surface(&self, surface: vk::SurfaceKHR) -> Result<VkWindow, String> {
        // SAFETY: valid device.
        let acquired = unsafe {
            self.device
                .create_fence(&vk::FenceCreateInfo::default(), None)
        }
        .map_err(err("vkCreateFence"))?;
        Ok(VkWindow {
            surface,
            swapchain: vk::SwapchainKHR::null(),
            images: Vec::new(),
            format: vk::Format::B8G8R8A8_UNORM,
            color_type: skia_safe::ColorType::BGRA8888,
            extent: (0, 0),
            acquired,
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
            self.device.destroy_fence(w.acquired, None);
            self.surface_fn.destroy_surface(w.surface, None);
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

    /// Draws and presents a frame of `size` device pixels. The caller has requested the frame
    /// callback (the present commits the surface).
    pub fn render(
        &mut self,
        w: &mut VkWindow,
        size: (i32, i32),
        draw: impl FnOnce(&skia_safe::Canvas),
    ) -> Result<(), String> {
        if w.swapchain == vk::SwapchainKHR::null() || w.extent != size {
            self.rebuild(w, size)?;
        }
        // SAFETY: valid swapchain and fence; the fence is unsignalled (reset after waiting).
        let index = unsafe {
            match self.swapchain_fn.acquire_next_image(
                w.swapchain,
                1_000_000_000,
                vk::Semaphore::null(),
                w.acquired,
            ) {
                Ok((i, _)) => i,
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                    self.rebuild(w, size)?;
                    self.swapchain_fn
                        .acquire_next_image(
                            w.swapchain,
                            1_000_000_000,
                            vk::Semaphore::null(),
                            w.acquired,
                        )
                        .map_err(err("vkAcquireNextImageKHR"))?
                        .0
                }
                Err(e) => return Err(format!("vkAcquireNextImageKHR: {e:?}")),
            }
        };
        // SAFETY: the fence was given to the acquire above.
        unsafe {
            let _ = self
                .device
                .wait_for_fences(&[w.acquired], true, 1_000_000_000);
            let _ = self.device.reset_fences(&[w.acquired]);
        }
        let Some(image) = w.images.get(index as usize).copied() else {
            return Err("swapchain image index out of range".into());
        };
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
        draw(surface.canvas());
        self.skia.flush_surface_with_access(
            &mut surface,
            skia_safe::surface::BackendSurfaceAccess::Present,
            &gpu::FlushInfo::default(),
        );
        self.skia.submit(gpu::SyncCpu::Yes);
        drop(surface);
        let swapchains = [w.swapchain];
        let indices = [index];
        let present = vk::PresentInfoKHR::default()
            .swapchains(&swapchains)
            .image_indices(&indices);
        // SAFETY: the GPU work is complete (SyncCpu::Yes) and the image is in the present
        // layout (Skia's Present access).
        match unsafe { self.swapchain_fn.queue_present(self.queue, &present) } {
            Ok(_) | Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => Ok(()),
            Err(e) => Err(format!("vkQueuePresentKHR: {e:?}")),
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
    for d in super::vulkan_drivers::for_this_machine().unwrap_or_default() {
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
            Ok(i) => return Ok((i, libraries)),
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
