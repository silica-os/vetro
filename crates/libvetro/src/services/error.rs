//! Errors of the services layer.

use std::fmt;

/// Names of the bus errors that mean "the service is not there".
const ABSENT: [&str; 3] = [
    "org.freedesktop.DBus.Error.ServiceUnknown",
    "org.freedesktop.DBus.Error.NameHasNoOwner",
    "org.freedesktop.DBus.Error.NoReply",
];

/// What can go wrong talking to the system services.
#[derive(Debug)]
pub enum Error {
    /// The session bus cannot be reached (no `DBUS_SESSION_BUS_ADDRESS`, refused, closed).
    NoBus(zbus::Error),
    /// A call, signal or property failed.
    Bus(zbus::Error),
    /// The service answered with something the client cannot use.
    Invalid(String),
}

impl Error {
    /// True when the failure means the daemon (or portal) is not running, which callers treat
    /// as "use the defaults", not as a fault.
    pub fn is_service_absent(&self) -> bool {
        let (Error::Bus(error) | Error::NoBus(error)) = self else {
            return false;
        };
        match error {
            zbus::Error::MethodError(name, ..) => ABSENT.contains(&name.as_str()),
            zbus::Error::FDO(error) => matches!(
                **error,
                zbus::fdo::Error::ServiceUnknown(_) | zbus::fdo::Error::NameHasNoOwner(_)
            ),
            _ => false,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NoBus(error) => write!(f, "session bus unavailable: {error}"),
            Error::Bus(error) => write!(f, "bus error: {error}"),
            Error::Invalid(what) => write!(f, "unusable reply: {what}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::NoBus(error) | Error::Bus(error) => Some(error),
            Error::Invalid(_) => None,
        }
    }
}

impl From<zbus::Error> for Error {
    fn from(error: zbus::Error) -> Self {
        Error::Bus(error)
    }
}

impl From<zbus::fdo::Error> for Error {
    fn from(error: zbus::fdo::Error) -> Self {
        Error::Bus(error.into())
    }
}
