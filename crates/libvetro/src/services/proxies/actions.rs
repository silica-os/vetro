/// `dev.frascella.Silica.Actions1`.
#[zbus::proxy(
    interface = "dev.frascella.Silica.Actions1",
    default_service = "dev.frascella.Silica",
    default_path = "/dev/frascella/Silica/Actions1"
)]
pub trait Actions1 {
    /// The actions as a JSON string (the interface carries JSON; parse defensively).
    fn list(&self) -> zbus::Result<String>;
}
