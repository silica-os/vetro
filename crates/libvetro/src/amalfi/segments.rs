//! Segmented controls (kit `Segments`): the segmented control (select one or any), the
//! action segments (toolbar capsules, the Align group), the Icon + Label control for window
//! panes, and the single items the kit also has as masters.

use crate::amalfi::foundation::{ControlSize, Label, control_shadow, focus_halo_named, whole};
use crate::amalfi::theme::Theme;
use crate::amalfi::tokens::{self, TypeStyle, control_heights, typography};
use crate::color::Color;
use crate::core::a11y::{A11y, Action, Role};
use crate::core::event::{Button, Event, Handled, Key};
use crate::core::layout::Proposal;
use crate::core::value::{Prop, Signal, Value};
use crate::core::widget::{EventCx, FocusKind, Interaction, LayoutCx, PaintCx, PropChange, Widget};
use crate::geom::{Point, Rect, Size, Transform};
use crate::motion::{Animated, Timing};
use crate::shape::Shape;
use crate::style::{Border, Style};
use crate::text::Align;

/// Fixed measures.
pub mod metrics {
    /// Space between a control's track and its items.
    pub const TRACK_PADDING: f32 = 4.0;
    /// Horizontal padding of a text item.
    pub const TEXT_ITEM_PADDING: f32 = 8.0;
    /// Separator width.
    pub const SEPARATOR_WIDTH: f32 = 1.0;
    /// Inset of a text highlight at its sides.
    pub const TEXT_HIGHLIGHT_INSET: f32 = 4.0;
    /// Text boxes are a multiple of this wide (the 4 pt grid).
    pub const STEP: f32 = 4.0;
    /// The Icon + Label control: bar height.
    pub const PANE_BAR_HEIGHT: f32 = 52.0;
    /// The Icon + Label control: pane height.
    pub const PANE_HEIGHT: f32 = 44.0;
    /// The Icon + Label control: narrowest pane.
    pub const MIN_PANE_WIDTH: f32 = 72.0;
    /// The Icon + Label control: glyph size.
    pub const PANE_GLYPH: f32 = 20.0;
    /// The Icon + Label control: top of the label in a pane.
    pub const PANE_LABEL_TOP: f32 = 26.0;
}

/// The highlight fades out in this many seconds (`motion.md`: push and icon buttons).
pub const HIGHLIGHT_OUT: f32 = 0.06;

/// The measures of segments at one control size (silica_ui's `AmalfiSegmentSizeMetrics`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SizeMetrics {
    /// The size.
    pub size: ControlSize,
    /// Track height.
    pub height: f32,
    /// Width of an action text segment.
    pub text_segment_width: f32,
    /// Separator height.
    pub separator_height: f32,
    /// Inset of a hover highlight.
    pub hover_inset: f32,
    /// Width of a text item.
    pub text_item_width: f32,
}

impl SizeMetrics {
    /// The metrics of `size`; a toolbar's Large is 36 tall.
    pub fn of(size: ControlSize, toolbar: bool) -> SizeMetrics {
        match size {
            ControlSize::Mini => SizeMetrics {
                size,
                height: control_heights::MINI,
                text_segment_width: 40.0,
                separator_height: 12.0,
                hover_inset: 0.0,
                text_item_width: 44.0,
            },
            ControlSize::Small => SizeMetrics {
                size,
                height: control_heights::SMALL,
                text_segment_width: 44.0,
                separator_height: 12.0,
                hover_inset: 0.0,
                text_item_width: 56.0,
            },
            ControlSize::Regular => SizeMetrics {
                size,
                height: control_heights::REGULAR,
                text_segment_width: 52.0,
                separator_height: 16.0,
                hover_inset: 4.0,
                text_item_width: 72.0,
            },
            ControlSize::Large => SizeMetrics {
                size,
                height: if toolbar {
                    control_heights::TOOLBAR
                } else {
                    control_heights::LARGE
                },
                text_segment_width: 56.0,
                separator_height: 16.0,
                hover_inset: 4.0,
                text_item_width: 88.0,
            },
        }
    }

    /// Whether separators take a place of their own between icon segments (toolbar items).
    pub fn separator_takes_space(&self) -> bool {
        self.height == control_heights::TOOLBAR
    }

    /// The glyph's size.
    pub fn glyph(&self) -> f32 {
        self.size.icon()
    }

    /// An item's height inside a track.
    pub fn item_height(&self) -> f32 {
        self.height - 2.0 * metrics::TRACK_PADDING
    }

    /// The label token of an action segment.
    pub fn action_token(&self) -> &'static TypeStyle {
        match self.size {
            ControlSize::Mini | ControlSize::Small => &typography::CALLOUT,
            _ => &typography::BODY,
        }
    }

    /// The label token of an item.
    pub fn item_token(&self) -> &'static TypeStyle {
        match self.size {
            ControlSize::Mini => &typography::CAPTION,
            ControlSize::Small => &typography::LABEL,
            _ => &typography::BODY,
        }
    }
}

/// What a segment shows.
#[derive(Debug, Clone, PartialEq)]
pub enum Content {
    /// A label.
    Text(String),
    /// A symbol (Silica Symbols name).
    Icon(String),
}

/// A segment of a control.
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    /// What it shows.
    pub content: Content,
    /// Whether it can be chosen.
    pub enabled: bool,
    /// The accessible label (icons need one).
    pub label: Option<String>,
    /// The tooltip.
    pub tooltip: Option<String>,
}

impl Segment {
    /// A text segment.
    pub fn text(label: &str) -> Segment {
        Segment {
            content: Content::Text(label.to_string()),
            enabled: true,
            label: None,
            tooltip: None,
        }
    }

    /// An icon segment with its accessible label.
    pub fn icon(symbol: &str, label: &str) -> Segment {
        Segment {
            content: Content::Icon(symbol.to_string()),
            enabled: true,
            label: Some(label.to_string()),
            tooltip: None,
        }
    }

    /// Disabled.
    pub fn disabled(mut self) -> Segment {
        self.enabled = false;
        self
    }

    fn a11y_label(&self) -> String {
        match (&self.label, &self.content) {
            (Some(l), _) => l.clone(),
            (None, Content::Text(t)) => t.clone(),
            (None, Content::Icon(_)) => String::new(),
        }
    }

    fn is_icon(&self) -> bool {
        matches!(self.content, Content::Icon(_))
    }
}

/// How an item looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ItemLook {
    /// No fill.
    #[default]
    Rest,
    /// `Line/Separator`.
    Hover,
    /// `Line/Border`.
    Pressed,
    /// `Surface/Control` with a border and the level 1 shadow.
    Selected,
    /// Selected and pressed.
    SelectedPressed,
}

impl ItemLook {
    fn style(self, theme: &Theme) -> Style {
        let c = &theme.colors;
        match self {
            ItemLook::Rest => Style::default(),
            ItemLook::Hover => Style::fill(c.line_separator),
            ItemLook::Pressed => Style::fill(c.line_border),
            ItemLook::Selected | ItemLook::SelectedPressed => {
                let fill = if self == ItemLook::Selected {
                    c.surface_control
                } else {
                    c.surface_control_pressed
                };
                Style::fill(fill)
                    .with_border(Border::inside(c.line_border, 1.0))
                    .with_shadow(control_shadow(theme))
            }
        }
    }
}

/// Text laid out for a segment, one cache per segment.
#[derive(Debug, Default)]
struct Labels {
    items: Vec<Label>,
    fallback: Label,
}

impl Labels {
    fn get(&mut self, i: usize) -> &mut Label {
        if self.items.len() <= i {
            self.items.resize_with(i + 1, Label::default);
        }
        match self.items.get_mut(i) {
            Some(l) => l,
            None => &mut self.fallback,
        }
    }
}

/// Records one label centred in `rect` (the layer's box is the item's width).
fn paint_label(
    cx: &mut PaintCx<'_>,
    cache: &mut Label,
    name: &str,
    rect: Rect,
    text: &str,
    token: &'static TypeStyle,
    color: Color,
) {
    let theme = *cx.theme();
    let style = theme.text(token, color);
    let lh = style.line_height.unwrap_or(token.size * 1.3);
    let layout = cache.layout(text, &style, Align::Center, rect.width, Some(1));
    cx.rec.text(
        name,
        Rect::new(rect.x, rect.y + (rect.height - lh) / 2.0, rect.width, lh),
        layout,
        Some(token.name),
        color,
    );
}

