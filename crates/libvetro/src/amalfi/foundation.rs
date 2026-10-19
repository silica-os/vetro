//! What every Amalfi component draws with: control sizes, whole-point label boxes, the
//! focus halo, the control shadow, cached labels and the hover/press transitions
//! (silica_ui's `foundation/`, ported).

use std::rc::Rc;

use crate::amalfi::theme::Theme;
use crate::amalfi::tokens::{self, TypeStyle, control_heights, control_icons, focus, typography};
use crate::color::Color;
use crate::core::display::Recorder;
use crate::core::tree::NodeFlags;
use crate::geom::Rect;
use crate::motion::{Animated, Timing};
use crate::shape::Shape;
use crate::style::{Border, BorderPosition, Shadow, Style};
use crate::text::{Align, AttributedString, ParagraphOptions, TextLayout, TextStyle};

/// The control sizes (one scale for every control, tokens' `controlHeights`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ControlSize {
    /// 20 pt.
    Mini,
    /// 24 pt.
    Small,
    /// 28 pt.
    #[default]
    Regular,
    /// 32 pt.
    Large,
}

impl ControlSize {
    /// The control's height.
    pub fn height(self) -> f32 {
        match self {
            ControlSize::Mini => control_heights::MINI,
            ControlSize::Small => control_heights::SMALL,
            ControlSize::Regular => control_heights::REGULAR,
            ControlSize::Large => control_heights::LARGE,
        }
    }

    /// The icon box inside the control.
    pub fn icon(self) -> f32 {
        match self {
            ControlSize::Mini => control_icons::MINI,
            ControlSize::Small => control_icons::SMALL,
            ControlSize::Regular => control_icons::REGULAR,
            ControlSize::Large => control_icons::LARGE,
        }
    }

    /// The ABI number (0 mini … 3 large).
    pub fn to_int(self) -> i64 {
        match self {
            ControlSize::Mini => 0,
            ControlSize::Small => 1,
            ControlSize::Regular => 2,
            ControlSize::Large => 3,
        }
    }

    /// The size of an ABI number.
    pub fn from_int(n: i64) -> Option<ControlSize> {
        Some(match n {
            0 => ControlSize::Mini,
            1 => ControlSize::Small,
            2 => ControlSize::Regular,
            3 => ControlSize::Large,
            _ => return None,
        })
    }

    /// The label style of a push-like control at this size.
    pub fn label_token(self) -> &'static TypeStyle {
        match self {
            ControlSize::Mini => &typography::CAPTION,
            ControlSize::Small => &typography::LABEL,
            ControlSize::Regular => &typography::BODY_EMPHASIS,
            ControlSize::Large => &typography::TITLE_3,
        }
    }
}

/// `v` rounded up to a multiple of `step` (label boxes on the kit's grid). A hair over a
/// multiple (layout noise) stays on it.
pub fn whole(v: f32, step: f32) -> f32 {
    if step <= 0.0 || !v.is_finite() {
        return v.max(0.0);
    }
    ((v - 0.01).max(0.0) / step).ceil() * step
}

/// The focus halo: a ring of `focus.halo` (3 pt) in `Line/Focus` outside the control's
/// shape, the kit's `Focus Halo` layer.
pub fn focus_halo(rec: &mut Recorder, rect: Rect, shape: Shape, theme: &Theme) {
    focus_halo_named(rec, "Focus Halo", rect, shape, theme);
}

/// [`focus_halo`] under another layer name (`Checkbox box/Focus Halo`).
pub fn focus_halo_named(rec: &mut Recorder, name: &str, rect: Rect, shape: Shape, theme: &Theme) {
    rec.layer(
        name,
        rect,
        shape,
        Style::default().with_border(Border::at(
            theme.colors.line_focus,
            focus::HALO,
            BorderPosition::Outside,
        )),
    );
}

/// The shadow of a raised control (`Shadow/1`: 0, 1, blur 2).
pub fn control_shadow(theme: &Theme) -> Shadow {
    theme
        .shadow(1)
        .unwrap_or(Shadow::new(Color::rgba(0, 0, 0, 26), 0.0, 1.0, 2.0))
}

/// A label laid out once and reused while its text, style and width do not change.
#[derive(Debug, Default)]
pub struct Label {
    key: Option<(String, TextStyle, Align, u32, Option<usize>)>,
    layout: Option<Rc<TextLayout>>,
}

