//! Settings1 as a live, typed mirror, and the values Vetro derives from it: key repeat and the
//! renderer backend choice.

use std::collections::HashMap;
use std::rc::Rc;

use futures_lite::StreamExt;
use zbus::Connection;
use zbus::zvariant::{OwnedValue, Value};

use crate::services::error::Error;
use crate::services::observable::Observable;
use crate::services::owner::daemon_presence;
use crate::services::proxies::Settings1Proxy;
use crate::services::wire;

/// A value of the daemon's settings store (the wire types are `b`, `x`, `d`, `s`, `as`, `ad`).
#[derive(Debug, Clone, PartialEq)]
pub enum SettingValue {
    Bool(bool),
    Int(i64),
    Num(f64),
    Text(String),
    Texts(Vec<String>),
    Nums(Vec<f64>),
}

impl SettingValue {
    /// Reads a variant; `None` for a type the store does not use.
    pub fn from_value(value: &Value<'_>) -> Option<Self> {
        match wire::inner(value) {
            Value::Bool(b) => Some(Self::Bool(*b)),
            Value::Str(s) => Some(Self::Text(s.to_string())),
            Value::F64(n) => Some(Self::Num(*n)),
            Value::Array(array) => {
                let items = array.inner();
                let mut texts = Vec::with_capacity(items.len());
                let mut nums = Vec::with_capacity(items.len());
                for item in items {
                    if let Some(text) = wire::as_str(item) {
                        texts.push(text.to_owned());
                    } else {
                        let n = wire::as_f64(item)?;
                        nums.push(n);
                    }
                }
                // An empty array is a list of either; the schema decides, texts win.
                if nums.is_empty() {
                    Some(Self::Texts(texts))
                } else if texts.is_empty() {
                    Some(Self::Nums(nums))
                } else {
                    None
                }
            }
            other => wire::as_i64(other).map(Self::Int),
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            Self::Int(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(s) => Some(s),
            _ => None,
        }
    }
}

/// Every key of the store.
pub type SettingsMap = HashMap<String, SettingValue>;

/// The keyboard's repeat: `keyboard.repeatDelay` (ms, 100-3000, default 375) and
/// `keyboard.repeatRate` (per second, 0-100, default 11; 0 turns repeat off).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyRepeat {
    pub delay_ms: u32,
    pub rate_hz: u32,
}

impl Default for KeyRepeat {
    fn default() -> Self {
        KeyRepeat {
            delay_ms: 375,
            rate_hz: 11,
        }
    }
}

impl KeyRepeat {
    pub fn from_settings(settings: &SettingsMap) -> Self {
        let default = KeyRepeat::default();
        let get = |key: &str, low: i64, high: i64, fallback: u32| {
            settings
                .get(key)
                .and_then(SettingValue::as_int)
                .map_or(fallback, |n| {
                    u32::try_from(n.clamp(low, high)).unwrap_or(fallback)
                })
        };
        KeyRepeat {
            delay_ms: get("keyboard.repeatDelay", 100, 3000, default.delay_ms),
            rate_hz: get("keyboard.repeatRate", 0, 100, default.rate_hz),
        }
    }
}

/// The renderer backend of a machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RendererBackend {
    Vulkan,
    Gl,
    Software,
}

impl RendererBackend {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "vulkan" => Some(Self::Vulkan),
            "gl" => Some(Self::Gl),
            "software" => Some(Self::Software),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Vulkan => "vulkan",
            Self::Gl => "gl",
            Self::Software => "software",
        }
    }
}

/// The key under which the renderer choice would live. **The daemon's schema has no such key
/// today** (a `Set` is refused as an unknown key), so [`SettingsWatcher::renderer`] stays `None`
/// until silica-desktop adds it; see `docs/services.md`.
pub const RENDERER_KEY: &str = "renderer.backend";

/// Follows the daemon's Settings1 store live.
pub struct SettingsWatcher {
    settings: Observable<Rc<SettingsMap>>,
    key_repeat: Observable<KeyRepeat>,
    renderer: Observable<Option<RendererBackend>>,
}

impl Default for SettingsWatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl SettingsWatcher {
    /// A watcher with no data: empty store, default key repeat, no renderer choice.
    pub fn new() -> Self {
        SettingsWatcher {
            settings: Observable::new(Rc::new(SettingsMap::new())),
            key_repeat: Observable::new(KeyRepeat::default()),
            renderer: Observable::new(None),
        }
    }

    /// All keys (empty while the daemon is absent).
    pub fn settings(&self) -> &Observable<Rc<SettingsMap>> {
        &self.settings
    }

