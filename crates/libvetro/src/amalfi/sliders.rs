//! Sliders (kit `Sliders`): the linear slider (horizontal or vertical, plain, with tick marks
//! or with glyphs), the circular dial, and the round thumb that turns into a `Glass/Clear`
//! lens while it is dragged (`materials.md`, lens rule).

use skia_safe::PathBuilder;

use crate::amalfi::foundation::{control_shadow, focus_halo_named};
use crate::amalfi::theme::Theme;
use crate::amalfi::tokens;
use crate::core::a11y::{A11y, Action, Role};
use crate::core::event::{Button, Event, Handled, Key};
use crate::core::layout::Proposal;
use crate::core::value::{Prop, Signal, Value};
use crate::core::widget::{EventCx, FocusKind, Interaction, LayoutCx, PaintCx, PropChange, Widget};
use crate::geom::{Point, Rect, Size};
use crate::motion::{Animated, Timing};
use crate::shape::Shape;
use crate::style::{Border, BorderPosition, Style};

/// Fixed measures (silica_ui's `AmalfiSliderMetrics`).
pub mod metrics {
    /// The thumb's diameter.
    pub const THUMB: f32 = 20.0;
    /// The track's thickness.
    pub const TRACK: f32 = 4.0;
    /// Default length of a horizontal slider.
    pub const HORIZONTAL_LENGTH: f32 = 200.0;
    /// Default length of a vertical slider.
    pub const VERTICAL_LENGTH: f32 = 160.0;
    /// Thickness of a slider.
    pub const THICKNESS: f32 = 20.0;
    /// Thickness with tick marks.
    pub const THICKNESS_WITH_TICKS: f32 = 36.0;
    /// Gap between the track and the tick marks.
    pub const TICK_GAP: f32 = 8.0;
    /// A tick mark's width.
    pub const TICK_WIDTH: f32 = 2.0;
    /// A tick mark's length.
    pub const TICK_LENGTH: f32 = 8.0;
    /// Glyph size at the ends.
    pub const GLYPH: f32 = 16.0;
    /// Gap between a glyph and the track.
    pub const GLYPH_GAP: f32 = 8.0;
    /// The lens: 8 beyond the thumb along the track, 12 beyond the track across it.
    pub const LENS_LONG: f32 = THUMB + 16.0;
    /// The lens across the track.
    pub const LENS_SHORT: f32 = TRACK + 24.0;
    /// The dial's side.
    pub const DIAL: f32 = 56.0;
    /// The dial's ring diameter.
    pub const DIAL_TRACK: f32 = 44.0;
    /// The distance of the dial's thumb from its centre.
    pub const DIAL_ORBIT: f32 = 20.0;
    /// The thumb is grabbed within this distance of its centre.
    pub const GRAB_RADIUS: f32 = 14.0;
    /// Movement before a press becomes a drag.
    pub const DRAG_SLOP: f32 = 2.0;
}

/// How a thumb looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ThumbLook {
    /// A press turns it `Surface/Control Pressed`.
    pub pressed: bool,
    /// Hover lays `Accent/Amalfi Soft` over it.
    pub hovered: bool,
    /// Keyboard focus adds the halo.
    pub focused: bool,
}

/// Records a round thumb at `rect`: `Thumb`, `State`, `Focus Halo` (names under `prefix`).
pub fn paint_thumb(cx: &mut PaintCx<'_>, prefix: &str, rect: Rect, look: ThumbLook) {
    let theme = *cx.theme();
    let c = theme.colors;
    let shape = Shape::rounded(metrics::THUMB / 2.0);
    let fill = if look.pressed {
        c.surface_control_pressed
    } else {
        c.surface_control
    };
    cx.rec.layer(
        &format!("{prefix}Thumb"),
        rect,
        shape.clone(),
        Style::fill(fill)
            .with_border(Border::inside(c.line_border, 1.0))
            .with_shadow(control_shadow(&theme)),
    );
    if look.hovered && !look.pressed {
        cx.rec.layer(
            &format!("{prefix}State"),
            rect,
            shape.clone(),
            Style::fill(c.accent_amalfi_soft),
        );
    }
    if look.focused {
        focus_halo_named(cx.rec, &format!("{prefix}Focus Halo"), rect, shape, &theme);
    }
}

