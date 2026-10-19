//! Input events as views receive them (positions in the view's own coordinates).

use crate::geom::Point;

bitflags::bitflags! {
    /// Keyboard modifiers. ⌘ is Logo (the macOS model: shortcuts use Logo).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct Mods: u32 {
        /// Shift.
        const SHIFT = 1 << 0;
        /// Control.
        const CTRL = 1 << 1;
        /// Alt (⌥ Option).
        const ALT = 1 << 2;
        /// Logo (⌘ Command).
        const LOGO = 1 << 3;
        /// Caps Lock.
        const CAPS = 1 << 4;
    }
}

impl Mods {
    /// Whether ⌘ is the only command modifier held (Shift allowed).
    pub fn command(self) -> bool {
        self.contains(Mods::LOGO) && !self.intersects(Mods::CTRL | Mods::ALT)
    }
}

/// A pointer button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Button {
    /// Primary.
    Left,
    /// Secondary (context menu).
    Right,
    /// Middle.
    Middle,
    /// Others (back, forward …).
    Other(u32),
}

/// A key, by meaning.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Key {
    /// A character key (lowercase without Shift, as the layout produces it).
    Char(char),
    /// Return / Enter.
    Enter,
    /// Tab.
    Tab,
    /// Escape.
    Escape,
    /// Backspace.
    Backspace,
    /// Delete (forward).
    Delete,
    /// Space bar.
    Space,
    /// Arrows.
    Left,
    /// Arrows.
    Right,
    /// Arrows.
    Up,
    /// Arrows.
    Down,
    /// Home.
    Home,
    /// End.
    End,
    /// Page Up.
    PageUp,
    /// Page Down.
    PageDown,
    /// Function keys.
    F(u8),
    /// A modifier key itself.
    Modifier,
    /// Anything else (keysym).
    Other(u32),
}

impl Key {
    /// The key's name for shortcuts and logs (`a`, `enter`, `f5` …).
    pub fn name(&self) -> String {
        match self {
            Key::Char(c) => c.to_lowercase().collect(),
            Key::Enter => "enter".into(),
            Key::Tab => "tab".into(),
            Key::Escape => "escape".into(),
            Key::Backspace => "backspace".into(),
            Key::Delete => "delete".into(),
            Key::Space => "space".into(),
            Key::Left => "left".into(),
            Key::Right => "right".into(),
            Key::Up => "up".into(),
            Key::Down => "down".into(),
            Key::Home => "home".into(),
            Key::End => "end".into(),
            Key::PageUp => "pageup".into(),
            Key::PageDown => "pagedown".into(),
            Key::F(n) => format!("f{n}"),
            Key::Modifier => "modifier".into(),
            Key::Other(k) => format!("key{k}"),
        }
    }
}

/// A key press or release.
#[derive(Debug, Clone, PartialEq)]
pub struct KeyEvent {
    /// The key.
    pub key: Key,
    /// The text it types (none for function keys and with ⌘ or Ctrl).
    pub text: Option<String>,
    /// Modifiers held.
    pub mods: Mods,
    /// Pressed (false: released).
    pub pressed: bool,
    /// A repeat of a held key.
    pub repeat: bool,
}

impl KeyEvent {
    /// A press of `key` with `mods`, typing `text`.
    pub fn press(key: Key, mods: Mods) -> Self {
        let text = match &key {
            Key::Char(c) if !mods.intersects(Mods::CTRL | Mods::LOGO) => {
                Some(if mods.contains(Mods::SHIFT) {
                    c.to_uppercase().collect()
                } else {
                    c.to_string()
                })
            }
            Key::Space if !mods.intersects(Mods::CTRL | Mods::LOGO) => Some(" ".into()),
            _ => None,
        };
        KeyEvent {
            key,
            text,
            mods,
            pressed: true,
            repeat: false,
        }
    }

    /// Whether this matches a shortcut like `cmd+shift+s` (names: cmd, shift, alt, ctrl).
    pub fn matches(&self, shortcut: &str) -> bool {
        let Some(sc) = Shortcut::parse(shortcut) else {
            return false;
        };
        self.pressed && sc.key == self.key.name() && sc.mods == (self.mods - Mods::CAPS)
    }
}

/// A parsed keyboard shortcut.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shortcut {
    /// Modifiers.
    pub mods: Mods,
    /// Key name.
    pub key: String,
}

impl Shortcut {
    /// Parses `cmd+shift+s`, `ctrl+alt+delete` …
    pub fn parse(text: &str) -> Option<Shortcut> {
        let mut mods = Mods::empty();
        let mut key = None;
        for part in text.split('+').map(str::trim) {
            match part.to_ascii_lowercase().as_str() {
                "cmd" | "command" | "logo" | "super" | "⌘" => mods |= Mods::LOGO,
                "shift" | "⇧" => mods |= Mods::SHIFT,
                "alt" | "option" | "opt" | "⌥" => mods |= Mods::ALT,
                "ctrl" | "control" | "⌃" => mods |= Mods::CTRL,
                "" => return None,
                other => key = Some(other.to_string()),
            }
        }
        Some(Shortcut { mods, key: key? })
    }

