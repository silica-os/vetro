//! xkb keysyms to Vetro keys.

use smithay_client_toolkit::seat::keyboard::{Keysym, Modifiers};

use crate::core::event::{Key, Mods};

/// The key of a keysym (the layout's character for printable keys, lowercased).
pub fn key_of(sym: Keysym) -> Key {
    match sym {
        Keysym::Return | Keysym::KP_Enter | Keysym::ISO_Enter => Key::Enter,
        Keysym::Tab | Keysym::ISO_Left_Tab | Keysym::KP_Tab => Key::Tab,
        Keysym::Escape => Key::Escape,
        Keysym::BackSpace => Key::Backspace,
        Keysym::Delete | Keysym::KP_Delete => Key::Delete,
        Keysym::space | Keysym::KP_Space => Key::Space,
        Keysym::Left | Keysym::KP_Left => Key::Left,
        Keysym::Right | Keysym::KP_Right => Key::Right,
        Keysym::Up | Keysym::KP_Up => Key::Up,
        Keysym::Down | Keysym::KP_Down => Key::Down,
        Keysym::Home | Keysym::KP_Home => Key::Home,
        Keysym::End | Keysym::KP_End => Key::End,
        Keysym::Page_Up | Keysym::KP_Page_Up => Key::PageUp,
        Keysym::Page_Down | Keysym::KP_Page_Down => Key::PageDown,
        Keysym::Shift_L
        | Keysym::Shift_R
        | Keysym::Control_L
        | Keysym::Control_R
        | Keysym::Alt_L
        | Keysym::Alt_R
        | Keysym::Super_L
        | Keysym::Super_R
        | Keysym::Meta_L
        | Keysym::Meta_R
        | Keysym::Caps_Lock
        | Keysym::ISO_Level3_Shift => Key::Modifier,
        other => {
            let raw = other.raw();
            if (Keysym::F1.raw()..=Keysym::F24.raw()).contains(&raw) {
                return Key::F((raw - Keysym::F1.raw() + 1) as u8);
            }
            match other.key_char() {
                Some(c) if !c.is_control() => Key::Char(c.to_lowercase().next().unwrap_or(c)),
                _ => Key::Other(raw),
            }
        }
    }
}

/// Vetro modifiers from SCTK's.
pub fn mods_of(m: &Modifiers) -> Mods {
    let mut out = Mods::empty();
    out.set(Mods::SHIFT, m.shift);
    out.set(Mods::CTRL, m.ctrl);
    out.set(Mods::ALT, m.alt);
    out.set(Mods::LOGO, m.logo);
    out.set(Mods::CAPS, m.caps_lock);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mapping() {
        assert_eq!(key_of(Keysym::Return), Key::Enter);
        assert_eq!(key_of(Keysym::a), Key::Char('a'));
        assert_eq!(key_of(Keysym::A), Key::Char('a'));
        assert_eq!(key_of(Keysym::F5), Key::F(5));
        assert_eq!(key_of(Keysym::Shift_L), Key::Modifier);
    }
}
