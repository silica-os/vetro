use crate::services::types::AuthRequest;

/// `dev.frascella.Silica.Authentication1`: the polkit requests for the shell's dialog.
#[zbus::proxy(
    interface = "dev.frascella.Silica.Authentication1",
    default_service = "dev.frascella.Silica",
    default_path = "/dev/frascella/Silica/Authentication1"
)]
pub trait Authentication1 {
    fn pending(&self) -> zbus::Result<Vec<AuthRequest>>;
    /// True: right, the request closes; false: wrong, try again.
    fn respond(&self, id: u32, user: &str, password: &str) -> zbus::Result<bool>;
    fn cancel(&self, id: u32) -> zbus::Result<()>;
    /// The agent is registered with polkit.
    #[zbus(property)]
    fn registered(&self) -> zbus::Result<bool>;

    #[zbus(signal)]
    fn requested(&self, request: AuthRequest) -> zbus::Result<()>;
    /// `outcome`: `success` or `cancelled`.
    #[zbus(signal)]
    fn closed(&self, id: u32, outcome: &str) -> zbus::Result<()>;
}