    /// The menu glyphs of the shortcut (`⇧⌘S`), as menus show it.
    pub fn display(&self) -> String {
        let mut s = String::new();
        if self.mods.contains(Mods::CTRL) {
            s.push('⌃');
        }
        if self.mods.contains(Mods::ALT) {
            s.push('⌥');
        }
        if self.mods.contains(Mods::SHIFT) {
            s.push('⇧');
        }
        if self.mods.contains(Mods::LOGO) {
            s.push('⌘');
        }
        let k = match self.key.as_str() {
            "enter" => "↩".to_string(),
            "escape" => "⎋".to_string(),
            "backspace" => "⌫".to_string(),
            "delete" => "⌦".to_string(),
            "tab" => "⇥".to_string(),
            "space" => "Space".to_string(),
            "left" => "←".to_string(),
            "right" => "→".to_string(),
            "up" => "↑".to_string(),
            "down" => "↓".to_string(),
            other => other.to_uppercase(),
        };
        s.push_str(&k);
        s
    }
}

/// The phase of a continuous gesture (touchpad scroll, pinch).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// No phase (a wheel click).
    None,
    /// Began.
    Began,
    /// Changed.
    Changed,
    /// Ended (fingers lifted: momentum may follow).
    Ended,
}

/// An event delivered to a view.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// A button went down.
    PointerDown {
        /// Position.
        pos: Point,
        /// Button.
        button: Button,
        /// Click count (2 for a double click).
        clicks: u32,
        /// Modifiers.
        mods: Mods,
    },
    /// A button went up.
    PointerUp {
        /// Position.
        pos: Point,
        /// Button.
        button: Button,
        /// Modifiers.
        mods: Mods,
    },
    /// The pointer moved (also while dragging, to the view that took the press).
    PointerMove {
        /// Position.
        pos: Point,
        /// Modifiers.
        mods: Mods,
        /// Whether the primary button is down.
        dragging: bool,
    },
    /// The pointer entered the view.
    PointerEnter,
    /// The pointer left the view.
    PointerLeave,
    /// Scroll, in points (positive: content moves up / left, i.e. scrolls down / right).
    Scroll {
        /// Position.
        pos: Point,
        /// Horizontal delta.
        dx: f32,
        /// Vertical delta.
        dy: f32,
        /// Precise (touchpad) or wheel clicks.
        precise: bool,
        /// Gesture phase.
        phase: Phase,
    },
    /// Pinch.
    Magnify {
        /// Scale change since the last event (1: none).
        scale: f32,
        /// Phase.
        phase: Phase,
    },
    /// A key.
    Key(KeyEvent),
    /// Committed text from the input method or the keyboard.
    Text(String),
    /// The input method's pre-edit (composition) text and caret.
    Preedit {
        /// Text being composed (empty: composition ended).
        text: String,
        /// Caret byte offset in it.
        cursor: Option<(usize, usize)>,
    },
    /// The input method deletes text around the caret (bytes before, after).
    DeleteSurrounding {
        /// Bytes before the caret.
        before: usize,
        /// Bytes after.
        after: usize,
    },
    /// The control's action: a click released inside, or an activator key.
    Activate,
    /// Keyboard focus came in.
    FocusIn,
    /// Keyboard focus went out.
    FocusOut,
    /// A clipboard paste of text.
    Paste(String),
    /// Files or text dropped (URIs or text).
    Drop {
        /// Position.
        pos: Point,
        /// URIs (text/uri-list) or one text item.
        items: Vec<String>,
    },
}

/// Whether a view handled an event (an unhandled one bubbles to the parent).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handled {
    /// Handled: stop.
    Yes,
    /// Not handled: bubble up.
    No,
}

impl From<bool> for Handled {
    fn from(b: bool) -> Self {
        if b { Handled::Yes } else { Handled::No }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcuts() {
        let sc = Shortcut::parse("cmd+shift+s").unwrap();
        assert_eq!(sc.mods, Mods::LOGO | Mods::SHIFT);
        assert_eq!(sc.display(), "⇧⌘S");
        let e = KeyEvent::press(Key::Char('s'), Mods::LOGO | Mods::SHIFT);
        assert!(e.matches("cmd+shift+s"));
        assert!(!e.matches("cmd+s"));
        assert!(Shortcut::parse("cmd+").is_none());
        assert_eq!(
            KeyEvent::press(Key::Char('a'), Mods::SHIFT).text.as_deref(),
            Some("A")
        );
        assert_eq!(KeyEvent::press(Key::Char('a'), Mods::LOGO).text, None);
    }
}
