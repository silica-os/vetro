use std::collections::HashMap;
use zbus::zvariant::{OwnedValue, Value};

use crate::services::types::VolumeEntry;

/// `dev.frascella.Silica.Volumes1`: disk images (`.sxv`).
#[zbus::proxy(
    interface = "dev.frascella.Silica.Volumes1",
    default_service = "dev.frascella.Silica",
    default_path = "/dev/frascella/Silica/Volumes1"
)]
pub trait Volumes1 {
    /// Reads and verifies an image without mounting it (`name`, `signature`,
    /// `developer_status`, `key`, `license`, ...).
    fn inspect(&self, image: &str) -> zbus::Result<HashMap<String, OwnedValue>>;
    /// Mounts; returns the mount point.
    fn attach(&self, image: &str, options: HashMap<&str, &Value<'_>>) -> zbus::Result<String>;
    fn detach(&self, mountpoint: &str) -> zbus::Result<()>;
    fn list(&self) -> zbus::Result<Vec<VolumeEntry>>;

    #[zbus(signal)]
    fn attached(&self, mountpoint: &str) -> zbus::Result<()>;
    #[zbus(signal)]
    fn detached(&self, mountpoint: &str) -> zbus::Result<()>;
}
