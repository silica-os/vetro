//! The system look and the accessibility switches, followed live from System1 and mapped to
//! [`Appearance`] (and the full [`SystemLook`] for the parts a toolkit needs beyond it).
//!
//! Source: the flat `State` dictionary of `dev.frascella.Silica.System1` (`appearance.dark`,
//! `look.*`) and its `Changed` signal (which carries the whole dictionary). Settings1 holds no
//! appearance keys today; Settings (the app) writes them through `System1.SetLook`. Outside
//! Silica, or while the daemon is down, everything is the default.

use std::collections::HashMap;

use futures_lite::StreamExt;
use zbus::Connection;
use zbus::zvariant::OwnedValue;

use crate::color::Color;
use crate::core::env::Appearance;
use crate::services::error::Error;
use crate::services::observable::Observable;
use crate::services::owner::daemon_presence;
use crate::services::proxies::System1Proxy;
use crate::services::wire::{dict_bool, dict_f64, dict_i64, dict_str};

/// The nine accent names of the daemon (`look.accent`, `look.highlight`, `look.folderColour`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AccentName {
    /// The Amalfi blue, no override.
    #[default]
    Multicolour,
    Blue,
    Purple,
    Pink,
    Red,
    Orange,
    Yellow,
    Green,
    Graphite,
}

impl AccentName {
    /// `multicolour` (and `multicolor`) and the other names; `automatic` and unknown are `None`.
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "multicolour" | "multicolor" => Self::Multicolour,
            "blue" => Self::Blue,
            "purple" => Self::Purple,
            "pink" => Self::Pink,
            "red" => Self::Red,
            "orange" => Self::Orange,
            "yellow" => Self::Yellow,
            "green" => Self::Green,
            "graphite" => Self::Graphite,
            _ => return None,
        })
    }

    /// The colour of the accent (the plain system colour of the kit, CT-CL-01), or `None` for
    /// multicolour, which keeps the Amalfi blue.
    pub fn color(self, dark: bool) -> Option<Color> {
        let (light, dim) = match self {
            Self::Multicolour => return None,
            Self::Blue => ((0x00, 0x7A, 0xFF), (0x00, 0x7A, 0xFF)),
            Self::Purple => ((0x95, 0x3D, 0x96), (0xA6, 0x50, 0xA7)),
            Self::Pink => ((0xF7, 0x4F, 0x9F), (0xF7, 0x4F, 0x9F)),
            Self::Red => ((0xE0, 0x39, 0x3E), (0xFF, 0x52, 0x57)),
            Self::Orange => ((0xF9, 0x8A, 0x17), (0xFF, 0x9F, 0x40)),
            Self::Yellow => ((0xFF, 0xC7, 0x27), (0xFF, 0xC6, 0x01)),
            Self::Green => ((0x62, 0xBA, 0x46), (0x62, 0xBA, 0x46)),
            Self::Graphite => ((0x98, 0x98, 0x98), (0x8C, 0x8C, 0x8C)),
        };
        let (r, g, b) = if dark { dim } else { light };
        Some(Color::rgb(r, g, b))
    }
}

macro_rules! wire_enum {
    ($(#[$meta:meta])* $name:ident { $($variant:ident = $wire:literal),+ $(,)? } default $default:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum $name { $($variant),+ }

        impl Default for $name {
            fn default() -> Self { Self::$default }
        }

        impl $name {
            /// The value on the wire; unknown text is the default.
            pub fn parse(text: &str) -> Self {
                match text { $($wire => Self::$variant,)+ _ => Self::$default }
            }
        }
    };
}

wire_enum!(
    /// `look.sidebarIconSize`.
    SidebarIconSize { Small = "small", Medium = "medium", Large = "large" } default Medium);
wire_enum!(
    /// `look.scrollBars`.
    ScrollBars { Automatic = "automatic", WhenScrolling = "whenScrolling", Always = "always" }
    default Automatic);
wire_enum!(
    /// `look.scrollClick`.
    ScrollClick { Page = "page", Spot = "spot" } default Page);
wire_enum!(
    /// `look.iconStyle`.
    IconStyle { Default = "default", Dark = "dark", Clear = "clear", Tinted = "tinted" }
    default Default);

/// The look chosen in Settings, as the daemon keeps it (`look.*` of the System1 state).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SystemLook {
    pub dark: bool,
    pub accent: AccentName,
    /// `None`: automatic (follows the accent).
    pub highlight: Option<AccentName>,
    pub sidebar_icon_size: SidebarIconSize,
    pub scroll_bars: ScrollBars,
    pub scroll_click: ScrollClick,
    pub icon_style: IconStyle,
    pub folder_colour: Option<AccentName>,
    /// Glass opacity, 0-1.
    pub glass: f64,
    pub window_tint: bool,
    pub keyboard_navigation: bool,
    /// Longest time between the clicks of a double click, 150-1000 ms.
    pub double_click_ms: u32,
    pub reduce_transparency: bool,
    pub reduce_motion: bool,
    pub increase_contrast: bool,
    /// 1-1.5.
    pub text_scale: f32,
}

