//! Static text (a label) and symbol images: the type styles of the tokens, the colour roles
//! of the palette, wrapping, line limits with ellipsis, alignment.

use crate::amalfi::foundation::Label as LabelCache;
use crate::amalfi::theme::Theme;
use crate::amalfi::tokens::{self, TypeStyle};
use crate::color::Color;
use crate::core::a11y::{A11y, Role};
use crate::core::layout::Proposal;
use crate::core::value::{Prop, Value};
use crate::core::widget::{LayoutCx, PaintCx, PropChange, Widget};
use crate::geom::{Point, Rect, Size};
use crate::text::{Align, TextStyle};

/// A colour given by role (follows the appearance) or fixed.
#[derive(Debug, Clone, PartialEq)]
pub enum ColorRole {
    /// A token name (`Text/Primary`, `Accent/Amalfi`, `App/Clock/Accent` …).
    Token(String),
    /// A fixed colour.
    Fixed(Color),
}

impl ColorRole {
    /// The colour in `theme`.
    pub fn resolve(&self, theme: &Theme) -> Color {
        match self {
            ColorRole::Token(t) => theme.color(t).unwrap_or(theme.colors.text_primary),
            ColorRole::Fixed(c) => *c,
        }
    }

    fn from_value(v: &Value) -> Option<ColorRole> {
        match v {
            Value::Str(s) => Some(ColorRole::Token(s.clone())),
            other => other.as_color().map(ColorRole::Fixed),
        }
    }

    fn to_value(&self) -> Value {
        match self {
            ColorRole::Token(t) => Value::Str(t.clone()),
            ColorRole::Fixed(c) => Value::Color(*c),
        }
    }
}

/// A label.
#[derive(Debug)]
pub struct Label {
    /// The text.
    pub text: String,
    /// The type style.
    pub style: &'static TypeStyle,
    /// The colour.
    pub color: ColorRole,
    /// Alignment of lines.
    pub align: Align,
    /// Maximum lines (0: unlimited).
    pub max_lines: usize,
    /// Wrap (false: one line, truncated with an ellipsis when narrower).
    pub wraps: bool,
    /// Weight override.
    pub weight: Option<u16>,
    cache: LabelCache,
}

impl Label {
    /// A label in Body, `Text/Primary`.
    pub fn new(text: &str) -> Self {
        Label {
            text: text.to_string(),
            style: &tokens::typography::BODY,
            color: ColorRole::Token("Text/Primary".into()),
            align: Align::Start,
            max_lines: 0,
            wraps: true,
            weight: None,
            cache: LabelCache::default(),
        }
    }

    /// With a type style.
    pub fn style(mut self, s: &'static TypeStyle) -> Self {
        self.style = s;
        self
    }

    /// With a colour role.
    pub fn color(mut self, token: &str) -> Self {
        self.color = ColorRole::Token(token.to_string());
        self
    }

    fn text_style(&self, theme: &Theme) -> TextStyle {
        let mut s = theme.text(self.style, self.color.resolve(theme));
        if let Some(w) = self.weight {
            s.weight = w;
        }
        s
    }

    fn lines(&self) -> Option<usize> {
        if !self.wraps {
            Some(1)
        } else if self.max_lines == 0 {
            None
        } else {
            Some(self.max_lines)
        }
    }
}

