//! The buttons besides the push button (kit `Buttons`): icon buttons (round and square), the
//! help button, the link, the add/remove pair, toolbar buttons, and the pop-up and pull-down
//! buttons.

use crate::amalfi::button::{ButtonLook, paint_surface};
use crate::amalfi::foundation::{
    ControlSize, Interaction as Transitions, Label, focus_halo_named, whole,
};
use crate::amalfi::theme::Theme;
use crate::amalfi::tokens::{TypeStyle, typography};
use crate::color::Color;
use crate::core::a11y::{A11y, Action, Role};
use crate::core::event::{Button, Event, Handled, Key};
use crate::core::layout::Proposal;
use crate::core::tree::{NodeFlags, ViewId};
use crate::core::ui::Ui;
use crate::core::value::{Prop, Signal, Value};
use crate::core::widget::{EventCx, FocusKind, Interaction, LayoutCx, PaintCx, PropChange, Widget};
use crate::geom::{Corners, Point, Rect, Size};
use crate::shape::Shape;
use crate::style::Style;
use crate::text::Align;

/// The kit's name of a glyph layer: `Glyph Plus`, `Glyph Chevron Up Down`.
pub fn glyph_layer(symbol: &str) -> String {
    let mut s = String::from("Glyph");
    for word in symbol.split('-') {
        s.push(' ');
        let mut chars = word.chars();
        if let Some(c) = chars.next() {
            s.extend(c.to_uppercase());
            s.push_str(chars.as_str());
        }
    }
    s
}

/// The state every button here reacts to (hover, press, focus), with the transitions.
#[derive(Debug, Default)]
struct Core {
    transitions: Transitions,
}

impl Core {
    fn state_changed(&mut self, cx: &mut EventCx<'_>, new: NodeFlags) {
        let reduce = cx.theme().appearance.reduce_motion;
        if self
            .transitions
            .update(new, new.contains(NodeFlags::DISABLED), reduce)
        {
            cx.animate();
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn label_text(
    cx: &mut PaintCx<'_>,
    cache: &mut Label,
    name: &str,
    rect: Rect,
    text: &str,
    token: &'static TypeStyle,
    color: Color,
    align: Align,
) {
    let style = cx.theme().text(token, color);
    let layout = cache.layout(text, &style, align, rect.width, Some(1));
    cx.rec.text(name, rect, layout, Some(token.name), color);
}

/// The common button behaviour: Activate emits `Activated`.
fn activate(cx: &mut EventCx<'_>, event: &Event) -> Handled {
    if *event == Event::Activate {
        cx.emit(Signal::Activated, Value::None);
        Handled::Yes
    } else {
        Handled::No
    }
}

macro_rules! button_plumbing {
    () => {
        fn state_changed(&mut self, cx: &mut EventCx<'_>, _old: NodeFlags, new: NodeFlags) {
            self.core.state_changed(cx, new);
        }

        fn tick(&mut self, _cx: &mut EventCx<'_>, dt: f32) -> bool {
            self.core.transitions.tick(dt)
        }

        fn interaction(&self) -> Interaction {
            Interaction::Button
        }

        fn focus(&self) -> FocusKind {
            FocusKind::Control
        }
    };
}

// ---------------------------------------------------------------------------------------
// Icon buttons.
// ---------------------------------------------------------------------------------------

/// An icon button, round or square.
#[derive(Debug)]
pub struct IconButton {
    /// The symbol.
    pub symbol: String,
    /// The accessible label.
    pub title: String,
    /// Round (a circle) or square (rounded corners).
    pub round: bool,
    /// Size.
    pub size: ControlSize,
    core: Core,
}

impl IconButton {
    /// A round regular icon button.
    pub fn new(symbol: &str) -> IconButton {
        IconButton {
            symbol: symbol.to_string(),
            title: String::new(),
            round: true,
            size: ControlSize::Regular,
            core: Core::default(),
        }
    }

    /// Square.
    pub fn square(mut self) -> IconButton {
        self.round = false;
        self
    }

    /// With a size.
    pub fn size(mut self, size: ControlSize) -> IconButton {
        self.size = size;
        self
    }

    /// With an accessible label.
    pub fn titled(mut self, title: &str) -> IconButton {
        self.title = title.to_string();
        self
    }

    fn radius(&self) -> f32 {
        if self.round {
            self.size.height() / 2.0
        } else {
            match self.size {
                ControlSize::Mini => 6.0,
                ControlSize::Small => 7.0,
                ControlSize::Regular => 9.0,
                ControlSize::Large => 10.0,
            }
        }
    }
}

impl Widget for IconButton {
    fn kind(&self) -> &'static str {
        "IconButton"
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        let s = self.size.height();
        Size::new(s, s)
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let r = cx.bounds();
        let look = ButtonLook::control(&theme);
        paint_surface(
            cx,
            r,
            Shape::rounded(self.radius()),
            &look,
            &self.core.transitions,
        );
        let g = self.size.icon();
        cx.rec.glyph(
            &glyph_layer(&self.symbol),
            Rect::new(r.x + (r.width - g) / 2.0, r.y + (r.height - g) / 2.0, g, g),
            &self.symbol,
            400,
            look.content,
        );
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        activate(cx, event)
    }

    button_plumbing!();

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match prop {
            Prop::Symbol => value.as_str().map_or(PropChange::Unknown, |s| {
                self.symbol = s.to_string();
                PropChange::Paint
            }),
            Prop::Title => value.as_str().map_or(PropChange::Unknown, |s| {
                self.title = s.to_string();
                PropChange::Nothing
            }),
            Prop::Style => match value.as_int() {
                Some(0) => {
                    self.round = true;
                    PropChange::Paint
                }
                Some(1) => {
                    self.round = false;
                    PropChange::Paint
                }
                _ => PropChange::Unknown,
            },
            Prop::ControlSize => {
                value
                    .as_int()
                    .and_then(ControlSize::from_int)
                    .map_or(PropChange::Unknown, |s| {
                        self.size = s;
                        PropChange::Layout
                    })
            }
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::Symbol => Some(Value::Str(self.symbol.clone())),
            Prop::Title => Some(Value::Str(self.title.clone())),
            Prop::Style => Some(Value::Int(i64::from(!self.round))),
            Prop::ControlSize => Some(Value::Int(self.size.to_int())),
            _ => None,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Button;
        a.label = Some(self.title.clone()).filter(|t| !t.is_empty());
        a.actions = vec![Action::Click, Action::Focus];
    }
}

/// The help button: a 24 pt circle with a question mark.
#[derive(Debug, Default)]
pub struct HelpButton {
    core: Core,
    label: Label,
}

impl Widget for HelpButton {
    fn kind(&self) -> &'static str {
        "HelpButton"
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        Size::new(24.0, 24.0)
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let r = cx.bounds();
        let look = ButtonLook::control(&theme);
        paint_surface(cx, r, Shape::rounded(11.0), &look, &self.core.transitions);
        let style = theme.text(&typography::LABEL, look.content);
        let w = self.label.width("?", &style).ceil();
        let lh = style.line_height.unwrap_or(14.0);
        let mut cache = std::mem::take(&mut self.label);
        label_text(
            cx,
            &mut cache,
            "Help",
            Rect::new(r.x + 8.0, r.y + 5.0, w, lh),
            "?",
            &typography::LABEL,
            look.content,
            Align::Start,
        );
        self.label = cache;
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        activate(cx, event)
    }

    button_plumbing!();

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Button;
        a.label = Some("Help".to_string());
        a.actions = vec![Action::Click, Action::Focus];
    }
}

