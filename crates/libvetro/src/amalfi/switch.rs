//! The switch (kit `Toggles/Switch`): Mini, Small and Regular, alone or in a labelled row.
//! The thumb stretches while pressed; dragging turns it into a `Glass/Clear` lens that
//! follows the pointer (`materials.md` lens rule).

use crate::amalfi::foundation::{
    ControlSize, Interaction as Transitions, Label, control_shadow, focus_halo_named,
};
use crate::amalfi::theme::Theme;
use crate::amalfi::tokens::{self, typography};
use crate::color::Color;
use crate::core::a11y::{A11y, Action, Role, Toggled};
use crate::core::event::{Button, Event, Handled};
use crate::core::layout::Proposal;
use crate::core::tree::NodeFlags;
use crate::core::value::{Prop, Signal, Value};
use crate::core::widget::{EventCx, FocusKind, Interaction, LayoutCx, PaintCx, PropChange, Widget};
use crate::geom::{Rect, Size};
use crate::motion::{Animated, Timing};
use crate::shape::Shape;
use crate::style::{Border, Style};
use crate::text::Align;

/// The measures of a switch (silica_ui's `AmalfiSwitchMetrics`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SwitchMetrics {
    /// Track size.
    pub track: Size,
    /// Track corner radius.
    pub track_radius: f32,
    /// Thumb diameter.
    pub thumb: f32,
    /// Thumb corner radius.
    pub thumb_radius: f32,
    /// How much the lens exceeds the thumb.
    pub lens_growth: f32,
}

impl SwitchMetrics {
    /// Thumb inset on every side (`switch.thumbInset`).
    pub const INSET: f32 = tokens::switch::THUMB_INSET;
    /// How far the pressed thumb stretches toward the middle (`switch.pressedStretch`).
    pub const PRESSED_STRETCH: f32 = tokens::switch::PRESSED_STRETCH;

    /// The metrics at `size` (Large is Regular).
    pub fn of(size: ControlSize) -> SwitchMetrics {
        match size {
            ControlSize::Mini => SwitchMetrics {
                track: Size::new(24.0, 16.0),
                track_radius: 7.0,
                thumb: 12.0,
                thumb_radius: 5.0,
                lens_growth: 8.0,
            },
            ControlSize::Small => SwitchMetrics {
                track: Size::new(32.0, 20.0),
                track_radius: 9.0,
                thumb: 16.0,
                thumb_radius: 7.0,
                lens_growth: 8.0,
            },
            ControlSize::Regular | ControlSize::Large => SwitchMetrics {
                track: Size::new(40.0, 24.0),
                track_radius: 12.0,
                thumb: 20.0,
                thumb_radius: 10.0,
                lens_growth: 12.0,
            },
        }
    }

    /// The thumb's left edge at rest.
    pub fn rest_x(&self, on: bool) -> f32 {
        if on {
            self.track.width - Self::INSET - self.thumb
        } else {
            Self::INSET
        }
    }

    /// `x` kept on the track.
    pub fn clamp_x(&self, x: f32) -> f32 {
        x.clamp(0.0, (self.track.width - self.thumb).max(0.0))
    }
}

/// The labelled row's measures (`AmalfiSwitchRowMetrics`).
pub mod row {
    /// Horizontal padding.
    pub const PAD_X: f32 = 12.0;
    /// Vertical padding.
    pub const PAD_Y: f32 = 4.0;
    /// Gap between label and switch (inside the label's frame).
    pub const GAP: f32 = 12.0;
}

/// Movement before a press becomes a drag.
const DRAG_SLOP: f32 = 3.0;

/// A switch.
#[derive(Debug)]
pub struct Switch {
    /// On.
    pub on: bool,
    /// Size.
    pub size: ControlSize,
    /// The row's label.
    pub label: Option<String>,
    text: Label,
    transitions: Transitions,
    thumb: Animated<f32>,
    track_on: Animated<f32>,
    lens: Animated<f32>,
    dragging: bool,
    press_x: Option<f32>,
    drag_from: f32,
    thumb_from: f32,
    suppress_activate: bool,
}