impl Default for SystemLook {
    fn default() -> Self {
        SystemLook {
            dark: false,
            accent: AccentName::Multicolour,
            highlight: None,
            sidebar_icon_size: SidebarIconSize::default(),
            scroll_bars: ScrollBars::default(),
            scroll_click: ScrollClick::default(),
            icon_style: IconStyle::default(),
            folder_colour: None,
            glass: 0.5,
            window_tint: false,
            keyboard_navigation: false,
            double_click_ms: 300,
            reduce_transparency: false,
            reduce_motion: false,
            increase_contrast: false,
            text_scale: 1.0,
        }
    }
}

impl SystemLook {
    /// Reads the System1 state dictionary; a missing or odd value is the default.
    pub fn from_state(state: &HashMap<String, OwnedValue>) -> Self {
        let d = SystemLook::default();
        let text = |key: &str| dict_str(state, key);
        let flag = |key: &str| dict_bool(state, key).unwrap_or(false);
        SystemLook {
            dark: flag("appearance.dark"),
            accent: text("look.accent")
                .and_then(AccentName::parse)
                .unwrap_or(d.accent),
            highlight: text("look.highlight").and_then(AccentName::parse),
            sidebar_icon_size: text("look.sidebarIconSize")
                .map_or(d.sidebar_icon_size, SidebarIconSize::parse),
            scroll_bars: text("look.scrollBars").map_or(d.scroll_bars, ScrollBars::parse),
            scroll_click: text("look.scrollClick").map_or(d.scroll_click, ScrollClick::parse),
            icon_style: text("look.iconStyle").map_or(d.icon_style, IconStyle::parse),
            folder_colour: text("look.folderColour").and_then(AccentName::parse),
            glass: dict_f64(state, "look.glass").map_or(d.glass, |g| g.clamp(0.0, 1.0)),
            window_tint: flag("look.windowTint"),
            keyboard_navigation: flag("look.keyboardNavigation"),
            double_click_ms: dict_i64(state, "look.doubleClickMs")
                .map_or(d.double_click_ms, |n| n.clamp(150, 1000) as u32),
            reduce_transparency: flag("look.reduceTransparency"),
            reduce_motion: flag("look.reduceMotion"),
            increase_contrast: flag("look.increaseContrast"),
            text_scale: dict_f64(state, "look.textScale")
                .map_or(d.text_scale, |n| n.clamp(1.0, 1.5) as f32),
        }
    }

    /// The toolkit's [`Appearance`]: the accent and highlight become colours (multicolour and
    /// automatic stay `None`: the Amalfi defaults). A named highlight uses the accent colour of
    /// that name; the daemon has no separate selection tones.
    pub fn appearance(&self) -> Appearance {
        Appearance {
            dark: self.dark,
            increase_contrast: self.increase_contrast,
            reduce_transparency: self.reduce_transparency,
            reduce_motion: self.reduce_motion,
            accent: self.accent.color(self.dark),
            highlight: self.highlight.and_then(|h| h.color(self.dark)),
            full_keyboard_access: self.keyboard_navigation,
            text_scale: self.text_scale,
        }
    }
}

/// Follows System1 and publishes the look.
pub struct AppearanceWatcher {
    look: Observable<SystemLook>,
    appearance: Observable<Appearance>,
}

impl Default for AppearanceWatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl AppearanceWatcher {
    /// Starts with the defaults (light, nothing reduced, text scale 1).
    pub fn new() -> Self {
        AppearanceWatcher {
            look: Observable::new(SystemLook::default()),
            appearance: Observable::new(Appearance::default()),
        }
    }

    pub fn look(&self) -> &Observable<SystemLook> {
        &self.look
    }

    pub fn appearance(&self) -> &Observable<Appearance> {
        &self.appearance
    }

