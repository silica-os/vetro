//! The stepper (kit `Steppers/Stepper`): a small `Surface/Control` bezel split by a divider
//! into an up and a down half, each with a 12 pt chevron. Holding a half repeats it.

use crate::amalfi::foundation::{ControlSize, control_shadow, focus_halo_named};
use crate::core::a11y::{A11y, Action, Role};
use crate::core::event::{Button, Event, Handled, Key};
use crate::core::layout::Proposal;
use crate::core::tree::ViewId;
use crate::core::ui::Ui;
use crate::core::value::{Prop, Signal, Value};
use crate::core::widget::{EventCx, FocusKind, Interaction, LayoutCx, PaintCx, PropChange, Widget};
use crate::geom::{Corners, Point, Rect, Size};
use crate::shape::Shape;
use crate::style::{Border, Style};

/// Fixed measures (silica_ui's `AmalfiStepperMetrics`).
pub mod metrics {
    use crate::amalfi::foundation::ControlSize;
    use crate::geom::Size;

    /// The bezel's size: as wide as the control is tall minus 4.
    pub fn size_of(size: ControlSize) -> Size {
        Size::new(size.height() - 4.0, size.height())
    }

    /// The bezel's corner radius.
    pub fn radius_of(size: ControlSize) -> f32 {
        match size {
            ControlSize::Mini => 7.0,
            ControlSize::Small => 8.0,
            ControlSize::Regular => 9.0,
            ControlSize::Large => 10.0,
        }
    }

    /// The divider's y.
    pub fn divider_of(size: ControlSize) -> f32 {
        size.height() / 2.0
    }

    /// The divider's inset from the sides.
    pub const DIVIDER_INSET: f32 = 4.0;
    /// The chevrons' size.
    pub const GLYPH: f32 = 12.0;
    /// Seconds a held half waits before it repeats.
    pub const REPEAT_DELAY: f64 = 0.4;
    /// Seconds between repeats.
    pub const REPEAT_INTERVAL: f64 = 0.08;
}

/// One half of the stepper.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Half {
    Up,
    Down,
}

/// A stepper.
#[derive(Debug)]
pub struct Stepper {
    /// The value.
    pub value: f64,
    /// The smallest value.
    pub min: f64,
    /// The largest value.
    pub max: f64,
    /// How much a step changes it.
    pub step: f64,
    /// Whether it wraps from one end to the other.
    pub wraps: bool,
    /// Size.
    pub size: ControlSize,
    hovered: Option<Half>,
    held: Option<Half>,
    generation: u64,
}

impl Stepper {
    /// A stepper from 0 to 100 at 0, step 1.
    pub fn new() -> Stepper {
        Stepper {
            value: 0.0,
            min: 0.0,
            max: 100.0,
            step: 1.0,
            wraps: false,
            size: ControlSize::Regular,
            hovered: None,
            held: None,
            generation: 0,
        }
    }

    /// With a value.
    pub fn value(mut self, v: f64) -> Stepper {
        self.value = v;
        self
    }

    /// With a range.
    pub fn range(mut self, min: f64, max: f64) -> Stepper {
        self.min = min;
        self.max = max;
        self
    }

    /// With a step.
    pub fn step(mut self, step: f64) -> Stepper {
        self.step = step;
        self
    }

    /// With a size.
    pub fn size(mut self, size: ControlSize) -> Stepper {
        self.size = size;
        self
    }

    /// Wrapping.
    pub fn wraps(mut self) -> Stepper {
        self.wraps = true;
        self
    }

    fn can(&self, half: Half, enabled: bool) -> bool {
        if !enabled {
            return false;
        }
        if self.wraps {
            return true;
        }
        match half {
            Half::Up => self.value < self.max,
            Half::Down => self.value > self.min,
        }
    }

    fn next(&self, half: Half) -> f64 {
        let d = if half == Half::Up {
            self.step
        } else {
            -self.step
        };
        let v = self.value + d;
        if self.wraps {
            if v > self.max {
                return self.min;
            }
            if v < self.min {
                return self.max;
            }
            return v;
        }
        v.clamp(self.min.min(self.max), self.max)
    }

    fn half_at(&self, p: Point) -> Half {
        if p.y < metrics::divider_of(self.size) {
            Half::Up
        } else {
            Half::Down
        }
    }

    fn half_rect(&self, half: Half) -> Rect {
        let b = metrics::size_of(self.size);
        let d = metrics::divider_of(self.size);
        match half {
            Half::Up => Rect::new(0.0, 0.0, b.width, d),
            Half::Down => Rect::new(0.0, d, b.width, b.height - d),
        }
    }

