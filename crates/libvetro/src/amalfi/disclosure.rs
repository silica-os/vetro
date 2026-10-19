//! Disclosure (kit `Disclosure`): the triangle and the round button that show whether a
//! section is collapsed, and the group, a header that shows or hides the content under it.

use crate::amalfi::foundation::{Label, control_shadow, focus_halo_named};
use crate::amalfi::tokens::{self, typography};
use crate::core::a11y::{A11y, Action, Role};
use crate::core::event::{Button, Event, Handled, Key};
use crate::core::layout::Proposal;
use crate::core::ui::Ui;
use crate::core::value::{Prop, Signal, Value};
use crate::core::widget::{EventCx, FocusKind, Interaction, LayoutCx, PaintCx, PropChange, Widget};
use crate::geom::{Point, Rect, Size, Transform};
use crate::motion::{Animated, Timing};
use crate::shape::Shape;
use crate::style::{Border, Style};
use crate::text::Align;

/// Fixed measures (silica_ui's `AmalfiDisclosureMetrics`, `AmalfiDisclosureGroupMetrics`).
pub mod metrics {
    /// The control's side.
    pub const SIDE: f32 = 24.0;
    /// The chevron's size.
    pub const GLYPH: f32 = 12.0;
    /// The triangle's corner radius.
    pub const TRIANGLE_RADIUS: f32 = 7.0;
    /// The button's corner radius.
    pub const BUTTON_RADIUS: f32 = 11.0;
    /// A group header's height.
    pub const HEADER_HEIGHT: f32 = 28.0;
    /// A group header's left padding.
    pub const HEADER_LEFT: f32 = 4.0;
    /// A group header's right padding.
    pub const HEADER_RIGHT: f32 = 12.0;
    /// Gap between the triangle and the title.
    pub const TITLE_GAP: f32 = 4.0;
    /// A group header's corner radius.
    pub const HEADER_RADIUS: f32 = 9.0;
    /// The content's indent.
    pub const CONTENT_INDENT: f32 = 32.0;
    /// Space above and below the content.
    pub const CONTENT_GAP: f32 = 4.0;
    /// A group's default width.
    pub const GROUP_WIDTH: f32 = 280.0;
}

/// The look of a disclosure control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DisclosureStyle {
    /// A chevron that turns right to down (outline style lists, groups).
    #[default]
    Triangle,
    /// A round button with a chevron that turns down to up.
    Button,
}

/// How the pointer touches a control.
#[derive(Debug, Clone, Copy, Default)]
struct Touch {
    hovered: bool,
    pressed: bool,
    focused: bool,
}

/// Records a disclosure control at `rect` (names under `prefix`). `turn` is how far it has
/// turned (0 collapsed, 1 expanded), `expanded` names the glyph.
fn paint_control(
    cx: &mut PaintCx<'_>,
    prefix: &str,
    rect: Rect,
    style: DisclosureStyle,
    expanded: bool,
    turn: f32,
    touch: Touch,
) {
    let theme = *cx.theme();
    let c = theme.colors;
    let button = style == DisclosureStyle::Button;
    let radius = if button {
        metrics::BUTTON_RADIUS
    } else {
        metrics::TRIANGLE_RADIUS
    };
    let shape = Shape::rounded(radius);
    let pressed = touch.pressed;
    let hovered = touch.hovered && !pressed;
    let mut bg = Style::default();
    if button {
        bg.fills.push(
            if pressed {
                c.surface_control_pressed
            } else {
                c.surface_control
            }
            .into(),
        );
        bg.borders.push(Border::inside(c.line_border, 1.0));
        bg.shadows.push(control_shadow(&theme));
    } else if pressed {
        bg.fills.push(c.surface_control_pressed.into());
    } else if hovered {
        bg.fills.push(c.accent_amalfi_soft.into());
    }
    cx.rec
        .layer(&format!("{prefix}Background"), rect, shape.clone(), bg);
    if button && hovered {
        cx.rec.layer(
            &format!("{prefix}State"),
            rect,
            shape.clone(),
            Style::fill(c.accent_amalfi_soft),
        );
    }
    if touch.focused {
        focus_halo_named(cx.rec, &format!("{prefix}Focus Halo"), rect, shape, &theme);
    }
    let inset = (metrics::SIDE - metrics::GLYPH) / 2.0;
    let glyph = Rect::new(
        rect.x + inset,
        rect.y + inset,
        metrics::GLYPH,
        metrics::GLYPH,
    );
    let color = if button || pressed || hovered {
        c.text_primary
    } else {
        c.text_secondary
    };
    let (name, symbol, quarter) = match (button, expanded) {
        (false, false) => ("Glyph Chevron Right", "chevron-right", 0.25),
        (false, true) => ("Glyph Chevron Down", "chevron-right", 0.25),
        (true, false) => ("Glyph Chevron Down", "chevron-down", 0.5),
        (true, true) => ("Glyph Chevron Up", "chevron-down", 0.5),
    };
    let centre = glyph.center();
    cx.rec.push_transform(
        Transform::translate(-centre.x, -centre.y)
            .then(Transform::rotate(turn * quarter * std::f32::consts::TAU))
            .then(Transform::translate(centre.x, centre.y)),
    );
    cx.rec
        .glyph(&format!("{prefix}{name}"), glyph, symbol, 400, color);
    cx.rec.pop();
}