impl Switch {
    /// An off, regular switch.
    pub fn new() -> Switch {
        let m = SwitchMetrics::of(ControlSize::Regular);
        Switch {
            on: false,
            size: ControlSize::Regular,
            label: None,
            text: Label::default(),
            transitions: Transitions::default(),
            thumb: Animated::new(m.rest_x(false)),
            track_on: Animated::new(0.0),
            lens: Animated::new(0.0),
            dragging: false,
            press_x: None,
            drag_from: 0.0,
            thumb_from: 0.0,
            suppress_activate: false,
        }
    }

    /// With a state.
    pub fn on(mut self, on: bool) -> Switch {
        self.on = on;
        self.thumb = Animated::new(self.metrics().rest_x(on));
        self.track_on = Animated::new(if on { 1.0 } else { 0.0 });
        self
    }

    /// With a size.
    pub fn size(mut self, size: ControlSize) -> Switch {
        self.size = size;
        self.thumb = Animated::new(self.metrics().rest_x(self.on));
        self
    }

    /// With a label (a row).
    pub fn label(mut self, label: &str) -> Switch {
        self.label = Some(label.to_string());
        self
    }

    fn metrics(&self) -> SwitchMetrics {
        SwitchMetrics::of(self.size)
    }

    fn thumb_is_on(&self) -> bool {
        let m = self.metrics();
        self.thumb.get() + m.thumb / 2.0 > m.track.width / 2.0
    }

    /// Settles the thumb and the track on the state.
    fn settle(&mut self, reduce: bool) -> bool {
        let m = self.metrics();
        let spring = if reduce {
            Timing::None
        } else {
            Timing::Spring(tokens::motion::SPRING_SNAPPY)
        };
        let fade = if reduce {
            Timing::None
        } else {
            Timing::curve(tokens::motion::FADE_QUICK_CURVE, tokens::motion::FADE_QUICK)
        };
        self.thumb.animate_to(m.rest_x(self.on), spring);
        self.track_on
            .animate_to(if self.on { 1.0 } else { 0.0 }, fade);
        self.animating()
    }

    fn animating(&self) -> bool {
        self.thumb.is_animating() || self.track_on.is_animating() || self.lens.is_animating()
    }

    fn row_height(&self) -> f32 {
        self.metrics().track.height + row::PAD_Y * 2.0
    }

    fn label_width(&mut self, theme: &Theme) -> f32 {
        let Some(l) = self.label.clone() else {
            return 0.0;
        };
        let style = theme.text(&typography::BODY, theme.colors.text_primary);
        self.text.width(&l, &style) + row::GAP
    }

    /// The track's origin in the content box.
    fn track_origin(&self, bounds: Rect) -> (f32, f32) {
        if self.label.is_some() {
            let m = self.metrics();
            (
                bounds.right() - row::PAD_X - m.track.width,
                bounds.y + row::PAD_Y,
            )
        } else {
            (bounds.x, bounds.y)
        }
    }
}

impl Default for Switch {
    fn default() -> Self {
        Switch::new()
    }
}

impl Widget for Switch {
    fn kind(&self) -> &'static str {
        "Switch"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        let m = self.metrics();
        if self.label.is_none() {
            return m.track;
        }
        let theme = cx.theme();
        let natural = row::PAD_X * 2.0 + self.label_width(&theme) + m.track.width;
        let min = row::PAD_X * 2.0 + m.track.width;
        let w = match p.width {
            Some(w) if w.is_finite() => w.max(min),
            _ => natural,
        };
        Size::new(w, self.row_height())
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let c = theme.colors;
        let m = self.metrics();
        let r = cx.bounds();
        let labelled = self.label.is_some();
        let prefix = if labelled { "Switch/" } else { "" };
        let (tx, ty) = self.track_origin(r);
        let track = Rect::new(tx, ty, m.track.width, m.track.height);
        let shape = Shape::rounded(m.track_radius);
        let (hover, press) = (
            self.transitions.hover(),
            if self.dragging {
                0.0
            } else {
                self.transitions.press()
            },
        );
        let t = self.track_on.get().clamp(0.0, 1.0);

