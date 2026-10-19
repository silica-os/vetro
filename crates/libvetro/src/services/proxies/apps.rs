use std::collections::HashMap;
use zbus::zvariant::{OwnedValue, Value};

use crate::services::types::AppEntry;

/// `dev.frascella.Silica.Apps1`: the registry of installed apps.
#[zbus::proxy(
    interface = "dev.frascella.Silica.Apps1",
    default_service = "dev.frascella.Silica",
    default_path = "/dev/frascella/Silica/Apps1"
)]
pub trait Apps1 {
    /// Every app: id, name, kind, path, properties.
    fn list(&self) -> zbus::Result<Vec<AppEntry>>;
    /// One app's properties (`id`, `name`, `kind`, `path`, `exec`, `icon`, ...).
    fn get(&self, id: &str) -> zbus::Result<HashMap<String, OwnedValue>>;
    /// Ids of the apps that open `type`, the default first.
    fn apps_for(&self, r#type: &str) -> zbus::Result<Vec<String>>;
    /// The default app of `type`, empty when none.
    fn default_for(&self, r#type: &str) -> zbus::Result<String>;
    fn set_default(&self, r#type: &str, id: &str) -> zbus::Result<()>;
    /// Starts `id` with `uris`; returns the pid.
    fn open(
        &self,
        id: &str,
        uris: &[&str],
        options: HashMap<&str, &Value<'_>>,
    ) -> zbus::Result<u32>;
    fn open_uris(&self, uris: &[&str]) -> zbus::Result<u32>;
    fn note_launched(&self, id: &str) -> zbus::Result<()>;
    fn uninstall(&self, id: &str) -> zbus::Result<bool>;

    #[zbus(signal)]
    fn added(&self, id: &str) -> zbus::Result<()>;
    #[zbus(signal)]
    fn removed(&self, id: &str) -> zbus::Result<()>;
    #[zbus(signal)]
    fn changed(&self, id: &str) -> zbus::Result<()>;
    #[zbus(signal)]
    fn default_changed(&self, r#type: &str, id: &str) -> zbus::Result<()>;
}
