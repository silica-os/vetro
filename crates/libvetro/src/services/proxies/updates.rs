use crate::services::types::UpdateEntry;

/// `dev.frascella.Silica.Updates1`.
#[zbus::proxy(
    interface = "dev.frascella.Silica.Updates1",
    default_service = "dev.frascella.Silica",
    default_path = "/dev/frascella/Silica/Updates1"
)]
pub trait Updates1 {
    /// Looks for updates of every source.
    fn check(&self) -> zbus::Result<Vec<UpdateEntry>>;
    /// `installed`, `pending_restart` or `staged`.
    fn update(&self, id: &str) -> zbus::Result<String>;
    /// (id, result) per update.
    fn update_all(&self) -> zbus::Result<Vec<(String, String)>>;

    #[zbus(signal)]
    fn progress(&self, id: &str, fraction: f64) -> zbus::Result<()>;
    #[zbus(signal)]
    fn finished(&self, id: &str, result: &str) -> zbus::Result<()>;
}