/// The lens over a dragged thumb: `Lens` (a `Glass/Clear` capsule) and `Core` (the thumb's
/// disc on it). `rect` is the lens, `core` the disc.
pub fn paint_lens(cx: &mut PaintCx<'_>, prefix: &str, rect: Rect, core: Rect, opacity: f32) {
    let theme = *cx.theme();
    let c = theme.colors;
    let mut style = theme.material_style("Glass/Clear");
    style.opacity = opacity;
    cx.rec.layer(
        &format!("{prefix}Lens"),
        rect,
        Shape::rounded((rect.width.min(rect.height) / 2.0 - 1.0).max(0.0)),
        style,
    );
    cx.rec.layer(
        &format!("{prefix}Core"),
        core,
        Shape::rounded(metrics::THUMB / 2.0),
        Style::fill(c.surface_control)
            .with_border(Border::inside(c.line_border, 1.0))
            .with_opacity(opacity),
    );
}

/// The value, the pointer and the keyboard of a slider, shared by the line and the dial.
#[derive(Debug)]
struct Core {
    value: f32,
    min: f32,
    max: f32,
    divisions: Option<u32>,
    pressed: bool,
    dragged: bool,
    active: bool,
    grab: Point,
    down_at: Point,
    growth: Animated<f32>,
    opacity: Animated<f32>,
    keyed: Option<f32>,
}

impl Core {
    fn new() -> Core {
        Core {
            value: 0.0,
            min: 0.0,
            max: 1.0,
            divisions: None,
            pressed: false,
            dragged: false,
            active: false,
            grab: Point::ZERO,
            down_at: Point::ZERO,
            growth: Animated::new(0.0),
            opacity: Animated::new(0.0),
            keyed: None,
        }
    }

    fn fraction(&self) -> f32 {
        let span = self.max - self.min;
        if !span.is_finite() || span <= 0.0 || !self.value.is_finite() {
            return 0.0;
        }
        ((self.value - self.min) / span).clamp(0.0, 1.0)
    }

    fn value_of(&self, f: f32) -> f32 {
        let mut t = if f.is_finite() {
            f.clamp(0.0, 1.0)
        } else {
            0.0
        };
        if let Some(d) = self.divisions.filter(|d| *d > 0) {
            let d = d as f32;
            t = (t * d).round() / d;
        }
        self.min + t * (self.max - self.min)
    }

    fn emit(&mut self, cx: &mut EventCx<'_>, f: f32) {
        let v = self.value_of(f);
        if v != self.value {
            self.value = v;
            cx.emit(Signal::ValueChanged, Value::Float(f64::from(v)));
            cx.request_paint();
        }
    }

    fn lens_growth(&self) -> f32 {
        self.growth.get().clamp(0.0, 1.2)
    }

    fn lens_opacity(&self) -> f32 {
        tokens::motion::FADE_QUICK_CURVE.transform(self.opacity.get().clamp(0.0, 1.0))
    }

    fn shows_lens(&self) -> bool {
        self.lens_opacity() > 0.0 && self.lens_growth() > 0.0
    }

    fn start_drag(&mut self, reduce: bool) {
        self.dragged = true;
        if reduce {
            self.growth.jump(1.0);
            self.opacity.animate_to(1.0, Timing::None);
            return;
        }
        self.opacity.jump(1.0);
        self.growth
            .animate_to(1.0, Timing::Spring(tokens::motion::SPRING_SNAPPY));
    }

    fn end(&mut self, cx: &mut EventCx<'_>, reduce: bool) {
        if !self.active {
            return;
        }
        self.active = false;
        self.pressed = false;
        if self.dragged {
            self.dragged = false;
            let t = if reduce {
                Timing::None
            } else {
                Timing::curve(tokens::motion::FADE_QUICK_CURVE, tokens::motion::FADE_QUICK)
            };
            self.opacity.animate_to(0.0, t);
            cx.animate();
        }
        cx.emit(Signal::EditingEnded, Value::Float(f64::from(self.value)));
        cx.request_paint();
    }

    fn tick(&mut self, dt: f32) -> bool {
        let a = self.growth.tick(dt);
        let b = self.opacity.tick(dt);
        if !self.dragged && !b && self.opacity.get() <= 0.0 && self.growth.get() != 0.0 {
            self.growth.jump(0.0);
        }
        a || b
    }

    fn step(&self) -> f32 {
        self.divisions
            .filter(|d| *d > 0)
            .map_or(0.05, |d| 1.0 / d as f32)
    }

    fn key(&mut self, cx: &mut EventCx<'_>, next: impl Fn(f32) -> f32) {
        let from = self.keyed.unwrap_or_else(|| self.fraction());
        let span = self.max - self.min;
        let to = self.value_of(next(from));
        cx.emit(Signal::EditingBegan, Value::Float(f64::from(self.value)));
        if to != self.value {
            self.value = to;
            cx.emit(Signal::ValueChanged, Value::Float(f64::from(to)));
        }
        self.keyed = Some(if span > 0.0 && span.is_finite() {
            (to - self.min) / span
        } else {
            0.0
        });
        cx.emit(Signal::EditingEnded, Value::Float(f64::from(to)));
        cx.request_paint();
    }