fn spring(cx: &mut EventCx<'_>, a: &mut Animated<f32>, target: f32, token: tokens::SpringToken) {
    if cx.theme().appearance.reduce_motion {
        a.jump(target);
    } else {
        a.animate_to(target, Timing::Spring(token));
    }
}

/// A disclosure triangle or button.
#[derive(Debug)]
pub struct Disclosure {
    /// Expanded.
    pub expanded: bool,
    /// The look.
    pub style: DisclosureStyle,
    turn: Animated<f32>,
}

impl Disclosure {
    /// A collapsed triangle.
    pub fn new() -> Disclosure {
        Disclosure {
            expanded: false,
            style: DisclosureStyle::Triangle,
            turn: Animated::new(0.0),
        }
    }

    /// With a state.
    pub fn expanded(mut self, expanded: bool) -> Disclosure {
        self.expanded = expanded;
        self.turn = Animated::new(if expanded { 1.0 } else { 0.0 });
        self
    }

    /// With a look.
    pub fn style(mut self, style: DisclosureStyle) -> Disclosure {
        self.style = style;
        self
    }
}

impl Default for Disclosure {
    fn default() -> Self {
        Disclosure::new()
    }
}

impl Widget for Disclosure {
    fn kind(&self) -> &'static str {
        "Disclosure"
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        Size::new(metrics::SIDE, metrics::SIDE)
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let r = cx.bounds();
        let enabled = !cx.is_disabled();
        let touch = Touch {
            hovered: cx.is_hovered() && enabled,
            pressed: cx.is_pressed() && enabled,
            focused: cx.focus_visible() && enabled,
        };
        paint_control(cx, "", r, self.style, self.expanded, self.turn.get(), touch);
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        if *event == Event::Activate {
            self.expanded = !self.expanded;
            let target = if self.expanded { 1.0 } else { 0.0 };
            let mut turn = self.turn;
            spring(cx, &mut turn, target, tokens::motion::SPRING_SNAPPY);
            self.turn = turn;
            cx.emit(Signal::ExpandedChanged, Value::Bool(self.expanded));
            cx.emit(Signal::ValueChanged, Value::Bool(self.expanded));
            cx.request_paint();
            cx.animate();
            return Handled::Yes;
        }
        Handled::No
    }

    fn tick(&mut self, _cx: &mut EventCx<'_>, dt: f32) -> bool {
        self.turn.tick(dt)
    }

    fn interaction(&self) -> Interaction {
        Interaction::Toggle
    }

    fn focus(&self) -> FocusKind {
        FocusKind::Control
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match prop {
            Prop::Expanded => value.as_bool().map_or(PropChange::Unknown, |b| {
                if b != self.expanded {
                    self.expanded = b;
                    self.turn.animate_to(
                        if b { 1.0 } else { 0.0 },
                        Timing::Spring(tokens::motion::SPRING_SNAPPY),
                    );
                }
                PropChange::Paint
            }),
            Prop::Style => match value.as_int() {
                Some(0) => {
                    self.style = DisclosureStyle::Triangle;
                    PropChange::Paint
                }
                Some(1) => {
                    self.style = DisclosureStyle::Button;
                    PropChange::Paint
                }
                _ => PropChange::Unknown,
            },
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::Expanded => Some(Value::Bool(self.expanded)),
            Prop::Style => Some(Value::Int(match self.style {
                DisclosureStyle::Triangle => 0,
                DisclosureStyle::Button => 1,
            })),
            _ => None,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Disclosure;
        a.expanded = Some(self.expanded);
        a.actions = vec![
            Action::Click,
            Action::Focus,
            if self.expanded {
                Action::Collapse
            } else {
                Action::Expand
            },
        ];
    }
}

