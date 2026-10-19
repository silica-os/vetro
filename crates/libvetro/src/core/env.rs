//! The environment views are drawn in: appearance and accessibility settings, followed live
//! from System1 and Settings1 by the services layer.

use crate::color::Color;

/// Appearance and accessibility settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Appearance {
    /// Dark appearance.
    pub dark: bool,
    /// Increase Contrast.
    pub increase_contrast: bool,
    /// Reduce Transparency: glass becomes opaque.
    pub reduce_transparency: bool,
    /// Reduce Motion: springs become cross-fades.
    pub reduce_motion: bool,
    /// Accent colour override (`None`: Amalfi).
    pub accent: Option<Color>,
    /// Highlight (text selection) colour override.
    pub highlight: Option<Color>,
    /// Full Keyboard Access: Tab reaches every control, not only text fields and lists.
    pub full_keyboard_access: bool,
    /// Text size category multiplier (1: default).
    pub text_scale: f32,
}

impl Default for Appearance {
    fn default() -> Self {
        Appearance {
            dark: false,
            increase_contrast: false,
            reduce_transparency: false,
            reduce_motion: false,
            accent: None,
            highlight: None,
            full_keyboard_access: false,
            text_scale: 1.0,
        }
    }
}

impl Appearance {
    /// The light appearance.
    pub fn light() -> Self {
        Appearance::default()
    }

    /// The dark appearance.
    pub fn dark() -> Self {
        Appearance {
            dark: true,
            ..Appearance::default()
        }
    }
}