fn is_disabled(cx: &mut EventCx<'_>) -> bool {
    let id = cx.id();
    cx.ui().is_disabled(id)
}

/// Scales what `f` records about the centre of `rect`.
fn magnified(cx: &mut PaintCx<'_>, rect: Rect, k: f32, f: impl FnOnce(&mut PaintCx<'_>)) {
    if (k - 1.0).abs() < 1e-3 {
        f(cx);
        return;
    }
    let c = rect.center();
    cx.rec.push_transform(
        Transform::translate(-c.x, -c.y)
            .then(Transform::scale(k, k))
            .then(Transform::translate(c.x, c.y)),
    );
    f(cx);
    cx.rec.pop();
}

fn text_width(cache: &mut Label, theme: &Theme, token: &'static TypeStyle, text: &str) -> f32 {
    let style = theme.text(token, theme.colors.text_primary);
    whole(cache.width(text, &style), metrics::STEP)
}

// ---------------------------------------------------------------------------------------
// The segmented control.
// ---------------------------------------------------------------------------------------

/// Whether a segmented control selects one segment or any number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Selection {
    /// Exactly one (a radio group).
    #[default]
    One,
    /// Each segment on or off.
    Any,
}

/// The lens over a pressed segment of a select-one control (`materials.md`, lens rule): 8
/// beyond the item on each side, 12 above and below it, growing from the item's own size.
pub fn lens_rect(item: Rect, centre_x: f32, presence: f32) -> Rect {
    let p = if presence.is_finite() {
        presence.max(0.0)
    } else {
        0.0
    };
    let from = Rect::new(centre_x - item.width / 2.0, item.y, item.width, item.height);
    let full = Rect::new(
        from.x - 8.0,
        from.y - 12.0,
        from.width + 16.0,
        from.height + 24.0,
    );
    Rect::new(
        from.x + (full.x - from.x) * p,
        from.y + (full.y - from.y) * p,
        from.width + (full.width - from.width) * p,
        from.height + (full.height - from.height) * p,
    )
}

/// A segmented control.
#[derive(Debug)]
pub struct SegmentedControl {
    /// The segments.
    pub segments: Vec<Segment>,
    /// Select one or any.
    pub mode: Selection,
    /// Selected indices, ascending.
    pub selected: Vec<usize>,
    /// Size.
    pub size: ControlSize,
    /// A toolbar's Large control is 36 tall.
    pub toolbar: bool,
    /// Minimum width of a text item.
    pub text_item_width: Option<f32>,
    labels: Labels,
    items: Vec<Rect>,
    width: f32,
    thumb: Animated<Rect>,
    presence: Animated<f32>,
    lens_x: Animated<f32>,
    lens_opacity: Animated<f32>,
    lens_up: bool,
    grab: f32,
    hovered: Option<usize>,
    down: bool,
    pressed_item: Option<usize>,
    pressed_inside: bool,
    focus_index: usize,
}

impl SegmentedControl {
    /// A control of `segments`, select one, nothing selected.
    pub fn new(segments: Vec<Segment>) -> SegmentedControl {
        SegmentedControl {
            segments,
            mode: Selection::One,
            selected: Vec::new(),
            size: ControlSize::Regular,
            toolbar: false,
            text_item_width: None,
            labels: Labels::default(),
            items: Vec::new(),
            width: 0.0,
            thumb: Animated::new(Rect::ZERO),
            presence: Animated::new(0.0),
            lens_x: Animated::new(0.0),
            lens_opacity: Animated::new(0.0),
            lens_up: false,
            grab: 0.0,
            hovered: None,
            down: false,
            pressed_item: None,
            pressed_inside: false,
            focus_index: 0,
        }
    }

    /// Text segments.
    pub fn texts(labels: &[&str]) -> SegmentedControl {
        SegmentedControl::new(labels.iter().map(|l| Segment::text(l)).collect())
    }

    /// With a selection mode.
    pub fn mode(mut self, mode: Selection) -> SegmentedControl {
        self.mode = mode;
        self
    }

    /// With the selected segments.
    pub fn selected(mut self, selected: &[usize]) -> SegmentedControl {
        self.selected = selected.to_vec();
        self.selected.sort_unstable();
        self.selected.dedup();
        if let Some(i) = self.selected.first() {
            self.focus_index = *i;
        }
        self
    }

    /// With a size.
    pub fn size(mut self, size: ControlSize) -> SegmentedControl {
        self.size = size;
        self
    }

    fn metrics(&self) -> SizeMetrics {
        SizeMetrics::of(self.size, self.toolbar)
    }

    fn can_choose(&self, i: usize, enabled: bool) -> bool {
        enabled && self.segments.get(i).is_some_and(|s| s.enabled)
    }

    /// Lays the items out: text items share the widest width.
    fn compute(&mut self, theme: &Theme) {
        let m = self.metrics();
        let mut text_w = self.text_item_width.unwrap_or(m.text_item_width);
        let token = m.item_token();
        for i in 0..self.segments.len() {
            let Some(Content::Text(t)) = self.segments.get(i).map(|s| s.content.clone()) else {
                continue;
            };
            let w =
                text_width(self.labels.get(i), theme, token, &t) + 2.0 * metrics::TEXT_ITEM_PADDING;
            text_w = text_w.max(w);
        }
        self.items.clear();
        let mut x = metrics::TRACK_PADDING;
        for s in &self.segments {
            let w = if s.is_icon() { m.height } else { text_w };
            self.items
                .push(Rect::new(x, metrics::TRACK_PADDING, w, m.item_height()));
            x += w;
        }
        self.width = x + metrics::TRACK_PADDING;
        if !self.thumb.is_animating() {
            let r = self.single().and_then(|i| self.items.get(i)).copied();
            self.thumb.jump(r.unwrap_or(Rect::ZERO));
        }
    }

    fn single(&self) -> Option<usize> {
        match self.mode {
            Selection::One => self.selected.first().copied(),
            Selection::Any => None,
        }
    }

    fn index_at_x(&self, x: f32) -> Option<usize> {
        if self.items.is_empty() {
            return None;
        }
        let mut best = 0;
        let mut distance = f32::INFINITY;
        for (i, r) in self.items.iter().enumerate() {
            if x >= r.x && x < r.right() {
                return Some(i);
            }
            let d = (x - r.x).abs().min((x - r.right()).abs());
            if d < distance {
                distance = d;
                best = i;
            }
        }
        Some(best)
    }

    fn index_at(&self, p: Point) -> Option<usize> {
        let h = self.metrics().height;
        if p.y < 0.0 || p.y > h || p.x < 0.0 || p.x > self.width {
            return None;
        }
        self.index_at_x(p.x)
    }

    fn payload(&self) -> Value {
        match self.mode {
            Selection::One => self
                .selected
                .first()
                .map_or(Value::None, |i| Value::Int(*i as i64)),
            Selection::Any => Value::IntList(self.selected.iter().map(|i| *i as i64).collect()),
        }
    }

    fn choose(&mut self, cx: &mut EventCx<'_>, i: usize) {
        if !self.can_choose(i, !is_disabled(cx)) {
            return;
        }
        match self.mode {
            Selection::One => {
                if self.selected == [i] {
                    return;
                }
                let from = self.thumb.get();
                self.selected = vec![i];
                self.focus_index = i;
                if let Some(to) = self.items.get(i).copied() {
                    if cx.theme().appearance.reduce_motion || from == Rect::ZERO {
                        self.thumb.jump(to);
                    } else {
                        self.thumb
                            .animate_to(to, Timing::Spring(tokens::motion::SPRING_MORPH));
                    }
                }
            }
            Selection::Any => {
                if let Some(at) = self.selected.iter().position(|s| *s == i) {
                    self.selected.remove(at);
                } else {
                    self.selected.push(i);
                    self.selected.sort_unstable();
                }
            }
        }
        let payload = self.payload();
        cx.emit(Signal::ValueChanged, payload.clone());
        cx.emit(Signal::SelectionChanged, payload);
        cx.request_paint();
        cx.animate();
    }