    /// Handles a key; `rtl` is ignored (left-to-right layouts).
    fn handle_key(&mut self, cx: &mut EventCx<'_>, key: &Key) -> Handled {
        let step = self.step();
        match key {
            Key::Up | Key::Right => self.key(cx, |f| f + step),
            Key::Down | Key::Left => self.key(cx, |f| f - step),
            Key::Home => self.key(cx, |_| 0.0),
            Key::End => self.key(cx, |_| 1.0),
            _ => return Handled::No,
        }
        Handled::Yes
    }

    fn set_common(&mut self, prop: Prop, value: &Value) -> PropChange {
        match prop {
            Prop::Value => match value.as_f32() {
                Some(v) if v.is_finite() => {
                    self.value = v;
                    self.keyed = None;
                    PropChange::Paint
                }
                _ => PropChange::Unknown,
            },
            Prop::MinValue => match value.as_f32() {
                Some(v) if v.is_finite() => {
                    self.min = v;
                    PropChange::Paint
                }
                _ => PropChange::Unknown,
            },
            Prop::MaxValue => match value.as_f32() {
                Some(v) if v.is_finite() => {
                    self.max = v;
                    PropChange::Paint
                }
                _ => PropChange::Unknown,
            },
            Prop::Step => match value.as_f32() {
                Some(s) if s.is_finite() && s > 0.0 => {
                    let n = ((self.max - self.min) / s).round();
                    self.divisions = (1.0..10_000.0).contains(&n).then_some(n as u32);
                    PropChange::Paint
                }
                _ => PropChange::Unknown,
            },
            _ => PropChange::Unknown,
        }
    }

    fn get_common(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::Value => Some(Value::Float(f64::from(self.value))),
            Prop::MinValue => Some(Value::Float(f64::from(self.min))),
            Prop::MaxValue => Some(Value::Float(f64::from(self.max))),
            Prop::Step => Some(self.divisions.map_or(Value::None, |d| {
                Value::Float(f64::from((self.max - self.min) / d as f32))
            })),
            _ => None,
        }
    }

    fn a11y(&self, a: &mut A11y) {
        a.role = Role::Slider;
        a.numeric = Some((
            f64::from(self.value),
            f64::from(self.min),
            f64::from(self.max),
            f64::from(self.step() * (self.max - self.min)),
        ));
        a.value = Some(format!("{}%", (self.fraction() * 100.0).round()));
        a.actions = vec![Action::Increment, Action::Decrement, Action::Focus];
    }
}

fn reduce_motion(cx: &mut EventCx<'_>) -> bool {
    cx.theme().appearance.reduce_motion
}

fn whole(r: Rect) -> Rect {
    let (l, t) = (r.x.round(), r.y.round());
    Rect::new(l, t, r.right().round() - l, r.bottom().round() - t)
}

// ---------------------------------------------------------------------------------------
// The linear slider.
// ---------------------------------------------------------------------------------------

/// A linear slider.
#[derive(Debug)]
pub struct Slider {
    core: Core,
    /// Vertical (0 at the bottom).
    pub vertical: bool,
    /// Tick marks (needs divisions).
    pub tick_marks: bool,
    /// A glyph at the minimum end.
    pub min_glyph: Option<String>,
    /// A glyph at the maximum end.
    pub max_glyph: Option<String>,
    /// A read-only level indicator: no thumb, no input.
    pub indicator: bool,
    length: f32,
}

impl Slider {
    /// A horizontal slider from 0 to 1 at 0.
    pub fn new() -> Slider {
        Slider {
            core: Core::new(),
            vertical: false,
            tick_marks: false,
            min_glyph: None,
            max_glyph: None,
            indicator: false,
            length: metrics::HORIZONTAL_LENGTH,
        }
    }

    /// With a value.
    pub fn value(mut self, v: f32) -> Slider {
        self.core.value = v;
        self
    }

    /// With a range.
    pub fn range(mut self, min: f32, max: f32) -> Slider {
        self.core.min = min;
        self.core.max = max;
        self
    }

    /// With discrete steps.
    pub fn divisions(mut self, n: u32) -> Slider {
        self.core.divisions = Some(n);
        self
    }

    /// With tick marks (one per division and one more).
    pub fn ticks(mut self) -> Slider {
        self.tick_marks = true;
        self
    }

    /// Vertical.
    pub fn vertical(mut self) -> Slider {
        self.vertical = true;
        self
    }

    /// With glyphs at the ends.
    pub fn glyphs(mut self, min: &str, max: &str) -> Slider {
        self.min_glyph = Some(min.to_string());
        self.max_glyph = Some(max.to_string());
        self
    }

