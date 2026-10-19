//! Colours: straight (not premultiplied) sRGB with alpha, 8 bits per channel like the kit.

/// An sRGB colour with straight alpha.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Color {
    /// Red.
    pub r: u8,
    /// Green.
    pub g: u8,
    /// Blue.
    pub b: u8,
    /// Alpha (255 opaque).
    pub a: u8,
}

impl Color {
    /// Fully transparent.
    pub const TRANSPARENT: Color = Color::rgba(0, 0, 0, 0);
    /// Opaque black.
    pub const BLACK: Color = Color::rgba(0, 0, 0, 255);
    /// Opaque white.
    pub const WHITE: Color = Color::rgba(255, 255, 255, 255);

    /// A colour from its channels.
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Color { r, g, b, a }
    }

    /// An opaque colour.
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color { r, g, b, a: 255 }
    }

    /// `0xRRGGBBAA`.
    pub const fn from_u32(v: u32) -> Self {
        Color {
            r: (v >> 24) as u8,
            g: (v >> 16) as u8,
            b: (v >> 8) as u8,
            a: v as u8,
        }
    }

    /// `0xRRGGBBAA`.
    pub const fn to_u32(self) -> u32 {
        ((self.r as u32) << 24) | ((self.g as u32) << 16) | ((self.b as u32) << 8) | self.a as u32
    }

    /// Parses `#rgb`, `#rrggbb` or `#rrggbbaa` (the kit's and the tokens' format).
    pub fn parse(text: &str) -> Option<Color> {
        let hex = text.trim().strip_prefix('#')?;
        let digit = |c: u8| -> Option<u8> {
            match c {
                b'0'..=b'9' => Some(c - b'0'),
                b'a'..=b'f' => Some(c - b'a' + 10),
                b'A'..=b'F' => Some(c - b'A' + 10),
                _ => None,
            }
        };
        let bytes = hex.as_bytes();
        let pair = |i: usize| -> Option<u8> {
            Some(digit(*bytes.get(i)?)? * 16 + digit(*bytes.get(i + 1)?)?)
        };
        match bytes.len() {
            3 => {
                let d = |i: usize| -> Option<u8> { Some(digit(*bytes.get(i)?)? * 17) };
                Some(Color::rgb(d(0)?, d(1)?, d(2)?))
            }
            6 => Some(Color::rgb(pair(0)?, pair(2)?, pair(4)?)),
            8 => Some(Color::rgba(pair(0)?, pair(2)?, pair(4)?, pair(6)?)),
            _ => None,
        }
    }

    /// `#rrggbbaa`.
    pub fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}{:02x}", self.r, self.g, self.b, self.a)
    }

    /// The same colour with its alpha multiplied by `k` (0–1).
    pub fn with_opacity(self, k: f32) -> Color {
        let k = if k.is_finite() {
            k.clamp(0.0, 1.0)
        } else {
            1.0
        };
        Color {
            a: (f32::from(self.a) * k).round() as u8,
            ..self
        }
    }

    /// The same colour with alpha `a`.
    pub const fn with_alpha(self, a: u8) -> Color {
        Color { a, ..self }
    }

    /// Alpha as 0–1.
    pub fn alpha_f(self) -> f32 {
        f32::from(self.a) / 255.0
    }

    /// `self` drawn over `below` (source-over, straight alpha).
    pub fn over(self, below: Color) -> Color {
        let sa = self.alpha_f();
        let da = below.alpha_f();
        let oa = sa + da * (1.0 - sa);
        if oa <= 0.0 {
            return Color::TRANSPARENT;
        }
        let ch = |s: u8, d: u8| -> u8 {
            let v = (f32::from(s) * sa + f32::from(d) * da * (1.0 - sa)) / oa;
            v.round().clamp(0.0, 255.0) as u8
        };
        Color::rgba(
            ch(self.r, below.r),
            ch(self.g, below.g),
            ch(self.b, below.b),
            (oa * 255.0).round() as u8,
        )
    }

    /// Linear interpolation in sRGB, `t` from 0 (`self`) to 1 (`other`).
    pub fn lerp(self, other: Color, t: f32) -> Color {
        let t = if t.is_finite() {
            t.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let l = |a: u8, b: u8| -> u8 {
            (f32::from(a) + (f32::from(b) - f32::from(a)) * t)
                .round()
                .clamp(0.0, 255.0) as u8
        };
        Color::rgba(
            l(self.r, other.r),
            l(self.g, other.g),
            l(self.b, other.b),
            l(self.a, other.a),
        )
    }

    /// Relative luminance (WCAG), 0–1.
    pub fn luminance(self) -> f32 {
        let lin = |c: u8| {
            let c = f32::from(c) / 255.0;
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * lin(self.r) + 0.7152 * lin(self.g) + 0.0722 * lin(self.b)
    }

    /// The Skia colour.
    pub fn to_sk(self) -> skia_safe::Color {
        skia_safe::Color::from_argb(self.a, self.r, self.g, self.b)
    }

    /// The Skia colour as floats.
    pub fn to_sk4f(self) -> skia_safe::Color4f {
        skia_safe::Color4f::from(self.to_sk())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_formats() {
        assert_eq!(Color::parse("#fff"), Some(Color::WHITE));
        assert_eq!(Color::parse("#0000001f"), Some(Color::rgba(0, 0, 0, 0x1f)));
        assert_eq!(Color::parse("#FE6E66"), Some(Color::rgb(0xfe, 0x6e, 0x66)));
        assert_eq!(Color::parse("fff"), None);
        assert_eq!(Color::parse("#ggg"), None);
        assert_eq!(Color::parse("#12345"), None);
        assert_eq!(Color::rgba(1, 2, 3, 4).to_hex(), "#01020304");
        assert_eq!(Color::from_u32(0x11223344).to_u32(), 0x11223344);
    }

    #[test]
    fn compositing() {
        let half_white = Color::rgba(255, 255, 255, 128);
        let c = half_white.over(Color::BLACK);
        assert_eq!(c.a, 255);
        assert!((i32::from(c.r) - 128).abs() <= 1);
        assert_eq!(Color::BLACK.lerp(Color::WHITE, 0.5).r, 128);
    }
}
