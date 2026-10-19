//! Push buttons (kit `Buttons/Push`): primary, secondary, destructive, borderless and glass,
//! at the four control sizes, with an optional leading symbol.

use crate::amalfi::foundation::{
    ControlSize, Interaction as Transitions, Label, control_shadow, focus_halo, mix, whole,
};
use crate::amalfi::theme::Theme;
use crate::color::Color;
use crate::core::a11y::{A11y, Action, Role};
use crate::core::event::{Event, Handled};
use crate::core::layout::Proposal;
use crate::core::tree::NodeFlags;
use crate::core::value::{Prop, Signal, Value};
use crate::core::widget::{EventCx, FocusKind, Interaction, LayoutCx, PaintCx, PropChange, Widget};
use crate::geom::{Rect, Size};
use crate::shape::Shape;
use crate::style::{Border, Style};
use crate::text::{Align, TextStyle};

/// The look of a push button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PushVariant {
    /// Accent fill (the default action).
    Primary,
    /// Control fill with a border and a shadow.
    #[default]
    Secondary,
    /// Terracotta fill.
    Destructive,
    /// No fill until hovered, accent text.
    Borderless,
    /// Glass (Regular at rest, Thick when hovered or pressed).
    Glass,
}

impl PushVariant {
    /// The ABI number.
    pub fn to_int(self) -> i64 {
        match self {
            PushVariant::Primary => 0,
            PushVariant::Secondary => 1,
            PushVariant::Destructive => 2,
            PushVariant::Borderless => 3,
            PushVariant::Glass => 4,
        }
    }

    /// The variant of an ABI number.
    pub fn from_int(n: i64) -> Option<PushVariant> {
        Some(match n {
            0 => PushVariant::Primary,
            1 => PushVariant::Secondary,
            2 => PushVariant::Destructive,
            3 => PushVariant::Borderless,
            4 => PushVariant::Glass,
            _ => return None,
        })
    }
}

/// The colours of a button in one variant (silica_ui's `AmalfiButtonLook`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ButtonLook {
    /// Fill at rest.
    pub fill: Option<Color>,
    /// Fill when hovered.
    pub fill_hovered: Option<Color>,
    /// Fill when pressed.
    pub fill_pressed: Option<Color>,
    /// Inside border.
    pub border: Option<Color>,
    /// The control shadow.
    pub shadow: bool,
    /// Wash over the fill when hovered.
    pub hover_overlay: Option<Color>,
    /// Wash over the fill when pressed.
    pub press_overlay: Option<Color>,
    /// Glass material at rest.
    pub glass: Option<&'static str>,
    /// Glass material when hovered or pressed.
    pub glass_active: Option<&'static str>,
    /// Text and symbol colour.
    pub content: Color,
}

impl ButtonLook {
    /// The look of the secondary (control) style.
    pub fn control(theme: &Theme) -> ButtonLook {
        let c = &theme.colors;
        ButtonLook {
            fill: Some(c.surface_control),
            fill_hovered: None,
            fill_pressed: Some(c.surface_control_pressed),
            border: Some(c.line_border),
            shadow: true,
            hover_overlay: Some(c.accent_amalfi_soft),
            press_overlay: None,
            glass: None,
            glass_active: None,
            content: c.text_primary,
        }
    }

    /// The look of a push button variant.
    pub fn push(variant: PushVariant, theme: &Theme) -> ButtonLook {
        let c = &theme.colors;
        let none = ButtonLook {
            fill: None,
            fill_hovered: None,
            fill_pressed: None,
            border: None,
            shadow: false,
            hover_overlay: None,
            press_overlay: None,
            glass: None,
            glass_active: None,
            content: c.text_primary,
        };
        match variant {
            PushVariant::Primary => ButtonLook {
                fill: Some(c.accent_amalfi),
                fill_pressed: Some(c.accent_amalfi_pressed),
                hover_overlay: Some(c.state_hover_overlay),
                content: c.text_on_accent,
                ..none
            },
            PushVariant::Secondary => ButtonLook::control(theme),
            PushVariant::Destructive => ButtonLook {
                fill: Some(c.palette_terracotta),
                hover_overlay: Some(c.state_hover_overlay),
                press_overlay: Some(c.state_press_overlay),
                content: c.text_on_accent,
                ..none
            },
            PushVariant::Borderless => ButtonLook {
                fill_hovered: Some(c.accent_amalfi_soft),
                fill_pressed: Some(c.accent_amalfi_soft),
                content: c.accent_amalfi,
                ..none
            },
            PushVariant::Glass => ButtonLook {
                glass: Some("Glass/Regular"),
                glass_active: Some("Glass/Thick"),
                press_overlay: Some(c.accent_amalfi_soft),
                content: c.text_primary,
                ..none
            },
        }
    }
}

