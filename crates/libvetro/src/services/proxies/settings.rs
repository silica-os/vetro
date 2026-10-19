use std::collections::HashMap;
use zbus::zvariant::{OwnedValue, Value};

/// `dev.frascella.Silica.Settings1`: one typed value per key, live. Values are `x` (int), `d`,
/// `b`, `s`, `as` or `ad` variants; `Changed` also fires for `Reset` (with the default).
#[zbus::proxy(
    interface = "dev.frascella.Silica.Settings1",
    default_service = "dev.frascella.Silica",
    default_path = "/dev/frascella/Silica/Settings1"
)]
pub trait Settings1 {
    fn get(&self, key: &str) -> zbus::Result<OwnedValue>;
    fn get_all(&self) -> zbus::Result<HashMap<String, OwnedValue>>;
    fn set(&self, key: &str, value: &Value<'_>) -> zbus::Result<()>;
    /// All or none.
    fn set_many(&self, values: HashMap<&str, &Value<'_>>) -> zbus::Result<()>;
    fn reset(&self, key: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    fn changed(&self, key: &str, value: Value<'_>) -> zbus::Result<()>;
}