    fn settle(&mut self, cx: &mut EventCx<'_>, target: Option<usize>) {
        let reduce = cx.theme().appearance.reduce_motion;
        if reduce {
            self.hide_lens();
            return;
        }
        if let Some(r) = target.and_then(|t| self.items.get(t)) {
            self.lens_x
                .animate_to(r.center().x, Timing::Spring(tokens::motion::SPRING_MORPH));
        }
        self.lens_opacity.animate_to(
            0.0,
            Timing::curve(tokens::motion::FADE_QUICK_CURVE, tokens::motion::FADE_QUICK),
        );
        cx.animate();
    }

    fn hide_lens(&mut self) {
        self.presence.jump(0.0);
        self.lens_x.jump(self.lens_x.get());
        self.lens_opacity.jump(0.0);
        self.lens_up = false;
    }

    fn step(&mut self, cx: &mut EventCx<'_>, step: isize) {
        let n = self.segments.len() as isize;
        let enabled = !is_disabled(cx);
        let mut i = self.focus_index as isize + step;
        while i >= 0 && i < n {
            let idx = i as usize;
            if self.can_choose(idx, enabled) {
                self.focus_index = idx;
                if self.mode == Selection::One {
                    self.choose(cx, idx);
                }
                cx.request_paint();
                return;
            }
            i += step;
        }
    }
}

