//! Typed zbus proxies for every interface of Silica's daemon (`dev.frascella.Silica`) and for the
//! portals Vetro apps use.
//!
//! The signatures are read from the daemon's server implementations (`crates/daemon/src` of
//! silica-desktop); `docs/services.md` lists what each proxy covers. All proxies are async
//! (`zbus::proxy` default) and are built on any [`zbus::Connection`]; they carry the daemon's
//! bus name and object path as defaults, so `Settings1Proxy::new(&connection)` is enough.

mod actions;
mod apps;
mod authentication;
mod clipboard;
mod notifications;
mod portal;
mod settings;
mod share;
mod sound;
mod system;
mod thumbnails;
mod trust;
mod updates;
mod volumes;

pub use actions::Actions1Proxy;
pub use apps::Apps1Proxy;
pub use authentication::Authentication1Proxy;
pub use clipboard::Clipboard1Proxy;
pub use notifications::{FdoNotificationsProxy, Notifications1Proxy};
pub use portal::{
    PortalFileChooserProxy, PortalOpenUriProxy, PortalRequestProxy, PortalSettingsProxy,
};
pub use settings::Settings1Proxy;
pub use share::Share1Proxy;
pub use sound::Sound1Proxy;
pub use system::System1Proxy;
pub use thumbnails::Thumbnails1Proxy;
pub use trust::Trust1Proxy;
pub use updates::Updates1Proxy;
pub use volumes::Volumes1Proxy;

/// The well-known name of Silica's daemon.
pub const DAEMON_NAME: &str = "dev.frascella.Silica";
/// The well-known name of the freedesktop notification service (also owned by the daemon).
pub const FDO_NOTIFICATIONS_NAME: &str = "org.freedesktop.Notifications";
/// The well-known name of the desktop portal.
pub const PORTAL_NAME: &str = "org.freedesktop.portal.Desktop";
