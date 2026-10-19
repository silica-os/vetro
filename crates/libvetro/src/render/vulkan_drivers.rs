//! Which Vulkan drivers the embedder loads (#191).
//!
//! The Khronos loader opens every installed driver at `vkCreateInstance`: on Fedora a dozen
//! Mesa drivers of 12 to 26 MB each, for hardware that is not there. That cost 200 ms per
//! launch with the files in memory and 0.4 to 0.8 s from a cold disk, the largest part of an
//! app's start. The GPUs present are known from their DRM render nodes: the embedder hands the
//! loader only the drivers of those kernel drivers (`VK_LUNARG_direct_driver_loading`,
//! `ffi/vulkan.rs`), without touching the environment its children inherit.
//!
//! Nothing is narrowed when it cannot be decided safely: a kernel driver not in the table, a
//! GPU without a matching manifest, or the user choosing the drivers with the loader's own
//! variables (`VK_DRIVER_FILES`, `VK_LOADER_DRIVERS_SELECT`, ...).

use std::path::{Path, PathBuf};

/// The loader's variables that choose the drivers: when one is set, the user decides.
const LOADER_DRIVER_VARIABLES: [&str; 5] = [
    "VK_DRIVER_FILES",
    "VK_ICD_FILENAMES",
    "VK_ADD_DRIVER_FILES",
    "VK_LOADER_DRIVERS_SELECT",
    "VK_LOADER_DRIVERS_DISABLE",
];

/// Manifests read at most from one directory (they are a few hundred bytes each).
const MAX_MANIFESTS: usize = 256;
const MAX_MANIFEST_BYTES: u64 = 64 * 1024;

/// The Vulkan drivers, by the name of their manifest before `_icd` (Mesa's
/// `<name>_icd.<arch>.json`, `nvidia_icd.json`, AMDVLK's `amd_icd64.json`), that drive the GPUs
/// of a kernel DRM driver. `None` for a kernel driver this table does not know.
pub fn vulkan_drivers_for(kernel_driver: &str) -> Option<&'static [&'static str]> {
    Some(match kernel_driver {
        // A virtio GPU's render node hangs from the PCI device (driver virtio-pci).
        "virtio_gpu" | "virtio-gpu" | "virtio-pci" => &["virtio"],
        "i915" => &["intel", "intel_hasvk"],
        "xe" => &["intel"],
        "amdgpu" => &["radeon", "amd"],
        "radeon" => &["radeon"],
        "nouveau" => &["nouveau"],
        "nvidia" | "nvidia-drm" => &["nvidia"],
        "msm" => &["freedreno"],
        "v3d" | "vc4" => &["broadcom"],
        "panfrost" | "panthor" => &["panfrost"],
        "asahi" => &["asahi"],
        "powervr" => &["powervr_mesa"],
        _ => return None,
    })
}

/// The driver name of a manifest file (`intel_hasvk_icd.x86_64.json` is `intel_hasvk`).
pub fn manifest_driver(file_name: &str) -> Option<&str> {
    if !file_name.ends_with(".json") {
        return None;
    }
    file_name
        .split_once("_icd")
        .map(|(driver, _)| driver)
        .filter(|driver| !driver.is_empty())
}

/// The driver library a manifest names (`ICD.library_path`): an absolute path, a path
/// relative to the manifest's directory, or a bare name for the dynamic linker's search.
pub fn manifest_library(manifest: &Path, json: &str) -> Option<PathBuf> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    let library = value.get("ICD")?.get("library_path")?.as_str()?;
    if library.is_empty() {
        return None;
    }
    let path = Path::new(library);
    Some(if path.is_absolute() || !library.contains('/') {
        path.to_path_buf()
    } else {
        manifest.parent()?.join(path)
    })
}

/// A driver chosen by [`select`]: its manifest and the library it names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Driver {
    pub manifest: PathBuf,
    pub library: PathBuf,
}

