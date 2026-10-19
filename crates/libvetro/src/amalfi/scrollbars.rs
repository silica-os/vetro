//! Scrollbars (kit `Scrollbars`): the bar (overlay or legacy), the corner, and the scroll view
//! that owns its content, its offset and its two bars.

use crate::amalfi::tokens;
use crate::color::Color;
use crate::core::a11y::{A11y, Action, Role};
use crate::core::event::{Button, Event, Handled};
use crate::core::layout::{Axis, Proposal};
use crate::core::ui::Ui;
use crate::core::value::{Prop, Signal, Value};
use crate::core::widget::{EventCx, Interaction, LayoutCx, PaintCx, PropChange, Widget};
use crate::geom::{Point, Rect, Size};
use crate::motion::{Animated, Timing};
use crate::shape::Shape;
use crate::style::{Border, Style};

/// Fixed measures (silica_ui's `AmalfiScrollbarMetrics`).
pub mod metrics {
    /// The thin overlay thumb.
    pub const THIN_THUMB: f32 = 4.0;
    /// The thumb of an expanded overlay bar and of a legacy bar.
    pub const THUMB: f32 = 8.0;
    /// The shortest thumb.
    pub const MIN_THUMB: f32 = 24.0;
    /// The corner's side.
    pub const CORNER: f32 = 16.0;
    /// Seconds an overlay bar stays after the scrolling stops (an assumption: the reference
    /// does not measure it).
    pub const HIDE_DELAY: f64 = 1.0;
}

/// The two kinds of scrollbar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScrollbarStyle {
    /// 12 pt, floating over the content, shown while scrolling.
    #[default]
    Overlay,
    /// 16 pt, beside the content, always there.
    Legacy,
}

impl ScrollbarStyle {
    /// The bar's thickness.
    pub fn thickness(self) -> f32 {
        match self {
            ScrollbarStyle::Overlay => 12.0,
            ScrollbarStyle::Legacy => 16.0,
        }
    }
}

/// How a bar looks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BarLook {
    /// The axis it scrolls.
    pub axis: Axis,
    /// The kind.
    pub style: ScrollbarStyle,
    /// How far an overlay bar has expanded (track and wider thumb), 0-1.
    pub expansion: f32,
    /// The pointer is over a legacy bar.
    pub hovered: bool,
    /// The thumb is being dragged.
    pub dragging: bool,
    /// The thumb's offset and extent along the bar.
    pub thumb: Option<(f32, f32)>,
}

/// Records a bar at `rect` (`Track`, `Thumb` under `prefix`).
pub fn paint_bar(cx: &mut PaintCx<'_>, prefix: &str, rect: Rect, look: &BarLook) {
    let c = cx.theme().colors;
    let overlay = look.style == ScrollbarStyle::Overlay;
    let t = if overlay {
        look.expansion.clamp(0.0, 1.0)
    } else {
        1.0
    };
    let thickness = look.style.thickness();
    let thumb_t = if overlay {
        metrics::THIN_THUMB + (metrics::THUMB - metrics::THIN_THUMB) * t
    } else {
        metrics::THUMB
    };
    let inset = ((thickness - thumb_t) / 2.0).round();
    if !overlay || t > 0.0 {
        let radius = if overlay {
            Shape::rounded(thickness / 2.0)
        } else {
            Shape::RECT
        };
        let mut style =
            Style::fill(c.surface_sunken).with_border(Border::inside(c.line_separator, 1.0));
        if overlay {
            style.opacity = t;
        }
        cx.rec.layer(&format!("{prefix}Track"), rect, radius, style);
    }
    let Some((offset, extent)) = look.thumb.filter(|(_, e)| *e > 0.0) else {
        return;
    };
    let color: Color = if look.dragging {
        c.text_primary
    } else if overlay {
        c.text_tertiary.lerp(c.text_secondary, t)
    } else if look.hovered {
        c.text_secondary
    } else {
        c.text_tertiary
    };
    let (o, e) = (offset.round(), extent.round());
    let r = if look.axis == Axis::Vertical {
        Rect::new(rect.x + inset, rect.y + o, thumb_t, e)
    } else {
        Rect::new(rect.x + o, rect.y + inset, e, thumb_t)
    };
    cx.rec.layer(
        &format!("{prefix}Thumb"),
        r,
        Shape::rounded(thumb_t / 2.0),
        Style::fill(color),
    );
}