// ---------------------------------------------------------------------------------------
// The link.
// ---------------------------------------------------------------------------------------

/// A link: accent Body text, 24 tall, no background until pressed.
#[derive(Debug)]
pub struct LinkButton {
    /// The label.
    pub title: String,
    core: Core,
    label: Label,
}

impl LinkButton {
    /// A link.
    pub fn new(title: &str) -> LinkButton {
        LinkButton {
            title: title.to_string(),
            core: Core::default(),
            label: Label::default(),
        }
    }

    fn look(theme: &Theme) -> ButtonLook {
        let c = &theme.colors;
        ButtonLook {
            fill: None,
            fill_hovered: None,
            fill_pressed: Some(c.accent_amalfi_soft),
            border: None,
            shadow: false,
            hover_overlay: None,
            press_overlay: None,
            glass: None,
            glass_active: None,
            content: c.accent_amalfi,
        }
    }

    fn box_width(&mut self, theme: &Theme) -> f32 {
        let style = theme.text(&typography::BODY, theme.colors.accent_amalfi);
        whole(self.label.width(&self.title, &style), 8.0)
    }
}

impl Widget for LinkButton {
    fn kind(&self) -> &'static str {
        "LinkButton"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        let theme = cx.theme();
        let natural = 8.0 + self.box_width(&theme);
        let w = match p.width {
            Some(w) if w.is_finite() => w.clamp(8.0, natural.max(8.0)),
            _ => natural,
        };
        Size::new(w, 24.0)
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let r = cx.bounds();
        let look = LinkButton::look(&theme);
        paint_surface(cx, r, Shape::rounded(7.0), &look, &self.core.transitions);
        let w = self.box_width(&theme).min((r.width - 8.0).max(0.0));
        let style = theme.text(&typography::BODY, look.content);
        let lh = style.line_height.unwrap_or(18.0);
        let title = self.title.clone();
        let mut cache = std::mem::take(&mut self.label);
        label_text(
            cx,
            &mut cache,
            "Label",
            Rect::new(r.x + 4.0, r.y + (r.height - lh) / 2.0, w, lh),
            &title,
            &typography::BODY,
            look.content,
            Align::Center,
        );
        self.label = cache;
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        match event {
            Event::PointerEnter => {
                cx.set_cursor(crate::core::ui::Cursor::Pointer);
                Handled::No
            }
            e => activate(cx, e),
        }
    }

    button_plumbing!();

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match prop {
            Prop::Title => value.as_str().map_or(PropChange::Unknown, |s| {
                self.title = s.to_string();
                PropChange::Layout
            }),
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::Title => Some(Value::Str(self.title.clone())),
            _ => None,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Link;
        a.label = Some(self.title.clone());
        a.actions = vec![Action::Click, Action::Focus];
    }
}

