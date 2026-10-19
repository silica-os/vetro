use std::collections::HashMap;

use zbus::zvariant::{Fd, OwnedObjectPath, OwnedValue, Value};

/// `org.freedesktop.portal.FileChooser`. The calls return the `Request` object path; the
/// result arrives as that object's `Response` signal ([`PortalRequestProxy`]).
#[zbus::proxy(
    interface = "org.freedesktop.portal.FileChooser",
    default_service = "org.freedesktop.portal.Desktop",
    default_path = "/org/freedesktop/portal/desktop"
)]
pub trait PortalFileChooser {
    fn open_file(
        &self,
        parent_window: &str,
        title: &str,
        options: HashMap<&str, &Value<'_>>,
    ) -> zbus::Result<OwnedObjectPath>;
    fn save_file(
        &self,
        parent_window: &str,
        title: &str,
        options: HashMap<&str, &Value<'_>>,
    ) -> zbus::Result<OwnedObjectPath>;
    fn save_files(
        &self,
        parent_window: &str,
        title: &str,
        options: HashMap<&str, &Value<'_>>,
    ) -> zbus::Result<OwnedObjectPath>;
    #[zbus(property)]
    fn version(&self) -> zbus::Result<u32>;
}

/// `org.freedesktop.portal.OpenURI`.
#[zbus::proxy(
    interface = "org.freedesktop.portal.OpenURI",
    default_service = "org.freedesktop.portal.Desktop",
    default_path = "/org/freedesktop/portal/desktop"
)]
pub trait PortalOpenUri {
    #[zbus(name = "OpenURI")]
    fn open_uri(
        &self,
        parent_window: &str,
        uri: &str,
        options: HashMap<&str, &Value<'_>>,
    ) -> zbus::Result<OwnedObjectPath>;
    fn open_file(
        &self,
        parent_window: &str,
        fd: Fd<'_>,
        options: HashMap<&str, &Value<'_>>,
    ) -> zbus::Result<OwnedObjectPath>;
    fn open_directory(
        &self,
        parent_window: &str,
        fd: Fd<'_>,
        options: HashMap<&str, &Value<'_>>,
    ) -> zbus::Result<OwnedObjectPath>;
    #[zbus(property)]
    fn version(&self) -> zbus::Result<u32>;
}

/// `org.freedesktop.portal.Settings` (`org.freedesktop.appearance` and friends).
#[zbus::proxy(
    interface = "org.freedesktop.portal.Settings",
    default_service = "org.freedesktop.portal.Desktop",
    default_path = "/org/freedesktop/portal/desktop"
)]
pub trait PortalSettings {
    fn read_all(
        &self,
        namespaces: &[&str],
    ) -> zbus::Result<HashMap<String, HashMap<String, OwnedValue>>>;
    fn read(&self, namespace: &str, key: &str) -> zbus::Result<OwnedValue>;
    #[zbus(property)]
    fn version(&self) -> zbus::Result<u32>;

    #[zbus(signal)]
    fn setting_changed(&self, namespace: &str, key: &str, value: Value<'_>) -> zbus::Result<()>;
}

/// `org.freedesktop.portal.Request`: the object a portal call returns. Build the proxy with
/// the returned path as `path` (and [`crate::services::PORTAL_NAME`] as destination).
#[zbus::proxy(
    interface = "org.freedesktop.portal.Request",
    default_service = "org.freedesktop.portal.Desktop"
)]
pub trait PortalRequest {
    fn close(&self) -> zbus::Result<()>;
    /// `response`: 0 success, 1 cancelled by the user, 2 ended otherwise.
    #[zbus(signal)]
    fn response(&self, response: u32, results: HashMap<String, OwnedValue>) -> zbus::Result<()>;
}