    /// Steps `half` once; returns whether the value changed.
    fn step_by(&mut self, ui: &mut Ui, id: ViewId, half: Half) -> bool {
        let enabled = !ui.is_disabled(id);
        if !self.can(half, enabled) {
            return false;
        }
        let v = self.next(half);
        if v == self.value {
            return false;
        }
        self.value = v;
        ui.emit(id, Signal::ValueChanged, Value::Float(v));
        ui.request_paint(id);
        true
    }

    fn schedule_repeat(ui: &mut Ui, id: ViewId, generation: u64, delay: f64) {
        ui.after(delay, move |ui| {
            let Some(half) = ui
                .widget::<Stepper>(id)
                .filter(|s| s.generation == generation)
                .and_then(|s| s.held)
            else {
                return;
            };
            let stepped = ui
                .with_widget::<Stepper, _>(id, |s| {
                    let enabled = true;
                    if s.can(half, enabled) && s.next(half) != s.value {
                        s.value = s.next(half);
                        Some(s.value)
                    } else {
                        s.held = None;
                        None
                    }
                })
                .flatten();
            if let Some(v) = stepped {
                ui.emit(id, Signal::ValueChanged, Value::Float(v));
                Stepper::schedule_repeat(ui, id, generation, metrics::REPEAT_INTERVAL);
            }
        });
    }
}

impl Default for Stepper {
    fn default() -> Self {
        Stepper::new()
    }
}