    /// A read-only indicator.
    pub fn indicator(mut self) -> Slider {
        self.indicator = true;
        self
    }

    fn has_glyphs(&self) -> bool {
        !self.vertical && (self.min_glyph.is_some() || self.max_glyph.is_some())
    }

    fn glyph_offset(&self) -> f32 {
        if self.has_glyphs() {
            metrics::GLYPH + metrics::GLYPH_GAP
        } else {
            0.0
        }
    }

    fn ticks_count(&self) -> usize {
        match (self.tick_marks, self.core.divisions) {
            (true, Some(d)) => d as usize + 1,
            _ => 0,
        }
    }

    fn across(&self) -> f32 {
        if self.ticks_count() > 0 {
            metrics::THICKNESS_WITH_TICKS
        } else {
            metrics::THICKNESS
        }
    }

    fn track_at() -> f32 {
        (metrics::THUMB - metrics::TRACK) / 2.0
    }

    /// A rectangle `along` the slider, `across` it, in slider coordinates.
    fn rect(&self, along: f32, across: f32, extent: f32, thickness: f32) -> Rect {
        if self.vertical {
            Rect::new(across, self.length - along - extent, thickness, extent)
        } else {
            Rect::new(along, across, extent, thickness)
        }
    }

    fn thumb_rect(&self) -> Rect {
        let along = (self.core.fraction() * (self.length - metrics::THUMB)).round();
        self.rect(along, 0.0, metrics::THUMB, metrics::THUMB)
    }

    fn lens_rect(&self, growth: f32) -> Rect {
        let (w, h) = if self.vertical {
            (metrics::LENS_SHORT, metrics::LENS_LONG)
        } else {
            (metrics::LENS_LONG, metrics::LENS_SHORT)
        };
        let t = self.thumb_rect();
        let c = t.center();
        let to = Rect::new(c.x - w / 2.0, c.y - h / 2.0, w, h);
        whole(Rect::new(
            t.x + (to.x - t.x) * growth,
            t.y + (to.y - t.y) * growth,
            t.width + (to.width - t.width) * growth,
            t.height + (to.height - t.height) * growth,
        ))
    }

    fn fraction_at(&self, p: Point) -> f32 {
        let span = self.length - metrics::THUMB;
        if span <= 0.0 {
            return 0.0;
        }
        let along = if self.vertical {
            self.length - p.y
        } else {
            p.x
        };
        ((along - metrics::THUMB / 2.0) / span).clamp(0.0, 1.0)
    }

    fn size(&self) -> Size {
        let across = self.across();
        let slider = if self.vertical {
            Size::new(across, self.length)
        } else {
            Size::new(self.length, across)
        };
        if self.has_glyphs() {
            Size::new(slider.width + 2.0 * self.glyph_offset(), slider.height)
        } else {
            slider
        }
    }

    fn bed(&self, cx: &mut PaintCx<'_>, prefix: &str, o: Point) {
        let theme = *cx.theme();
        let c = theme.colors;
        let at = Slider::track_at();
        let shift = |r: Rect| r.translate(o);
        let radius = Shape::rounded(metrics::TRACK / 2.0);
        cx.rec.layer(
            &format!("{prefix}Track"),
            shift(self.rect(0.0, at, self.length, metrics::TRACK)),
            radius.clone(),
            Style::fill(c.surface_sunken).with_border(Border::inside(c.line_separator, 1.0)),
        );
        let fill = (self.core.fraction() * self.length).round();
        cx.rec.layer(
            &format!("{prefix}Fill"),
            shift(self.rect(0.0, at, fill, metrics::TRACK)),
            radius,
            Style::fill(c.accent_amalfi),
        );
        let n = self.ticks_count();
        for i in 0..n {
            let f = if n <= 1 {
                0.0
            } else {
                i as f32 / (n - 1) as f32
            };
            let centre = metrics::THUMB / 2.0 + f * (self.length - metrics::THUMB);
            let r = self.rect(
                (centre - metrics::TICK_WIDTH / 2.0).round(),
                at + metrics::TRACK + metrics::TICK_GAP,
                metrics::TICK_WIDTH,
                metrics::TICK_LENGTH,
            );
            // A tick is `TICK_WIDTH` across the track and `TICK_LENGTH` along the drop.
            let r = if self.vertical {
                Rect::new(r.x, r.y, metrics::TICK_LENGTH, metrics::TICK_WIDTH)
            } else {
                r
            };
            cx.rec.layer(
                &format!("{prefix}Tick {i}"),
                shift(r),
                Shape::RECT,
                Style::fill(c.text_tertiary),
            );
        }
    }
}