/// A scrollbar on its own: what the kit's bar masters show. Give it a [`BarLook`].
#[derive(Debug)]
pub struct Scrollbar {
    /// The look.
    pub look: BarLook,
    /// The bar's length.
    pub length: f32,
}

impl Scrollbar {
    /// A vertical overlay bar `length` long.
    pub fn new(length: f32) -> Scrollbar {
        Scrollbar {
            look: BarLook {
                axis: Axis::Vertical,
                style: ScrollbarStyle::Overlay,
                expansion: 0.0,
                hovered: false,
                dragging: false,
                thumb: None,
            },
            length,
        }
    }
}

impl Widget for Scrollbar {
    fn kind(&self) -> &'static str {
        "Scrollbar"
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        let t = self.look.style.thickness();
        match self.look.axis {
            Axis::Vertical => Size::new(t, self.length),
            Axis::Horizontal => Size::new(self.length, t),
        }
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let b = cx.bounds();
        let look = self.look;
        paint_bar(cx, "", b, &look);
    }

    fn hit(&self, _p: Point, _size: Size) -> bool {
        false
    }
}

/// The square where legacy bars meet.
#[derive(Debug, Default)]
pub struct ScrollbarCorner;

impl Widget for ScrollbarCorner {
    fn kind(&self) -> &'static str {
        "ScrollbarCorner"
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        Size::new(metrics::CORNER, metrics::CORNER)
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        paint_corner(cx, "", cx.bounds());
    }

    fn hit(&self, _p: Point, _size: Size) -> bool {
        false
    }
}

fn paint_corner(cx: &mut PaintCx<'_>, prefix: &str, rect: Rect) {
    let c = cx.theme().colors;
    cx.rec.layer(
        &format!("{prefix}Corner"),
        rect,
        Shape::RECT,
        Style::fill(c.surface_sunken).with_border(Border::inside(c.line_separator, 1.0)),
    );
}

/// Keeps a bar visible whatever the pointer does (a gallery of the states, the kit's masters).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Hold {
    /// Follow the pointer and the scrolling.
    #[default]
    None,
    /// Shown thin (overlay), the thumb under the pointer (legacy).
    Thin,
    /// Shown expanded.
    Expanded,
}

/// A scroll view: its first child is the content, which scrolls in both directions under two
/// bars. Overlay bars float over the content and fade out after the scrolling stops; legacy
/// bars sit beside it with the corner between them. `Signal::Scrolled` carries the offset.
#[derive(Debug)]
pub struct ScrollView {
    /// The kind of bars.
    pub style: ScrollbarStyle,
    /// The vertical bar exists.
    pub vertical: bool,
    /// The horizontal bar exists.
    pub horizontal: bool,
    /// Keeps the bars in a state.
    pub hold: Hold,
    offset: Point,
    content: Size,
    viewport: Size,
    shown: Animated<f32>,
    expanded: Animated<f32>,
    hovered: Option<Axis>,
    drag: Option<(Axis, f32, f32)>,
    last_scroll: f64,
    waiting: bool,
}

impl ScrollView {
    /// A scroll view with overlay bars.
    pub fn new() -> ScrollView {
        ScrollView {
            style: ScrollbarStyle::Overlay,
            vertical: true,
            horizontal: true,
            hold: Hold::None,
            offset: Point::ZERO,
            content: Size::ZERO,
            viewport: Size::ZERO,
            shown: Animated::new(0.0),
            expanded: Animated::new(0.0),
            hovered: None,
            drag: None,
            last_scroll: f64::NEG_INFINITY,
            waiting: false,
        }
    }

    /// With a kind of bars.
    pub fn style(mut self, style: ScrollbarStyle) -> ScrollView {
        self.style = style;
        self
    }

    /// With the bars held in a state.
    pub fn held(mut self, hold: Hold) -> ScrollView {
        self.hold = hold;
        self
    }

    /// With an offset.
    pub fn at(mut self, x: f32, y: f32) -> ScrollView {
        self.offset = Point::new(x, y);
        self
    }

    /// The offset.
    pub fn offset(&self) -> Point {
        self.offset
    }

    fn thickness(&self) -> f32 {
        self.style.thickness()
    }

