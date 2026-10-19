use std::collections::HashMap;

use zbus::zvariant::{OwnedValue, Value};

use crate::services::types::{AudioOutput, BluetoothDevice, WifiNetwork};

/// `dev.frascella.Silica.System1`: the state of the machine (network, audio, brightness,
/// appearance, look, Focus, night light) and the calls that change it.
///
/// `State` is a method, not a property: the dictionary has flat keys (`appearance.dark`,
/// `look.accent`, `audio.volume`, ...) and `Changed` carries the whole dictionary again.
#[zbus::proxy(
    interface = "dev.frascella.Silica.System1",
    default_service = "dev.frascella.Silica",
    default_path = "/dev/frascella/Silica/System1"
)]
pub trait System1 {
    /// The whole state, flat keys.
    fn state(&self) -> zbus::Result<HashMap<String, OwnedValue>>;

    fn set_wifi_enabled(&self, enabled: bool) -> zbus::Result<()>;
    fn set_bluetooth_powered(&self, powered: bool) -> zbus::Result<()>;
    fn set_volume(&self, volume: f64) -> zbus::Result<()>;
    fn set_muted(&self, muted: bool) -> zbus::Result<()>;
    fn set_brightness(&self, brightness: f64) -> zbus::Result<()>;
    fn set_keyboard_brightness(&self, brightness: f64) -> zbus::Result<()>;

    /// Visible networks: (name, signal 0-100, protected, saved, in use).
    fn wifi_networks(&self) -> zbus::Result<Vec<WifiNetwork>>;
    fn connect_wifi(&self, ssid: &str) -> zbus::Result<()>;

    /// Paired devices: (address, name, icon name, connected).
    fn bluetooth_devices(&self) -> zbus::Result<Vec<BluetoothDevice>>;
    fn connect_bluetooth(&self, address: &str, connect: bool) -> zbus::Result<()>;

    /// Audio outputs: (id, name, default).
    fn audio_outputs(&self) -> zbus::Result<Vec<AudioOutput>>;
    fn set_audio_output(&self, id: u32) -> zbus::Result<()>;

    /// The server method has no reply value, so the call returns at once.
    fn set_do_not_disturb(&self, enabled: bool) -> zbus::Result<()>;
    fn set_dark(&self, dark: bool) -> zbus::Result<()>;
    /// `light`, `dark` or `auto`.
    fn set_appearance_mode(&self, mode: &str) -> zbus::Result<()>;
    /// Any of `accent`, `highlight`, `sidebarIconSize`, `scrollBars`, `scrollClick`, `iconStyle`,
    /// `folderColour` (strings), `glass` (d), `windowTint` (b). The server also stores
    /// `keyboardNavigation`, `doubleClickMs`, `reduceTransparency`, `reduceMotion`,
    /// `increaseContrast`, `textScale` in the same dictionary.
    fn set_look(&self, settings: HashMap<&str, &Value<'_>>) -> zbus::Result<()>;
    fn set_text_replacements(&self, list: &[(&str, &str)]) -> zbus::Result<()>;

    /// The displays as a JSON string (the interface carries JSON; parse defensively).
    fn outputs(&self) -> zbus::Result<String>;
    /// `{connector?, mode?, scale?, transform?}` as JSON; returns the display as JSON.
    fn configure_output(&self, json: &str) -> zbus::Result<String>;
    /// Any of `enabled` (b), `temperature` (u), `schedule` (s), `from`, `to` (`HH:MM`).
    fn set_night_light(&self, settings: HashMap<&str, &Value<'_>>) -> zbus::Result<()>;
    /// A Focus mode id (or `dnd`); empty turns Focus off.
    fn set_focus(&self, id: &str) -> zbus::Result<()>;
    fn suspend(&self) -> zbus::Result<()>;
    /// Checks the user's password for the lock screen.
    fn unlock(&self, password: &str) -> zbus::Result<bool>;
    /// Ossido's report (JSON object); the compositor calls it, apps do not.
    fn report_compositor(&self, json: &str) -> zbus::Result<()>;

    /// The state changed; carries the whole dictionary.
    #[zbus(signal)]
    fn changed(&self, state: HashMap<String, OwnedValue>) -> zbus::Result<()>;
}