impl Default for Slider {
    fn default() -> Self {
        Slider::new()
    }
}

impl Widget for Slider {
    fn kind(&self) -> &'static str {
        "Slider"
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        let room = if self.vertical { p.height } else { p.width };
        let glyphs = 2.0 * self.glyph_offset();
        let default = if self.vertical {
            metrics::VERTICAL_LENGTH
        } else {
            metrics::HORIZONTAL_LENGTH
        };
        self.length = match room {
            Some(r) if r.is_finite() => (r - glyphs).max(metrics::THUMB),
            _ => default,
        };
        self.size()
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let b = cx.bounds();
        // Placed in a frame of another size: the slider takes the room it is given.
        let glyphs = 2.0 * self.glyph_offset();
        let room = if self.vertical { b.height } else { b.width } - glyphs;
        if room.is_finite() && room >= metrics::THUMB {
            self.length = room;
        }
        let theme = *cx.theme();
        let glyph_off = self.glyph_offset();
        let o = Point::new(b.x + glyph_off, b.y);
        let prefix = if self.has_glyphs() { "Slider/" } else { "" };
        if self.has_glyphs() {
            let y = b.y + (self.across() - metrics::GLYPH) / 2.0;
            let g = Rect::new(b.x, y, metrics::GLYPH, metrics::GLYPH);
            if let Some(s) = &self.min_glyph {
                cx.rec
                    .glyph("Minimum glyph", g, s, 400, theme.colors.text_secondary);
            }
            if let Some(s) = &self.max_glyph {
                cx.rec.glyph(
                    "Maximum glyph",
                    Rect::new(
                        b.right() - metrics::GLYPH,
                        y,
                        metrics::GLYPH,
                        metrics::GLYPH,
                    ),
                    s,
                    400,
                    theme.colors.text_secondary,
                );
            }
        }
        self.bed(cx, prefix, o);
        if self.indicator {
            return;
        }
        let thumb = self.thumb_rect().translate(o);
        if !self.core.dragged {
            let look = ThumbLook {
                pressed: self.core.pressed,
                hovered: cx.is_hovered() && !cx.is_disabled(),
                focused: cx.focus_visible() && !cx.is_disabled(),
            };
            paint_thumb(cx, &format!("{prefix}Thumb/"), thumb, look);
        }
        if self.core.shows_lens() || self.core.dragged {
            let lens = self.lens_rect(self.core.lens_growth()).translate(o);
            let c = self.thumb_rect().translate(o).center();
            let core = Rect::new(
                c.x - metrics::THUMB / 2.0,
                c.y - metrics::THUMB / 2.0,
                metrics::THUMB,
                metrics::THUMB,
            );
            paint_lens(
                cx,
                &format!("{prefix}Lens thumb/"),
                lens,
                core,
                self.core.lens_opacity(),
            );
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        if self.indicator {
            return Handled::No;
        }
        let b = cx.bounds();
        let o = Point::new(b.x + self.glyph_offset(), b.y);
        let local = |p: &Point| Point::new(p.x - o.x, p.y - o.y);
        let reduce = reduce_motion(cx);
        match event {
            Event::PointerDown {
                pos,
                button: Button::Left,
                ..
            } => {
                let p = local(pos);
                self.core.active = true;
                self.core.pressed = true;
                self.core.down_at = p;
                cx.emit(
                    Signal::EditingBegan,
                    Value::Float(f64::from(self.core.value)),
                );
                let from_thumb = Point::new(
                    p.x - self.thumb_rect().center().x,
                    p.y - self.thumb_rect().center().y,
                );
                if from_thumb.length() <= metrics::GRAB_RADIUS {
                    self.core.grab = from_thumb;
                } else {
                    self.core.grab = Point::ZERO;
                    let f = self.fraction_at(p);
                    self.core.emit(cx, f);
                }
                cx.request_paint();
                Handled::Yes
            }
            Event::PointerMove {
                pos,
                dragging: true,
                ..
            } if self.core.active => {
                let p = local(pos);
                if !self.core.dragged
                    && Point::new(p.x - self.core.down_at.x, p.y - self.core.down_at.y).length()
                        >= metrics::DRAG_SLOP
                {
                    self.core.start_drag(reduce);
                    cx.animate();
                }
                let g = self.core.grab;
                let f = self.fraction_at(Point::new(p.x - g.x, p.y - g.y));
                self.core.emit(cx, f);
                Handled::Yes
            }
            Event::PointerUp {
                button: Button::Left,
                ..
            } if self.core.active => {
                self.core.end(cx, reduce);
                Handled::Yes
            }
            Event::Key(k) if k.pressed => {
                let key = k.key.clone();
                self.core.handle_key(cx, &key)
            }
            _ => Handled::No,
        }
    }

