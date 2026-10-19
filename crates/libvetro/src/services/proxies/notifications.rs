use std::collections::HashMap;

use zbus::zvariant::Value;

use crate::services::types::Notification;

/// `dev.frascella.Silica.Notifications1`: the shell's view of the notifications.
#[zbus::proxy(
    interface = "dev.frascella.Silica.Notifications1",
    default_service = "dev.frascella.Silica",
    default_path = "/dev/frascella/Silica/Notifications1"
)]
pub trait Notifications1 {
    fn list(&self) -> zbus::Result<Vec<Notification>>;
    fn dismiss(&self, id: u32) -> zbus::Result<()>;
    fn clear(&self) -> zbus::Result<()>;
    fn invoke_action(&self, id: u32, action_key: &str) -> zbus::Result<()>;
    fn reply(&self, id: u32, text: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    fn added(&self, notification: Notification) -> zbus::Result<()>;
    /// `reason` as in the specification (1 expired, 2 dismissed, 3 closed by a call, 4 other).
    #[zbus(signal)]
    fn removed(&self, id: u32, reason: u32) -> zbus::Result<()>;
}

/// `org.freedesktop.Notifications`, served by the daemon (plus the `NotificationReplied`
/// extension signal).
#[zbus::proxy(
    interface = "org.freedesktop.Notifications",
    default_service = "org.freedesktop.Notifications",
    default_path = "/org/freedesktop/Notifications"
)]
pub trait FdoNotifications {
    #[allow(clippy::too_many_arguments)]
    fn notify(
        &self,
        app_name: &str,
        replaces_id: u32,
        app_icon: &str,
        summary: &str,
        body: &str,
        actions: &[&str],
        hints: HashMap<&str, &Value<'_>>,
        expire_timeout: i32,
    ) -> zbus::Result<u32>;
    fn close_notification(&self, id: u32) -> zbus::Result<()>;
    fn get_capabilities(&self) -> zbus::Result<Vec<String>>;
    /// (name, vendor, version, spec version).
    fn get_server_information(&self) -> zbus::Result<(String, String, String, String)>;

    #[zbus(signal)]
    fn notification_closed(&self, id: u32, reason: u32) -> zbus::Result<()>;
    #[zbus(signal)]
    fn action_invoked(&self, id: u32, action_key: &str) -> zbus::Result<()>;
    /// Silica's extension: the text of an inline reply.
    #[zbus(signal)]
    fn notification_replied(&self, id: u32, text: &str) -> zbus::Result<()>;
}