// ---------------------------------------------------------------------------------------
// The add/remove pair.
// ---------------------------------------------------------------------------------------

/// The add/remove pair under a list: one 56 x 24 control split by a divider into a "+" and a
/// "-" button. Each half is a button of its own (`Signal::Activated`, payload: 0 add, 1
/// remove); a disabled half (`add_enabled`, `remove_enabled`) does nothing.
#[derive(Debug)]
pub struct GradientButtons {
    /// The add half works.
    pub add_enabled: bool,
    /// The remove half works.
    pub remove_enabled: bool,
    hovered: Option<usize>,
    pressed: Option<usize>,
    inside: bool,
}

impl GradientButtons {
    /// A pair with both halves enabled.
    pub fn new() -> GradientButtons {
        GradientButtons {
            add_enabled: true,
            remove_enabled: true,
            hovered: None,
            pressed: None,
            inside: false,
        }
    }

    fn half_at(p: Point) -> Option<usize> {
        if p.y < 0.0 || p.y >= 24.0 || p.x < 0.0 || p.x >= 56.0 {
            None
        } else if p.x < 28.0 {
            Some(0)
        } else {
            Some(1)
        }
    }

    fn can(&self, half: usize, disabled: bool) -> bool {
        !disabled
            && if half == 0 {
                self.add_enabled
            } else {
                self.remove_enabled
            }
    }
}

impl Default for GradientButtons {
    fn default() -> Self {
        GradientButtons::new()
    }
}