    /// The area the content is seen in, for a frame of `frame`.
    fn viewport_for(&self, frame: Size) -> Size {
        if self.style == ScrollbarStyle::Legacy {
            let t = self.thickness();
            Size::new(
                (frame.width - if self.vertical { t } else { 0.0 }).max(0.0),
                (frame.height - if self.horizontal { t } else { 0.0 }).max(0.0),
            )
        } else {
            frame
        }
    }

    fn max_offset(&self) -> Point {
        Point::new(
            (self.content.width - self.viewport.width).max(0.0),
            (self.content.height - self.viewport.height).max(0.0),
        )
    }

    fn clamp(&self, p: Point) -> Point {
        let m = self.max_offset();
        Point::new(
            if p.x.is_finite() {
                p.x.clamp(0.0, m.x)
            } else {
                0.0
            },
            if p.y.is_finite() {
                p.y.clamp(0.0, m.y)
            } else {
                0.0
            },
        )
    }

    /// The rectangles of the vertical and the horizontal bar, in the view's coordinates.
    fn bar_rects(&self, frame: Size) -> (Rect, Rect) {
        let t = self.thickness();
        match self.style {
            ScrollbarStyle::Legacy => (
                Rect::new(frame.width - t, 0.0, t, frame.height - t),
                Rect::new(0.0, frame.height - t, frame.width - t, t),
            ),
            ScrollbarStyle::Overlay => (
                Rect::new(frame.width - t, 0.0, t, frame.height - t),
                Rect::new(0.0, frame.height - t, frame.width - t, t),
            ),
        }
    }

    fn thumb(&self, axis: Axis, length: f32) -> Option<(f32, f32)> {
        let (content, view, offset) = match axis {
            Axis::Vertical => (self.content.height, self.viewport.height, self.offset.y),
            Axis::Horizontal => (self.content.width, self.viewport.width, self.offset.x),
        };
        let range = content - view;
        if range <= 0.0 || length <= 0.0 || content <= 0.0 {
            return None;
        }
        let extent = (length * view / content).clamp(metrics::MIN_THUMB.min(length), length);
        let f = (offset / range).clamp(0.0, 1.0);
        Some((f * (length - extent), extent))
    }

    fn fits(&self, axis: Axis) -> bool {
        match axis {
            Axis::Vertical => self.content.height <= self.viewport.height,
            Axis::Horizontal => self.content.width <= self.viewport.width,
        }
    }

    fn has(&self, axis: Axis) -> bool {
        match axis {
            Axis::Vertical => self.vertical,
            Axis::Horizontal => self.horizontal,
        }
    }

    fn held_now(&self) -> bool {
        self.hovered.is_some() || self.drag.is_some() || self.hold != Hold::None
    }

    fn sync_animation(&mut self, cx: &mut EventCx<'_>) {
        let reduce = cx.theme().appearance.reduce_motion;
        let quick = if reduce {
            Timing::None
        } else {
            Timing::curve(tokens::motion::FADE_QUICK_CURVE, tokens::motion::FADE_QUICK)
        };
        let expand = self.hovered.is_some() || self.drag.is_some() || self.hold == Hold::Expanded;
        self.expanded
            .animate_to(if expand { 1.0 } else { 0.0 }, quick);
        if self.held_now() {
            self.shown.animate_to(1.0, quick);
        }
        cx.request_paint();
        cx.animate();
    }

    fn set_offset(&mut self, cx: &mut EventCx<'_>, p: Point) {
        let p = self.clamp(p);
        if p == self.offset {
            return;
        }
        self.offset = p;
        self.last_scroll = cx.now();
        self.waiting = true;
        let reduce = cx.theme().appearance.reduce_motion;
        self.shown.animate_to(
            1.0,
            if reduce {
                Timing::None
            } else {
                Timing::curve(tokens::motion::FADE_QUICK_CURVE, tokens::motion::FADE_QUICK)
            },
        );
        cx.emit(Signal::Scrolled, Value::Point(p));
        cx.request_layout();
        cx.request_paint();
        cx.animate();
    }
}

impl Default for ScrollView {
    fn default() -> Self {
        ScrollView::new()
    }
}