    fn tick(&mut self, _cx: &mut EventCx<'_>, dt: f32) -> bool {
        self.core.tick(dt)
    }

    fn interaction(&self) -> Interaction {
        Interaction::Hover
    }

    fn focus(&self) -> FocusKind {
        if self.indicator {
            FocusKind::None
        } else {
            FocusKind::Control
        }
    }

    fn hit(&self, p: Point, size: Size) -> bool {
        !self.indicator && Rect::from_size(size).contains(p)
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match prop {
            Prop::TickMarks => match value.as_int() {
                Some(n) if n >= 2 => {
                    self.core.divisions = u32::try_from(n - 1).ok();
                    self.tick_marks = true;
                    PropChange::Layout
                }
                Some(_) => {
                    self.tick_marks = false;
                    PropChange::Layout
                }
                None => PropChange::Unknown,
            },
            Prop::Vertical => match value.as_bool() {
                Some(v) => {
                    self.vertical = v;
                    PropChange::Layout
                }
                None => PropChange::Unknown,
            },
            Prop::ItemSymbols => match value.as_str_list() {
                Some(l) => {
                    self.min_glyph = l.first().filter(|s| !s.is_empty()).cloned();
                    self.max_glyph = l.get(1).filter(|s| !s.is_empty()).cloned();
                    PropChange::Layout
                }
                None => PropChange::Unknown,
            },
            other => self.core.set_common(other, value),
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::Vertical => Some(Value::Bool(self.vertical)),
            Prop::TickMarks => Some(Value::Int(self.ticks_count() as i64)),
            other => self.core.get_common(other),
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        self.core.a11y(a);
    }

    fn dims_when_disabled(&self) -> bool {
        !self.indicator
    }
}

// ---------------------------------------------------------------------------------------
// The dial.
// ---------------------------------------------------------------------------------------

/// A circular slider: a 56 pt dial with a 4 pt ring, the value drawn as an arc from the top
/// clockwise and the round thumb on the ring at the value.
#[derive(Debug)]
pub struct Dial {
    core: Core,
}

impl Dial {
    /// A dial from 0 to 1 at 0.
    pub fn new() -> Dial {
        Dial { core: Core::new() }
    }

    /// With a value.
    pub fn value(mut self, v: f32) -> Dial {
        self.core.value = v;
        self
    }

    fn centre() -> Point {
        Point::new(metrics::DIAL / 2.0, metrics::DIAL / 2.0)
    }

    fn thumb(f: f32) -> Rect {
        let a = f * std::f32::consts::TAU;
        let c = Dial::centre();
        let (x, y) = (
            c.x + a.sin() * metrics::DIAL_ORBIT,
            c.y - a.cos() * metrics::DIAL_ORBIT,
        );
        Rect::new(
            (x - metrics::THUMB / 2.0).round(),
            (y - metrics::THUMB / 2.0).round(),
            metrics::THUMB,
            metrics::THUMB,
        )
    }

