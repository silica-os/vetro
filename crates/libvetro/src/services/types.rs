//! Plain data types of the daemon's interfaces: the structures of the replies and signals, as
//! the server defines them (field order is the wire order).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use zbus::zvariant::{OwnedValue, Type};

/// A Wi-Fi network of `System1.WifiNetworks` (`(sybbb)`), the one in use first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct WifiNetwork {
    pub name: String,
    /// Signal strength, 0-100.
    pub signal: u8,
    pub protected: bool,
    pub saved: bool,
    pub in_use: bool,
}

/// A paired device of `System1.BluetoothDevices` (`(sssb)`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct BluetoothDevice {
    pub address: String,
    pub name: String,
    pub icon: String,
    pub connected: bool,
}

/// An output of `System1.AudioOutputs` (`(usb)`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct AudioOutput {
    pub id: u32,
    pub name: String,
    pub default: bool,
}

/// An app of `Apps1.List` (`(ssssa{sv})`).
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct AppEntry {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub path: String,
    pub properties: HashMap<String, OwnedValue>,
}

/// A notification of `Notifications1` (`(usssssasysssbbti)`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct Notification {
    pub id: u32,
    pub app_name: String,
    pub app_icon: String,
    pub summary: String,
    pub body: String,
    /// Key, label pairs, flattened as in the specification.
    pub actions: Vec<String>,
    /// 0 low, 1 normal, 2 critical.
    pub urgency: u8,
    pub category: String,
    pub desktop_entry: String,
    pub image_path: String,
    /// Only the banner, not the Notification Center history.
    pub transient: bool,
    /// Stays after an action is invoked.
    pub resident: bool,
    /// Milliseconds from the Unix epoch to reception.
    pub timestamp_ms: u64,
    /// As requested: -1 default, 0 never, otherwise milliseconds.
    pub expire_timeout: i32,
}

/// A prompt waiting in `Trust1.Pending` (`(usa{sv})`).
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct PendingPrompt {
    pub id: u32,
    pub verdict: String,
    pub info: HashMap<String, OwnedValue>,
}

/// A mounted image of `Volumes1.List` (`(ssa{sv})`).
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct VolumeEntry {
    pub mountpoint: String,
    pub image: String,
    pub info: HashMap<String, OwnedValue>,
}

/// An update of `Updates1.Check` (`(sssssa{sv})`). `details` has `size` among others.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct UpdateEntry {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub current: String,
    pub available: String,
    pub details: HashMap<String, OwnedValue>,
}

/// A share target of `Share1.Targets` (`(sss)`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct ShareTarget {
    pub id: String,
    pub name: String,
    pub icon: String,
}

/// A polkit request of `Authentication1` (`(ussssa(ss)s)`): id, action, message, icon, app,
/// users (user name, real name) and the default user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct AuthRequest {
    pub id: u32,
    pub action: String,
    pub message: String,
    pub icon: String,
    pub app: String,
    pub users: Vec<(String, String)>,
    pub default_user: String,
}