impl Widget for Label {
    fn kind(&self) -> &'static str {
        "Label"
    }

    fn hit(&self, _p: Point, _size: Size) -> bool {
        false
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        let theme = cx.theme();
        let style = self.text_style(&theme);
        let natural = self.cache.width(&self.text, &style);
        let width = match p.width {
            Some(w) if w.is_finite() => w.min(natural.ceil()),
            _ => natural.ceil(),
        };
        let lines = self.lines();
        let layout = self
            .cache
            .layout(&self.text, &style, self.align, width.max(1.0), lines);
        let h = layout.size().height;
        let w = if self.wraps {
            layout.size().width.ceil().min(width)
        } else {
            width
        };
        Size::new(w, h)
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let style = self.text_style(&theme);
        let r = cx.bounds();
        let layout = self.cache.layout(
            &self.text,
            &style,
            self.align,
            r.width.max(1.0),
            self.lines(),
        );
        let color = style.color;
        cx.rec.text(
            "Label",
            Rect::new(r.x, r.y, r.width, layout.size().height.max(r.height)),
            layout,
            Some(self.style.name),
            color,
        );
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match prop {
            Prop::Text | Prop::Title => match value.as_str() {
                Some(t) => {
                    self.text = t.to_string();
                    PropChange::Layout
                }
                None => PropChange::Unknown,
            },
            Prop::TextStyle => match value.as_str().and_then(tokens::type_style_ref) {
                Some(s) => {
                    self.style = s;
                    PropChange::Layout
                }
                None => PropChange::Unknown,
            },
            Prop::TextColor => match ColorRole::from_value(value) {
                Some(c) => {
                    self.color = c;
                    PropChange::Paint
                }
                None => PropChange::Unknown,
            },
            Prop::TextAlignment => {
                self.align = match value.as_int() {
                    Some(1) => Align::Center,
                    Some(2) => Align::End,
                    Some(3) => Align::Justify,
                    Some(0) => Align::Start,
                    _ => return PropChange::Unknown,
                };
                PropChange::Paint
            }
            Prop::MaxLines => match value.as_int() {
                Some(n) if n >= 0 => {
                    self.max_lines = usize::try_from(n).unwrap_or(0);
                    PropChange::Layout
                }
                _ => PropChange::Unknown,
            },
            Prop::Wraps => match value.as_bool() {
                Some(b) => {
                    self.wraps = b;
                    PropChange::Layout
                }
                None => PropChange::Unknown,
            },
            Prop::FontWeight => match value.as_int() {
                Some(w) => {
                    self.weight = u16::try_from(w).ok();
                    PropChange::Layout
                }
                None => PropChange::Unknown,
            },
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::Text | Prop::Title => Some(Value::Str(self.text.clone())),
            Prop::TextStyle => Some(Value::Str(self.style.name.to_string())),
            Prop::TextColor => Some(self.color.to_value()),
            Prop::MaxLines => Some(Value::from(self.max_lines)),
            Prop::Wraps => Some(Value::Bool(self.wraps)),
            _ => None,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Label;
        a.label = Some(self.text.clone());
    }

    fn baseline(&self) -> Option<f32> {
        let lh = self.style.line_height.unwrap_or(self.style.size * 1.25);
        Some((lh - self.style.size) / 2.0 + self.style.size * 0.8)
    }
}

/// A Silica Symbols glyph at an icon size, in a colour role.
#[derive(Debug, Clone)]
pub struct SymbolImage {
    /// Symbol name.
    pub name: String,
    /// Icon box (= font size).
    pub size: f32,
    /// Weight: 300, 400 or 600.
    pub weight: u16,
    /// Colour.
    pub color: ColorRole,
}

impl SymbolImage {
    /// A 20 pt Regular symbol in `Text/Primary`.
    pub fn new(name: &str) -> Self {
        SymbolImage {
            name: name.to_string(),
            size: 20.0,
            weight: 400,
            color: ColorRole::Token("Text/Primary".into()),
        }
    }
}

impl Widget for SymbolImage {
    fn kind(&self) -> &'static str {
        "Symbol"
    }

    fn hit(&self, _p: Point, _size: Size) -> bool {
        false
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        Size::new(self.size, self.size)
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let r = cx.bounds();
        let s = self.size.min(r.width).min(r.height);
        let rect = Rect::new(r.x + (r.width - s) / 2.0, r.y + (r.height - s) / 2.0, s, s);
        cx.rec.glyph(
            "Icon",
            rect,
            &self.name,
            self.weight,
            self.color.resolve(&theme),
        );
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match prop {
            Prop::Symbol => match value.as_str() {
                Some(n) => {
                    self.name = n.to_string();
                    PropChange::Paint
                }
                None => PropChange::Unknown,
            },
            Prop::FontSize => match value.as_f32() {
                Some(s) if s > 0.0 => {
                    self.size = s;
                    PropChange::Layout
                }
                _ => PropChange::Unknown,
            },
            Prop::FontWeight => match value.as_int().and_then(|w| u16::try_from(w).ok()) {
                Some(w) => {
                    self.weight = w;
                    PropChange::Paint
                }
                None => PropChange::Unknown,
            },
            Prop::TextColor | Prop::Tint => match ColorRole::from_value(value) {
                Some(c) => {
                    self.color = c;
                    PropChange::Paint
                }
                None => PropChange::Unknown,
            },
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::Symbol => Some(Value::Str(self.name.clone())),
            Prop::FontSize => Some(Value::from(self.size)),
            _ => None,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Image;
        a.hidden = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Ui;

    #[test]
    fn label_measures_and_wraps() {
        let mut ui = Ui::new_headless();
        let l = ui.create(Label::new("Hello there, general"));
        let one = ui.measure(l, Proposal::UNSPECIFIED);
        assert_eq!(one.height, 18.0);
        let wrapped = ui.measure(l, Proposal::new(Some(60.0), None));
        assert!(wrapped.height >= 36.0 && wrapped.width <= 60.0);
        ui.set(l, Prop::Wraps, Value::Bool(false)).unwrap();
        assert_eq!(ui.measure(l, Proposal::new(Some(60.0), None)).height, 18.0);
        ui.set(l, Prop::TextStyle, Value::from("Title 1")).unwrap();
        assert_eq!(ui.measure(l, Proposal::UNSPECIFIED).height, 32.0);
        assert!(ui.set(l, Prop::TextStyle, Value::from("Nope")).is_err());
    }
}