/// Records a button's surface: `Background` (fill, border, shadow, or glass), the hover and
/// press washes (`State`) and the focus halo (silica_ui's `AmalfiButtonSurface`).
pub fn paint_surface(
    cx: &mut PaintCx<'_>,
    rect: Rect,
    shape: Shape,
    look: &ButtonLook,
    t: &Transitions,
) {
    let theme = *cx.theme();
    let hover = t.hover();
    let press = t.press();
    if let Some(glass) = look.glass {
        let active = look.glass_active.filter(|_| hover >= 0.5 || press >= 0.5);
        let style = theme.material_style(active.unwrap_or(glass));
        cx.rec.layer("Background", rect, shape.clone(), style);
    } else {
        let hovered = mix(look.fill, look.fill_hovered.or(look.fill), hover);
        let fill = mix(hovered, look.fill_pressed.or(look.fill), press);
        let mut style = Style::default();
        if let Some(f) = fill {
            style.fills.push(f.into());
        }
        if let Some(b) = look.border {
            style.borders.push(Border::inside(b, 1.0));
        }
        if look.shadow {
            style.shadows.push(control_shadow(&theme));
        }
        cx.rec.layer("Background", rect, shape.clone(), style);
    }
    if let Some(o) = look.hover_overlay.filter(|_| hover > 0.0) {
        cx.rec.layer(
            "State",
            rect,
            shape.clone(),
            Style::fill(o).with_opacity(hover),
        );
    }
    if let Some(o) = look.press_overlay.filter(|_| press > 0.0) {
        cx.rec.layer(
            "State",
            rect,
            shape.clone(),
            Style::fill(o).with_opacity(press),
        );
    }
    if cx.focus_visible() && !cx.is_disabled() {
        focus_halo(cx.rec, rect, shape, &theme);
    }
}

/// Push button metrics (silica_ui's `AmalfiPushButtonMetrics`).
pub mod metrics {
    use super::ControlSize;

    /// Horizontal padding.
    pub fn padding(size: ControlSize) -> f32 {
        match size {
            ControlSize::Mini | ControlSize::Small => 8.0,
            ControlSize::Regular => 12.0,
            ControlSize::Large => 16.0,
        }
    }

    /// The label box's width is a multiple of this.
    pub const LABEL_STEP: f32 = 8.0;

    /// Gap between the symbol and the label.
    pub fn gap(size: ControlSize) -> f32 {
        if size == ControlSize::Large { 8.0 } else { 4.0 }
    }
}

/// A push button.
#[derive(Debug, Default)]
pub struct PushButton {
    /// Title.
    pub title: String,
    /// Leading symbol (Silica Symbols name).
    pub symbol: Option<String>,
    /// Variant.
    pub variant: PushVariant,
    /// Size.
    pub size: ControlSize,
    /// Return activates it.
    pub is_default: bool,
    /// Escape activates it.
    pub is_cancel: bool,
    label: Label,
    transitions: Transitions,
}

impl PushButton {
    /// A secondary regular button.
    pub fn new(title: &str) -> Self {
        PushButton {
            title: title.to_string(),
            ..PushButton::default()
        }
    }

    /// With a variant.
    pub fn variant(mut self, v: PushVariant) -> Self {
        self.variant = v;
        self
    }

    /// With a size.
    pub fn size(mut self, s: ControlSize) -> Self {
        self.size = s;
        self
    }

    /// With a leading symbol.
    pub fn symbol(mut self, name: &str) -> Self {
        self.symbol = Some(name.to_string());
        self
    }

    fn text_style(&self, theme: &Theme, color: Color) -> TextStyle {
        theme.text(self.size.label_token(), color)
    }

    fn natural_label(&mut self, theme: &Theme) -> f32 {
        let style = self.text_style(theme, theme.colors.text_primary);
        let w = self.label.width(&self.title, &style);
        whole(w, metrics::LABEL_STEP)
    }

    fn icon_part(&self) -> f32 {
        match &self.symbol {
            Some(_) => self.size.icon() + metrics::gap(self.size),
            None => 0.0,
        }
    }
}