impl Widget for ScrollView {
    fn kind(&self) -> &'static str {
        "ScrollView"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        let mut content = Size::ZERO;
        if let Some(c) = cx.children().first().copied() {
            content = cx.measure(c, Proposal::UNSPECIFIED);
        }
        self.content = content;
        p.replacing_unspecified(content)
    }

    fn place(&mut self, cx: &mut LayoutCx<'_>, size: Size) {
        self.viewport = self.viewport_for(size);
        self.offset = self.clamp(self.offset);
        if let Some(c) = cx.children().first().copied() {
            let s = cx.measure(c, Proposal::UNSPECIFIED);
            self.content = s;
            self.viewport = self.viewport_for(size);
            self.offset = self.clamp(self.offset);
            cx.place(
                c,
                Rect::new(-self.offset.x, -self.offset.y, s.width, s.height),
            );
        }
    }

    fn paint_over(&mut self, cx: &mut PaintCx<'_>) {
        let b = cx.bounds();
        let frame = b.size();
        let (v, h) = self.bar_rects(frame);
        let overlay = self.style == ScrollbarStyle::Overlay;
        let enabled = !cx.is_disabled();
        let expansion = tokens::motion::FADE_QUICK_CURVE.transform(self.expanded.get());
        let shown = tokens::motion::FADE_QUICK_CURVE.transform(self.shown.get());
        for (axis, rect, name) in [
            (Axis::Vertical, v, "Vertical scrollbar/"),
            (Axis::Horizontal, h, "Horizontal scrollbar/"),
        ] {
            if !self.has(axis) {
                continue;
            }
            if overlay && (self.fits(axis) || shown <= 0.0 && !self.held_now()) {
                continue;
            }
            let length = if axis == Axis::Vertical {
                rect.height
            } else {
                rect.width
            };
            let look = BarLook {
                axis,
                style: self.style,
                expansion: if overlay { expansion } else { 1.0 },
                hovered: self.hovered == Some(axis) || self.hold != Hold::None,
                dragging: self.drag.is_some_and(|(a, _, _)| a == axis),
                thumb: self.thumb(axis, length),
            };
            let at = Rect::new(b.x + rect.x, b.y + rect.y, rect.width, rect.height);
            let fade = overlay && !self.held_now() && shown < 1.0;
            if fade {
                cx.rec.push_opacity(shown);
            }
            if !enabled {
                cx.rec.push_opacity(cx.theme().disabled_opacity());
            }
            paint_bar(cx, name, at, &look);
            if !enabled {
                cx.rec.pop();
            }
            if fade {
                cx.rec.pop();
            }
        }
        if !overlay && self.vertical && self.horizontal {
            let t = self.thickness();
            paint_corner(
                cx,
                "Corner/",
                Rect::new(b.x + frame.width - t, b.y + frame.height - t, t, t),
            );
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        let origin = cx.bounds().origin();
        let frame = cx.size();
        let (vr, hr) = self.bar_rects(frame);
        let local = |p: &Point| Point::new(p.x - origin.x, p.y - origin.y);
        let overlay = self.style == ScrollbarStyle::Overlay;
        let bar_at = |s: &ScrollView, p: Point| -> Option<Axis> {
            for (axis, r) in [(Axis::Vertical, vr), (Axis::Horizontal, hr)] {
                let visible = s.has(axis) && !(overlay && s.fits(axis));
                if visible && r.contains(p) {
                    return Some(axis);
                }
            }
            None
        };
        match event {
            Event::PointerMove {
                pos,
                dragging: false,
                ..
            } => {
                let h = bar_at(self, local(pos));
                if h != self.hovered {
                    self.hovered = h;
                    self.sync_animation(cx);
                }
                Handled::No
            }
            Event::PointerLeave => {
                if self.hovered.take().is_some() {
                    self.sync_animation(cx);
                }
                Handled::No
            }
            Event::PointerDown {
                pos,
                button: Button::Left,
                ..
            } => {
                let p = local(pos);
                let Some(axis) = bar_at(self, p) else {
                    return Handled::No;
                };
                let rect = if axis == Axis::Vertical { vr } else { hr };
                let (length, at) = if axis == Axis::Vertical {
                    (rect.height, p.y - rect.y)
                } else {
                    (rect.width, p.x - rect.x)
                };
                let Some((off, ext)) = self.thumb(axis, length) else {
                    return Handled::Yes;
                };
                if at >= off && at <= off + ext {
                    let from = if axis == Axis::Vertical {
                        self.offset.y
                    } else {
                        self.offset.x
                    };
                    self.drag = Some((axis, at, from));
                    self.sync_animation(cx);
                } else {
                    // A click on the track pages toward it.
                    let sign = if at < off { -1.0 } else { 1.0 };
                    let mut o = self.offset;
                    if axis == Axis::Vertical {
                        o.y += sign * self.viewport.height;
                    } else {
                        o.x += sign * self.viewport.width;
                    }
                    self.set_offset(cx, o);
                }
                Handled::Yes
            }
            Event::PointerMove {
                pos,
                dragging: true,
                ..
            } if self.drag.is_some() => {
                let Some((axis, from, start)) = self.drag else {
                    return Handled::No;
                };
                let p = local(pos);
                let rect = if axis == Axis::Vertical { vr } else { hr };
                let (length, at) = if axis == Axis::Vertical {
                    (rect.height, p.y - rect.y)
                } else {
                    (rect.width, p.x - rect.x)
                };
                let Some((_, ext)) = self.thumb(axis, length) else {
                    return Handled::Yes;
                };
                let room = length - ext;
                if room > 0.0 {
                    let range = if axis == Axis::Vertical {
                        self.max_offset().y
                    } else {
                        self.max_offset().x
                    };
                    let target = start + (at - from) * range / room;
                    let mut o = self.offset;
                    if axis == Axis::Vertical {
                        o.y = target;
                    } else {
                        o.x = target;
                    }
                    self.set_offset(cx, o);
                }
                Handled::Yes
            }
            Event::PointerUp {
                button: Button::Left,
                ..
            } if self.drag.is_some() => {
                self.drag = None;
                self.sync_animation(cx);
                Handled::Yes
            }
            Event::Scroll { dx, dy, .. } => {
                let o = Point::new(self.offset.x + dx, self.offset.y + dy);
                let before = self.offset;
                self.set_offset(cx, o);
                Handled::from(self.offset != before)
            }
            _ => Handled::No,
        }
    }

    fn tick(&mut self, cx: &mut EventCx<'_>, dt: f32) -> bool {
        let a = self.shown.tick(dt);
        let b = self.expanded.tick(dt);
        if a || b {
            cx.request_paint();
        }
        let mut waiting = false;
        if self.waiting {
            if self.held_now() {
                self.waiting = false;
            } else if cx.now() - self.last_scroll >= metrics::HIDE_DELAY {
                self.waiting = false;
                let reduce = cx.theme().appearance.reduce_motion;
                self.shown.animate_to(
                    0.0,
                    if reduce {
                        Timing::None
                    } else {
                        Timing::curve(
                            tokens::motion::FADE_STANDARD_CURVE,
                            tokens::motion::FADE_STANDARD,
                        )
                    },
                );
                cx.request_paint();
            } else {
                waiting = true;
            }
        }
        a || b || waiting || self.shown.is_animating()
    }

    fn attached(&mut self, cx: &mut EventCx<'_>) {
        // Held bars are shown from the first frame.
        if self.hold != Hold::None {
            self.shown.jump(1.0);
            self.expanded.jump(if self.hold == Hold::Expanded {
                1.0
            } else {
                0.0
            });
        }
        cx.request_paint();
    }

    fn interaction(&self) -> Interaction {
        Interaction::Hover
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match prop {
            Prop::ScrollOffset => match value {
                Value::Point(p) if p.x.is_finite() && p.y.is_finite() => {
                    // Kept as given: it is clamped when the content's size is known (layout).
                    self.offset = Point::new(p.x.max(0.0), p.y.max(0.0));
                    PropChange::Layout
                }
                _ => PropChange::Unknown,
            },
            Prop::Style => match value.as_int() {
                Some(0) => {
                    self.style = ScrollbarStyle::Overlay;
                    PropChange::Layout
                }
                Some(1) => {
                    self.style = ScrollbarStyle::Legacy;
                    PropChange::Layout
                }
                _ => PropChange::Unknown,
            },
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::ScrollOffset => Some(Value::Point(self.offset)),
            Prop::ContentSize => Some(Value::Size(self.content)),
            Prop::Style => Some(Value::Int(match self.style {
                ScrollbarStyle::Overlay => 0,
                ScrollbarStyle::Legacy => 1,
            })),
            _ => None,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::ScrollArea;
        a.actions = vec![Action::ScrollIntoView];
    }
}

/// Makes a scroll view over `content` (clipped to its bounds).
pub fn scroll_view(
    ui: &mut Ui,
    style: ScrollbarStyle,
    content: crate::core::tree::ViewId,
) -> crate::core::tree::ViewId {
    let v = ui.create(ScrollView::new().style(style));
    let _ = ui.set(v, Prop::ClipsChildren, Value::Bool(true));
    ui.add_child(v, content);
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::event::{Mods, Phase};
    use crate::core::window::{WindowId, WindowKind};

    struct Big(Size);

    impl Widget for Big {
        fn kind(&self) -> &'static str {
            "Big"
        }
        fn measure(&mut self, _cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
            self.0
        }
    }

    fn scene(style: ScrollbarStyle) -> (Ui, WindowId, crate::core::ViewId) {
        let mut ui = Ui::new_headless();
        let win = ui.create_window(WindowKind::Normal, Size::new(300.0, 300.0));
        let content = ui.create(Big(Size::new(700.0, 420.0)));
        let sv = scroll_view(&mut ui, style, content);
        let _ = ui.set(sv, Prop::MaxWidth, Value::from(264.0));
        let _ = ui.set(sv, Prop::MaxHeight, Value::from(180.0));
        ui.set_content(win, sv);
        ui.show_window(win);
        ui.prepare_frame(win);
        (ui, win, sv)
    }

    #[test]
    fn the_wheel_scrolls_and_stays_in_range() {
        let (mut ui, win, sv) = scene(ScrollbarStyle::Legacy);
        let c = ui.tree.window_rect(sv).center();
        ui.scroll(win, c, 0.0, 50.0, false, Phase::None);
        assert_eq!(
            ui.get(sv, Prop::ScrollOffset),
            Some(Value::Point(Point::new(0.0, 50.0)))
        );
        ui.scroll(win, c, 0.0, 5000.0, false, Phase::None);
        // 420 - (180 - 16)
        assert_eq!(
            ui.get(sv, Prop::ScrollOffset),
            Some(Value::Point(Point::new(0.0, 256.0)))
        );
        ui.scroll(win, c, 0.0, -9999.0, false, Phase::None);
        assert_eq!(
            ui.get(sv, Prop::ScrollOffset),
            Some(Value::Point(Point::ZERO))
        );
    }

    #[test]
    fn dragging_the_thumb_scrolls_one_to_one() {
        let (mut ui, win, sv) = scene(ScrollbarStyle::Legacy);
        let r = ui.tree.window_rect(sv);
        // The vertical bar is the right 16 pt; the thumb starts at the top.
        let down = Point::new(r.right() - 8.0, r.y + 10.0);
        ui.pointer_move(win, down, Mods::empty());
        ui.pointer_down(win, down, Button::Left, Mods::empty());
        let to = Point::new(down.x, down.y + 50.0);
        ui.pointer_move(win, to, Mods::empty());
        ui.pointer_up(win, to, Button::Left, Mods::empty());
        let Some(Value::Point(p)) = ui.get(sv, Prop::ScrollOffset) else {
            panic!("no offset");
        };
        // 50 of the track's 100 free points: half of the range 256.
        assert!((p.y - 128.0).abs() < 4.0, "{p:?}");
    }

    #[test]
    fn a_click_on_the_track_pages() {
        let (mut ui, win, sv) = scene(ScrollbarStyle::Legacy);
        let r = ui.tree.window_rect(sv);
        let p = Point::new(r.right() - 8.0, r.y + 150.0);
        ui.pointer_move(win, p, Mods::empty());
        ui.pointer_down(win, p, Button::Left, Mods::empty());
        ui.pointer_up(win, p, Button::Left, Mods::empty());
        // One viewport (164) further.
        assert_eq!(
            ui.get(sv, Prop::ScrollOffset),
            Some(Value::Point(Point::new(0.0, 164.0)))
        );
    }

    #[test]
    fn overlay_bars_show_while_scrolling_and_fade_after_a_second() {
        let (mut ui, win, sv) = scene(ScrollbarStyle::Overlay);
        let c = ui.tree.window_rect(sv).center();
        assert_eq!(
            ui.widget::<ScrollView>(sv).map(|s| s.shown.get()),
            Some(0.0)
        );
        ui.scroll(win, c, 0.0, 20.0, false, Phase::None);
        for _ in 0..10 {
            ui.advance(0.05);
        }
        assert_eq!(
            ui.widget::<ScrollView>(sv).map(|s| s.shown.get()),
            Some(1.0)
        );
        for _ in 0..40 {
            ui.advance(0.05);
        }
        assert_eq!(
            ui.widget::<ScrollView>(sv).map(|s| s.shown.get()),
            Some(0.0)
        );
    }
}