impl Widget for GradientButtons {
    fn kind(&self) -> &'static str {
        "GradientButtons"
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        Size::new(56.0, 24.0)
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let c = theme.colors;
        let b = cx.bounds();
        let rect = Rect::new(b.x, b.y, 56.0, 24.0);
        let look = ButtonLook::control(&theme);
        let shape = Shape::rounded(7.0);
        let idle = Transitions::default();
        // The whole control at rest; the halves lay their own state over it.
        paint_surface(cx, rect, shape.clone(), &look, &idle);
        let disabled = cx.is_disabled();
        for half in 0..2usize {
            if !self.can(half, disabled) {
                continue;
            }
            let (x, corners) = if half == 0 {
                (0.0, Corners::new(7.0, 0.0, 0.0, 7.0))
            } else {
                (28.0, Corners::new(0.0, 7.0, 7.0, 0.0))
            };
            let hr = Rect::new(b.x + x, b.y, 28.0, 24.0);
            let half_shape = Shape::Rect {
                corners,
                smoothing: crate::shape::AMALFI_SMOOTHING,
            };
            if self.pressed == Some(half) && self.inside {
                cx.rec.push_clip(rect, shape.clone());
                cx.rec.layer(
                    "State",
                    Rect::new(
                        hr.x + if half == 0 { 1.0 } else { 0.0 },
                        hr.y + 1.0,
                        27.0,
                        22.0,
                    ),
                    Shape::RECT,
                    Style::fill(c.surface_control_pressed),
                );
                cx.rec.pop();
            } else if self.hovered == Some(half) && self.pressed.is_none() {
                cx.rec
                    .layer("State", hr, half_shape, Style::fill(c.accent_amalfi_soft));
            }
        }
        cx.rec.layer(
            "Divider",
            Rect::new(b.x + 28.0, b.y + 6.0, 1.0, 12.0),
            Shape::RECT,
            Style::fill(c.line_separator),
        );
        let dim = !disabled && !(self.add_enabled && self.remove_enabled);
        for (half, symbol, name) in [(0usize, "plus", "Glyph Plus"), (1, "minus", "Glyph Minus")] {
            let off = !self.can(half, disabled) && !disabled && dim;
            if off {
                cx.rec.push_opacity(theme.disabled_opacity());
            }
            cx.rec.glyph(
                name,
                Rect::new(b.x + 6.0 + half as f32 * 28.0, b.y + 4.0, 16.0, 16.0),
                symbol,
                400,
                look.content,
            );
            if off {
                cx.rec.pop();
            }
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        let origin = cx.bounds().origin();
        let id = cx.id();
        let disabled = cx.ui().is_disabled(id);
        let local = |p: &Point| Point::new(p.x - origin.x, p.y - origin.y);
        match event {
            Event::PointerMove {
                pos,
                dragging: false,
                ..
            } => {
                let h = GradientButtons::half_at(local(pos)).filter(|h| self.can(*h, disabled));
                if h != self.hovered {
                    self.hovered = h;
                    cx.request_paint();
                }
                Handled::No
            }
            Event::PointerLeave => {
                if self.hovered.take().is_some() {
                    cx.request_paint();
                }
                Handled::No
            }
            Event::PointerDown {
                pos,
                button: Button::Left,
                ..
            } => {
                let Some(h) =
                    GradientButtons::half_at(local(pos)).filter(|h| self.can(*h, disabled))
                else {
                    return Handled::No;
                };
                self.pressed = Some(h);
                self.inside = true;
                cx.request_paint();
                Handled::Yes
            }
            Event::PointerMove {
                pos,
                dragging: true,
                ..
            } if self.pressed.is_some() => {
                let inside = GradientButtons::half_at(local(pos)) == self.pressed;
                if inside != self.inside {
                    self.inside = inside;
                    cx.request_paint();
                }
                Handled::Yes
            }
            Event::PointerUp {
                pos,
                button: Button::Left,
                ..
            } if self.pressed.is_some() => {
                let p = self.pressed.take();
                self.inside = false;
                if let Some(h) = p
                    && GradientButtons::half_at(local(pos)) == Some(h)
                    && self.can(h, disabled)
                {
                    cx.emit(Signal::Activated, Value::Int(h as i64));
                }
                cx.request_paint();
                Handled::Yes
            }
            Event::Key(k) if k.pressed && !disabled => match k.key {
                Key::Char('+') | Key::Char('=') if self.add_enabled => {
                    cx.emit(Signal::Activated, Value::Int(0));
                    Handled::Yes
                }
                Key::Char('-') if self.remove_enabled => {
                    cx.emit(Signal::Activated, Value::Int(1));
                    Handled::Yes
                }
                _ => Handled::No,
            },
            _ => Handled::No,
        }
    }

    fn interaction(&self) -> Interaction {
        Interaction::Hover
    }

    fn focus(&self) -> FocusKind {
        FocusKind::Control
    }

    fn dims_when_disabled(&self) -> bool {
        true
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Group;
        a.actions = vec![Action::Focus];
    }
}

// ---------------------------------------------------------------------------------------
// Toolbar buttons.
// ---------------------------------------------------------------------------------------

/// A toolbar button: `Glass/Regular`, a 36 pt circle with an icon, a 56 x 44 button with an
/// icon over a label, or a 24 pt small one.
#[derive(Debug)]
pub struct ToolbarButton {
    /// The symbol.
    pub symbol: String,
    /// The label under the icon.
    pub label: Option<String>,
    /// The small (24 pt) icon button.
    pub small: bool,
    /// Whether the window is active (`None`: ask the window).
    pub active: Option<bool>,
    core: Core,
    text: Label,
}

impl ToolbarButton {
    /// An icon button.
    pub fn new(symbol: &str) -> ToolbarButton {
        ToolbarButton {
            symbol: symbol.to_string(),
            label: None,
            small: false,
            active: None,
            core: Core::default(),
            text: Label::default(),
        }
    }

    /// With a label.
    pub fn labelled(mut self, label: &str) -> ToolbarButton {
        self.label = Some(label.to_string());
        self
    }

    /// Small.
    pub fn small(mut self) -> ToolbarButton {
        self.small = true;
        self
    }

    /// With the window's active state fixed.
    pub fn active(mut self, active: bool) -> ToolbarButton {
        self.active = Some(active);
        self
    }

    fn is_small(&self) -> bool {
        self.small && self.label.is_none()
    }

    fn side(&self) -> f32 {
        if self.is_small() { 24.0 } else { 36.0 }
    }

    fn glyph(&self) -> f32 {
        if self.is_small() { 16.0 } else { 20.0 }
    }

    fn width(&mut self, theme: &Theme) -> f32 {
        match self.label.clone() {
            None => self.side(),
            Some(l) => {
                let style = theme.text(&typography::CAPTION, theme.colors.text_primary);
                whole(self.text.width(&l, &style), 4.0).max(56.0)
            }
        }
    }
}