        // Track.
        let on_fill = c.accent_amalfi.lerp(c.accent_amalfi_pressed, press);
        let mut style = Style::fill(c.surface_sunken.lerp(on_fill, t));
        if t < 1.0 {
            style.borders.push(Border::inside(
                c.line_border.lerp(Color::rgba(0, 0, 0, 0), t),
                1.0,
            ));
        }
        cx.rec
            .layer(&format!("{prefix}Track"), track, shape.clone(), style);
        if hover > 0.0 && !self.dragging {
            let wash = if self.on {
                c.state_hover_overlay
            } else {
                c.accent_amalfi_soft
            };
            cx.rec.layer(
                &format!("{prefix}State"),
                track,
                shape.clone(),
                Style::fill(wash).with_opacity(hover),
            );
        }
        if cx.focus_visible() && !cx.is_disabled() {
            focus_halo_named(cx.rec, &format!("{prefix}Focus Halo"), track, shape, &theme);
        }

        // Thumb.
        let x = m.clamp_x(self.thumb.get());
        if !self.dragging {
            let stretch = SwitchMetrics::PRESSED_STRETCH * press;
            let left = if self.on { x - stretch } else { x };
            cx.rec.layer(
                &format!("{prefix}Thumb"),
                Rect::new(
                    track.x + left,
                    track.y + SwitchMetrics::INSET,
                    m.thumb + stretch,
                    m.thumb,
                ),
                Shape::rounded(m.thumb_radius),
                Style::fill(c.text_on_accent).with_shadow(control_shadow(&theme)),
            );
        }

        // The lens over the thumb.
        let presence = self.lens.get().clamp(0.0, 1.0);
        if self.dragging || presence > 0.0 {
            let lens = m.thumb + m.lens_growth;
            let side = if self.dragging {
                m.thumb + (lens - m.thumb) * presence
            } else {
                lens
            };
            let centre_x = track.x + x + m.thumb / 2.0;
            let centre_y = track.y + m.track.height / 2.0;
            let rect = Rect::new(centre_x - side / 2.0, centre_y - side / 2.0, side, side);
            let mut style = theme.material_style("Glass/Clear");
            if !self.dragging {
                style.opacity = presence;
            }
            cx.rec.layer(
                &format!("{prefix}Lens"),
                rect,
                Shape::rounded((side / 2.0 - 1.0).max(0.0)),
                style,
            );
        }

