//! A static text label (kit text layers): one type token in one colour role, optionally on
//! the kit's whole-point grid, with an ellipsis when it does not fit.

use crate::amalfi::foundation::{Label, whole};
use crate::amalfi::theme::Theme;
use crate::amalfi::tokens::{self, TypeStyle};
use crate::color::Color;
use crate::core::a11y::{A11y, Role};
use crate::core::layout::Proposal;
use crate::core::value::{Prop, Value};
use crate::core::widget::{LayoutCx, PaintCx, PropChange, Widget};
use crate::geom::{Rect, Size};
use crate::text::Align;

/// Which text colour a label uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextRole {
    /// `Text/Primary`.
    #[default]
    Primary,
    /// `Text/Secondary`.
    Secondary,
    /// `Text/Tertiary`.
    Tertiary,
    /// `Text/On Accent`.
    OnAccent,
    /// `Accent/Amalfi`.
    Accent,
}

impl TextRole {
    /// The colour in `theme`.
    pub fn color(self, theme: &Theme) -> Color {
        let c = &theme.colors;
        match self {
            TextRole::Primary => c.text_primary,
            TextRole::Secondary => c.text_secondary,
            TextRole::Tertiary => c.text_tertiary,
            TextRole::OnAccent => c.text_on_accent,
            TextRole::Accent => c.accent_amalfi,
        }
    }
}

/// A static label.
#[derive(Debug)]
pub struct Text {
    /// The text.
    pub text: String,
    /// The type token.
    pub token: &'static TypeStyle,
    /// The colour role.
    pub role: TextRole,
    /// Horizontal alignment inside the box.
    pub align: Align,
    /// The kit layer's name.
    pub layer: String,
    /// The box's width is a multiple of this (0: the text's own width).
    pub step: f32,
    /// Space after the text inside the box (the kit's label trailing).
    pub trailing: f32,
    /// Wraps onto several lines in the width it is given (a paragraph) instead of one line
    /// with an ellipsis.
    pub wraps: bool,
    label: Label,
}

impl Text {
    /// A body label named `Label`.
    pub fn new(text: &str) -> Text {
        Text {
            text: text.to_string(),
            token: &tokens::typography::BODY,
            role: TextRole::Primary,
            align: Align::Start,
            layer: "Label".to_string(),
            step: 0.0,
            trailing: 0.0,
            wraps: false,
            label: Label::default(),
        }
    }

    /// With a type token.
    pub fn token(mut self, token: &'static TypeStyle) -> Text {
        self.token = token;
        self
    }

    /// With a colour role.
    pub fn role(mut self, role: TextRole) -> Text {
        self.role = role;
        self
    }

    /// With the kit layer's name.
    pub fn layer(mut self, name: &str) -> Text {
        self.layer = name.to_string();
        self
    }

    /// On the whole-point grid.
    pub fn whole(mut self, step: f32, trailing: f32) -> Text {
        self.step = step;
        self.trailing = trailing;
        self
    }

    /// A paragraph: wraps in the width it is given.
    pub fn wrapping(mut self) -> Text {
        self.wraps = true;
        self
    }

    /// With an alignment.
    pub fn align(mut self, align: Align) -> Text {
        self.align = align;
        self
    }

    fn line_height(&self) -> f32 {
        self.token.line_height.unwrap_or(self.token.size * 1.3)
    }

    fn natural(&mut self, theme: &Theme) -> f32 {
        let style = theme.text(self.token, self.role.color(theme));
        let w = self.label.width(&self.text, &style) + self.trailing;
        if self.step > 0.0 {
            whole(w, self.step)
        } else {
            w
        }
    }
}

impl Widget for Text {
    fn kind(&self) -> &'static str {
        "Text"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        let theme = cx.theme();
        if self.wraps
            && let Some(avail) = p.width.filter(|w| w.is_finite())
        {
            let style = theme.text(self.token, self.role.color(&theme));
            let layout = self
                .label
                .layout(&self.text, &style, self.align, avail.max(0.0), None);
            return Size::new(avail.max(0.0), layout.size().height);
        }
        let natural = self.natural(&theme);
        let w = match p.width {
            Some(w) if w.is_finite() => natural.min(w.max(0.0)),
            _ => natural,
        };
        let lh = theme.text(self.token, Color::BLACK).line_height;
        Size::new(w, lh.unwrap_or_else(|| self.line_height()))
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let r = cx.bounds();
        let style = theme.text(self.token, self.role.color(&theme));
        let width = (r.width - self.trailing).max(0.0);
        let layout = self.label.layout(
            &self.text,
            &style,
            self.align,
            width,
            if self.wraps { None } else { Some(1) },
        );
        cx.rec.text(
            &self.layer,
            Rect::new(r.x, r.y, r.width, r.height),
            layout,
            Some(self.token.name),
            style.color,
        );
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match prop {
            Prop::Title | Prop::Text => match value.as_str() {
                Some(t) => {
                    self.text = t.to_string();
                    PropChange::Layout
                }
                None => PropChange::Unknown,
            },
            Prop::TextStyle => match value.as_str().and_then(tokens::type_style_ref) {
                Some(t) => {
                    self.token = t;
                    PropChange::Layout
                }
                None => PropChange::Unknown,
            },
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::Title | Prop::Text => Some(Value::Str(self.text.clone())),
            Prop::TextStyle => Some(Value::Str(self.token.name.to_string())),
            _ => None,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Label;
        a.label = Some(self.text.clone());
    }

    fn hit(&self, _p: crate::geom::Point, _size: Size) -> bool {
        false
    }

    fn baseline(&self) -> Option<f32> {
        Some(self.line_height() * 0.75)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Ui;

    #[test]
    fn whole_point_width() {
        let mut ui = Ui::new_headless();
        let t = ui.create(Text::new("Browsing").whole(8.0, 0.0));
        let s = ui.measure(t, Proposal::UNSPECIFIED);
        assert_eq!(s.width % 8.0, 0.0);
        assert_eq!(s.height, 18.0);
    }
}