impl Widget for ToolbarButton {
    fn kind(&self) -> &'static str {
        "ToolbarButton"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        let theme = cx.theme();
        let w = self.width(&theme);
        Size::new(
            w,
            if self.label.is_some() {
                44.0
            } else {
                self.side()
            },
        )
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let c = theme.colors;
        let r = cx.bounds();
        let active = self.active.unwrap_or_else(|| cx.window_active());
        let ink = if active {
            c.text_primary
        } else {
            c.text_tertiary
        };
        let radius = if self.label.is_some() {
            16.0
        } else {
            r.height / 2.0
        };
        let shape = Shape::rounded(radius);
        cx.rec.layer(
            "Background",
            r,
            shape.clone(),
            theme.material_style("Glass/Regular"),
        );
        let t = &self.core.transitions;
        let (hover, press) = (t.hover(), t.press());
        if hover > 0.0 {
            cx.rec.layer(
                "Highlight",
                r,
                shape.clone(),
                Style::fill(c.line_separator).with_opacity(hover),
            );
        }
        if press > 0.0 {
            cx.rec.layer(
                "Highlight",
                r,
                shape.clone(),
                Style::fill(c.line_border).with_opacity(press),
            );
        }
        if cx.focus_visible() && !cx.is_disabled() {
            focus_halo_named(cx.rec, "Focus Halo", r, shape, &theme);
        }
        let g = self.glyph();
        let name = glyph_layer(&self.symbol);
        match self.label.clone() {
            None => {
                cx.rec.glyph(
                    &name,
                    Rect::new(r.x + (r.width - g) / 2.0, r.y + (r.height - g) / 2.0, g, g),
                    &self.symbol,
                    400,
                    ink,
                );
            }
            Some(label) => {
                cx.rec.glyph(
                    &name,
                    Rect::new(r.x + (r.width - g) / 2.0, r.y + 4.0, g, g),
                    &self.symbol,
                    400,
                    ink,
                );
                let mut cache = std::mem::take(&mut self.text);
                label_text(
                    cx,
                    &mut cache,
                    "Label",
                    Rect::new(r.x, r.y + 26.0, r.width, 14.0),
                    &label,
                    &typography::CAPTION,
                    ink,
                    Align::Center,
                );
                self.text = cache;
            }
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        activate(cx, event)
    }

    button_plumbing!();

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match prop {
            Prop::Symbol => value.as_str().map_or(PropChange::Unknown, |s| {
                self.symbol = s.to_string();
                PropChange::Paint
            }),
            Prop::Title => match value {
                Value::Str(s) => {
                    self.label = Some(s.clone()).filter(|s| !s.is_empty());
                    PropChange::Layout
                }
                Value::None => {
                    self.label = None;
                    PropChange::Layout
                }
                _ => PropChange::Unknown,
            },
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::Symbol => Some(Value::Str(self.symbol.clone())),
            Prop::Title => Some(self.label.clone().map(Value::Str).unwrap_or_default()),
            _ => None,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Button;
        a.label.clone_from(&self.label);
        a.actions = vec![Action::Click, Action::Focus];
    }
}

// ---------------------------------------------------------------------------------------
// Pop-up and pull-down buttons.
// ---------------------------------------------------------------------------------------

/// A pop-up or pull-down button: a label on the left and a chevron on the right. Pressed,
/// it opens its list (`Signal::ExpandedChanged`); the chosen row is `Signal::SelectionChanged`
/// (payload: the index) once the menu reports it with [`MenuButton::choose`].
#[derive(Debug)]
pub struct MenuButton {
    /// A pop-up (the label is the chosen item, the list opens over it) or a pull-down.
    pub pop_up: bool,
    /// The pull-down's title.
    pub title: String,
    /// The items.
    pub items: Vec<String>,
    /// The chosen item of a pop-up.
    pub selected: Option<usize>,
    /// Size.
    pub size: ControlSize,
    /// Width (`None`: the size's own).
    pub width: Option<f32>,
    /// The list is open (the button looks pressed).
    pub open: bool,
    popup: Option<crate::core::window::WindowId>,
    core: Core,
    text: Label,
}

impl MenuButton {
    /// A pop-up button.
    pub fn pop_up(items: &[&str], selected: usize) -> MenuButton {
        MenuButton {
            pop_up: true,
            title: String::new(),
            items: items.iter().map(|i| (*i).to_string()).collect(),
            selected: Some(selected),
            size: ControlSize::Regular,
            width: None,
            open: false,
            popup: None,
            core: Core::default(),
            text: Label::default(),
        }
    }