        // The label.
        if let Some(label) = self.label.clone() {
            let style = theme.text(&typography::BODY, c.text_primary);
            let lh = style.line_height.unwrap_or(18.0);
            let box_w = (tx - r.x - row::PAD_X).max(0.0);
            let layout = self.text.layout(
                &label,
                &style,
                Align::Start,
                (box_w - row::GAP).max(0.0),
                Some(1),
            );
            cx.rec.text(
                "Label",
                Rect::new(r.x + row::PAD_X, r.y + (r.height - lh) / 2.0, box_w, lh),
                layout,
                Some(typography::BODY.name),
                style.color,
            );
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        let reduce = cx.theme().appearance.reduce_motion;
        match event {
            Event::PointerDown {
                pos,
                button: Button::Left,
                ..
            } => {
                self.press_x = Some(pos.x);
                self.suppress_activate = false;
                Handled::Yes
            }
            Event::PointerMove {
                pos,
                dragging: true,
                ..
            } => {
                let Some(down) = self.press_x else {
                    return Handled::No;
                };
                if !self.dragging {
                    if (pos.x - down).abs() < DRAG_SLOP {
                        return Handled::Yes;
                    }
                    self.dragging = true;
                    self.drag_from = down;
                    self.thumb_from = self.thumb.get();
                    let timing = if reduce {
                        Timing::None
                    } else {
                        Timing::Spring(tokens::motion::SPRING_SNAPPY)
                    };
                    self.lens.animate_to(1.0, timing);
                }
                let x = self
                    .metrics()
                    .clamp_x(self.thumb_from + pos.x - self.drag_from);
                self.thumb.jump(x);
                let fade = if reduce {
                    Timing::None
                } else {
                    Timing::curve(tokens::motion::FADE_QUICK_CURVE, tokens::motion::FADE_QUICK)
                };
                let target = if self.thumb_is_on() { 1.0 } else { 0.0 };
                self.track_on.animate_to(target, fade);
                cx.request_paint();
                if self.animating() {
                    cx.animate();
                }
                Handled::Yes
            }
            Event::PointerUp {
                button: Button::Left,
                ..
            } => {
                self.press_x = None;
                if self.dragging {
                    self.dragging = false;
                    self.suppress_activate = true;
                    let on = self.thumb_is_on();
                    let fade = if reduce {
                        Timing::None
                    } else {
                        Timing::curve(tokens::motion::FADE_QUICK_CURVE, tokens::motion::FADE_QUICK)
                    };
                    self.lens.animate_to(0.0, fade);
                    if on != self.on {
                        self.on = on;
                        cx.emit(Signal::ValueChanged, Value::Bool(on));
                    }
                    self.settle(reduce);
                    cx.request_paint();
                    cx.animate();
                }
                Handled::Yes
            }
            Event::Activate => {
                if std::mem::take(&mut self.suppress_activate) {
                    return Handled::Yes;
                }
                self.on = !self.on;
                cx.emit(Signal::ValueChanged, Value::Bool(self.on));
                self.settle(reduce);
                cx.request_paint();
                if self.animating() {
                    cx.animate();
                }
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
        let a = self.thumb.tick(dt);
        let b = self.track_on.tick(dt);
        let c = self.lens.tick(dt);
        let d = self.transitions.tick(dt);
        a || b || c || d
    }

    fn interaction(&self) -> Interaction {
        Interaction::Toggle
    }

    fn focus(&self) -> FocusKind {
        FocusKind::Control
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match prop {
            Prop::State | Prop::Value => match value.as_bool() {
                Some(on) => {
                    self.on = on;
                    self.thumb.jump(self.metrics().rest_x(on));
                    self.track_on.jump(if on { 1.0 } else { 0.0 });
                    PropChange::Paint
                }
                None => PropChange::Unknown,
            },
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
            Prop::ControlSize => match value.as_int().and_then(ControlSize::from_int) {
                Some(s) => {
                    self.size = s;
                    self.thumb.jump(self.metrics().rest_x(self.on));
                    PropChange::Layout
                }
                None => PropChange::Unknown,
            },
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::State | Prop::Value => Some(Value::Bool(self.on)),
            Prop::Title => Some(self.label.clone().map(Value::Str).unwrap_or_default()),
            Prop::ControlSize => Some(Value::Int(self.size.to_int())),
            _ => None,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Switch;
        a.label.clone_from(&self.label);
        a.toggled = Some(if self.on {
            Toggled::True
        } else {
            Toggled::False
        });
        a.actions = vec![Action::Click, Action::Focus];
    }

    fn baseline(&self) -> Option<f32> {
        self.label.as_ref()?;
        Some((self.row_height() - 18.0) / 2.0 + 18.0 * 0.75)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::event::{Key, KeyEvent, Mods};
    use crate::core::ui::Ui;
    use crate::core::window::{WindowId, WindowKind};
    use crate::geom::Point;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn scene(sw: Switch) -> (Ui, WindowId, crate::core::ViewId, Rc<RefCell<Vec<bool>>>) {
        let mut ui = Ui::new_headless();
        let win = ui.create_window(WindowKind::Normal, Size::new(200.0, 100.0));
        let id = ui.create(sw);
        let log = Rc::new(RefCell::new(Vec::new()));
        let l = log.clone();
        ui.connect(id, Signal::ValueChanged, move |_, _, v| {
            l.borrow_mut().push(v.as_bool().unwrap_or(false));
        });
        ui.set_content(win, id);
        ui.show_window(win);
        ui.prepare_frame(win);
        (ui, win, id, log)
    }

    fn on(ui: &Ui, id: crate::core::ViewId) -> bool {
        ui.widget::<Switch>(id).is_some_and(|s| s.on)
    }

    #[test]
    fn click_and_space_toggle() {
        let (mut ui, win, id, log) = scene(Switch::new().label("Wi-Fi"));
        let c = ui.tree.window_rect(id).center();
        ui.pointer_down(win, c, Button::Left, Mods::empty());
        ui.pointer_up(win, c, Button::Left, Mods::empty());
        assert!(on(&ui, id));
        ui.set_focus(id, true);
        ui.key(win, KeyEvent::press(Key::Space, Mods::empty()));
        let mut up = KeyEvent::press(Key::Space, Mods::empty());
        up.pressed = false;
        ui.key(win, up);
        assert!(!on(&ui, id));
        assert_eq!(*log.borrow(), vec![true, false]);
        // The thumb springs to its rest position.
        ui.advance(2.0);
        let rest = SwitchMetrics::of(ControlSize::Regular).rest_x(false);
        assert_eq!(ui.widget::<Switch>(id).map(|s| s.thumb.get()), Some(rest));
    }

    #[test]
    fn dragging_the_thumb_past_the_middle_turns_it_on_once() {
        let (mut ui, win, id, log) = scene(Switch::new());
        let r = ui.tree.window_rect(id);
        let start = Point::new(r.x + 12.0, r.center().y);
        ui.pointer_down(win, start, Button::Left, Mods::empty());
        ui.pointer_move(win, Point::new(start.x + 5.0, start.y), Mods::empty());
        ui.pointer_move(win, Point::new(start.x + 16.0, start.y), Mods::empty());
        assert!(ui.widget::<Switch>(id).is_some_and(|s| s.dragging));
        ui.pointer_up(
            win,
            Point::new(start.x + 16.0, start.y),
            Button::Left,
            Mods::empty(),
        );
        assert!(on(&ui, id));
        // The release over the switch must not toggle it back.
        assert_eq!(*log.borrow(), vec![true]);
        ui.advance(2.0);
        assert!(
            ui.widget::<Switch>(id)
                .is_some_and(|s| !s.lens.is_animating())
        );
    }

    #[test]
    fn a_short_drag_back_keeps_the_state() {
        let (mut ui, win, id, log) = scene(Switch::new().on(true));
        let r = ui.tree.window_rect(id);
        let start = Point::new(r.x + 30.0, r.center().y);
        ui.pointer_down(win, start, Button::Left, Mods::empty());
        ui.pointer_move(win, Point::new(start.x - 4.0, start.y), Mods::empty());
        ui.pointer_move(win, Point::new(start.x - 5.0, start.y), Mods::empty());
        ui.pointer_up(
            win,
            Point::new(start.x - 5.0, start.y),
            Button::Left,
            Mods::empty(),
        );
        assert!(on(&ui, id));
        assert!(log.borrow().is_empty());
    }

    #[test]
    fn properties_and_accessibility() {
        let (mut ui, _win, id, _log) = scene(Switch::new());
        assert!(ui.set(id, Prop::State, Value::Bool(true)).is_ok());
        assert_eq!(ui.get(id, Prop::State), Some(Value::Bool(true)));
        let a = ui.accessibility_of(id).unwrap_or_default();
        assert_eq!(a.role, Role::Switch);
        assert_eq!(a.toggled, Some(Toggled::True));
    }
}