/// A header with a triangle and a title that shows or hides the content under it. The content
/// is the group's child view; it is indented 32 and 4 below the header.
#[derive(Debug)]
pub struct DisclosureGroup {
    /// The title.
    pub title: String,
    /// Expanded.
    pub expanded: bool,
    turn: Animated<f32>,
    open: Animated<f32>,
    title_text: Label,
    hovered: bool,
    pressed: bool,
    content_height: f32,
}

impl DisclosureGroup {
    /// A collapsed group.
    pub fn new(title: &str) -> DisclosureGroup {
        DisclosureGroup {
            title: title.to_string(),
            expanded: false,
            turn: Animated::new(0.0),
            open: Animated::new(0.0),
            title_text: Label::default(),
            hovered: false,
            pressed: false,
            content_height: 0.0,
        }
    }

    /// With a state.
    pub fn expanded(mut self, expanded: bool) -> DisclosureGroup {
        self.expanded = expanded;
        let v = if expanded { 1.0 } else { 0.0 };
        self.turn = Animated::new(v);
        self.open = Animated::new(v);
        self
    }

    fn in_header(p: Point) -> bool {
        p.y >= 0.0 && p.y < metrics::HEADER_HEIGHT
    }

    fn set_expanded(&mut self, cx: &mut EventCx<'_>, expanded: bool) {
        if expanded == self.expanded {
            return;
        }
        self.expanded = expanded;
        let target = if expanded { 1.0 } else { 0.0 };
        let (mut turn, mut open) = (self.turn, self.open);
        spring(cx, &mut turn, target, tokens::motion::SPRING_SNAPPY);
        spring(cx, &mut open, target, tokens::motion::SPRING_PANEL);
        self.turn = turn;
        self.open = open;
        cx.emit(Signal::ExpandedChanged, Value::Bool(expanded));
        cx.request_layout();
        cx.request_paint();
        cx.animate();
    }

    fn open_factor(&self) -> f32 {
        self.open.get().clamp(0.0, 1.0)
    }
}