    /// A pull-down button.
    pub fn pull_down(title: &str, items: &[&str]) -> MenuButton {
        MenuButton {
            pop_up: false,
            title: title.to_string(),
            items: items.iter().map(|i| (*i).to_string()).collect(),
            selected: None,
            size: ControlSize::Regular,
            width: None,
            open: false,
            popup: None,
            core: Core::default(),
            text: Label::default(),
        }
    }

    /// With a size.
    pub fn size(mut self, size: ControlSize) -> MenuButton {
        self.size = size;
        self
    }

    fn default_width(size: ControlSize) -> f32 {
        match size {
            ControlSize::Mini => 92.0,
            ControlSize::Small => 108.0,
            ControlSize::Regular => 124.0,
            ControlSize::Large => 148.0,
        }
    }

    fn leading(size: ControlSize) -> f32 {
        match size {
            ControlSize::Mini | ControlSize::Small => 8.0,
            _ => 12.0,
        }
    }

    fn trailing(size: ControlSize) -> f32 {
        match size {
            ControlSize::Mini | ControlSize::Small => 4.0,
            _ => 8.0,
        }
    }

    fn token(size: ControlSize) -> &'static TypeStyle {
        match size {
            ControlSize::Mini => &typography::CAPTION,
            ControlSize::Small => &typography::LABEL,
            _ => &typography::BODY,
        }
    }

    /// The text on the button.
    pub fn shown(&self) -> String {
        if self.pop_up {
            self.selected
                .and_then(|i| self.items.get(i))
                .cloned()
                .unwrap_or_default()
        } else {
            self.title.clone()
        }
    }

    /// The menu of `id` chose item `i`: a pop-up takes it as its label; the menu closes.
    pub fn choose_in(ui: &mut Ui, id: ViewId, i: usize) {
        let mut closing = None;
        let ok = ui
            .with_widget::<MenuButton, _>(id, |b| {
                if i >= b.items.len() {
                    return false;
                }
                if b.pop_up {
                    b.selected = Some(i);
                }
                b.open = false;
                closing = b.popup.take();
                true
            })
            .unwrap_or(false);
        if let Some(w) = closing {
            ui.close_window(w);
        }
        if ok {
            ui.emit(id, Signal::SelectionChanged, Value::Int(i as i64));
            ui.emit(id, Signal::ExpandedChanged, Value::Bool(false));
        }
    }

    /// Closes the menu of `id` without a choice.
    pub fn dismiss(ui: &mut Ui, id: ViewId) {
        let mut closing = None;
        ui.with_widget::<MenuButton, _>(id, |b| {
            b.open = false;
            closing = b.popup.take();
        });
        if let Some(w) = closing {
            ui.close_window(w);
            ui.emit(id, Signal::ExpandedChanged, Value::Bool(false));
        }
    }

    fn open_menu(&mut self, cx: &mut EventCx<'_>) {
        let id = cx.id();
        let Some(parent) = cx.window() else {
            return;
        };
        let anchor = cx.window_rect();
        let rows: Vec<crate::amalfi::menus::Row> = self
            .items
            .iter()
            .enumerate()
            .map(|(i, l)| crate::amalfi::menus::Row {
                checked: self.pop_up && self.selected == Some(i),
                ..crate::amalfi::menus::Row::item(l)
            })
            .collect();
        let ui = cx.ui();
        let (m, _) = crate::amalfi::menus::menu(ui, &rows);
        if let Some(i) = self.selected.filter(|_| self.pop_up) {
            let _ = ui.set(m, Prop::SelectedIndex, Value::Int(i as i64));
        }
        ui.connect(m, Signal::Activated, move |ui, _, v| {
            if let Some(i) = v.as_int().and_then(|i| usize::try_from(i).ok()) {
                MenuButton::choose_in(ui, id, i);
            }
        });
        ui.connect(m, Signal::CloseRequested, move |ui, _, _| {
            MenuButton::dismiss(ui, id);
        });
        let placement = if self.pop_up {
            crate::amalfi::menus::Placement::PopUp {
                row: self.selected.unwrap_or(0),
            }
        } else {
            crate::amalfi::menus::Placement::PullDown
        };
        let w = crate::amalfi::menus::present(ui, parent, anchor, m, placement);
        self.popup = Some(w);
        self.open = true;
        ui.set_focus(m, true);
    }
}