impl Widget for PushButton {
    fn kind(&self) -> &'static str {
        "PushButton"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        let theme = cx.theme();
        let pad = metrics::padding(self.size);
        let label = if self.title.is_empty() {
            0.0
        } else {
            self.natural_label(&theme)
        };
        let natural = pad * 2.0 + self.icon_part() + label;
        let min = pad * 2.0 + self.icon_part();
        let w = match p.width {
            Some(w) if w.is_finite() => w.clamp(min, natural.max(min)),
            _ => natural,
        };
        Size::new(w, self.size.height())
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let r = cx.bounds();
        let shape = Shape::rounded(r.height / 2.0);
        let look = ButtonLook::push(self.variant, &theme);
        paint_surface(cx, r, shape, &look, &self.transitions);
        let pad = metrics::padding(self.size);
        let style = self.text_style(&theme, look.content);
        let natural = if self.title.is_empty() {
            0.0
        } else {
            whole(self.label.width(&self.title, &style), metrics::LABEL_STEP)
        };
        let avail = (r.width - pad * 2.0 - self.icon_part()).max(0.0);
        let label_w = natural.min(avail);
        let content = self.icon_part() + label_w;
        let mut x = r.x + (r.width - content) / 2.0;
        if let Some(sym) = &self.symbol {
            let s = self.size.icon();
            cx.rec.glyph(
                "Icon",
                Rect::new(x, r.y + (r.height - s) / 2.0, s, s),
                sym,
                400,
                look.content,
            );
            x += s + metrics::gap(self.size);
        }
        if !self.title.is_empty() {
            let lh = style.line_height.unwrap_or(style.size * 1.3);
            let layout = self
                .label
                .layout(&self.title, &style, Align::Center, label_w, Some(1));
            let token = self.size.label_token().name;
            cx.rec.text(
                "Label",
                Rect::new(x, r.y + (r.height - lh) / 2.0, label_w, lh),
                layout,
                Some(token),
                look.content,
            );
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        match event {
            Event::Activate => {
                cx.emit(Signal::Activated, Value::None);
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn state_changed(&mut self, cx: &mut EventCx<'_>, _old: NodeFlags, new: NodeFlags) {
        let reduce = cx.theme().appearance.reduce_motion;
        if self
            .transitions
            .update(new, new.contains(NodeFlags::DISABLED), reduce)
        {
            cx.animate();
        }
    }

    fn tick(&mut self, _cx: &mut EventCx<'_>, dt: f32) -> bool {
        self.transitions.tick(dt)
    }

    fn interaction(&self) -> Interaction {
        Interaction::Button
    }

    fn focus(&self) -> FocusKind {
        FocusKind::Control
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match prop {
            Prop::Title => match value.as_str() {
                Some(t) => {
                    self.title = t.to_string();
                    PropChange::Layout
                }
                None => PropChange::Unknown,
            },
            Prop::Symbol => {
                self.symbol = value.as_str().filter(|s| !s.is_empty()).map(str::to_string);
                PropChange::Layout
            }
            Prop::Style => match value.as_int().and_then(PushVariant::from_int) {
                Some(v) => {
                    self.variant = v;
                    PropChange::Paint
                }
                None => PropChange::Unknown,
            },
            Prop::ControlSize => match value.as_int().and_then(ControlSize::from_int) {
                Some(s) => {
                    self.size = s;
                    PropChange::Layout
                }
                None => PropChange::Unknown,
            },
            Prop::IsDefault => match value.as_bool() {
                Some(b) => {
                    self.is_default = b;
                    PropChange::Nothing
                }
                None => PropChange::Unknown,
            },
            Prop::IsCancel => match value.as_bool() {
                Some(b) => {
                    self.is_cancel = b;
                    PropChange::Nothing
                }
                None => PropChange::Unknown,
            },
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::Title => Some(Value::Str(self.title.clone())),
            Prop::Symbol => Some(self.symbol.clone().map(Value::Str).unwrap_or_default()),
            Prop::Style => Some(Value::Int(self.variant.to_int())),
            Prop::ControlSize => Some(Value::Int(self.size.to_int())),
            Prop::IsDefault => Some(Value::Bool(self.is_default)),
            Prop::IsCancel => Some(Value::Bool(self.is_cancel)),
            _ => None,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Button;
        a.label = Some(self.title.clone());
        a.actions = vec![Action::Click, Action::Focus];
    }

    fn baseline(&self) -> Option<f32> {
        let lh = self.size.label_token().line_height.unwrap_or(16.0);
        Some((self.size.height() - lh) / 2.0 + lh * 0.75)
    }
}