impl Widget for DisclosureGroup {
    fn kind(&self) -> &'static str {
        "DisclosureGroup"
    }

    fn clips_children(&self) -> bool {
        // The content slides in and out under the header.
        true
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        let w = match p.width {
            Some(w) if w.is_finite() => w,
            _ => metrics::GROUP_WIDTH,
        };
        let mut h = metrics::HEADER_HEIGHT;
        if let Some(child) = cx.children().first().copied() {
            let size = cx.measure(
                child,
                Proposal::new(Some((w - metrics::CONTENT_INDENT).max(0.0)), None),
            );
            self.content_height = size.height;
            let open = self.open.get().max(0.0);
            if open > 0.0 {
                h += (size.height + 2.0 * metrics::CONTENT_GAP) * self.open_factor();
            }
        }
        Size::new(w, h)
    }

    fn place(&mut self, cx: &mut LayoutCx<'_>, size: Size) {
        if let Some(child) = cx.children().first().copied() {
            let w = (size.width - metrics::CONTENT_INDENT).max(0.0);
            let s = cx.measure(child, Proposal::new(Some(w), None));
            cx.place(
                child,
                Rect::new(
                    metrics::CONTENT_INDENT,
                    metrics::HEADER_HEIGHT + metrics::CONTENT_GAP,
                    w,
                    s.height,
                ),
            );
        }
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let c = theme.colors;
        let r = cx.bounds();
        let enabled = !cx.is_disabled();
        let has_content = !cx.ui.tree.children(cx.id).is_empty();
        let prefix = if has_content { "Header/" } else { "" };
        let header = Rect::new(r.x, r.y, r.width, metrics::HEADER_HEIGHT);
        let shape = Shape::rounded(metrics::HEADER_RADIUS);
        if enabled && (self.hovered || self.pressed) {
            cx.rec.layer(
                &format!("{prefix}Background"),
                header,
                shape.clone(),
                Style::fill(c.accent_amalfi_soft),
            );
        }
        if enabled && cx.focus_visible() {
            focus_halo_named(
                cx.rec,
                &format!("{prefix}Focus Halo"),
                header,
                shape,
                &theme,
            );
        }
        paint_control(
            cx,
            &format!("{prefix}Triangle/"),
            Rect::new(
                r.x + metrics::HEADER_LEFT,
                r.y + (metrics::HEADER_HEIGHT - metrics::SIDE) / 2.0,
                metrics::SIDE,
                metrics::SIDE,
            ),
            DisclosureStyle::Triangle,
            self.expanded,
            self.turn.get(),
            Touch::default(),
        );
        let style = theme.text(&typography::HEADLINE, c.text_primary);
        let lh = style.line_height.unwrap_or(18.0);
        let x = metrics::HEADER_LEFT + metrics::SIDE + metrics::TITLE_GAP;
        let w = (r.width - x - metrics::HEADER_RIGHT).max(0.0);
        let layout = self
            .title_text
            .layout(&self.title, &style, Align::Start, w, Some(1));
        cx.rec.text(
            &format!("{prefix}Title"),
            Rect::new(r.x + x, r.y + (metrics::HEADER_HEIGHT - lh) / 2.0, w, lh),
            layout,
            Some(typography::HEADLINE.name),
            style.color,
        );
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        let origin = cx.bounds().origin();
        let id = cx.id();
        let enabled = !cx.ui().is_disabled(id);
        let local = |p: &Point| Point::new(p.x - origin.x, p.y - origin.y);
        match event {
            Event::PointerMove {
                pos,
                dragging: false,
                ..
            } => {
                let h = enabled && DisclosureGroup::in_header(local(pos));
                if h != self.hovered {
                    self.hovered = h;
                    cx.request_paint();
                }
                Handled::No
            }
            Event::PointerLeave => {
                if std::mem::take(&mut self.hovered) {
                    cx.request_paint();
                }
                Handled::No
            }
            Event::PointerDown {
                pos,
                button: Button::Left,
                ..
            } if enabled && DisclosureGroup::in_header(local(pos)) => {
                self.pressed = true;
                cx.request_paint();
                Handled::Yes
            }
            Event::PointerMove {
                pos,
                dragging: true,
                ..
            } if self.pressed => {
                let inside = DisclosureGroup::in_header(local(pos));
                if inside != self.pressed {
                    cx.request_paint();
                }
                Handled::Yes
            }
            Event::PointerUp {
                pos,
                button: Button::Left,
                ..
            } if self.pressed => {
                self.pressed = false;
                if DisclosureGroup::in_header(local(pos)) {
                    let e = !self.expanded;
                    self.set_expanded(cx, e);
                }
                cx.request_paint();
                Handled::Yes
            }
            Event::Key(k) if k.pressed && enabled => match k.key {
                Key::Space | Key::Enter => {
                    let e = !self.expanded;
                    self.set_expanded(cx, e);
                    Handled::Yes
                }
                Key::Right if !self.expanded => {
                    self.set_expanded(cx, true);
                    Handled::Yes
                }
                Key::Left if self.expanded => {
                    self.set_expanded(cx, false);
                    Handled::Yes
                }
                _ => Handled::No,
            },
            _ => Handled::No,
        }
    }

    fn tick(&mut self, cx: &mut EventCx<'_>, dt: f32) -> bool {
        let a = self.turn.tick(dt);
        let b = self.open.tick(dt);
        if b || self.open.is_animating() {
            cx.request_layout();
        }
        a || b
    }

    fn interaction(&self) -> Interaction {
        Interaction::Hover
    }

    fn focus(&self) -> FocusKind {
        FocusKind::Control
    }

    fn hit(&self, p: Point, size: Size) -> bool {
        Rect::from_size(size).contains(p)
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match prop {
            Prop::Title => value.as_str().map_or(PropChange::Unknown, |t| {
                self.title = t.to_string();
                PropChange::Paint
            }),
            Prop::Expanded => value.as_bool().map_or(PropChange::Unknown, |b| {
                if b != self.expanded {
                    self.expanded = b;
                    let v = if b { 1.0 } else { 0.0 };
                    self.turn.jump(v);
                    self.open.jump(v);
                }
                PropChange::Layout
            }),
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::Title => Some(Value::Str(self.title.clone())),
            Prop::Expanded => Some(Value::Bool(self.expanded)),
            _ => None,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Disclosure;
        a.label = Some(self.title.clone());
        a.expanded = Some(self.expanded);
        a.actions = vec![
            Action::Click,
            Action::Focus,
            if self.expanded {
                Action::Collapse
            } else {
                Action::Expand
            },
        ];
    }
}

