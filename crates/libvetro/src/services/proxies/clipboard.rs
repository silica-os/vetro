/// `dev.frascella.Silica.Clipboard1`.
#[zbus::proxy(
    interface = "dev.frascella.Silica.Clipboard1",
    default_service = "dev.frascella.Silica",
    default_path = "/dev/frascella/Silica/Clipboard1"
)]
pub trait Clipboard1 {
    /// The history as a JSON string (the interface carries JSON; parse defensively).
    fn history(&self) -> zbus::Result<String>;
    fn copy(&self, id: u64) -> zbus::Result<bool>;
    fn copy_image(&self, path: &str) -> zbus::Result<bool>;
    fn remove(&self, id: u64) -> zbus::Result<bool>;
    fn clear(&self) -> zbus::Result<()>;

    #[zbus(signal)]
    fn changed(&self) -> zbus::Result<()>;
}
