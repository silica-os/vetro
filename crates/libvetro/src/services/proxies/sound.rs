/// `dev.frascella.Silica.Sound1`.
#[zbus::proxy(
    interface = "dev.frascella.Silica.Sound1",
    default_service = "dev.frascella.Silica",
    default_path = "/dev/frascella/Silica/Sound1"
)]
pub trait Sound1 {
    fn play_sound(&self, name: &str) -> zbus::Result<()>;
    fn play_alert(&self, name: &str) -> zbus::Result<()>;
    fn alert_sounds(&self) -> zbus::Result<Vec<String>>;
    fn play_volume_feedback(&self) -> zbus::Result<bool>;
    fn ui_sounds_enabled(&self) -> zbus::Result<bool>;
    fn set_ui_sounds_enabled(&self, enabled: bool) -> zbus::Result<()>;

    #[zbus(signal)]
    fn ui_sounds_changed(&self, enabled: bool) -> zbus::Result<()>;
}