    pub fn key_repeat(&self) -> &Observable<KeyRepeat> {
        &self.key_repeat
    }

    pub fn renderer(&self) -> &Observable<Option<RendererBackend>> {
        &self.renderer
    }

    /// Replaces the store and updates the derived values.
    pub fn apply_all(&self, map: &HashMap<String, OwnedValue>) {
        let parsed: SettingsMap = map
            .iter()
            .filter_map(|(k, v)| SettingValue::from_value(v).map(|v| (k.clone(), v)))
            .collect();
        self.publish(parsed);
    }

    /// One key changed (`Changed` signal).
    pub fn apply_change(&self, key: &str, value: &Value<'_>) {
        let mut next = (*self.settings.get()).clone();
        match SettingValue::from_value(value) {
            Some(v) => {
                next.insert(key.to_owned(), v);
            }
            None => {
                next.remove(key);
            }
        }
        self.publish(next);
    }

    /// Back to the empty store and the defaults (the daemon went away).
    pub fn clear(&self) {
        self.publish(SettingsMap::new());
    }

    fn publish(&self, map: SettingsMap) {
        self.key_repeat.set(KeyRepeat::from_settings(&map));
        self.renderer.set(
            map.get(RENDERER_KEY)
                .and_then(SettingValue::as_text)
                .and_then(RendererBackend::parse),
        );
        self.settings.set(Rc::new(map));
    }

    /// Follows the daemon until the connection closes: reads the store, applies `Changed`,
    /// reads again when the daemon (re)appears and clears when it goes. Never fails: without a
    /// daemon the observables keep their defaults. Run it on the platform's executor.
    pub async fn run(&self, connection: &Connection) {
        let _ = self.try_run(connection).await;
    }

    async fn try_run(&self, connection: &Connection) -> Result<(), Error> {
        enum Event {
            Changed(String, OwnedValue),
            Present(bool),
        }
        let proxy = Settings1Proxy::new(connection).await?;
        let changed = proxy.receive_changed().await?.filter_map(|signal| {
            let args = signal.args().ok()?;
            let value = OwnedValue::try_from(args.value()).ok()?;
            Some(Event::Changed(args.key().to_string(), value))
        });
        let presence = daemon_presence(connection).await?.map(Event::Present);
        let mut events = Box::pin(changed.race(presence));
        if let Ok(all) = proxy.get_all().await {
            self.apply_all(&all);
        }
        while let Some(event) = events.next().await {
            match event {
                Event::Changed(key, value) => self.apply_change(&key, &value),
                Event::Present(true) => {
                    if let Ok(all) = proxy.get_all().await {
                        self.apply_all(&all);
                    }
                }
                Event::Present(false) => self.clear(),
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_repeat_clamps_and_defaults() {
        let mut map = SettingsMap::new();
        assert_eq!(KeyRepeat::from_settings(&map), KeyRepeat::default());
        map.insert("keyboard.repeatDelay".into(), SettingValue::Int(10));
        map.insert("keyboard.repeatRate".into(), SettingValue::Int(500));
        assert_eq!(
            KeyRepeat::from_settings(&map),
            KeyRepeat {
                delay_ms: 100,
                rate_hz: 100
            }
        );
        map.insert(
            "keyboard.repeatDelay".into(),
            SettingValue::Text("x".into()),
        );
        assert_eq!(KeyRepeat::from_settings(&map).delay_ms, 375);
    }

    #[test]
    fn values_convert_defensively() {
        assert_eq!(
            SettingValue::from_value(&Value::I64(5)),
            Some(SettingValue::Int(5))
        );
        assert_eq!(
            SettingValue::from_value(&Value::from(vec!["a", "b"])),
            Some(SettingValue::Texts(vec!["a".into(), "b".into()]))
        );
        assert_eq!(
            SettingValue::from_value(&Value::from(vec![1.0f64, 2.0])),
            Some(SettingValue::Nums(vec![1.0, 2.0]))
        );
        assert_eq!(
            SettingValue::from_value(&Value::U8(3)),
            Some(SettingValue::Int(3))
        );
    }

    #[test]
    fn renderer_choice_comes_from_the_key() {
        let watcher = SettingsWatcher::new();
        watcher.apply_change(RENDERER_KEY, &Value::from("gl"));
        assert_eq!(watcher.renderer().get(), Some(RendererBackend::Gl));
        watcher.apply_change(RENDERER_KEY, &Value::from("bogus"));
        assert_eq!(watcher.renderer().get(), None);
    }
}
