//! Progress indicators (kit `Progress`): the bar (determinate or indeterminate), the
//! circular indicator and the spinner.

use skia_safe::PathBuilder;

use crate::amalfi::foundation::ControlSize;
use crate::amalfi::tokens;
use crate::core::a11y::{A11y, Role};
use crate::core::layout::Proposal;
use crate::core::value::{Prop, Value};
use crate::core::widget::{EventCx, LayoutCx, PaintCx, PropChange, Widget};
use crate::geom::{Point, Rect, Size, Transform};
use crate::shape::Shape;
use crate::style::{Border, BorderPosition, Style};

/// The three sizes of progress indicators (Mini is Small).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProgressSize {
    /// Small.
    Small,
    /// Regular.
    #[default]
    Regular,
    /// Large.
    Large,
}

impl ProgressSize {
    /// The progress size of a control size.
    pub fn of(size: ControlSize) -> ProgressSize {
        match size {
            ControlSize::Mini | ControlSize::Small => ProgressSize::Small,
            ControlSize::Regular => ProgressSize::Regular,
            ControlSize::Large => ProgressSize::Large,
        }
    }
}

/// Fixed measures (silica_ui's `AmalfiProgressBarMetrics` and friends).
pub mod metrics {
    use super::ProgressSize;
    use crate::geom::Rect;

    /// A bar's default length.
    pub const LENGTH: f32 = 200.0;
    /// An indeterminate segment's length.
    pub const SEGMENT: f32 = 72.0;
    /// Seconds for the segment to cross the bar.
    pub const SWEEP: f32 = 1.2;
    /// Seconds for the spinner to turn once.
    pub const TURN: f32 = 1.0;
    /// Number of spokes.
    pub const SPOKES: usize = 12;
    /// How much fainter each spoke is than the one before.
    pub const FADE: f32 = 0.075;

    /// A bar's thickness.
    pub fn thickness(size: ProgressSize) -> f32 {
        if size == ProgressSize::Small {
            4.0
        } else {
            8.0
        }
    }

    /// A bar's corner radius.
    pub fn radius(size: ProgressSize) -> f32 {
        match size {
            ProgressSize::Small => 2.0,
            ProgressSize::Regular => 3.0,
            ProgressSize::Large => 4.0,
        }
    }

    /// The spinner's side.
    pub fn spinner_side(size: ProgressSize) -> f32 {
        match size {
            ProgressSize::Small => 16.0,
            ProgressSize::Regular => 24.0,
            ProgressSize::Large => 32.0,
        }
    }

    /// A spoke's frame in the spinner.
    pub fn spoke(size: ProgressSize) -> Rect {
        let side = spinner_side(size);
        let (w, h) = if size == ProgressSize::Small {
            (2.0, 4.0)
        } else {
            (4.0, 8.0)
        };
        Rect::new((side - w) / 2.0, 0.0, w, h)
    }

    /// A spoke's corner radius.
    pub fn spoke_radius(size: ProgressSize) -> f32 {
        match size {
            ProgressSize::Small => 1.0,
            ProgressSize::Regular => 1.02,
            ProgressSize::Large => 1.36,
        }
    }

    /// The circular indicator's side.
    pub fn circle_side(size: ProgressSize) -> f32 {
        match size {
            ProgressSize::Small => 16.0,
            ProgressSize::Regular => 24.0,
            ProgressSize::Large => 40.0,
        }
    }

    /// The circular indicator's ring.
    pub fn ring(size: ProgressSize) -> Rect {
        match size {
            ProgressSize::Small => Rect::new(1.0, 1.0, 14.0, 14.0),
            ProgressSize::Regular => Rect::new(2.0, 2.0, 20.0, 20.0),
            ProgressSize::Large => Rect::new(2.0, 2.0, 36.0, 36.0),
        }
    }

    /// The circular indicator's stroke.
    pub fn stroke(size: ProgressSize) -> f32 {
        match size {
            ProgressSize::Small => 2.0,
            ProgressSize::Regular => 3.0,
            ProgressSize::Large => 4.0,
        }
    }
}

