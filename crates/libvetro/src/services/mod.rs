//! System service clients: typed proxies for Silica's daemon and the portals, live watchers
//! (appearance, settings, key repeat, renderer choice) and the [`Services`] facade.
//!
//! Everything is async and runs on the platform's executor (calloop on the UI thread); no
//! thread is spawned here (zbus keeps its own I/O thread). Observables are single-threaded.
//! The interface tables are in `docs/services.md`.

pub mod appearance;
pub mod error;
pub mod observable;
pub mod proxies;
pub mod settings;
pub mod types;

mod owner;
mod wire;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

use std::cell::OnceCell;

use futures_lite::future::zip;
use zbus::Connection;

pub use appearance::{AccentName, AppearanceWatcher, SystemLook};
pub use error::Error;
pub use observable::{Changes, Observable, Subscription};
pub use proxies::*;
pub use settings::{
    KeyRepeat, RENDERER_KEY, RendererBackend, SettingValue, SettingsMap, SettingsWatcher,
};

use crate::core::env::Appearance;

/// Where the session bus is.
#[derive(Debug, Clone)]
enum Bus {
    /// `DBUS_SESSION_BUS_ADDRESS` (or the default socket).
    Session,
    /// An explicit address (tests, nested sessions).
    Address(String),
}

/// The entry point: connects to the session bus on first use, hands out proxies and owns the
/// watchers. Not `Send`: keep it on the UI thread.
///
/// ```ignore
/// let services = Services::new();
/// executor.spawn(services.run());              // keeps the watchers live
/// services.appearance().subscribe(|a| restyle(a));
/// let apps = services.apps().await?.list().await?;
/// ```
pub struct Services {
    bus: Bus,
    connection: OnceCell<Connection>,
    appearance: AppearanceWatcher,
    settings: SettingsWatcher,
}

macro_rules! proxy_getters {
    ($($(#[$meta:meta])* $name:ident -> $proxy:ident;)+) => {
        $(
            $(#[$meta])*
            pub async fn $name(&self) -> Result<$proxy<'static>, Error> {
                let connection = self.connection().await?;
                Ok($proxy::new(connection).await?)
            }
        )+
    };
}

impl Default for Services {
    fn default() -> Self {
        Self::new()
    }
}

impl Services {
    /// On the session bus; nothing is connected until the first call.
    pub fn new() -> Self {
        Self::with_bus(Bus::Session)
    }

    /// On the bus at `address` (a D-Bus address such as `unix:path=/run/user/1000/bus`).
    pub fn with_address(address: impl Into<String>) -> Self {
        Self::with_bus(Bus::Address(address.into()))
    }

    /// Over a connection the caller made.
    pub fn with_connection(connection: Connection) -> Self {
        let services = Self::with_bus(Bus::Session);
        let _ = services.connection.set(connection);
        services
    }

    fn with_bus(bus: Bus) -> Self {
        Services {
            bus,
            connection: OnceCell::new(),
            appearance: AppearanceWatcher::new(),
            settings: SettingsWatcher::new(),
        }
    }

    /// The connection, made on first use. Fails with [`Error::NoBus`] when there is no bus.
    pub async fn connection(&self) -> Result<&Connection, Error> {
        if let Some(connection) = self.connection.get() {
            return Ok(connection);
        }
        let made = match &self.bus {
            Bus::Session => Connection::session().await,
            Bus::Address(address) => match zbus::connection::Builder::address(address.as_str()) {
                Ok(builder) => builder.build().await,
                Err(error) => Err(error),
            },
        }
        .map_err(Error::NoBus)?;
        // Another call may have connected meanwhile; keep the first.
        let _ = self.connection.set(made);
        self.connection
            .get()
            .ok_or_else(|| Error::Invalid("connection lost".into()))
    }

    /// The live appearance and accessibility switches.
    pub fn appearance(&self) -> &Observable<Appearance> {
        self.appearance.appearance()
    }

    /// The whole system look (scroll bars, sidebar size, double-click time, ...).
    pub fn look(&self) -> &Observable<SystemLook> {
        self.appearance.look()
    }

    /// The key repeat delay and rate.
    pub fn key_repeat(&self) -> &Observable<KeyRepeat> {
        self.settings.key_repeat()
    }

    /// The renderer choice of this machine, if the daemon stores one (see [`RENDERER_KEY`]).
    pub fn renderer(&self) -> &Observable<Option<RendererBackend>> {
        self.settings.renderer()
    }

    /// Every Settings1 key, live.
    pub fn settings(&self) -> &Observable<std::rc::Rc<SettingsMap>> {
        self.settings.settings()
    }

    pub fn appearance_watcher(&self) -> &AppearanceWatcher {
        &self.appearance
    }

    pub fn settings_watcher(&self) -> &SettingsWatcher {
        &self.settings
    }

    /// Keeps the watchers live until the connection closes. Fails at once only when there is
    /// no bus (the observables then keep their defaults); a missing daemon is not an error.
    pub async fn run(&self) -> Result<(), Error> {
        let connection = self.connection().await?;
        zip(
            self.appearance.run(connection),
            self.settings.run(connection),
        )
        .await;
        Ok(())
    }

    proxy_getters! {
        /// `Settings1`.
        settings_proxy -> Settings1Proxy;
        /// `System1`.
        system -> System1Proxy;
        /// `Apps1`.
        apps -> Apps1Proxy;
        /// `Trust1`.
        trust -> Trust1Proxy;
        /// `Volumes1`.
        volumes -> Volumes1Proxy;
        /// `Updates1`.
        updates -> Updates1Proxy;
        /// `Thumbnails1`.
        thumbnails -> Thumbnails1Proxy;
        /// `Share1`.
        share -> Share1Proxy;
        /// `Notifications1`.
        notifications -> Notifications1Proxy;
        /// `org.freedesktop.Notifications`.
        fdo_notifications -> FdoNotificationsProxy;
        /// `Clipboard1`.
        clipboard -> Clipboard1Proxy;
        /// `Actions1`.
        actions -> Actions1Proxy;
        /// `Sound1`.
        sound -> Sound1Proxy;
        /// `Authentication1`.
        authentication -> Authentication1Proxy;
        /// `org.freedesktop.portal.FileChooser`.
        file_chooser -> PortalFileChooserProxy;
        /// `org.freedesktop.portal.OpenURI`.
        open_uri -> PortalOpenUriProxy;
        /// `org.freedesktop.portal.Settings`.
        portal_settings -> PortalSettingsProxy;
    }
}