impl Label {
    /// The layout of `text` in `style`, aligned in `width` (∞: one line at its own width).
    pub fn layout(
        &mut self,
        text: &str,
        style: &TextStyle,
        align: Align,
        width: f32,
        max_lines: Option<usize>,
    ) -> Rc<TextLayout> {
        let key = (
            text.to_string(),
            style.clone(),
            align,
            width.to_bits(),
            max_lines,
        );
        if self.key.as_ref() == Some(&key)
            && let Some(l) = &self.layout
        {
            return l.clone();
        }
        let l = Rc::new(TextLayout::new(
            &AttributedString::plain(text),
            style,
            &ParagraphOptions {
                align,
                max_lines,
                ..ParagraphOptions::default()
            },
            width,
        ));
        self.key = Some(key);
        self.layout = Some(l.clone());
        l
    }

    /// The width of `text` in `style` on one line.
    pub fn width(&mut self, text: &str, style: &TextStyle) -> f32 {
        self.layout(text, style, Align::Start, f32::INFINITY, Some(1))
            .max_intrinsic_width()
    }
}

/// The hover and press transitions of a control (silica_ui's `AmalfiButtonBehavior`):
/// in with `fade.quick`, out in 60 ms.
#[derive(Debug, Clone, Copy)]
pub struct Interaction {
    /// Hover, 0–1.
    pub hover: Animated<f32>,
    /// Press, 0–1.
    pub press: Animated<f32>,
}

/// Fade-out of hover and press (silica_ui `amalfiButtonFadeOut`).
pub const FADE_OUT: f32 = 0.06;

impl Default for Interaction {
    fn default() -> Self {
        Interaction {
            hover: Animated::new(0.0),
            press: Animated::new(0.0),
        }
    }
}

impl Interaction {
    /// Follows the node's flags; returns whether anything animates.
    pub fn update(&mut self, flags: NodeFlags, disabled: bool, reduce_motion: bool) -> bool {
        let fade = |on: bool| {
            if reduce_motion {
                Timing::None
            } else if on {
                Timing::curve(tokens::motion::FADE_QUICK_CURVE, tokens::motion::FADE_QUICK)
            } else {
                Timing::curve(tokens::motion::FADE_QUICK_CURVE, FADE_OUT)
            }
        };
        let hovered = flags.contains(NodeFlags::HOVERED) && !disabled;
        let pressed = flags.contains(NodeFlags::PRESSED) && !disabled;
        let h = if hovered { 1.0 } else { 0.0 };
        let p = if pressed { 1.0 } else { 0.0 };
        if self.hover.target() != h {
            self.hover.animate_to(h, fade(hovered));
        }
        if self.press.target() != p {
            self.press.animate_to(p, fade(pressed));
        }
        self.hover.is_animating() || self.press.is_animating()
    }

    /// Advances; returns whether anything still animates.
    pub fn tick(&mut self, dt: f32) -> bool {
        let a = self.hover.tick(dt);
        let b = self.press.tick(dt);
        a || b
    }

    /// The visible hover (none while pressed).
    pub fn hover(&self) -> f32 {
        self.hover.get() * (1.0 - self.press.get())
    }

    /// The press.
    pub fn press(&self) -> f32 {
        self.press.get()
    }
}

/// A colour between two by `t`, or the first when the second is missing.
pub fn mix(a: Option<Color>, b: Option<Color>, t: f32) -> Option<Color> {
    match (a, b) {
        _ if t <= 0.0 => a,
        (_, _) if t >= 1.0 => b.or(a),
        (Some(x), Some(y)) => Some(x.lerp(y, t)),
        (None, Some(y)) => Some(y.with_opacity(t)),
        (Some(x), None) => Some(x.with_opacity(1.0 - t)),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_points() {
        assert_eq!(whole(44.6, 8.0), 48.0);
        assert_eq!(whole(48.0, 8.0), 48.0);
        assert_eq!(whole(48.004, 8.0), 48.0);
        assert_eq!(whole(0.0, 8.0), 0.0);
        assert_eq!(ControlSize::from_int(3), Some(ControlSize::Large));
        assert_eq!(ControlSize::Mini.height(), 20.0);
    }

    #[test]
    fn interaction_fades() {
        let mut i = Interaction::default();
        assert!(i.update(NodeFlags::HOVERED, false, false));
        i.tick(0.075);
        assert!(i.hover() > 0.4 && i.hover() < 1.0);
        i.tick(0.2);
        assert_eq!(i.hover(), 1.0);
        i.update(NodeFlags::HOVERED | NodeFlags::PRESSED, false, false);
        i.tick(1.0);
        assert_eq!(i.hover(), 0.0);
        assert_eq!(i.press(), 1.0);
        assert!(!i.update(NodeFlags::empty(), false, true));
        assert_eq!(i.press(), 0.0);
    }
}