/// The part of an indeterminate bar's track the segment covers at `phase` (0: entering, 1:
/// gone), clipped to the track.
pub fn segment_at(phase: f32, length: f32) -> Option<(f32, f32)> {
    let p = if phase.is_finite() {
        phase.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let x = (-metrics::SEGMENT + p * (length + metrics::SEGMENT)).round();
    let start = x.max(0.0);
    let end = (x + metrics::SEGMENT).min(length);
    (end - start > 0.0).then_some((start, end))
}

fn reduce_motion(cx: &PaintCx<'_>) -> bool {
    cx.theme().appearance.reduce_motion
}

/// A progress bar: determinate with a `value` from 0 to 1, or indeterminate (`value` is
/// `None`) with a segment sweeping across the track.
#[derive(Debug)]
pub struct ProgressBar {
    /// The done part, 0-1 (`None`: indeterminate).
    pub value: Option<f32>,
    /// Size.
    pub size: ProgressSize,
    /// A fixed phase of the segment (tests, still images); `None` animates it.
    pub phase: Option<f32>,
    length: f32,
    t: f32,
}

impl ProgressBar {
    /// An indeterminate regular bar.
    pub fn new() -> ProgressBar {
        ProgressBar {
            value: None,
            size: ProgressSize::Regular,
            phase: None,
            length: metrics::LENGTH,
            t: 0.0,
        }
    }

    /// Determinate at `v`.
    pub fn value(mut self, v: f32) -> ProgressBar {
        self.value = Some(v);
        self
    }

    /// With a size.
    pub fn size(mut self, size: ProgressSize) -> ProgressBar {
        self.size = size;
        self
    }

    /// With the segment held at a phase.
    pub fn phase(mut self, phase: f32) -> ProgressBar {
        self.phase = Some(phase);
        self
    }

    fn animates(&self, disabled: bool, reduce: bool) -> bool {
        self.value.is_none() && self.phase.is_none() && !disabled && !reduce
    }
}

impl Default for ProgressBar {
    fn default() -> Self {
        ProgressBar::new()
    }
}

impl Widget for ProgressBar {
    fn kind(&self) -> &'static str {
        "ProgressBar"
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        self.length = match p.width {
            Some(w) if w.is_finite() => w.max(0.0),
            _ => metrics::LENGTH,
        };
        Size::new(self.length, metrics::thickness(self.size))
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let c = theme.colors;
        let b = cx.bounds();
        let length = if b.width > 0.0 { b.width } else { self.length };
        let thick = metrics::thickness(self.size);
        let radius = metrics::radius(self.size);
        let disabled = cx.is_disabled();
        cx.rec.layer(
            "Track",
            Rect::new(b.x, b.y, length, thick),
            Shape::rounded(radius),
            Style::fill(c.surface_sunken).with_border(Border::inside(c.line_separator, 1.0)),
        );
        let (name, part, color) = match self.value {
            None => {
                let phase = match self.phase {
                    Some(p) => p,
                    None if self.animates(disabled, reduce_motion(cx)) => {
                        tokens::motion::EASE_IN_OUT.transform(self.t / metrics::SWEEP)
                    }
                    None => 0.5,
                };
                // Disabled, the segment rests at the start of the track.
                let part = if disabled {
                    Some((0.0, metrics::SEGMENT.min(length)))
                } else {
                    segment_at(phase, length)
                };
                ("Segment", part, c.accent_amalfi)
            }
            Some(v) => {
                let v = if v.is_finite() {
                    v.clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let end = ((v * length) / 4.0).round() * 4.0;
                let end = end.clamp(0.0, length);
                (
                    "Fill",
                    (end > 0.0).then_some((0.0, end)),
                    if v >= 1.0 {
                        c.palette_ulivo
                    } else {
                        c.accent_amalfi
                    },
                )
            }
        };
        if let Some((s, e)) = part {
            cx.rec.layer(
                name,
                Rect::new(b.x + s, b.y, e - s, thick),
                Shape::rounded(radius),
                Style::fill(color),
            );
        }
    }

    fn attached(&mut self, cx: &mut EventCx<'_>) {
        cx.animate();
    }

    fn tick(&mut self, cx: &mut EventCx<'_>, dt: f32) -> bool {
        let disabled = {
            let id = cx.id();
            cx.ui().is_disabled(id)
        };
        let reduce = cx.theme().appearance.reduce_motion;
        if !self.animates(disabled, reduce) {
            return false;
        }
        self.t = (self.t + dt) % metrics::SWEEP;
        cx.request_paint();
        true
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match prop {
            Prop::Value => match value {
                Value::None => {
                    self.value = None;
                    PropChange::Paint
                }
                v => v.as_f32().map_or(PropChange::Unknown, |f| {
                    self.value = Some(f);
                    PropChange::Paint
                }),
            },
            Prop::Indeterminate => match value.as_bool() {
                Some(true) => {
                    self.value = None;
                    PropChange::Paint
                }
                Some(false) => {
                    self.value.get_or_insert(0.0);
                    PropChange::Paint
                }
                None => PropChange::Unknown,
            },
            Prop::ControlSize => {
                value
                    .as_int()
                    .and_then(ControlSize::from_int)
                    .map_or(PropChange::Unknown, |s| {
                        self.size = ProgressSize::of(s);
                        PropChange::Layout
                    })
            }
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::Value => Some(self.value.map_or(Value::None, Value::from)),
            Prop::Indeterminate => Some(Value::Bool(self.value.is_none())),
            _ => None,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::ProgressIndicator;
        a.value = self
            .value
            .filter(|v| v.is_finite())
            .map(|v| format!("{}%", (v.clamp(0.0, 1.0) * 100.0).round()));
        a.numeric = self.value.map(|v| (f64::from(v), 0.0, 1.0, 0.0));
    }

    fn hit(&self, _p: Point, _size: Size) -> bool {
        false
    }
}

/// A circular progress indicator: the part done is an arc from the top clockwise with round
/// ends.
#[derive(Debug)]
pub struct CircularProgress {
    /// The done part, 0-1.
    pub value: f32,
    /// Size.
    pub size: ProgressSize,
}

impl CircularProgress {
    /// A regular indicator at `value`.
    pub fn new(value: f32) -> CircularProgress {
        CircularProgress {
            value,
            size: ProgressSize::Regular,
        }
    }

    /// With a size.
    pub fn size(mut self, size: ProgressSize) -> CircularProgress {
        self.size = size;
        self
    }
}

impl Widget for CircularProgress {
    fn kind(&self) -> &'static str {
        "CircularProgress"
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        let s = metrics::circle_side(self.size);
        Size::new(s, s)
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let c = theme.colors;
        let b = cx.bounds();
        let o = Point::new(b.x, b.y);
        let ring = metrics::ring(self.size);
        let stroke = metrics::stroke(self.size);
        let v = if self.value.is_finite() {
            self.value.clamp(0.0, 1.0)
        } else {
            0.0
        };
        cx.rec.layer(
            "Track",
            ring.translate(o),
            Shape::Ellipse,
            Style::default().with_border(Border::at(
                c.surface_sunken,
                stroke,
                BorderPosition::Center,
            )),
        );
        if v <= 0.0 {
            return;
        }
        let mut border = Border::at(
            if v >= 1.0 {
                c.palette_ulivo
            } else {
                c.accent_amalfi
            },
            stroke,
            BorderPosition::Center,
        );
        border.round_ends = true;
        if v >= 1.0 {
            cx.rec.layer(
                "Progress",
                ring.translate(o),
                Shape::Ellipse,
                Style::default().with_border(border),
            );
            return;
        }
        let mut pb = PathBuilder::new();
        pb.add_arc(ring.to_sk(), -90.0, v * 360.0);
        let path = pb.detach();
        let t = path.compute_tight_bounds();
        let r = Rect::new(t.left, t.top, t.width(), t.height());
        cx.rec.layer(
            "Progress",
            r.translate(o),
            Shape::Path(path.make_offset((-r.x, -r.y))),
            Style::default().with_border(border),
        );
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match prop {
            Prop::Value => value.as_f32().map_or(PropChange::Unknown, |f| {
                self.value = f;
                PropChange::Paint
            }),
            Prop::ControlSize => {
                value
                    .as_int()
                    .and_then(ControlSize::from_int)
                    .map_or(PropChange::Unknown, |s| {
                        self.size = ProgressSize::of(s);
                        PropChange::Layout
                    })
            }
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::Value => Some(Value::from(self.value)),
            _ => None,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::ProgressIndicator;
        a.value = Some(format!("{}%", (self.value.clamp(0.0, 1.0) * 100.0).round()));
        a.numeric = Some((f64::from(self.value), 0.0, 1.0, 0.0));
    }

    fn hit(&self, _p: Point, _size: Size) -> bool {
        false
    }
}

/// The spinner: twelve spokes around the centre, the bright one stepping clockwise.
#[derive(Debug)]
pub struct Spinner {
    /// Size.
    pub size: ProgressSize,
    t: f32,
}

impl Spinner {
    /// A regular spinner.
    pub fn new() -> Spinner {
        Spinner {
            size: ProgressSize::Regular,
            t: 0.0,
        }
    }

    /// With a size.
    pub fn size(mut self, size: ProgressSize) -> Spinner {
        self.size = size;
        self
    }

    fn head(&self) -> usize {
        ((self.t / metrics::TURN) * metrics::SPOKES as f32).floor() as usize % metrics::SPOKES
    }
}

impl Default for Spinner {
    fn default() -> Self {
        Spinner::new()
    }
}

impl Widget for Spinner {
    fn kind(&self) -> &'static str {
        "Spinner"
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        let s = metrics::spinner_side(self.size);
        Size::new(s, s)
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let b = cx.bounds();
        let side = metrics::spinner_side(self.size);
        let spoke = metrics::spoke(self.size);
        let centre = Point::new(b.x + side / 2.0, b.y + side / 2.0);
        let head = if cx.is_disabled() || reduce_motion(cx) {
            0
        } else {
            self.head()
        };
        let n = metrics::SPOKES;
        for k in 1..=n {
            // Spoke k of the kit stands (k - 1) steps counter-clockwise from the top; its
            // age counts back from the head, which steps clockwise.
            let position = (n + 1 - k) % n;
            let age = (head + n - position) % n;
            let opacity = 1.0 - metrics::FADE * age as f32;
            let angle = -((k - 1) as f32) * std::f32::consts::TAU / n as f32;
            cx.rec.push_transform(
                Transform::translate(-centre.x, -centre.y)
                    .then(Transform::rotate(angle))
                    .then(Transform::translate(centre.x, centre.y)),
            );
            cx.rec.layer(
                &format!("Spoke {k}/Tick"),
                Rect::new(b.x + spoke.x, b.y + spoke.y, spoke.width, spoke.height),
                Shape::rounded(metrics::spoke_radius(self.size)),
                Style::fill(theme.colors.text_secondary).with_opacity(opacity),
            );
            cx.rec.pop();
        }
    }

    fn attached(&mut self, cx: &mut EventCx<'_>) {
        cx.animate();
    }

    fn tick(&mut self, cx: &mut EventCx<'_>, dt: f32) -> bool {
        let id = cx.id();
        if cx.ui().is_disabled(id) || cx.theme().appearance.reduce_motion {
            return false;
        }
        let before = self.head();
        self.t = (self.t + dt) % metrics::TURN;
        if self.head() != before {
            cx.request_paint();
        }
        true
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match prop {
            Prop::ControlSize => {
                value
                    .as_int()
                    .and_then(ControlSize::from_int)
                    .map_or(PropChange::Unknown, |s| {
                        self.size = ProgressSize::of(s);
                        PropChange::Layout
                    })
            }
            _ => PropChange::Unknown,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::ProgressIndicator;
        a.live = true;
    }

    fn hit(&self, _p: Point, _size: Size) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ui::Ui;
    use crate::core::window::WindowKind;

    #[test]
    fn segment_enters_crosses_and_leaves() {
        assert_eq!(segment_at(0.0, 200.0), None);
        assert_eq!(segment_at(0.5, 200.0), Some((64.0, 136.0)));
        assert_eq!(segment_at(0.75, 200.0), Some((132.0, 200.0)));
        assert_eq!(segment_at(1.0, 200.0), None);
    }

    #[test]
    fn determinate_fill_snaps_to_the_grid() {
        let mut ui = Ui::new_headless();
        let win = ui.create_window(WindowKind::Normal, Size::new(300.0, 100.0));
        let bar = ui.create(ProgressBar::new().value(0.25));
        let _ = ui.set(bar, Prop::MaxWidth, Value::from(200.0));
        ui.set_content(win, bar);
        ui.show_window(win);
        ui.prepare_frame(win);
        let layers = ui.inspect_layers(bar);
        let fill = layers.iter().find(|l| l.path == "Fill").map(|l| l.frame);
        assert_eq!(fill.map(|f| f.width), Some(52.0));
    }

    #[test]
    fn the_spinner_steps_its_head() {
        let mut ui = Ui::new_headless();
        let win = ui.create_window(WindowKind::Normal, Size::new(100.0, 100.0));
        let s = ui.create(Spinner::new());
        ui.set_content(win, s);
        ui.show_window(win);
        ui.prepare_frame(win);
        assert!(ui.is_animating());
        for _ in 0..7 {
            ui.advance(0.05);
        }
        assert!(ui.widget::<Spinner>(s).is_some_and(|s| s.head() >= 3));
        let _ = ui.set(s, Prop::Disabled, Value::Bool(true));
        ui.advance(0.05);
        ui.advance(0.05);
        assert!(!ui.is_animating());
    }

    #[test]
    fn an_indeterminate_bar_animates_until_it_gets_a_value() {
        let mut ui = Ui::new_headless();
        let win = ui.create_window(WindowKind::Normal, Size::new(300.0, 100.0));
        let bar = ui.create(ProgressBar::new());
        ui.set_content(win, bar);
        ui.show_window(win);
        ui.prepare_frame(win);
        ui.advance(0.1);
        assert!(ui.is_animating());
        let _ = ui.set(bar, Prop::Value, Value::from(0.5));
        ui.advance(0.1);
        ui.advance(0.1);
        assert!(!ui.is_animating());
    }
}
