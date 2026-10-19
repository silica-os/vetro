//! The resolved Amalfi theme of an appearance: the palette (with Increase Contrast and the
//! accent applied), materials as styles, shadows and type styles.

use crate::amalfi::tokens::{self, MaterialToken, Palette, TypeStyle, materials};
use crate::color::Color;
use crate::core::env::Appearance;
use crate::style::{Blur, Border, Glass, Shadow, Style};
use crate::text::TextStyle;

/// The theme of one appearance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    /// The settings it was made from.
    pub appearance: Appearance,
    /// The system colours.
    pub colors: Palette,
}

impl Theme {
    /// The theme of `appearance`.
    pub fn new(appearance: Appearance) -> Theme {
        let base = if appearance.dark {
            tokens::DARK
        } else {
            tokens::LIGHT
        };
        let mut colors = if appearance.increase_contrast {
            increase_contrast(base, appearance.dark)
        } else {
            base
        };
        if let Some(accent) = appearance.accent {
            colors.accent_amalfi = accent;
            colors.accent_amalfi_pressed = accent.lerp(Color::BLACK, 0.15);
            colors.accent_amalfi_soft = accent.with_alpha(colors.accent_amalfi_soft.a);
            colors.line_focus = accent.with_alpha(colors.line_focus.a);
        }
        Theme { appearance, colors }
    }

    /// Whether this is the dark appearance.
    pub fn dark(&self) -> bool {
        self.appearance.dark
    }

    /// Light theme.
    pub fn light() -> Theme {
        Theme::new(Appearance::light())
    }

    /// Dark theme.
    pub fn dark_theme() -> Theme {
        Theme::new(Appearance::dark())
    }

    /// A colour by token name (`Text/Primary`, `App/Clock/Window` …).
    pub fn color(&self, name: &str) -> Option<Color> {
        let system = match name {
            "Accent/Amalfi" => Some(self.colors.accent_amalfi),
            "Accent/Amalfi Pressed" => Some(self.colors.accent_amalfi_pressed),
            "Accent/Amalfi Soft" => Some(self.colors.accent_amalfi_soft),
            "Text/Primary" => Some(self.colors.text_primary),
            "Text/Secondary" => Some(self.colors.text_secondary),
            "Text/Tertiary" => Some(self.colors.text_tertiary),
            "Text/On Accent" => Some(self.colors.text_on_accent),
            "Line/Separator" => Some(self.colors.line_separator),
            "Line/Border" => Some(self.colors.line_border),
            "Line/Focus" => Some(self.colors.line_focus),
            "State/Hover Overlay" => Some(self.colors.state_hover_overlay),
            "State/Press Overlay" => Some(self.colors.state_press_overlay),
            _ => None,
        };
        system.or_else(|| tokens::app_color(self.dark(), name))
    }

    /// Shadow level 1–3 (0: none).
    pub fn shadow(&self, level: u8) -> Option<Shadow> {
        let table = if self.dark() {
            tokens::shadows::DARK
        } else {
            tokens::shadows::LIGHT
        };
        level
            .checked_sub(1)
            .and_then(|i| table.get(usize::from(i)))
            .copied()
    }

    /// The material token by name in this appearance.
    pub fn material(&self, name: &str) -> Option<MaterialToken> {
        materials::ALL
            .iter()
            .find(|(l, _)| l.name == name)
            .map(|(l, d)| if self.dark() { *d } else { *l })
    }

    /// A material as a layer style: blur and refraction, tint, inside rim, rim light and
    /// drop shadow (`materials.md` render order). With Reduce Transparency the blur is
    /// replaced by an opaque fill (tint over Surface/Window).
    pub fn material_style(&self, name: &str) -> Style {
        let Some(m) = self.material(name) else {
            return Style::default();
        };
        let mut style = Style::default();
        if let Some(level) = (m.shadow_level > 0).then_some(m.shadow_level)
            && let Some(s) = self.shadow(level)
        {
            style.shadows.push(s);
        }
        if self.appearance.reduce_transparency {
            let base = self.colors.surface_window;
            let fill = m.tint.map(|t| t.over(base)).unwrap_or(base);
            style.fills.push(fill.into());
        } else {
            if m.blur > 0.0 || m.distortion > 0.0 || m.lens {
                style.blurs.push(Blur::Glass(Glass {
                    radius: m.blur,
                    saturation: m.saturation,
                    brightness: m.brightness,
                    distortion: m.distortion,
                    depth: m.depth,
                    chromatic_aberration: m.chromatic_aberration,
                    specular: m.specular,
                    magnification: m.magnification,
                }));
            }
            if let Some(t) = m.tint {
                style.fills.push(t.into());
            }
        }
        if let Some(rim) = m.rim_light {
            style.inner_shadows.push(Shadow::new(rim, 0.0, 1.0, 1.0));
        }
        if let Some(rim) = m.rim_inside {
            style.borders.push(Border::inside(rim, 1.0));
        }
        style
    }