/// The drivers for the GPUs of `kernel_drivers`, from `manifests` (path and contents), in
/// the order of the manifests, one per library. `None` (load them all) when there is no GPU,
/// when a kernel driver is unknown or when a GPU has no manifest among them.
pub fn select(kernel_drivers: &[String], manifests: &[(PathBuf, String)]) -> Option<Vec<Driver>> {
    if kernel_drivers.is_empty() {
        return None;
    }
    let mut wanted: Vec<&str> = Vec::new();
    for kernel in kernel_drivers {
        let names = vulkan_drivers_for(kernel)?;
        let found = manifests.iter().any(|(path, _)| {
            file_name(path)
                .and_then(manifest_driver)
                .is_some_and(|driver| names.contains(&driver))
        });
        if !found {
            return None;
        }
        wanted.extend_from_slice(names);
    }
    let mut drivers: Vec<Driver> = Vec::new();
    for (path, json) in manifests {
        let Some(driver) = file_name(path).and_then(manifest_driver) else {
            continue;
        };
        if !wanted.contains(&driver) {
            continue;
        }
        if let Some(library) = manifest_library(path, json)
            && !drivers.iter().any(|d| d.library == library)
        {
            drivers.push(Driver {
                manifest: path.clone(),
                library,
            });
        }
    }
    (!drivers.is_empty()).then_some(drivers)
}

fn file_name(path: &Path) -> Option<&str> {
    path.file_name().and_then(|name| name.to_str())
}

/// The kernel drivers of the render nodes under `drm` (`/sys/class/drm`), sorted, once each.
pub fn kernel_drivers(drm: &Path) -> Vec<String> {
    render_node_drivers(drm, |_| true)
}

/// The kernel driver of the GPU the firmware set up as the display (`boot_vga`): on a
/// laptop the integrated one.
#[allow(dead_code)]
pub fn primary_kernel_drivers(drm: &Path) -> Vec<String> {
    render_node_drivers(drm, |device| {
        std::fs::read_to_string(device.join("boot_vga")).is_ok_and(|v| v.trim() == "1")
    })
}

fn render_node_drivers(drm: &Path, keep: impl Fn(&Path) -> bool) -> Vec<String> {
    let mut drivers: Vec<String> = std::fs::read_dir(drm)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("renderD"))
        .map(|entry| entry.path().join("device"))
        .filter(|device| keep(device))
        .filter_map(|device| std::fs::read_link(device.join("driver")).ok())
        .filter_map(|link| link.file_name().map(|n| n.to_string_lossy().into_owned()))
        .collect();
    drivers.sort();
    drivers.dedup();
    drivers
}

/// The directories the loader searches for driver manifests, in its order.
pub fn manifest_dirs(var: impl Fn(&str) -> Option<String>) -> Vec<PathBuf> {
    let list = |name: &str, default: &str| -> Vec<PathBuf> {
        var(name)
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| default.to_owned())
            .split(':')
            .filter(|dir| !dir.is_empty())
            .map(PathBuf::from)
            .collect()
    };
    let mut dirs = list("XDG_CONFIG_DIRS", "/etc/xdg");
    dirs.push(PathBuf::from("/etc"));
    match var("XDG_DATA_HOME").filter(|v| !v.is_empty()) {
        Some(home) => dirs.push(PathBuf::from(home)),
        None => {
            if let Some(home) = var("HOME").filter(|v| !v.is_empty()) {
                dirs.push(Path::new(&home).join(".local/share"));
            }
        }
    }
    dirs.extend(list("XDG_DATA_DIRS", "/usr/local/share:/usr/share"));
    let mut out: Vec<PathBuf> = Vec::new();
    for dir in dirs {
        let dir = dir.join("vulkan/icd.d");
        if !out.contains(&dir) {
            out.push(dir);
        }
    }
    out
}

/// The manifests (path and contents) in `dirs`, sorted by name within a directory.
pub fn read_manifests(dirs: &[PathBuf]) -> Vec<(PathBuf, String)> {
    let mut manifests = Vec::new();
    for dir in dirs {
        let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| file_name(path).is_some_and(|n| n.ends_with(".json")))
            .take(MAX_MANIFESTS)
            .collect();
        paths.sort();
        for path in paths {
            let small = std::fs::metadata(&path).is_ok_and(|m| m.len() <= MAX_MANIFEST_BYTES);
            if small && let Ok(json) = std::fs::read_to_string(&path) {
                manifests.push((path, json));
            }
        }
    }
    manifests
}

