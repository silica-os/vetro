//! Whether the daemon owns its bus name, as a stream.

use futures_lite::Stream;
use futures_lite::StreamExt;
use zbus::Connection;
use zbus::fdo::DBusProxy;

use crate::services::error::Error;
use crate::services::proxies::DAEMON_NAME;

/// Yields `true` when `dev.frascella.Silica` gets an owner and `false` when it loses it.
pub(crate) async fn daemon_presence(
    connection: &Connection,
) -> Result<impl Stream<Item = bool> + use<>, Error> {
    let bus = DBusProxy::new(connection).await?;
    let changes = bus
        .receive_name_owner_changed_with_args(&[(0, DAEMON_NAME)])
        .await?;
    Ok(changes.filter_map(|signal| {
        let args = signal.args().ok()?;
        Some(args.new_owner().is_some())
    }))
}
