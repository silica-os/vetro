use crate::services::types::ShareTarget;

/// `dev.frascella.Silica.Share1`.
#[zbus::proxy(
    interface = "dev.frascella.Silica.Share1",
    default_service = "dev.frascella.Silica",
    default_path = "/dev/frascella/Silica/Share1"
)]
pub trait Share1 {
    /// (id, name, icon), `copy` first.
    fn targets(&self, uris: &[&str]) -> zbus::Result<Vec<ShareTarget>>;
    fn share(&self, target: &str, uris: &[&str]) -> zbus::Result<bool>;
}
