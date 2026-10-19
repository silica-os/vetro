//! The Amalfi tokens, generated at build time from `data/amalfi-tokens.json` (a copy of
//! silica-desktop's `docs/design/amalfi/tokens.json`) and `data/amalfi-motion.md`.

#![allow(missing_docs, clippy::excessive_precision, clippy::unreadable_literal)]

use crate::color::Color;
pub use crate::motion::{Cubic, SpringToken};
use crate::style::Shadow;

/// A type style.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TypeStyle {
    /// The token's name (`Body`, `Title 1` …).
    pub name: &'static str,
    /// Font family.
    pub family: &'static str,
    /// Size in points.
    pub size: f32,
    /// Line height in points (`None`: the font's natural one).
    pub line_height: Option<f32>,
    /// CSS weight (400 regular, 500 medium, 600 semibold, 700 bold).
    pub weight: u16,
    /// Tracking in points.
    pub tracking: f32,
    /// OpenType features (`tnum` …).
    pub features: &'static [&'static str],
}

/// A material in one appearance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MaterialToken {
    /// The token's name (`Glass/Regular` …).
    pub name: &'static str,
    /// Backdrop blur σ in points.
    pub blur: f32,
    /// Saturation of the backdrop.
    pub saturation: f32,
    /// Brightness of the backdrop.
    pub brightness: f32,
    /// Tint fill over the backdrop.
    pub tint: Option<Color>,
    /// The 1 pt inside rim.
    pub rim_inside: Option<Color>,
    /// The rim light (inner shadow 0, 1, blur 1).
    pub rim_light: Option<Color>,
    /// Refraction.
    pub distortion: f32,
    /// Refraction band.
    pub depth: f32,
    /// Chromatic split.
    pub chromatic_aberration: f32,
    /// Lens magnification.
    pub magnification: f32,
    /// Specular highlight on the rim.
    pub specular: bool,
    /// Drop shadow level (0: none).
    pub shadow_level: u8,
    /// A lens (Glass/Clear).
    pub lens: bool,
}

include!(concat!(env!("OUT_DIR"), "/amalfi_tokens.rs"));

/// A colour of an app's palette (`App/Clock/Window` …) or any token, by name.
pub fn app_color(dark: bool, name: &str) -> Option<Color> {
    let table = if dark { DARK_ALL } else { LIGHT_ALL };
    table
        .binary_search_by(|(k, _)| (*k).cmp(name))
        .ok()
        .and_then(|i| table.get(i))
        .map(|(_, c)| *c)
}

/// A type style by its token name.
pub fn type_style(name: &str) -> Option<TypeStyle> {
    typography::ALL.iter().find(|t| t.name == name).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_values_match_the_json() {
        assert_eq!(LIGHT.text_primary.a, 0xff);
        assert_eq!(app_color(true, "App/Calculator/Window").is_some(), true);
        assert_eq!(app_color(false, "App/Calculator/Window"), None);
        assert_eq!(typography::BODY.size, 13.0);
        assert_eq!(typography::BODY.line_height, Some(18.0));
        assert_eq!(typography::LOCK_CLOCK.features, &["tnum"]);
        assert_eq!(control_heights::REGULAR, 28.0);
        assert_eq!(radii::MENU, 12.0);
        assert_eq!(shadows::LIGHT[2].blur, 32.0);
        assert_eq!(states::DISABLED_OPACITY, 0.35);
        assert_eq!(materials::GLASS_THICK_LIGHT.blur, 10.0);
        assert_eq!(materials::GLASS_CLEAR_DARK.magnification, 1.1);
        assert_eq!(motion::SPRING_BOUNCY.damping_ratio, 0.5);
        assert_eq!(motion::FADE_QUICK, 0.15);
        assert_eq!(motion::FADE_STANDARD_CURVE, Cubic::new(0.4, 0.0, 0.2, 1.0));
        assert_eq!(window_controls::CLOSE.0[0], Color::rgb(0xfe, 0x6e, 0x66));
        assert!(type_style("Title 1").is_some());
    }
}