/// Makes a disclosure group over `content` (clipped while it opens).
pub fn disclosure_group(
    ui: &mut Ui,
    title: &str,
    expanded: bool,
    content: crate::core::ViewId,
) -> crate::core::ViewId {
    let g = ui.create(DisclosureGroup::new(title).expanded(expanded));
    let _ = ui.set(g, Prop::ClipsChildren, Value::Bool(true));
    ui.add_child(g, content);
    g
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::amalfi::text::Text;
    use crate::core::event::{KeyEvent, Mods};
    use crate::core::window::{WindowId, WindowKind};
    use std::cell::RefCell;
    use std::rc::Rc;

    fn at(ui: &Ui, id: crate::core::ViewId, x: f32, y: f32) -> Point {
        let r = ui.tree.window_rect(id);
        Point::new(r.x + x, r.y + y)
    }

    fn group() -> (Ui, WindowId, crate::core::ViewId, Rc<RefCell<Vec<bool>>>) {
        let mut ui = Ui::new_headless();
        let win = ui.create_window(WindowKind::Normal, Size::new(400.0, 200.0));
        let content = ui.create(
            Text::new("The content of the group sits below its header.")
                .token(&typography::CALLOUT)
                .layer("Content")
                .wrapping(),
        );
        let g = disclosure_group(&mut ui, "Advanced options", false, content);
        let _ = ui.set(g, Prop::MaxWidth, Value::from(280.0));
        let log = Rc::new(RefCell::new(Vec::new()));
        let l = log.clone();
        ui.connect(g, Signal::ExpandedChanged, move |_, _, v| {
            l.borrow_mut().push(v.as_bool().unwrap_or(false));
        });
        ui.set_content(win, g);
        ui.show_window(win);
        ui.prepare_frame(win);
        (ui, win, g, log)
    }

    #[test]
    fn clicking_the_header_opens_and_closes_the_group() {
        let (mut ui, win, g, log) = group();
        assert_eq!(ui.tree.window_rect(g).height, 28.0);
        let p = at(&ui, g, 100.0, 14.0);
        ui.pointer_move(win, p, Mods::empty());
        ui.pointer_down(win, p, Button::Left, Mods::empty());
        ui.pointer_up(win, p, Button::Left, Mods::empty());
        for _ in 0..60 {
            ui.advance(0.03);
        }
        ui.prepare_frame(win);
        assert!(ui.tree.window_rect(g).height > 40.0);
        // A click below the header does nothing.
        let below = at(&ui, g, 100.0, 40.0);
        ui.pointer_down(win, below, Button::Left, Mods::empty());
        ui.pointer_up(win, below, Button::Left, Mods::empty());
        assert_eq!(*log.borrow(), vec![true]);
        ui.set_focus(g, true);
        ui.key(win, KeyEvent::press(Key::Left, Mods::empty()));
        assert_eq!(*log.borrow(), vec![true, false]);
        for _ in 0..60 {
            ui.advance(0.03);
        }
        ui.prepare_frame(win);
        assert_eq!(ui.tree.window_rect(g).height, 28.0);
    }

    #[test]
    fn the_triangle_toggles_and_turns() {
        let mut ui = Ui::new_headless();
        let win = ui.create_window(WindowKind::Normal, Size::new(100.0, 100.0));
        let d = ui.create(Disclosure::new());
        ui.set_content(win, d);
        ui.show_window(win);
        ui.prepare_frame(win);
        let p = at(&ui, d, 12.0, 12.0);
        ui.pointer_move(win, p, Mods::empty());
        ui.pointer_down(win, p, Button::Left, Mods::empty());
        ui.pointer_up(win, p, Button::Left, Mods::empty());
        assert_eq!(ui.get(d, Prop::Expanded), Some(Value::Bool(true)));
        assert!(ui.is_animating());
        for _ in 0..60 {
            ui.advance(0.03);
        }
        assert!(!ui.is_animating());
    }
}