impl Widget for MenuButton {
    fn kind(&self) -> &'static str {
        if self.pop_up {
            "PopUpButton"
        } else {
            "PullDownButton"
        }
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        Size::new(
            self.width
                .unwrap_or_else(|| MenuButton::default_width(self.size)),
            self.size.height(),
        )
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let c = theme.colors;
        let r = cx.bounds();
        let look = ButtonLook::control(&theme);
        let mut t = Transitions::default();
        std::mem::swap(&mut t, &mut self.core.transitions);
        if self.open {
            // Held open: the pressed look.
            let mut pressed = Transitions::default();
            pressed.update(NodeFlags::PRESSED, false, true);
            paint_surface(cx, r, Shape::rounded(r.height / 2.0), &look, &pressed);
        } else {
            paint_surface(cx, r, Shape::rounded(r.height / 2.0), &look, &t);
        }
        std::mem::swap(&mut t, &mut self.core.transitions);
        let lead = MenuButton::leading(self.size);
        let trail = MenuButton::trailing(self.size);
        let chevron = 16.0;
        let token = MenuButton::token(self.size);
        let style = theme.text(token, look.content);
        let lh = style.line_height.unwrap_or(18.0);
        let w = (r.width - lead - trail - 4.0 - chevron).max(0.0);
        let shown = self.shown();
        let mut cache = std::mem::take(&mut self.text);
        label_text(
            cx,
            &mut cache,
            "Label",
            Rect::new(r.x + lead, r.y + (r.height - lh) / 2.0, w, lh),
            &shown,
            token,
            look.content,
            Align::Start,
        );
        self.text = cache;
        cx.rec.glyph(
            "Chevron",
            Rect::new(
                r.right() - trail - chevron,
                r.y + (r.height - chevron) / 2.0,
                chevron,
                chevron,
            ),
            if self.pop_up {
                "chevron-up-down"
            } else {
                "chevron-down"
            },
            400,
            c.text_secondary,
        );
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        match event {
            Event::Activate => {
                if self.items.is_empty() {
                    return Handled::Yes;
                }
                if let Some(w) = self.popup.take() {
                    self.open = false;
                    cx.ui().close_window(w);
                } else {
                    self.open_menu(cx);
                }
                cx.emit(Signal::ExpandedChanged, Value::Bool(self.open));
                cx.request_paint();
                Handled::Yes
            }
            Event::Key(k) if k.pressed && matches!(k.key, Key::Down | Key::Up) => {
                if !self.open && !self.items.is_empty() {
                    self.open_menu(cx);
                    cx.emit(Signal::ExpandedChanged, Value::Bool(true));
                    cx.request_paint();
                }
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    button_plumbing!();

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match prop {
            Prop::Title => value.as_str().map_or(PropChange::Unknown, |s| {
                self.title = s.to_string();
                PropChange::Paint
            }),
            Prop::Items => value.as_str_list().map_or(PropChange::Unknown, |l| {
                self.items = l.to_vec();
                PropChange::Paint
            }),
            Prop::SelectedIndex => value.as_int().map_or(PropChange::Unknown, |i| {
                self.selected = usize::try_from(i).ok().filter(|i| *i < self.items.len());
                PropChange::Paint
            }),
            Prop::PullsDown => value.as_bool().map_or(PropChange::Unknown, |b| {
                self.pop_up = !b;
                PropChange::Paint
            }),
            Prop::Expanded => value.as_bool().map_or(PropChange::Unknown, |b| {
                self.open = b;
                PropChange::Paint
            }),
            Prop::ControlSize => {
                value
                    .as_int()
                    .and_then(ControlSize::from_int)
                    .map_or(PropChange::Unknown, |s| {
                        self.size = s;
                        PropChange::Layout
                    })
            }
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::Title => Some(Value::Str(self.title.clone())),
            Prop::Items => Some(Value::StrList(self.items.clone())),
            Prop::SelectedIndex => Some(Value::Int(self.selected.map_or(-1, |i| i as i64))),
            Prop::PullsDown => Some(Value::Bool(!self.pop_up)),
            Prop::Expanded => Some(Value::Bool(self.open)),
            Prop::ControlSize => Some(Value::Int(self.size.to_int())),
            _ => None,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::PopUpButton;
        a.label = Some(self.shown()).filter(|s| !s.is_empty());
        a.expanded = Some(self.open);
        a.actions = vec![Action::Click, Action::Focus, Action::ShowMenu];
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::event::Mods;
    use crate::core::ui::Ui;
    use crate::core::window::{WindowId, WindowKind};
    use std::cell::RefCell;
    use std::rc::Rc;

    type Log = Rc<RefCell<Vec<String>>>;

    fn scene(w: impl Widget + 'static) -> (Ui, WindowId, crate::core::ViewId, Log) {
        let mut ui = Ui::new_headless();
        let win = ui.create_window(WindowKind::Normal, Size::new(300.0, 200.0));
        let id = ui.create(w);
        let log: Log = Rc::new(RefCell::new(Vec::new()));
        for (sig, tag) in [
            (Signal::Activated, "activated"),
            (Signal::ExpandedChanged, "expanded"),
            (Signal::SelectionChanged, "selected"),
        ] {
            let l = log.clone();
            ui.connect(id, sig, move |_, _, v| {
                let s = match v {
                    Value::Int(i) => i.to_string(),
                    Value::Bool(b) => b.to_string(),
                    _ => String::new(),
                };
                l.borrow_mut().push(format!("{tag}:{s}"));
            });
        }
        ui.set_content(win, id);
        ui.show_window(win);
        ui.prepare_frame(win);
        (ui, win, id, log)
    }

    fn click(ui: &mut Ui, win: WindowId, p: Point) {
        ui.pointer_move(win, p, Mods::empty());
        ui.pointer_down(win, p, Button::Left, Mods::empty());
        ui.pointer_up(win, p, Button::Left, Mods::empty());
    }

    #[test]
    fn glyph_layers_are_named_like_the_kit() {
        assert_eq!(glyph_layer("plus"), "Glyph Plus");
        assert_eq!(glyph_layer("chevron-up-down"), "Glyph Chevron Up Down");
    }

    #[test]
    fn icon_buttons_activate() {
        let (mut ui, win, id, log) = scene(IconButton::new("plus").titled("Add"));
        let c = ui.tree.window_rect(id).center();
        click(&mut ui, win, c);
        assert_eq!(*log.borrow(), vec!["activated:"]);
        let a = ui.accessibility_of(id).unwrap_or_default();
        assert_eq!(a.label.as_deref(), Some("Add"));
    }

    #[test]
    fn gradient_halves_act_on_their_own() {
        let (mut ui, win, id, log) = scene(GradientButtons::new());
        let r = ui.tree.window_rect(id);
        click(&mut ui, win, Point::new(r.x + 10.0, r.y + 12.0));
        click(&mut ui, win, Point::new(r.x + 40.0, r.y + 12.0));
        let _ = ui.set(id, Prop::Disabled, Value::Bool(false));
        ui.with_widget::<GradientButtons, _>(id, |g| g.remove_enabled = false);
        click(&mut ui, win, Point::new(r.x + 40.0, r.y + 12.0));
        assert_eq!(*log.borrow(), vec!["activated:0", "activated:1"]);
    }

    #[test]
    fn menu_buttons_open_and_choose() {
        let (mut ui, win, id, log) = scene(MenuButton::pop_up(&["Lemon", "Orange"], 0));
        let c = ui.tree.window_rect(id).center();
        click(&mut ui, win, c);
        assert_eq!(*log.borrow(), vec!["expanded:true"]);
        assert_eq!(
            ui.widget::<MenuButton>(id).map(MenuButton::shown),
            Some("Lemon".to_string())
        );
        let _ = ui.set(id, Prop::SelectedIndex, Value::Int(1));
        assert_eq!(
            ui.widget::<MenuButton>(id).map(MenuButton::shown),
            Some("Orange".to_string())
        );
    }

    #[test]
    fn a_pop_up_opens_its_menu_and_a_row_chooses() {
        let (mut ui, win, id, log) = scene(MenuButton::pop_up(&["Lemon", "Orange", "Fig"], 0));
        let c = ui.tree.window_rect(id).center();
        click(&mut ui, win, c);
        let popup = ui
            .windows()
            .find(|w| matches!(w.kind, WindowKind::Popup { .. }))
            .map(|w| (w.id, w.root));
        let Some((popup, root)) = popup else {
            panic!("no menu");
        };
        ui.prepare_frame(popup);
        let rows: Vec<_> = ui.tree.children(ui.tree.children(root)[0]).to_vec();
        let r = ui.tree.window_rect(rows[2]);
        ui.pointer_move(popup, r.center(), Mods::empty());
        ui.pointer_down(popup, r.center(), Button::Left, Mods::empty());
        ui.pointer_up(popup, r.center(), Button::Left, Mods::empty());
        assert_eq!(
            ui.widget::<MenuButton>(id).map(MenuButton::shown),
            Some("Fig".to_string())
        );
        assert!(
            ui.windows()
                .all(|w| !matches!(w.kind, WindowKind::Popup { .. }))
        );
        assert!(log.borrow().contains(&"selected:2".to_string()));
    }

    #[test]
    fn toolbar_buttons_size_by_kind() {
        let mut ui = Ui::new_headless();
        let a = ui.create(ToolbarButton::new("gear"));
        let b = ui.create(ToolbarButton::new("gear").labelled("Label"));
        let c = ui.create(ToolbarButton::new("gear").small());
        assert_eq!(ui.measure(a, Proposal::UNSPECIFIED), Size::new(36.0, 36.0));
        assert_eq!(ui.measure(b, Proposal::UNSPECIFIED), Size::new(56.0, 44.0));
        assert_eq!(ui.measure(c, Proposal::UNSPECIFIED), Size::new(24.0, 24.0));
    }
}