    fn fraction_at(&self, p: Point) -> Option<f32> {
        let c = Dial::centre();
        let (vx, vy) = (p.x - c.x, p.y - c.y);
        if vx.hypot(vy) < 1.0 {
            return None;
        }
        let mut f = vx.atan2(-vy) / std::f32::consts::TAU;
        if f < 0.0 {
            f += 1.0;
        }
        // While dragging, no wrap across the top: past it, stay at the end the value is at.
        let now = self.core.fraction();
        if self.core.dragged && (f - now).abs() > 0.5 {
            f = if now > 0.5 { 1.0 } else { 0.0 };
        }
        Some(f)
    }
}

impl Default for Dial {
    fn default() -> Self {
        Dial::new()
    }
}

impl Widget for Dial {
    fn kind(&self) -> &'static str {
        "Dial"
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        Size::new(metrics::DIAL, metrics::DIAL)
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme: Theme = *cx.theme();
        let c = theme.colors;
        let b = cx.bounds();
        let o = Point::new(b.x, b.y);
        let ring = Rect::new(
            (metrics::DIAL - metrics::DIAL_TRACK) / 2.0,
            (metrics::DIAL - metrics::DIAL_TRACK) / 2.0,
            metrics::DIAL_TRACK,
            metrics::DIAL_TRACK,
        );
        let f = self.core.fraction();
        cx.rec.layer(
            "Track",
            ring.translate(o),
            Shape::Ellipse,
            Style::default().with_border(Border::at(
                c.surface_sunken,
                metrics::TRACK,
                BorderPosition::Center,
            )),
        );
        if f > 0.0 {
            let mut border = Border::at(c.accent_amalfi, metrics::TRACK, BorderPosition::Center);
            border.round_ends = true;
            if f >= 1.0 {
                cx.rec.layer(
                    "Progress",
                    ring.translate(o),
                    Shape::Ellipse,
                    Style::default().with_border(border),
                );
            } else {
                let mut pb = PathBuilder::new();
                pb.add_arc(ring.to_sk(), -90.0, f * 360.0);
                let path = pb.detach();
                let tight = path.compute_tight_bounds();
                let r = Rect::new(tight.left, tight.top, tight.width(), tight.height());
                let local = path.make_offset((-r.x, -r.y));
                cx.rec.layer(
                    "Progress",
                    r.translate(o),
                    Shape::Path(local),
                    Style::default().with_border(border),
                );
            }
        }
        let thumb = Dial::thumb(f).translate(o);
        if !self.core.dragged {
            let look = ThumbLook {
                pressed: self.core.pressed,
                hovered: cx.is_hovered() && !cx.is_disabled(),
                focused: cx.focus_visible() && !cx.is_disabled(),
            };
            paint_thumb(cx, "Thumb/", thumb, look);
        }
        if self.core.shows_lens() || self.core.dragged {
            let g = self.core.lens_growth();
            let c = thumb.center();
            let (w, h) = (
                metrics::THUMB + (metrics::LENS_LONG - metrics::THUMB) * g,
                metrics::THUMB + (metrics::LENS_SHORT - metrics::THUMB) * g,
            );
            let lens = whole(Rect::new(c.x - w / 2.0, c.y - h / 2.0, w, h));
            let core = Rect::new(
                c.x - metrics::THUMB / 2.0,
                c.y - metrics::THUMB / 2.0,
                metrics::THUMB,
                metrics::THUMB,
            );
            paint_lens(cx, "Lens thumb/", lens, core, self.core.lens_opacity());
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        let b = cx.bounds();
        let local = |p: &Point| Point::new(p.x - b.x, p.y - b.y);
        let reduce = reduce_motion(cx);
        match event {
            Event::PointerDown {
                pos,
                button: Button::Left,
                ..
            } => {
                let p = local(pos);
                self.core.active = true;
                self.core.pressed = true;
                self.core.down_at = p;
                cx.emit(
                    Signal::EditingBegan,
                    Value::Float(f64::from(self.core.value)),
                );
                let t = Dial::thumb(self.core.fraction()).center();
                let from_thumb = Point::new(p.x - t.x, p.y - t.y);
                if from_thumb.length() <= metrics::GRAB_RADIUS {
                    self.core.grab = from_thumb;
                } else {
                    self.core.grab = Point::ZERO;
                    if let Some(f) = self.fraction_at(p) {
                        self.core.emit(cx, f);
                    }
                }
                cx.request_paint();
                Handled::Yes
            }
            Event::PointerMove {
                pos,
                dragging: true,
                ..
            } if self.core.active => {
                let p = local(pos);
                if !self.core.dragged
                    && Point::new(p.x - self.core.down_at.x, p.y - self.core.down_at.y).length()
                        >= metrics::DRAG_SLOP
                {
                    self.core.start_drag(reduce);
                    cx.animate();
                }
                let g = self.core.grab;
                if let Some(f) = self.fraction_at(Point::new(p.x - g.x, p.y - g.y)) {
                    self.core.emit(cx, f);
                }
                Handled::Yes
            }
            Event::PointerUp {
                button: Button::Left,
                ..
            } if self.core.active => {
                self.core.end(cx, reduce);
                Handled::Yes
            }
            Event::Key(k) if k.pressed => {
                let key = k.key.clone();
                self.core.handle_key(cx, &key)
            }
            _ => Handled::No,
        }
    }

    fn tick(&mut self, _cx: &mut EventCx<'_>, dt: f32) -> bool {
        self.core.tick(dt)
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
        self.core.set_common(prop, value)
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        self.core.get_common(prop)
    }

    fn accessibility(&self, a: &mut A11y) {
        self.core.a11y(a);
    }
}

// ---------------------------------------------------------------------------------------
// The thumbs as views of their own (the kit has masters for them).
// ---------------------------------------------------------------------------------------

/// A round thumb on its own (`Sliders/Thumb/Round`).
#[derive(Debug, Default)]
pub struct SliderThumb {
    /// How it looks.
    pub look: ThumbLook,
}

impl Widget for SliderThumb {
    fn kind(&self) -> &'static str {
        "SliderThumb"
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        Size::new(metrics::THUMB, metrics::THUMB)
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let b = cx.bounds();
        paint_thumb(cx, "", b, self.look);
    }

    fn hit(&self, _p: Point, _size: Size) -> bool {
        false
    }
}

