use std::collections::HashMap;
use zbus::zvariant::OwnedValue;

use crate::services::types::PendingPrompt;

/// `dev.frascella.Silica.Trust1`: the verdict on apps before they open.
#[zbus::proxy(
    interface = "dev.frascella.Silica.Trust1",
    default_service = "dev.frascella.Silica",
    default_path = "/dev/frascella/Silica/Trust1"
)]
pub trait Trust1 {
    /// (verdict, info).
    fn assess(&self, path: &str) -> zbus::Result<(String, HashMap<String, OwnedValue>)>;
    /// Shows the shell's alert and waits: `open`, `cancel` or `trash`.
    fn prompt(&self, path: &str) -> zbus::Result<String>;
    fn suggest_move(&self, path: &str) -> zbus::Result<String>;
    fn set_sandbox(&self, path: &str, on: bool) -> zbus::Result<bool>;
    /// The shell registers itself as the one that shows the alerts (by sender).
    fn register_agent(&self) -> zbus::Result<()>;
    fn answer(&self, id: u32, choice: &str) -> zbus::Result<()>;
    fn pending(&self) -> zbus::Result<Vec<PendingPrompt>>;
    /// (path, reason, time).
    fn blocked(&self) -> zbus::Result<Vec<(String, String, u64)>>;
    fn open_anyway(&self, path: &str) -> zbus::Result<bool>;

    #[zbus(signal)]
    fn prompt_requested(
        &self,
        id: u32,
        verdict: &str,
        info: HashMap<String, OwnedValue>,
    ) -> zbus::Result<()>;
    #[zbus(signal)]
    fn prompt_closed(&self, id: u32, choice: &str) -> zbus::Result<()>;
}