impl Widget for SegmentedControl {
    fn kind(&self) -> &'static str {
        "SegmentedControl"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        let theme = cx.theme();
        self.compute(&theme);
        Size::new(self.width, self.metrics().height)
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        self.compute(&theme);
        let m = self.metrics();
        let b = cx.bounds();
        let o = Point::new(b.x, b.y);
        let shift = |r: Rect| r.translate(o);
        let h = m.height;
        let dim = !cx.is_disabled();
        cx.rec.layer(
            "Track",
            Rect::new(b.x, b.y, self.width, h),
            Shape::rounded(h / 2.0),
            theme.material_style("Glass/Regular"),
        );
        let one = self.mode == Selection::One;
        let item_radius = m.item_height() / 2.0;
        // The selection of select one: a slide, or the chosen item's own background.
        let sliding = one && self.thumb.is_animating();
        if sliding {
            cx.rec.layer(
                "Thumb",
                shift(self.thumb.get()),
                Shape::rounded(item_radius),
                ItemLook::Selected.style(&theme),
            );
        }
        // The lens, under the items.
        let presence = self.presence.get();
        let lens_opacity = self.lens_opacity.get();
        let lens_on = self.lens_up && !self.items.is_empty() && lens_opacity > 0.0;
        let lens_x = self.lens_x.get();
        if lens_on {
            let under = self.index_at_x(lens_x).unwrap_or(0);
            if let Some(item) = self.items.get(under) {
                let rect = lens_rect(*item, lens_x, presence);
                let mut style = theme.material_style("Glass/Clear");
                style.opacity = lens_opacity;
                cx.rec.layer(
                    "Lens",
                    shift(rect),
                    Shape::rounded((rect.height.min(rect.width) / 2.0 - 1.0).max(0.0)),
                    style,
                );
            }
        }
        let magnification = theme
            .material("Glass/Clear")
            .map_or(1.0, |c| c.magnification)
            .max(1.0);
        for (i, seg) in self.segments.iter().enumerate() {
            let Some(item) = self.items.get(i).copied() else {
                continue;
            };
            let chosen = self.selected.contains(&i);
            let look = if one {
                if chosen && !sliding {
                    ItemLook::Selected
                } else if self.hovered == Some(i) && !self.down && !chosen {
                    ItemLook::Hover
                } else {
                    ItemLook::Rest
                }
            } else {
                let down = self.pressed_item == Some(i) && self.pressed_inside;
                match (chosen, down) {
                    (true, true) => ItemLook::SelectedPressed,
                    (true, false) => ItemLook::Selected,
                    (false, true) => ItemLook::Pressed,
                    (false, false) if self.hovered == Some(i) && !self.down => ItemLook::Hover,
                    _ => ItemLook::Rest,
                }
            };
            let dimmed = dim && !seg.enabled;
            if dimmed {
                cx.rec.push_opacity(theme.disabled_opacity());
            }
            let prefix = format!("Segment {}/", i + 1);
            let rect = shift(item);
            cx.rec.layer(
                &format!("{prefix}Background"),
                rect,
                Shape::rounded(item_radius),
                look.style(&theme),
            );
            let k = if lens_on {
                let cover =
                    (1.0 - (item.center().x - lens_x).abs() / item.width.max(1.0)).clamp(0.0, 1.0);
                1.0 + (magnification - 1.0) * (presence * lens_opacity).clamp(0.0, 1.0) * cover
            } else {
                1.0
            };
            let color = theme.colors.text_primary;
            match &seg.content {
                Content::Text(t) => {
                    let cache = self.labels.get(i);
                    magnified(cx, rect, k, |cx| {
                        paint_label(
                            cx,
                            cache,
                            &format!("{prefix}Label"),
                            rect,
                            t,
                            m.item_token(),
                            color,
                        );
                    });
                }
                Content::Icon(symbol) => {
                    let g = m.glyph();
                    let r = Rect::new(
                        rect.x + (rect.width - g) / 2.0,
                        rect.y + (rect.height - g) / 2.0,
                        g,
                        g,
                    );
                    magnified(cx, rect, k, |cx| {
                        cx.rec
                            .glyph(&format!("{prefix}Glyph Sidebar"), r, symbol, 400, color);
                    });
                }
            }
            if dimmed {
                cx.rec.pop();
            }
        }
        if cx.focus_visible()
            && !cx.is_disabled()
            && let Some(item) = self.items.get(self.focus_index)
        {
            focus_halo_named(
                cx.rec,
                "Focus Halo",
                shift(*item),
                Shape::rounded(item_radius),
                &theme,
            );
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        let theme = cx.theme();
        let origin = cx.bounds().origin();
        let enabled = !is_disabled(cx);
        match event {
            Event::PointerMove {
                pos,
                dragging: false,
                ..
            } => {
                self.compute(&theme);
                let local = Point::new(pos.x - origin.x, pos.y - origin.y);
                let i = self.index_at(local);
                let h = i.filter(|i| self.can_choose(*i, enabled));
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
                self.compute(&theme);
                if self.down {
                    return Handled::No;
                }
                let local = Point::new(pos.x - origin.x, pos.y - origin.y);
                let Some(i) = self
                    .index_at(local)
                    .filter(|i| self.can_choose(*i, enabled))
                else {
                    return Handled::No;
                };
                self.down = true;
                self.focus_index = i;
                if self.mode == Selection::Any {
                    // Select any has no lens: a flat pressed item, toggled on release.
                    self.pressed_item = Some(i);
                    self.pressed_inside = true;
                    cx.request_paint();
                    return Handled::Yes;
                }
                let Some(centre) = self.items.get(i).map(|r| r.center().x) else {
                    return Handled::No;
                };
                let reduce = theme.appearance.reduce_motion;
                self.grab = centre - local.x;
                self.lens_x.jump(centre);
                self.lens_opacity.jump(1.0);
                if !self.lens_up {
                    self.presence.jump(0.0);
                }
                self.lens_up = true;
                if reduce {
                    self.presence.jump(1.0);
                } else {
                    self.presence
                        .animate_to(1.0, Timing::Spring(tokens::motion::SPRING_SNAPPY));
                }
                cx.request_paint();
                cx.animate();
                Handled::Yes
            }
            Event::PointerMove {
                pos,
                dragging: true,
                ..
            } if self.down => {
                let local = Point::new(pos.x - origin.x, pos.y - origin.y);
                if let Some(p) = self.pressed_item {
                    let inside = self.index_at(local) == Some(p);
                    if inside != self.pressed_inside {
                        self.pressed_inside = inside;
                        cx.request_paint();
                    }
                } else if let (Some(first), Some(last)) = (self.items.first(), self.items.last()) {
                    let (lo, hi) = (first.center().x, last.center().x);
                    let x = (local.x + self.grab).clamp(lo.min(hi), lo.max(hi));
                    self.lens_x.jump(x);
                    cx.request_paint();
                }
                Handled::Yes
            }
            Event::PointerUp {
                pos,
                button: Button::Left,
                ..
            } if self.down => {
                self.down = false;
                let local = Point::new(pos.x - origin.x, pos.y - origin.y);
                if let Some(p) = self.pressed_item.take() {
                    if self.index_at(local) == Some(p) {
                        self.choose(cx, p);
                    }
                    cx.request_paint();
                    return Handled::Yes;
                }
                let inside = self.index_at(local).is_some();
                let target = if inside {
                    self.index_at_x(self.lens_x.get())
                } else {
                    None
                };
                match target.filter(|t| self.can_choose(*t, enabled)) {
                    Some(t) => {
                        self.focus_index = t;
                        self.choose(cx, t);
                        self.settle(cx, Some(t));
                    }
                    None => self.settle(cx, None),
                }
                cx.request_paint();
                Handled::Yes
            }
            Event::Key(k) if k.pressed => {
                let step = match k.key {
                    Key::Left => -1,
                    Key::Right => 1,
                    Key::Space | Key::Enter => {
                        let i = self.focus_index;
                        self.choose(cx, i);
                        return Handled::Yes;
                    }
                    _ => return Handled::No,
                };
                self.step(cx, step);
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn tick(&mut self, _cx: &mut EventCx<'_>, dt: f32) -> bool {
        let a = self.thumb.tick(dt);
        let b = self.presence.tick(dt);
        let c = self.lens_x.tick(dt);
        let d = self.lens_opacity.tick(dt);
        let busy = a || b || c || d;
        if self.lens_up && !self.down && !busy && self.lens_opacity.get() <= 0.0 {
            self.hide_lens();
        }
        busy
    }

    fn interaction(&self) -> Interaction {
        Interaction::Hover
    }

    fn focus(&self) -> FocusKind {
        FocusKind::Control
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match prop {
            Prop::Items => match value.as_str_list() {
                Some(items) => {
                    let icons: Vec<String> = self
                        .segments
                        .iter()
                        .map(|s| match &s.content {
                            Content::Icon(i) => i.clone(),
                            Content::Text(_) => String::new(),
                        })
                        .collect();
                    self.segments = items
                        .iter()
                        .enumerate()
                        .map(|(i, t)| match icons.get(i).filter(|s| !s.is_empty()) {
                            Some(sym) => Segment::icon(sym, t),
                            None => Segment::text(t),
                        })
                        .collect();
                    PropChange::Layout
                }
                None => PropChange::Unknown,
            },
            Prop::ItemSymbols => match value.as_str_list() {
                Some(symbols) => {
                    for (s, sym) in self.segments.iter_mut().zip(symbols) {
                        if !sym.is_empty() {
                            let label = s.a11y_label();
                            *s = Segment::icon(sym, &label);
                        }
                    }
                    PropChange::Layout
                }
                None => PropChange::Unknown,
            },
            Prop::SelectedIndex => match value.as_int() {
                Some(i) => {
                    self.selected = usize::try_from(i).map(|i| vec![i]).unwrap_or_default();
                    self.thumb.jump(Rect::ZERO);
                    PropChange::Paint
                }
                None => PropChange::Unknown,
            },
            Prop::SelectedRows => match value {
                Value::IntList(l) => {
                    self.selected = l.iter().filter_map(|i| usize::try_from(*i).ok()).collect();
                    self.selected.sort_unstable();
                    self.selected.dedup();
                    PropChange::Paint
                }
                _ => PropChange::Unknown,
            },
            Prop::ControlSize => match value.as_int().and_then(ControlSize::from_int) {
                Some(s) => {
                    self.size = s;
                    PropChange::Layout
                }
                None => PropChange::Unknown,
            },
            Prop::Style => match value.as_int() {
                Some(0) => {
                    self.mode = Selection::One;
                    PropChange::Paint
                }
                Some(1) => {
                    self.mode = Selection::Any;
                    PropChange::Paint
                }
                _ => PropChange::Unknown,
            },
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::Items => Some(Value::StrList(
                self.segments.iter().map(Segment::a11y_label).collect(),
            )),
            Prop::SelectedIndex => Some(
                self.selected
                    .first()
                    .map_or(Value::Int(-1), |i| Value::Int(*i as i64)),
            ),
            Prop::SelectedRows => Some(Value::IntList(
                self.selected.iter().map(|i| *i as i64).collect(),
            )),
            Prop::ControlSize => Some(Value::Int(self.size.to_int())),
            Prop::Style => Some(Value::Int(match self.mode {
                Selection::One => 0,
                Selection::Any => 1,
            })),
            _ => None,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::SegmentedControl;
        a.value = Some(
            self.selected
                .iter()
                .filter_map(|i| self.segments.get(*i))
                .map(Segment::a11y_label)
                .collect::<Vec<_>>()
                .join(", "),
        );
        a.actions = vec![Action::Click, Action::Focus];
    }
}

// ---------------------------------------------------------------------------------------
// Action segments.
// ---------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Look {
    #[default]
    None,
    Hover,
    Pressed,
    Selected,
}

/// A capsule of buttons (Back / Forward, Reply / Reply All / Forward, text alignment): a
/// `Glass/Regular` track split by separators. A segment fires on release over it
/// (`Signal::Activated`, payload: its index); the selected ones (the Align group) keep a
/// highlight and an accent glyph.
#[derive(Debug)]
pub struct ActionSegments {
    /// The segments.
    pub segments: Vec<Segment>,
    /// Selected segments.
    pub selected: Vec<usize>,
    /// Size (a toolbar's Large is 36 tall).
    pub size: ControlSize,
    /// Whether the window is active (`None`: ask the window).
    pub active: Option<bool>,
    labels: Labels,
    lefts: Vec<f32>,
    widths: Vec<f32>,
    width: f32,
    highlight: Vec<Animated<f32>>,
    separator: Vec<Animated<f32>>,
    last_look: Vec<Look>,
    hovered: Option<usize>,
    pressed: Option<usize>,
    inside: bool,
    focus_index: usize,
}

impl ActionSegments {
    /// A capsule of `segments`, Large (the toolbar size).
    pub fn new(segments: Vec<Segment>) -> ActionSegments {
        ActionSegments {
            segments,
            selected: Vec::new(),
            size: ControlSize::Large,
            active: None,
            labels: Labels::default(),
            lefts: Vec::new(),
            widths: Vec::new(),
            width: 0.0,
            highlight: Vec::new(),
            separator: Vec::new(),
            last_look: Vec::new(),
            hovered: None,
            pressed: None,
            inside: false,
            focus_index: 0,
        }
    }

    /// With a size.
    pub fn size(mut self, size: ControlSize) -> ActionSegments {
        self.size = size;
        self
    }

    /// With selected segments.
    pub fn selected(mut self, selected: &[usize]) -> ActionSegments {
        self.selected = selected.to_vec();
        self
    }

    /// With the window's active state fixed.
    pub fn active(mut self, active: bool) -> ActionSegments {
        self.active = Some(active);
        self
    }

    fn metrics(&self) -> SizeMetrics {
        SizeMetrics::of(self.size, true)
    }

    fn can_press(&self, i: usize, enabled: bool) -> bool {
        enabled && self.segments.get(i).is_some_and(|s| s.enabled)
    }

    fn compute(&mut self, theme: &Theme) {
        let m = self.metrics();
        let token = m.action_token();
        let mut text_w = m.text_segment_width;
        for i in 0..self.segments.len() {
            if let Some(Content::Text(t)) = self.segments.get(i).map(|s| s.content.clone()) {
                text_w = text_w.max(text_width(self.labels.get(i), theme, token, &t));
            }
        }
        self.lefts.clear();
        self.widths.clear();
        let mut x = 0.0;
        for (i, s) in self.segments.iter().enumerate() {
            let w = if s.is_icon() { m.height } else { text_w };
            // Large icon segments are 1 apart: the separator has a place of its own.
            if i > 0 && s.is_icon() && m.separator_takes_space() {
                x += metrics::SEPARATOR_WIDTH;
            }
            self.lefts.push(x);
            self.widths.push(w);
            x += w;
        }
        self.width = x;
        let n = self.segments.len();
        self.highlight.resize_with(n, || Animated::new(0.0));
        self.separator.resize_with(n, || Animated::new(1.0));
        self.last_look.resize(n, Look::Hover);
        // At rest the fades sit on their targets (a selected segment starts highlighted).
        let looks: Vec<Look> = (0..n).map(|i| self.look(i)).collect();
        for (i, look) in looks.iter().enumerate() {
            if *look != Look::None
                && let Some(l) = self.last_look.get_mut(i)
            {
                *l = *look;
            }
            if let Some(a) = self.highlight.get_mut(i)
                && !a.is_animating()
            {
                a.jump(if *look == Look::None { 0.0 } else { 1.0 });
            }
            if i > 0
                && let Some(a) = self.separator.get_mut(i)
                && !a.is_animating()
            {
                let shown = looks.get(i - 1) == Some(&Look::None) && *look == Look::None;
                a.jump(if shown { 1.0 } else { 0.0 });
            }
        }
    }

    fn index_at(&self, p: Point) -> Option<usize> {
        if p.y < 0.0 || p.y > self.metrics().height {
            return None;
        }
        self.lefts
            .iter()
            .zip(&self.widths)
            .position(|(l, w)| p.x >= *l && p.x < *l + *w)
    }

    fn look(&self, i: usize) -> Look {
        if self.pressed == Some(i) && self.inside {
            Look::Pressed
        } else if self.selected.contains(&i) {
            Look::Selected
        } else if self.hovered == Some(i) && self.pressed.is_none() {
            Look::Hover
        } else {
            Look::None
        }
    }

    fn highlight_rect(&self, i: usize, look: Look) -> Rect {
        let m = self.metrics();
        let x = self.lefts.get(i).copied().unwrap_or(0.0);
        let w = self.widths.get(i).copied().unwrap_or(0.0);
        let h = m.height;
        let pressed = look == Look::Pressed;
        let inset = if pressed { 0.0 } else { m.hover_inset };
        if self.segments.get(i).is_some_and(Segment::is_icon) {
            return Rect::new(x + inset, inset, w - 2.0 * inset, h - 2.0 * inset);
        }
        let side = if pressed && m.separator_takes_space() {
            0.0
        } else {
            metrics::TEXT_HIGHLIGHT_INSET
        };
        Rect::new(x + side, inset, w - 2.0 * side, h - 2.0 * inset)
    }

    /// Moves the fades toward the current looks.
    fn sync(&mut self, cx: &mut EventCx<'_>) {
        let reduce = cx.theme().appearance.reduce_motion;
        let n = self.segments.len();
        let looks: Vec<Look> = (0..n).map(|i| self.look(i)).collect();
        let timing = |shown: bool| {
            if reduce {
                Timing::None
            } else {
                Timing::curve(
                    tokens::motion::FADE_QUICK_CURVE,
                    if shown {
                        tokens::motion::FADE_QUICK
                    } else {
                        HIGHLIGHT_OUT
                    },
                )
            }
        };
        let mut busy = false;
        for (i, look) in looks.iter().enumerate() {
            let shown = *look != Look::None;
            if shown && let Some(l) = self.last_look.get_mut(i) {
                *l = *look;
            }
            if let Some(a) = self.highlight.get_mut(i) {
                let t = if shown { 1.0 } else { 0.0 };
                if a.target() != t {
                    a.animate_to(t, timing(shown));
                }
                busy |= a.is_animating();
            }
            if i > 0 {
                let sep_shown = looks.get(i - 1) == Some(&Look::None) && *look == Look::None;
                if let Some(a) = self.separator.get_mut(i) {
                    let t = if sep_shown { 1.0 } else { 0.0 };
                    if a.target() != t {
                        a.animate_to(t, timing(sep_shown));
                    }
                    busy |= a.is_animating();
                }
            }
        }
        cx.request_paint();
        if busy {
            cx.animate();
        }
    }

    fn fire(&mut self, cx: &mut EventCx<'_>, i: usize) {
        if self.can_press(i, !is_disabled(cx)) {
            cx.emit(Signal::Activated, Value::Int(i as i64));
        }
    }
}

impl Widget for ActionSegments {
    fn kind(&self) -> &'static str {
        "ActionSegments"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        let theme = cx.theme();
        self.compute(&theme);
        Size::new(self.width, self.metrics().height)
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        self.compute(&theme);
        let c = theme.colors;
        let m = self.metrics();
        let b = cx.bounds();
        let h = m.height;
        let active = self.active.unwrap_or_else(|| cx.window_active());
        cx.rec.layer(
            "Glass",
            Rect::new(b.x, b.y, self.width, h),
            Shape::rounded(h / 2.0),
            theme.material_style("Glass/Regular"),
        );
        let n = self.segments.len();
        for i in 0..n {
            let op = self.highlight.get(i).map_or(0.0, Animated::get);
            if op <= 0.0 {
                continue;
            }
            let look = self.last_look.get(i).copied().unwrap_or(Look::Hover);
            let rect = self.highlight_rect(i, look).translate(Point::new(b.x, b.y));
            let color = if look == Look::Pressed {
                c.line_border
            } else {
                c.line_separator
            };
            let name = if look == Look::Selected {
                "Selected highlight"
            } else {
                "Highlight"
            };
            cx.rec.layer(
                name,
                rect,
                Shape::rounded(rect.height / 2.0),
                Style::fill(color).with_opacity(op),
            );
        }
        for i in 1..n {
            let op = self.separator.get(i).map_or(0.0, Animated::get);
            if op <= 0.0 {
                continue;
            }
            let left = self.lefts.get(i).copied().unwrap_or(0.0)
                - if self.segments.get(i).is_some_and(Segment::is_icon) && m.separator_takes_space()
                {
                    metrics::SEPARATOR_WIDTH
                } else {
                    0.0
                };
            cx.rec.layer(
                &format!("Separator {i}"),
                Rect::new(
                    b.x + left,
                    b.y + (h - m.separator_height) / 2.0,
                    metrics::SEPARATOR_WIDTH,
                    m.separator_height,
                ),
                Shape::RECT,
                Style::fill(c.line_separator).with_opacity(op),
            );
        }
        let enabled = !cx.is_disabled();
        for i in 0..n {
            let Some(seg) = self.segments.get(i) else {
                continue;
            };
            let x = b.x + self.lefts.get(i).copied().unwrap_or(0.0);
            let w = self.widths.get(i).copied().unwrap_or(0.0);
            let color = if !active {
                c.text_tertiary
            } else if self.selected.contains(&i) {
                c.accent_amalfi
            } else {
                c.text_primary
            };
            let dim = enabled && !seg.enabled;
            if dim {
                cx.rec.push_opacity(theme.disabled_opacity());
            }
            match &seg.content {
                Content::Icon(symbol) => {
                    let g = m.glyph();
                    cx.rec.glyph(
                        &format!("Glyph {}", i + 1),
                        Rect::new(x + (w - g) / 2.0, b.y + (h - g) / 2.0, g, g),
                        symbol,
                        400,
                        color,
                    );
                }
                Content::Text(t) => {
                    let cache = self.labels.get(i);
                    paint_label(
                        cx,
                        cache,
                        &format!("Label {}", i + 1),
                        Rect::new(x, b.y, w, h),
                        t,
                        m.action_token(),
                        color,
                    );
                }
            }
            if dim {
                cx.rec.pop();
            }
        }
        if cx.focus_visible() && !cx.is_disabled() && self.focus_index < n {
            let rect = self
                .highlight_rect(self.focus_index, Look::Hover)
                .translate(Point::new(b.x, b.y));
            focus_halo_named(
                cx.rec,
                "Focus Halo",
                rect,
                Shape::rounded(rect.height / 2.0),
                &theme,
            );
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        let theme = cx.theme();
        let origin = cx.bounds().origin();
        let enabled = !is_disabled(cx);
        let local = |p: &Point| Point::new(p.x - origin.x, p.y - origin.y);
        match event {
            Event::PointerMove {
                pos,
                dragging: false,
                ..
            } => {
                self.compute(&theme);
                let i = self.index_at(local(pos));
                let h = i.filter(|i| self.can_press(*i, enabled));
                if h != self.hovered {
                    self.hovered = h;
                    self.sync(cx);
                }
                Handled::No
            }
            Event::PointerLeave => {
                if self.hovered.take().is_some() {
                    self.sync(cx);
                }
                Handled::No
            }
            Event::PointerDown {
                pos,
                button: Button::Left,
                ..
            } => {
                self.compute(&theme);
                let Some(i) = self
                    .index_at(local(pos))
                    .filter(|i| self.can_press(*i, enabled))
                else {
                    return Handled::No;
                };
                self.pressed = Some(i);
                self.inside = true;
                self.focus_index = i;
                self.sync(cx);
                Handled::Yes
            }
            Event::PointerMove {
                pos,
                dragging: true,
                ..
            } if self.pressed.is_some() => {
                let inside = self.index_at(local(pos)) == self.pressed;
                if inside != self.inside {
                    self.inside = inside;
                    self.sync(cx);
                }
                Handled::Yes
            }
            Event::PointerUp {
                pos,
                button: Button::Left,
                ..
            } if self.pressed.is_some() => {
                let pressed = self.pressed.take();
                let at = self.index_at(local(pos));
                self.inside = false;
                self.hovered = at.filter(|i| self.can_press(*i, enabled));
                self.sync(cx);
                if let Some(p) = pressed
                    && at == Some(p)
                {
                    self.fire(cx, p);
                }
                Handled::Yes
            }
            Event::Key(k) if k.pressed => {
                let step: isize = match k.key {
                    Key::Left => -1,
                    Key::Right => 1,
                    Key::Space | Key::Enter => {
                        let i = self.focus_index;
                        self.fire(cx, i);
                        return Handled::Yes;
                    }
                    _ => return Handled::No,
                };
                let n = self.segments.len() as isize;
                let mut i = self.focus_index as isize + step;
                while i >= 0 && i < n {
                    if self.can_press(i as usize, enabled) {
                        self.focus_index = i as usize;
                        cx.request_paint();
                        break;
                    }
                    i += step;
                }
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn tick(&mut self, _cx: &mut EventCx<'_>, dt: f32) -> bool {
        let mut busy = false;
        for a in self.highlight.iter_mut().chain(self.separator.iter_mut()) {
            busy |= a.tick(dt);
        }
        busy
    }

    fn interaction(&self) -> Interaction {
        Interaction::Hover
    }

    fn focus(&self) -> FocusKind {
        FocusKind::Control
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match prop {
            Prop::SelectedRows => match value.as_int_list() {
                Some(l) => {
                    self.selected = l.iter().filter_map(|i| usize::try_from(*i).ok()).collect();
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
            // Through the ABI: icon segments from symbols (their accessible labels from
            // `Items`), or text segments from `Items` alone.
            Prop::ItemSymbols => match value.as_str_list() {
                Some(l) => {
                    let labels: Vec<String> = self
                        .segments
                        .iter()
                        .map(|s| s.label.clone().unwrap_or_default())
                        .collect();
                    self.segments = l
                        .iter()
                        .enumerate()
                        .map(|(i, sym)| {
                            Segment::icon(sym, labels.get(i).map_or("", String::as_str))
                        })
                        .collect();
                    PropChange::Layout
                }
                None => PropChange::Unknown,
            },
            Prop::Items => match value.as_str_list() {
                Some(l) => {
                    let icons = self
                        .segments
                        .iter()
                        .any(|s| matches!(s.content, Content::Icon(_)));
                    if icons {
                        for (s, label) in self.segments.iter_mut().zip(l) {
                            s.label = Some(label.clone());
                        }
                    } else {
                        self.segments = l.iter().map(|t| Segment::text(t)).collect();
                    }
                    PropChange::Layout
                }
                None => PropChange::Unknown,
            },
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::SelectedRows => Some(Value::IntList(
                self.selected.iter().map(|i| *i as i64).collect(),
            )),
            Prop::ControlSize => Some(Value::Int(self.size.to_int())),
            _ => None,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Toolbar;
        a.actions = vec![Action::Click, Action::Focus];
    }
}

// ---------------------------------------------------------------------------------------
// The Icon + Label control (window panes).
// ---------------------------------------------------------------------------------------

/// A pane of the Icon + Label control.
#[derive(Debug, Clone, PartialEq)]
pub struct Pane {
    /// The label under the glyph.
    pub label: String,
    /// The symbol.
    pub symbol: String,
    /// Whether it can be chosen.
    pub enabled: bool,
}

impl Pane {
    /// An enabled pane.
    pub fn new(label: &str, symbol: &str) -> Pane {
        Pane {
            label: label.to_string(),
            symbol: symbol.to_string(),
            enabled: true,
        }
    }
}

/// The segmented control for switching the panes of a window (settings windows always use
/// it): a `Glass/Regular` capsule 52 tall of equal panes, each a 20 pt glyph over a Caption
/// label. Hover, press and selection change in one frame.
#[derive(Debug)]
pub struct PaneControl {
    /// The panes.
    pub panes: Vec<Pane>,
    /// The chosen pane.
    pub selected: usize,
    /// Pane width (`None`: the widest label plus 24, on the 8 pt grid, at least 72).
    pub pane_width: Option<f32>,
    labels: Labels,
    pw: f32,
    hovered: Option<usize>,
    pressed: Option<usize>,
}

impl PaneControl {
    /// A control of `panes` with the first chosen.
    pub fn new(panes: Vec<Pane>) -> PaneControl {
        PaneControl {
            panes,
            selected: 0,
            pane_width: None,
            labels: Labels::default(),
            pw: metrics::MIN_PANE_WIDTH,
            hovered: None,
            pressed: None,
        }
    }

    /// With a chosen pane.
    pub fn selected(mut self, i: usize) -> PaneControl {
        self.selected = i;
        self
    }

    /// With a fixed pane width.
    pub fn pane_width(mut self, w: f32) -> PaneControl {
        self.pane_width = Some(w);
        self
    }

    fn compute(&mut self, theme: &Theme) {
        self.pw = match self.pane_width {
            Some(w) => w,
            None => {
                let mut widest = 0.0f32;
                for i in 0..self.panes.len() {
                    let Some(l) = self.panes.get(i).map(|p| p.label.clone()) else {
                        continue;
                    };
                    let style = theme.text(&typography::CAPTION, theme.colors.text_primary);
                    widest = widest.max(self.labels.get(i).width(&l, &style).ceil());
                }
                whole(widest + 24.0, 8.0).max(metrics::MIN_PANE_WIDTH)
            }
        };
    }

    fn width(&self) -> f32 {
        self.panes.len() as f32 * self.pw + 2.0 * metrics::TRACK_PADDING
    }

    fn can(&self, i: usize, enabled: bool) -> bool {
        enabled && self.panes.get(i).is_some_and(|p| p.enabled)
    }

    fn index_at(&self, x: f32) -> usize {
        let i = ((x - metrics::TRACK_PADDING) / self.pw).floor();
        let n = self.panes.len().saturating_sub(1);
        if i.is_finite() && i > 0.0 {
            (i as usize).min(n)
        } else {
            0
        }
    }

    fn choose(&mut self, cx: &mut EventCx<'_>, i: usize) {
        if i == self.selected || !self.can(i, !is_disabled(cx)) {
            return;
        }
        self.selected = i;
        cx.emit(Signal::ValueChanged, Value::Int(i as i64));
        cx.emit(Signal::SelectionChanged, Value::Int(i as i64));
        cx.request_paint();
    }
}

impl Widget for PaneControl {
    fn kind(&self) -> &'static str {
        "PaneControl"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        let theme = cx.theme();
        self.compute(&theme);
        Size::new(self.width(), metrics::PANE_BAR_HEIGHT)
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        self.compute(&theme);
        let c = theme.colors;
        let b = cx.bounds();
        cx.rec.layer(
            "Track",
            Rect::new(b.x, b.y, self.width(), metrics::PANE_BAR_HEIGHT),
            Shape::rounded(metrics::PANE_BAR_HEIGHT / 2.0),
            theme.material_style("Glass/Regular"),
        );
        let enabled = !cx.is_disabled();
        let selected = self.selected.min(self.panes.len().saturating_sub(1));
        for (i, pane) in self.panes.iter().enumerate() {
            let chosen = i == selected;
            let on = pane.enabled;
            let look = if chosen {
                ItemLook::Selected
            } else if self.pressed == Some(i) && on && enabled {
                ItemLook::Pressed
            } else if self.hovered == Some(i) && on && enabled {
                ItemLook::Hover
            } else {
                ItemLook::Rest
            };
            let x = b.x + metrics::TRACK_PADDING + i as f32 * self.pw;
            let y = b.y + metrics::TRACK_PADDING;
            let prefix = format!("Pane {}/", i + 1);
            if !on {
                cx.rec.push_opacity(theme.disabled_opacity());
            }
            cx.rec.layer(
                &format!("{prefix}Background"),
                Rect::new(x, y, self.pw, metrics::PANE_HEIGHT),
                Shape::rounded(metrics::PANE_HEIGHT / 2.0),
                look.style(&theme),
            );
            let g = metrics::PANE_GLYPH;
            cx.rec.glyph_sized(
                &format!("{prefix}Glyph"),
                Rect::new(x + (self.pw - g) / 2.0, y + 4.0, g, g + 1.0),
                g,
                &pane.symbol,
                400,
                if chosen {
                    c.accent_amalfi
                } else {
                    c.text_primary
                },
            );
            let label = pane.label.clone();
            paint_label_top(
                cx,
                self.labels.get(i),
                &format!("{prefix}Label"),
                Rect::new(x, y + metrics::PANE_LABEL_TOP, self.pw, 14.0),
                &label,
                &typography::CAPTION,
                c.text_primary,
            );
            if !on {
                cx.rec.pop();
            }
        }
        if cx.focus_visible() && !cx.is_disabled() && !self.panes.is_empty() {
            focus_halo_named(
                cx.rec,
                "Focus Halo",
                Rect::new(
                    b.x + metrics::TRACK_PADDING + selected as f32 * self.pw,
                    b.y + metrics::TRACK_PADDING,
                    self.pw,
                    metrics::PANE_HEIGHT,
                ),
                Shape::rounded(metrics::PANE_HEIGHT / 2.0),
                &theme,
            );
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        let theme = cx.theme();
        let origin = cx.bounds().origin();
        let enabled = !is_disabled(cx);
        match event {
            Event::PointerMove {
                pos,
                dragging: false,
                ..
            } => {
                self.compute(&theme);
                let i = self.index_at(pos.x - origin.x);
                let h = Some(i).filter(|i| self.can(*i, enabled));
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
                self.compute(&theme);
                let i = self.index_at(pos.x - origin.x);
                if self.can(i, enabled) {
                    self.pressed = Some(i);
                    cx.request_paint();
                    return Handled::Yes;
                }
                Handled::No
            }
            Event::PointerUp {
                pos,
                button: Button::Left,
                ..
            } if self.pressed.is_some() => {
                let p = self.pressed.take();
                let local = Point::new(pos.x - origin.x, pos.y - origin.y);
                let inside = local.y >= 0.0
                    && local.y <= metrics::PANE_BAR_HEIGHT
                    && local.x >= 0.0
                    && local.x <= self.width();
                if let Some(p) = p
                    && inside
                    && self.index_at(local.x) == p
                {
                    self.choose(cx, p);
                }
                cx.request_paint();
                Handled::Yes
            }
            Event::Key(k) if k.pressed => {
                let step: isize = match k.key {
                    Key::Left => -1,
                    Key::Right => 1,
                    _ => return Handled::No,
                };
                let n = self.panes.len() as isize;
                let mut i = self.selected as isize + step;
                while i >= 0 && i < n {
                    if self.can(i as usize, enabled) {
                        self.choose(cx, i as usize);
                        break;
                    }
                    i += step;
                }
                Handled::Yes
            }
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
        match prop {
            Prop::SelectedIndex => match value.as_int().and_then(|i| usize::try_from(i).ok()) {
                Some(i) => {
                    self.selected = i;
                    PropChange::Paint
                }
                None => PropChange::Unknown,
            },
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::SelectedIndex => Some(Value::Int(self.selected as i64)),
            Prop::Items => Some(Value::StrList(
                self.panes.iter().map(|p| p.label.clone()).collect(),
            )),
            _ => None,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::SegmentedControl;
        a.value = self.panes.get(self.selected).map(|p| p.label.clone());
        a.actions = vec![Action::Click, Action::Focus];
    }
}

/// A label whose layer box is `rect` exactly (top aligned).
fn paint_label_top(
    cx: &mut PaintCx<'_>,
    cache: &mut Label,
    name: &str,
    rect: Rect,
    text: &str,
    token: &'static TypeStyle,
    color: Color,
) {
    let theme = *cx.theme();
    let style = theme.text(token, color);
    let layout = cache.layout(text, &style, Align::Center, rect.width, Some(1));
    cx.rec.text(name, rect, layout, Some(token.name), color);
}

// ---------------------------------------------------------------------------------------
// A single item.
// ---------------------------------------------------------------------------------------

/// What a standalone item shows.
#[derive(Debug, Clone, PartialEq)]
pub enum ItemContent {
    /// A label (`Segments/Text Item`).
    Text(String),
    /// A symbol (`Segments/Icon Item`).
    Icon(String),
    /// A glyph over a label (`Segments/Icon + Label Item`).
    IconLabel {
        /// The symbol.
        symbol: String,
        /// The label.
        label: String,
    },
}

/// One item of a segmented control as its own view: what the kit's item masters show. The
/// controls draw their items themselves.
#[derive(Debug)]
pub struct SegmentItem {
    /// What it shows.
    pub content: ItemContent,
    /// Size (text and icon items).
    pub size: ControlSize,
    /// How it looks.
    pub look: ItemLook,
    /// Width (`None`: the natural one).
    pub width: Option<f32>,
    label: Label,
}

impl SegmentItem {
    /// An item.
    pub fn new(content: ItemContent) -> SegmentItem {
        SegmentItem {
            content,
            size: ControlSize::Regular,
            look: ItemLook::Rest,
            width: None,
            label: Label::default(),
        }
    }

    /// With a size.
    pub fn size(mut self, size: ControlSize) -> SegmentItem {
        self.size = size;
        self
    }

    /// With a look.
    pub fn look(mut self, look: ItemLook) -> SegmentItem {
        self.look = look;
        self
    }

    /// With a width.
    pub fn width(mut self, width: f32) -> SegmentItem {
        self.width = Some(width);
        self
    }

    fn natural(&mut self, theme: &Theme) -> Size {
        let m = SizeMetrics::of(self.size, false);
        match self.content.clone() {
            ItemContent::Icon(_) => Size::new(self.width.unwrap_or(m.height), m.item_height()),
            ItemContent::Text(t) => {
                let w = text_width(&mut self.label, theme, m.item_token(), &t)
                    + 2.0 * metrics::TEXT_ITEM_PADDING;
                Size::new(
                    self.width.unwrap_or_else(|| w.max(m.text_item_width)),
                    m.item_height(),
                )
            }
            ItemContent::IconLabel { label, .. } => {
                let style = theme.text(&typography::CAPTION, theme.colors.text_primary);
                let w = whole(self.label.width(&label, &style).ceil() + 24.0, 8.0)
                    .max(metrics::MIN_PANE_WIDTH);
                Size::new(self.width.unwrap_or(w), metrics::PANE_HEIGHT)
            }
        }
    }
}

impl Widget for SegmentItem {
    fn kind(&self) -> &'static str {
        "SegmentItem"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        let theme = cx.theme();
        self.natural(&theme)
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let c = theme.colors;
        let r = cx.bounds();
        let m = SizeMetrics::of(self.size, false);
        let selected = matches!(self.look, ItemLook::Selected | ItemLook::SelectedPressed);
        match self.content.clone() {
            ItemContent::Text(t) => {
                cx.rec.layer(
                    "Background",
                    r,
                    Shape::rounded(r.height / 2.0),
                    self.look.style(&theme),
                );
                paint_label(
                    cx,
                    &mut self.label,
                    "Label",
                    r,
                    &t,
                    m.item_token(),
                    c.text_primary,
                );
            }
            ItemContent::Icon(symbol) => {
                cx.rec.layer(
                    "Background",
                    r,
                    Shape::rounded(r.height / 2.0),
                    self.look.style(&theme),
                );
                let g = m.glyph();
                cx.rec.glyph(
                    "Glyph Sidebar",
                    Rect::new(r.x + (r.width - g) / 2.0, r.y + (r.height - g) / 2.0, g, g),
                    &symbol,
                    400,
                    c.text_primary,
                );
            }
            ItemContent::IconLabel { symbol, label } => {
                cx.rec.layer(
                    "Background",
                    r,
                    Shape::rounded(r.height / 2.0),
                    self.look.style(&theme),
                );
                let g = metrics::PANE_GLYPH;
                cx.rec.glyph_sized(
                    "Glyph",
                    Rect::new(r.x + (r.width - g) / 2.0, r.y + 4.0, g, g + 1.0),
                    g,
                    &symbol,
                    400,
                    if selected {
                        c.accent_amalfi
                    } else {
                        c.text_primary
                    },
                );
                paint_label_top(
                    cx,
                    &mut self.label,
                    "Label",
                    Rect::new(r.x, r.y + metrics::PANE_LABEL_TOP, r.width, 14.0),
                    &label,
                    &typography::CAPTION,
                    c.text_primary,
                );
            }
        }
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

    fn scene(
        w: impl Widget + 'static,
    ) -> (Ui, WindowId, crate::core::ViewId, Rc<RefCell<Vec<Value>>>) {
        let mut ui = Ui::new_headless();
        let win = ui.create_window(WindowKind::Normal, Size::new(400.0, 200.0));
        let id = ui.create(w);
        let log = Rc::new(RefCell::new(Vec::new()));
        let l = log.clone();
        ui.connect(id, Signal::ValueChanged, move |_, _, v| {
            l.borrow_mut().push(v.clone());
        });
        let l = log.clone();
        ui.connect(id, Signal::Activated, move |_, _, v| {
            l.borrow_mut().push(v.clone());
        });
        ui.set_content(win, id);
        ui.show_window(win);
        ui.prepare_frame(win);
        (ui, win, id, log)
    }

    fn at(ui: &Ui, id: crate::core::ViewId, x: f32, y: f32) -> Point {
        let r = ui.tree.window_rect(id);
        Point::new(r.x + x, r.y + y)
    }

    fn click(ui: &mut Ui, win: WindowId, p: Point) {
        ui.pointer_move(win, p, Mods::empty());
        ui.pointer_down(win, p, Button::Left, Mods::empty());
        ui.pointer_up(win, p, Button::Left, Mods::empty());
    }

    #[test]
    fn select_one_chooses_on_release_and_slides() {
        let (mut ui, win, id, log) =
            scene(SegmentedControl::texts(&["One", "Two", "Three"]).selected(&[0]));
        // The second item's centre: 4 + 72 + 36.
        let p = at(&ui, id, 112.0, 14.0);
        click(&mut ui, win, p);
        assert_eq!(*log.borrow(), vec![Value::Int(1)]);
        assert_eq!(ui.get(id, Prop::SelectedIndex), Some(Value::Int(1)));
        for _ in 0..60 {
            ui.advance(0.05);
        }
        assert!(
            ui.widget::<SegmentedControl>(id)
                .is_some_and(|c| !c.lens_up)
        );
        // Choosing the chosen one again changes nothing.
        let n = log.borrow().len();
        let p = at(&ui, id, 112.0, 14.0);
        click(&mut ui, win, p);
        assert_eq!(log.borrow().len(), n);
    }

    #[test]
    fn dragging_the_lens_chooses_the_item_under_it() {
        let (mut ui, win, id, _log) =
            scene(SegmentedControl::texts(&["One", "Two", "Three"]).selected(&[0]));
        let start = at(&ui, id, 40.0, 14.0);
        ui.pointer_move(win, start, Mods::empty());
        ui.pointer_down(win, start, Button::Left, Mods::empty());
        ui.pointer_move(win, at(&ui, id, 190.0, 14.0), Mods::empty());
        ui.pointer_up(win, at(&ui, id, 190.0, 14.0), Button::Left, Mods::empty());
        assert_eq!(ui.get(id, Prop::SelectedIndex), Some(Value::Int(2)));
        // A release outside cancels.
        let start = at(&ui, id, 40.0, 14.0);
        ui.pointer_move(win, start, Mods::empty());
        ui.pointer_down(win, start, Button::Left, Mods::empty());
        ui.pointer_move(win, at(&ui, id, 40.0, 90.0), Mods::empty());
        ui.pointer_up(win, at(&ui, id, 40.0, 90.0), Button::Left, Mods::empty());
        assert_eq!(ui.get(id, Prop::SelectedIndex), Some(Value::Int(2)));
    }

    #[test]
    fn select_any_toggles_each_item() {
        let (mut ui, win, id, log) = scene(
            SegmentedControl::texts(&["Bold", "Italic"])
                .mode(Selection::Any)
                .selected(&[0]),
        );
        let p = at(&ui, id, 100.0, 14.0);
        click(&mut ui, win, p);
        let p = at(&ui, id, 30.0, 14.0);
        click(&mut ui, win, p);
        assert_eq!(
            *log.borrow(),
            vec![Value::IntList(vec![0, 1]), Value::IntList(vec![1])]
        );
    }

    #[test]
    fn arrows_move_the_choice_and_skip_disabled() {
        let (mut ui, win, id, _log) = scene(
            SegmentedControl::new(vec![
                Segment::text("A"),
                Segment::text("B").disabled(),
                Segment::text("C"),
            ])
            .selected(&[0]),
        );
        ui.set_focus(id, true);
        ui.key(win, KeyEvent::press(Key::Right, Mods::empty()));
        assert_eq!(ui.get(id, Prop::SelectedIndex), Some(Value::Int(2)));
        ui.key(win, KeyEvent::press(Key::Left, Mods::empty()));
        assert_eq!(ui.get(id, Prop::SelectedIndex), Some(Value::Int(0)));
    }

    #[test]
    fn action_segments_fire_on_release_over_the_pressed_one() {
        let (mut ui, win, id, log) = scene(ActionSegments::new(vec![
            Segment::icon("chevron-left", "Back"),
            Segment::icon("chevron-right", "Forward"),
        ]));
        let p = at(&ui, id, 55.0, 18.0);
        click(&mut ui, win, p);
        assert_eq!(*log.borrow(), vec![Value::Int(1)]);
        // Pressed on one, released on the other: nothing.
        let p = at(&ui, id, 18.0, 18.0);
        ui.pointer_move(win, p, Mods::empty());
        ui.pointer_down(win, p, Button::Left, Mods::empty());
        ui.pointer_move(win, at(&ui, id, 55.0, 18.0), Mods::empty());
        ui.pointer_up(win, at(&ui, id, 55.0, 18.0), Button::Left, Mods::empty());
        assert_eq!(log.borrow().len(), 1);
    }

    #[test]
    fn pane_control_chooses_a_pane() {
        let (mut ui, win, id, log) = scene(PaneControl::new(vec![
            Pane::new("General", "gear"),
            Pane::new("Network", "wifi"),
        ]));
        let p = at(&ui, id, 140.0, 20.0);
        click(&mut ui, win, p);
        assert_eq!(*log.borrow(), vec![Value::Int(1)]);
        assert_eq!(ui.get(id, Prop::SelectedIndex), Some(Value::Int(1)));
    }

    #[test]
    fn lens_geometry() {
        let item = Rect::new(76.0, 8.0, 72.0, 20.0);
        let full = lens_rect(item, item.center().x, 1.0);
        assert_eq!(full, Rect::new(68.0, -4.0, 88.0, 44.0));
        assert_eq!(lens_rect(item, item.center().x, 0.0), item);
    }
}