/// The lens thumb on its own (`Sliders/Thumb/Lens/Dragging`).
#[derive(Debug, Default)]
pub struct LensThumb;

impl Widget for LensThumb {
    fn kind(&self) -> &'static str {
        "LensThumb"
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        Size::new(metrics::LENS_LONG, metrics::LENS_SHORT)
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let b = cx.bounds();
        let core = Rect::new(
            b.x + (b.width - metrics::THUMB) / 2.0,
            b.y + (b.height - metrics::THUMB) / 2.0,
            metrics::THUMB,
            metrics::THUMB,
        );
        paint_lens(cx, "", b, core, 1.0);
    }

    fn hit(&self, _p: Point, _size: Size) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::event::{KeyEvent, Mods};
    use crate::core::ui::Ui;
    use crate::core::window::{WindowId, WindowKind};
    use std::cell::RefCell;
    use std::rc::Rc;

    type Log = Rc<RefCell<Vec<f64>>>;

    fn scene(w: impl Widget + 'static) -> (Ui, WindowId, crate::core::ViewId, Log) {
        let mut ui = Ui::new_headless();
        let win = ui.create_window(WindowKind::Normal, Size::new(300.0, 200.0));
        let id = ui.create(w);
        let log: Log = Rc::new(RefCell::new(Vec::new()));
        let l = log.clone();
        ui.connect(id, Signal::ValueChanged, move |_, _, v| {
            l.borrow_mut().push(v.as_f64().unwrap_or(f64::NAN));
        });
        let _ = ui.set(id, Prop::MaxWidth, Value::from(200.0));
        ui.set_content(win, id);
        ui.show_window(win);
        ui.prepare_frame(win);
        (ui, win, id, log)
    }

    fn at(ui: &Ui, id: crate::core::ViewId, x: f32, y: f32) -> Point {
        let r = ui.tree.window_rect(id);
        Point::new(r.x + x, r.y + y)
    }

    #[test]
    fn clicking_the_track_jumps_and_dragging_follows() {
        let (mut ui, win, id, log) = scene(Slider::new().value(0.0));
        // The track at 100 of 200: the centre of the thumb range is 10 + 90.
        let p = at(&ui, id, 100.0, 10.0);
        ui.pointer_move(win, p, Mods::empty());
        ui.pointer_down(win, p, Button::Left, Mods::empty());
        assert_eq!(log.borrow().len(), 1);
        assert!((log.borrow()[0] - 0.5).abs() < 0.01, "{:?}", log.borrow());
        let q = at(&ui, id, 190.0, 10.0);
        ui.pointer_move(win, q, Mods::empty());
        ui.pointer_up(win, q, Button::Left, Mods::empty());
        assert_eq!(ui.get(id, Prop::Value), Some(Value::Float(1.0)));
        for _ in 0..60 {
            ui.advance(0.05);
        }
        assert!(
            ui.widget::<Slider>(id)
                .is_some_and(|s| !s.core.shows_lens())
        );
    }

    #[test]
    fn steps_snap_and_keys_move_by_a_division() {
        let (mut ui, win, id, log) = scene(Slider::new().divisions(4).value(0.5));
        ui.set_focus(id, true);
        ui.key(win, KeyEvent::press(Key::Right, Mods::empty()));
        ui.key(win, KeyEvent::press(Key::Right, Mods::empty()));
        ui.key(win, KeyEvent::press(Key::End, Mods::empty()));
        ui.key(win, KeyEvent::press(Key::Home, Mods::empty()));
        assert_eq!(*log.borrow(), vec![0.75, 1.0, 0.0]);
    }

    #[test]
    fn dial_follows_the_angle() {
        let (mut ui, win, id, _log) = scene(Dial::new());
        // Straight right of the centre is a quarter turn.
        let p = at(&ui, id, 50.0, 28.0);
        ui.pointer_move(win, p, Mods::empty());
        ui.pointer_down(win, p, Button::Left, Mods::empty());
        ui.pointer_up(win, p, Button::Left, Mods::empty());
        let v = ui
            .get(id, Prop::Value)
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        assert!((v - 0.25).abs() < 0.01, "{v}");
    }

    #[test]
    fn disabled_ignores_input() {
        let (mut ui, win, id, log) = scene(Slider::new());
        let _ = ui.set(id, Prop::Disabled, Value::Bool(true));
        let p = at(&ui, id, 100.0, 10.0);
        ui.pointer_move(win, p, Mods::empty());
        ui.pointer_down(win, p, Button::Left, Mods::empty());
        ui.pointer_up(win, p, Button::Left, Mods::empty());
        assert!(log.borrow().is_empty());
    }
}