/// The user chose the drivers with the loader's own variables.
pub fn user_chooses() -> bool {
    LOADER_DRIVER_VARIABLES
        .iter()
        .any(|name| std::env::var_os(name).is_some())
}

/// The drivers for this machine's GPUs, or `None` to let the loader load them all.
pub fn for_this_machine() -> Option<Vec<Driver>> {
    if user_chooses() {
        return None;
    }
    let kernel = kernel_drivers(Path::new("/sys/class/drm"));
    let manifests = read_manifests(&manifest_dirs(|name| std::env::var(name).ok()));
    select(&kernel, &manifests)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(path: &str, library: &str) -> (PathBuf, String) {
        (
            PathBuf::from(path),
            format!(
                r#"{{"ICD": {{"api_version": "1.4.335", "library_path": "{library}"}}, "file_format_version": "1.0.1"}}"#
            ),
        )
    }

    fn fedora() -> Vec<(PathBuf, String)> {
        [
            "asahi",
            "intel_hasvk",
            "intel",
            "lvp",
            "nouveau",
            "radeon",
            "virtio",
        ]
        .iter()
        .map(|name| {
            manifest(
                &format!("/usr/share/vulkan/icd.d/{name}_icd.x86_64.json"),
                &format!("/usr/lib64/libvulkan_{name}.so"),
            )
        })
        .collect()
    }

    #[test]
    fn manifest_names() {
        assert_eq!(manifest_driver("virtio_icd.x86_64.json"), Some("virtio"));
        assert_eq!(
            manifest_driver("intel_hasvk_icd.x86_64.json"),
            Some("intel_hasvk")
        );
        assert_eq!(manifest_driver("nvidia_icd.json"), Some("nvidia"));
        assert_eq!(manifest_driver("amd_icd64.json"), Some("amd"));
        assert_eq!(
            manifest_driver("powervr_mesa_icd.x86_64.json"),
            Some("powervr_mesa")
        );
        assert_eq!(manifest_driver("_icd.json"), None);
        assert_eq!(manifest_driver("virtio_icd.x86_64.json.bak"), None);
        assert_eq!(manifest_driver("VkLayer_MESA_device_select.json"), None);
    }

    #[test]
    fn library_paths() {
        let at = Path::new("/usr/share/vulkan/icd.d/x_icd.json");
        let json = |lib: &str| format!(r#"{{"ICD": {{"library_path": "{lib}"}}}}"#);
        assert_eq!(
            manifest_library(at, &json("/usr/lib64/libvulkan_virtio.so")),
            Some(PathBuf::from("/usr/lib64/libvulkan_virtio.so"))
        );
        assert_eq!(
            manifest_library(at, &json("libGLX_nvidia.so.0")),
            Some(PathBuf::from("libGLX_nvidia.so.0"))
        );
        assert_eq!(
            manifest_library(at, &json("../../../lib64/libvulkan_x.so")),
            Some(PathBuf::from(
                "/usr/share/vulkan/icd.d/../../../lib64/libvulkan_x.so"
            ))
        );
        assert_eq!(manifest_library(at, &json("")), None);
        assert_eq!(manifest_library(at, "{not json"), None);
        assert_eq!(manifest_library(at, r#"{"layer": {}}"#), None);
    }

    fn libraries(drivers: Option<Vec<Driver>>) -> Option<Vec<PathBuf>> {
        drivers.map(|drivers| drivers.into_iter().map(|d| d.library).collect())
    }

    #[test]
    fn the_vm_loads_venus_only() {
        assert_eq!(
            select(&["virtio-pci".to_owned()], &fedora()),
            Some(vec![Driver {
                manifest: PathBuf::from("/usr/share/vulkan/icd.d/virtio_icd.x86_64.json"),
                library: PathBuf::from("/usr/lib64/libvulkan_virtio.so"),
            }])
        );
    }

    #[test]
    fn intel_loads_both_intel_drivers() {
        assert_eq!(
            libraries(select(&["i915".to_owned()], &fedora())),
            Some(vec![
                PathBuf::from("/usr/lib64/libvulkan_intel_hasvk.so"),
                PathBuf::from("/usr/lib64/libvulkan_intel.so"),
            ])
        );
    }

    #[test]
    fn hybrid_laptops_load_both_gpus() {
        let drivers = select(&["amdgpu".to_owned(), "i915".to_owned()], &fedora());
        assert_eq!(
            drivers.as_deref().map(<[Driver]>::len),
            Some(3),
            "{drivers:?}"
        );
    }

    #[test]
    fn unknown_or_missing_drivers_load_everything() {
        assert_eq!(select(&[], &fedora()), None);
        assert_eq!(select(&["vmwgfx".to_owned()], &fedora()), None);
        assert_eq!(
            select(&["virtio_gpu".to_owned(), "vmwgfx".to_owned()], &fedora()),
            None
        );
        // The proprietary NVIDIA driver without its manifest installed.
        assert_eq!(select(&["nvidia".to_owned()], &fedora()), None);
    }

    #[test]
    fn duplicate_manifests_load_the_library_once() {
        let mut manifests = fedora();
        manifests.push(manifest(
            "/etc/vulkan/icd.d/virtio_icd.x86_64.json",
            "/usr/lib64/libvulkan_virtio.so",
        ));
        assert_eq!(
            libraries(select(&["virtio_gpu".to_owned()], &manifests)),
            Some(vec![PathBuf::from("/usr/lib64/libvulkan_virtio.so")])
        );
    }

    #[test]
    fn manifest_directories_follow_the_loader() {
        let env = |name: &str| match name {
            "HOME" => Some("/home/u".to_owned()),
            _ => None,
        };
        assert_eq!(
            manifest_dirs(env),
            [
                "/etc/xdg/vulkan/icd.d",
                "/etc/vulkan/icd.d",
                "/home/u/.local/share/vulkan/icd.d",
                "/usr/local/share/vulkan/icd.d",
                "/usr/share/vulkan/icd.d",
            ]
            .map(PathBuf::from)
        );
        let env = |name: &str| match name {
            "XDG_DATA_DIRS" => Some("/usr/share:/opt/share".to_owned()),
            "XDG_DATA_HOME" => Some("/data".to_owned()),
            _ => None,
        };
        let dirs = manifest_dirs(env);
        assert_eq!(dirs.get(2), Some(&PathBuf::from("/data/vulkan/icd.d")));
        assert_eq!(dirs.last(), Some(&PathBuf::from("/opt/share/vulkan/icd.d")));
    }

    #[test]
    fn manifests_and_render_nodes_from_disk() {
        let root = std::env::temp_dir().join(format!("silica-vk-{}", std::process::id()));
        let icd = root.join("vulkan/icd.d");
        std::fs::create_dir_all(&icd).unwrap();
        std::fs::write(icd.join("virtio_icd.x86_64.json"), &fedora()[6].1).unwrap();
        std::fs::write(icd.join("notes.txt"), "x").unwrap();
        let manifests = read_manifests(&[icd.clone(), root.join("missing")]);
        assert_eq!(manifests.len(), 1);

        let drm = root.join("drm");
        let driver = root.join("bus/pci/drivers/virtio-pci");
        std::fs::create_dir_all(drm.join("renderD128/device")).unwrap();
        std::fs::create_dir_all(drm.join("card0/device")).unwrap();
        std::fs::create_dir_all(&driver).unwrap();
        std::os::unix::fs::symlink(&driver, drm.join("renderD128/device/driver")).unwrap();
        std::os::unix::fs::symlink(&driver, drm.join("card0/device/driver")).unwrap();
        assert_eq!(kernel_drivers(&drm), ["virtio-pci"]);
        assert!(primary_kernel_drivers(&drm).is_empty());
        std::fs::write(drm.join("renderD128/device/boot_vga"), "1\n").unwrap();
        assert_eq!(primary_kernel_drivers(&drm), ["virtio-pci"]);
        assert!(kernel_drivers(&root.join("none")).is_empty());
        std::fs::remove_dir_all(&root).unwrap();
    }
}