    /// Applies a System1 state dictionary.
    pub fn apply_state(&self, state: &HashMap<String, OwnedValue>) {
        self.set_look(SystemLook::from_state(state));
    }

    /// Back to the defaults (the daemon went away).
    pub fn reset(&self) {
        self.set_look(SystemLook::default());
    }

    fn set_look(&self, look: SystemLook) {
        self.appearance.set(look.appearance());
        self.look.set(look);
    }

    /// Follows the daemon until the connection closes: reads `State`, applies `Changed`, reads
    /// again when the daemon (re)appears and resets to the defaults when it goes. Never fails.
    /// Run it on the platform's executor.
    pub async fn run(&self, connection: &Connection) {
        let _ = self.try_run(connection).await;
    }

    async fn try_run(&self, connection: &Connection) -> Result<(), Error> {
        enum Event {
            State(HashMap<String, OwnedValue>),
            Present(bool),
        }
        let proxy = System1Proxy::new(connection).await?;
        let changed = proxy
            .receive_changed()
            .await?
            .filter_map(|signal| Some(Event::State(signal.args().ok()?.state)));
        let presence = daemon_presence(connection).await?.map(Event::Present);
        let mut events = Box::pin(changed.race(presence));
        if let Ok(state) = proxy.state().await {
            self.apply_state(&state);
        }
        while let Some(event) = events.next().await {
            match event {
                Event::State(state) => self.apply_state(&state),
                Event::Present(true) => {
                    if let Ok(state) = proxy.state().await {
                        self.apply_state(&state);
                    }
                }
                Event::Present(false) => self.reset(),
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zbus::zvariant::Value;

    fn owned(value: Value<'_>) -> OwnedValue {
        OwnedValue::try_from(value).unwrap()
    }

    #[test]
    fn empty_state_is_the_default() {
        let look = SystemLook::from_state(&HashMap::new());
        assert_eq!(look, SystemLook::default());
        assert_eq!(look.appearance(), Appearance::default());
    }

    #[test]
    fn maps_the_state() {
        let mut state = HashMap::new();
        state.insert("appearance.dark".to_owned(), owned(Value::Bool(true)));
        state.insert("look.accent".to_owned(), owned(Value::from("red")));
        state.insert("look.highlight".to_owned(), owned(Value::from("green")));
        state.insert("look.increaseContrast".to_owned(), owned(Value::Bool(true)));
        state.insert(
            "look.reduceTransparency".to_owned(),
            owned(Value::Bool(true)),
        );
        state.insert("look.reduceMotion".to_owned(), owned(Value::Bool(true)));
        state.insert(
            "look.keyboardNavigation".to_owned(),
            owned(Value::Bool(true)),
        );
        state.insert("look.textScale".to_owned(), owned(Value::F64(1.25)));
        state.insert("look.doubleClickMs".to_owned(), owned(Value::U32(5000)));
        let look = SystemLook::from_state(&state);
        assert_eq!(look.double_click_ms, 1000);
        let a = look.appearance();
        assert!(a.dark && a.increase_contrast && a.reduce_transparency && a.reduce_motion);
        assert!(a.full_keyboard_access);
        assert_eq!(a.accent, Some(Color::rgb(0xFF, 0x52, 0x57)));
        assert_eq!(a.highlight, Some(Color::rgb(0x62, 0xBA, 0x46)));
        assert_eq!(a.text_scale, 1.25);
    }

    #[test]
    fn odd_values_fall_back() {
        let mut state = HashMap::new();
        state.insert("look.textScale".to_owned(), owned(Value::F64(f64::NAN)));
        state.insert("look.accent".to_owned(), owned(Value::from("chartreuse")));
        state.insert("look.highlight".to_owned(), owned(Value::from("automatic")));
        state.insert("appearance.dark".to_owned(), owned(Value::from("yes")));
        state.insert("look.glass".to_owned(), owned(Value::F64(7.0)));
        let look = SystemLook::from_state(&state);
        assert_eq!(look.text_scale, 1.0);
        assert_eq!(look.accent, AccentName::Multicolour);
        assert_eq!(look.highlight, None);
        assert!(!look.dark);
        assert_eq!(look.glass, 1.0);
        let big = HashMap::from([("look.textScale".to_owned(), owned(Value::F64(9.0)))]);
        assert_eq!(SystemLook::from_state(&big).text_scale, 1.5);
    }
}