    /// A type token as a text style in `color`, scaled by the text size setting.
    pub fn text(&self, token: &TypeStyle, color: Color) -> TextStyle {
        let mut s = TextStyle::from_token(token, color);
        let k = self.appearance.text_scale;
        if k.is_finite() && k > 0.0 && (k - 1.0).abs() > 1e-3 {
            s.size *= k;
            s.line_height = s.line_height.map(|l| (l * k).round());
        }
        s
    }

    /// The disabled opacity.
    pub fn disabled_opacity(&self) -> f32 {
        tokens::states::DISABLED_OPACITY
    }
}

/// The high-contrast palette (Settings › Accessibility › Display › Increase Contrast):
/// secondary text and lines are stronger, the accent is deeper (light) or brighter (dark) and
/// the hover and press washes are firmer. Port of silica_ui's `increaseContrastOf`.
pub fn increase_contrast(mut c: Palette, dark: bool) -> Palette {
    let ink = if dark { Color::WHITE } else { Color::BLACK };
    let stronger = |col: Color, t: f32, alpha: f32| -> Color {
        let moved = col.with_alpha(255).lerp(ink, t);
        let a = col.alpha_f().max(alpha).clamp(0.0, 1.0);
        moved.with_alpha((a * 255.0).round() as u8)
    };
    let scaled = |col: Color, k: f32| -> Color {
        col.with_alpha((col.alpha_f() * k * 255.0).round().clamp(0.0, 255.0) as u8)
    };
    c.text_secondary = stronger(c.text_secondary, 0.55, 0.9);
    c.text_tertiary = stronger(c.text_tertiary, 0.5, 0.78);
    c.line_separator = ink.with_opacity(if dark { 0.42 } else { 0.36 });
    c.line_border = ink.with_opacity(if dark { 0.6 } else { 0.55 });
    c.accent_amalfi = c.accent_amalfi.lerp(ink, if dark { 0.16 } else { 0.2 });
    c.accent_amalfi_pressed = c
        .accent_amalfi_pressed
        .lerp(ink, if dark { 0.04 } else { 0.3 });
    c.accent_amalfi_soft = scaled(c.accent_amalfi_soft, 1.6);
    c.state_hover_overlay = scaled(c.state_hover_overlay, 1.6);
    c.state_press_overlay = scaled(c.state_press_overlay, 1.6);
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn material_styles() {
        let t = Theme::light();
        let s = t.material_style("Glass/Thick");
        assert_eq!(s.shadows.len(), 1);
        assert_eq!(s.shadows[0].blur, 32.0);
        assert_eq!(s.blurs.len(), 1);
        assert_eq!(s.borders.len(), 1);
        assert_eq!(s.inner_shadows.len(), 1);
        let rt = Theme::new(Appearance {
            reduce_transparency: true,
            ..Appearance::default()
        });
        let s = rt.material_style("Glass/Thick");
        assert!(s.blurs.is_empty());
        assert_eq!(s.fills.len(), 1);
        assert_eq!(s.fills[0].solid().map(|c| c.a), Some(255));
    }

    #[test]
    fn contrast_strengthens() {
        let base = Theme::light();
        let ic = Theme::new(Appearance {
            increase_contrast: true,
            ..Appearance::default()
        });
        assert!(ic.colors.line_border.a > base.colors.line_border.a);
        assert_eq!(ic.color("Text/Primary"), Some(base.colors.text_primary));
        assert!(Theme::dark_theme().color("App/Clock/Window").is_some());
    }
}