impl Widget for Stepper {
    fn kind(&self) -> &'static str {
        "Stepper"
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        metrics::size_of(self.size)
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let c = theme.colors;
        let b = cx.bounds();
        let size = metrics::size_of(self.size);
        let radius = metrics::radius_of(self.size);
        let enabled = !cx.is_disabled();
        let rect = Rect::new(b.x, b.y, size.width, size.height);
        let shape = Shape::rounded(radius);
        cx.rec.layer(
            "Background",
            rect,
            shape.clone(),
            Style::fill(c.surface_control)
                .with_border(Border::inside(c.line_border, 1.0))
                .with_shadow(control_shadow(&theme)),
        );
        for half in [Half::Up, Half::Down] {
            if !self.can(half, enabled) {
                continue;
            }
            let color = if self.held == Some(half) {
                c.surface_control_pressed
            } else if self.hovered == Some(half) && self.held.is_none() {
                c.accent_amalfi_soft
            } else {
                continue;
            };
            // Clipped to the bezel: the half's outer corners round.
            let corners = match half {
                Half::Up => Corners::new(radius, radius, 0.0, 0.0),
                Half::Down => Corners::new(0.0, 0.0, radius, radius),
            };
            // Clipped to the bezel, which has the smooth corners the half's own outline
            // would fit differently on a shorter side.
            cx.rec.push_clip(rect, shape.clone());
            cx.rec.layer(
                "State",
                self.half_rect(half).translate(Point::new(b.x, b.y)),
                Shape::Rect {
                    corners,
                    smoothing: 0.0,
                },
                Style::fill(color),
            );
            cx.rec.pop();
        }
        if cx.focus_visible() && enabled {
            focus_halo_named(cx.rec, "Focus Halo", rect, shape, &theme);
        }
        cx.rec.layer(
            "Divider",
            Rect::new(
                b.x + metrics::DIVIDER_INSET,
                b.y + metrics::divider_of(self.size),
                size.width - 2.0 * metrics::DIVIDER_INSET,
                1.0,
            ),
            Shape::RECT,
            Style::fill(c.line_separator),
        );
        for (half, name, symbol) in [
            (Half::Up, "Glyph Chevron Up", "chevron-up"),
            (Half::Down, "Glyph Chevron Down", "chevron-down"),
        ] {
            let r = self.half_rect(half);
            let g = metrics::GLYPH;
            let dim = enabled && !self.can(half, enabled);
            if dim {
                cx.rec.push_opacity(theme.disabled_opacity());
            }
            cx.rec.glyph(
                name,
                Rect::new(
                    b.x + ((size.width - g) / 2.0).round(),
                    b.y + (r.y + (r.height - g) / 2.0).round(),
                    g,
                    g,
                ),
                symbol,
                400,
                c.text_primary,
            );
            if dim {
                cx.rec.pop();
            }
        }
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
                let h = enabled.then(|| self.half_at(local(pos)));
                let h = h.filter(|h| self.can(*h, enabled));
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
                let h = self.half_at(local(pos));
                if !self.can(h, enabled) {
                    return Handled::No;
                }
                self.held = Some(h);
                self.generation += 1;
                let g = self.generation;
                self.step_by(cx.ui(), id, h);
                Stepper::schedule_repeat(cx.ui(), id, g, metrics::REPEAT_DELAY);
                cx.request_paint();
                Handled::Yes
            }
            Event::PointerUp {
                button: Button::Left,
                ..
            } => {
                self.held = None;
                self.generation += 1;
                cx.request_paint();
                Handled::Yes
            }
            Event::Key(k) if k.pressed => match k.key {
                Key::Up => {
                    self.step_by(cx.ui(), id, Half::Up);
                    Handled::Yes
                }
                Key::Down => {
                    self.step_by(cx.ui(), id, Half::Down);
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

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        let num = |v: &Value| v.as_f64().filter(|f| f.is_finite());
        match prop {
            Prop::Value => num(value).map_or(PropChange::Unknown, |v| {
                self.value = v;
                PropChange::Paint
            }),
            Prop::MinValue => num(value).map_or(PropChange::Unknown, |v| {
                self.min = v;
                PropChange::Paint
            }),
            Prop::MaxValue => num(value).map_or(PropChange::Unknown, |v| {
                self.max = v;
                PropChange::Paint
            }),
            Prop::Step => num(value)
                .filter(|s| *s > 0.0)
                .map_or(PropChange::Unknown, |v| {
                    self.step = v;
                    PropChange::Nothing
                }),
            Prop::Wraps => value.as_bool().map_or(PropChange::Unknown, |b| {
                self.wraps = b;
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
            Prop::Value => Some(Value::Float(self.value)),
            Prop::MinValue => Some(Value::Float(self.min)),
            Prop::MaxValue => Some(Value::Float(self.max)),
            Prop::Step => Some(Value::Float(self.step)),
            Prop::Wraps => Some(Value::Bool(self.wraps)),
            Prop::ControlSize => Some(Value::Int(self.size.to_int())),
            _ => None,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Stepper;
        a.numeric = Some((self.value, self.min, self.max, self.step));
        a.value = Some(if self.value.fract() == 0.0 && self.value.is_finite() {
            format!("{}", self.value as i64)
        } else {
            self.value.to_string()
        });
        a.actions = vec![Action::Increment, Action::Decrement, Action::Focus];
    }
}

/// A number as the value field shows it: whole numbers without a fraction.
pub fn format_value(v: f64) -> String {
    if v.is_finite() && v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        v.to_string()
    }
}

/// A value field with a stepper beside it (kit `Steppers/With Field`): a row of the field
/// (`Value field`) and the stepper (`Stepper`), 8 apart. Typing a number sets the stepper,
/// stepping sets the text, Up and Down in the field step. The row emits
/// `Signal::ValueChanged` (`Float`) when the value changes through the user.
pub fn stepper_with_field(ui: &mut Ui, stepper: Stepper) -> ViewId {
    use crate::core::layout::Stack;
    let value = stepper.value;
    let (min, max) = (stepper.min.min(stepper.max), stepper.max.max(stepper.min));
    let row = ui.create(Stack::row(8.0));
    let field =
        ui.create(crate::amalfi::fields::TextField::value().with_text(&format_value(value)));
    let step = ui.create(stepper);
    let _ = ui.set(field, Prop::Identifier, Value::from("Value field"));
    let _ = ui.set(step, Prop::Identifier, Value::from("Stepper"));
    ui.add_child(row, field);
    ui.add_child(row, step);
    ui.connect(step, Signal::ValueChanged, move |ui, _, v| {
        if let Some(n) = v.as_f64() {
            let _ = ui.set(field, Prop::Text, Value::Str(format_value(n)));
            ui.emit(row, Signal::ValueChanged, Value::Float(n));
        }
    });
    ui.connect(field, Signal::TextChanged, move |ui, _, v| {
        if let Some(n) = v.as_str().and_then(|s| s.trim().parse::<f64>().ok())
            && n.is_finite()
        {
            let n = n.clamp(min, max);
            let _ = ui.set(step, Prop::Value, Value::Float(n));
            ui.emit(row, Signal::ValueChanged, Value::Float(n));
        }
    });
    ui.connect(field, Signal::Activated, move |ui, _, v| {
        let up = v.as_int().is_some_and(|i| i > 0);
        let id = step;
        let mut stepped = None;
        ui.with_widget::<Stepper, _>(id, |s| {
            if s.can(if up { Half::Up } else { Half::Down }, true) {
                let h = if up { Half::Up } else { Half::Down };
                let n = s.next(h);
                if n != s.value {
                    s.value = n;
                    stepped = Some(n);
                }
            }
        });
        if let Some(n) = stepped {
            let _ = ui.set(field, Prop::Text, Value::Str(format_value(n)));
            ui.emit(row, Signal::ValueChanged, Value::Float(n));
        }
    });
    row
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::event::{KeyEvent, Mods};
    use crate::core::window::{WindowId, WindowKind};
    use std::cell::RefCell;
    use std::rc::Rc;

    type Log = Rc<RefCell<Vec<f64>>>;

    fn scene(s: Stepper) -> (Ui, WindowId, ViewId, Log) {
        let mut ui = Ui::new_headless();
        let win = ui.create_window(WindowKind::Normal, Size::new(100.0, 100.0));
        let id = ui.create(s);
        let log: Log = Rc::new(RefCell::new(Vec::new()));
        let l = log.clone();
        ui.connect(id, Signal::ValueChanged, move |_, _, v| {
            l.borrow_mut().push(v.as_f64().unwrap_or(f64::NAN));
        });
        ui.set_content(win, id);
        ui.show_window(win);
        ui.prepare_frame(win);
        (ui, win, id, log)
    }

    fn at(ui: &Ui, id: ViewId, x: f32, y: f32) -> Point {
        let r = ui.tree.window_rect(id);
        Point::new(r.x + x, r.y + y)
    }

    #[test]
    fn halves_step_and_clamp() {
        let (mut ui, win, id, log) = scene(Stepper::new().value(99.0));
        let up = at(&ui, id, 12.0, 4.0);
        let down = at(&ui, id, 12.0, 24.0);
        ui.pointer_move(win, up, Mods::empty());
        ui.pointer_down(win, up, Button::Left, Mods::empty());
        ui.pointer_up(win, up, Button::Left, Mods::empty());
        // At the maximum the up half does nothing.
        ui.pointer_down(win, up, Button::Left, Mods::empty());
        ui.pointer_up(win, up, Button::Left, Mods::empty());
        ui.pointer_move(win, down, Mods::empty());
        ui.pointer_down(win, down, Button::Left, Mods::empty());
        ui.pointer_up(win, down, Button::Left, Mods::empty());
        assert_eq!(*log.borrow(), vec![100.0, 99.0]);
    }

    #[test]
    fn holding_repeats_after_the_delay() {
        let (mut ui, win, id, log) = scene(Stepper::new().value(10.0));
        let up = at(&ui, id, 12.0, 4.0);
        ui.pointer_move(win, up, Mods::empty());
        ui.pointer_down(win, up, Button::Left, Mods::empty());
        for _ in 0..4 {
            ui.advance(0.1);
        }
        let n = log.borrow().len();
        assert!(n >= 2, "{n}");
        ui.pointer_up(win, up, Button::Left, Mods::empty());
        for _ in 0..10 {
            ui.advance(0.1);
        }
        assert_eq!(log.borrow().len(), n);
    }

    #[test]
    fn the_field_and_the_stepper_follow_each_other() {
        let mut ui = Ui::new_headless();
        let win = ui.create_window(WindowKind::Normal, Size::new(200.0, 100.0));
        let row = stepper_with_field(&mut ui, Stepper::new().value(12.0));
        let log: Log = Rc::new(RefCell::new(Vec::new()));
        let l = log.clone();
        ui.connect(row, Signal::ValueChanged, move |_, _, v| {
            l.borrow_mut().push(v.as_f64().unwrap_or(f64::NAN));
        });
        ui.set_content(win, row);
        ui.show_window(win);
        ui.prepare_frame(win);
        let field = ui.find(row, "Value field");
        let step = ui.find(row, "Stepper");
        let (Some(field), Some(step)) = (field, step) else {
            panic!("the parts are missing");
        };
        // The stepper's up half sets the text.
        let r = ui.tree.window_rect(step);
        let up = Point::new(r.x + 12.0, r.y + 4.0);
        ui.pointer_move(win, up, Mods::empty());
        ui.pointer_down(win, up, Button::Left, Mods::empty());
        ui.pointer_up(win, up, Button::Left, Mods::empty());
        assert_eq!(ui.get(field, Prop::Text), Some(Value::from("13")));
        // Typing sets the stepper; Down in the field steps.
        ui.set_focus(field, true);
        key_press(&mut ui, win, Key::Backspace);
        key_press(&mut ui, win, Key::Backspace);
        ui.text_input(win, "40");
        assert_eq!(ui.get(step, Prop::Value), Some(Value::Float(40.0)));
        key_press(&mut ui, win, Key::Down);
        assert_eq!(ui.get(field, Prop::Text), Some(Value::from("39")));
        assert_eq!(log.borrow().last(), Some(&39.0));
    }

    fn key_press(ui: &mut Ui, win: WindowId, k: Key) {
        ui.key(win, KeyEvent::press(k, Mods::empty()));
    }

    #[test]
    fn wraps_and_keys() {
        let (mut ui, win, id, log) = scene(Stepper::new().range(0.0, 2.0).value(2.0).wraps());
        ui.set_focus(id, true);
        ui.key(win, KeyEvent::press(Key::Up, Mods::empty()));
        ui.key(win, KeyEvent::press(Key::Down, Mods::empty()));
        assert_eq!(*log.borrow(), vec![0.0, 2.0]);
    }
}
