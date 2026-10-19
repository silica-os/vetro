//! Tooltips (kit `Tooltips/Tooltip`, `Tooltips/Placement`): the bubble (`Glass/Thick`, radius
//! 8, Callout text, padding 4 x 8, an optional shortcut 8 after the label), where it goes next
//! to its target, and the presenter that shows it when the pointer rests on a view with a
//! tooltip.

use crate::amalfi::foundation::{Label, whole};
use crate::amalfi::tokens::{self, typography};
use crate::core::a11y::{A11y, Role};
use crate::core::event::Shortcut;
use crate::core::layout::Proposal;
use crate::core::tree::ViewId;
use crate::core::ui::{TimerId, Ui};
use crate::core::value::{Prop, Value};
use crate::core::widget::{LayoutCx, PaintCx, PropChange, Widget};
use crate::core::window::{WindowId, WindowKind};
use crate::geom::{Point, Rect, Size};
use crate::motion::{Animated, Timing};
use crate::shape::Shape;
use crate::text::Align;

/// The bubble's measures (silica_ui's `AmalfiTooltipMetrics`).
pub mod metrics {
    /// Horizontal padding.
    pub const PAD_X: f32 = 8.0;
    /// Vertical padding.
    pub const PAD_Y: f32 = 4.0;
    /// Gap between the label and the shortcut.
    pub const SHORTCUT_GAP: f32 = 8.0;
    /// Corner radius.
    pub const RADIUS: f32 = 8.0;
    /// Widest bubble; longer messages wrap.
    pub const MAX_WIDTH: f32 = 224.0;
    /// Distance to the target.
    pub const DISTANCE: f32 = 8.0;
    /// Text boxes are a multiple of this wide (the 4 pt grid).
    pub const STEP: f32 = 4.0;
}

/// Seconds the pointer rests before the first tooltip appears. Measured (`motion.md`,
/// reference appkit-controls: ~1.5 s, appears in one frame); the token `delay.tooltip`
/// (0.7 s) is the earlier assumption.
pub const APPEAR_DELAY: f64 = 1.5;
/// Seconds a tooltip takes to leave (measured: ~450 ms).
pub const LEAVE_FADE: f32 = 0.45;
/// Moving to another tooltip within this many seconds of the last one shows it at once.
pub const SWITCH_WINDOW: f64 = 0.3;

/// A tooltip bubble.
#[derive(Debug)]
pub struct Tooltip {
    /// The message.
    pub message: String,
    /// The shortcut, as displayed (`⌘D`).
    pub shortcut: Option<String>,
    /// The widest the bubble gets.
    pub max_width: f32,
    message_text: Label,
    shortcut_text: Label,
    opacity: Animated<f32>,
    leaving: bool,
}

impl Tooltip {
    /// A bubble for `message`.
    pub fn new(message: &str) -> Tooltip {
        Tooltip {
            message: message.to_string(),
            shortcut: None,
            max_width: metrics::MAX_WIDTH,
            message_text: Label::default(),
            shortcut_text: Label::default(),
            opacity: Animated::new(1.0),
            leaving: false,
        }
    }

    /// With a shortcut.
    pub fn shortcut(mut self, shortcut: &str) -> Tooltip {
        self.shortcut = Some(shortcut.to_string());
        self
    }

    /// Starts the fade-out.
    pub fn begin_leave(&mut self) {
        self.leaving = true;
        self.opacity.animate_to(
            0.0,
            Timing::curve(tokens::motion::FADE_INSTANT_CURVE, LEAVE_FADE),
        );
    }

    /// The boxes of the message and the shortcut, and the message's height, for `width`
    /// available to the bubble.
    fn boxes(&mut self, theme: &crate::amalfi::theme::Theme, width: f32) -> (f32, f32, f32) {
        let c = &theme.colors;
        let msg_style = theme.text(&typography::CALLOUT, c.text_primary);
        let sc_style = theme.text(&typography::CALLOUT, c.text_secondary);
        let lh = msg_style.line_height.unwrap_or(16.0);
        let shortcut_w = match self.shortcut.clone() {
            Some(s) => whole(self.shortcut_text.width(&s, &sc_style), metrics::STEP),
            None => 0.0,
        };
        let gap = if self.shortcut.is_some() {
            metrics::SHORTCUT_GAP
        } else {
            0.0
        };
        let avail = (width - metrics::PAD_X * 2.0 - shortcut_w - gap).max(0.0);
        let natural = self.message_text.width(&self.message, &msg_style);
        if natural <= avail {
            (whole(natural, metrics::STEP), shortcut_w, lh)
        } else {
            let box_w = (avail / metrics::STEP).floor() * metrics::STEP;
            let layout =
                self.message_text
                    .layout(&self.message, &msg_style, Align::Start, box_w, None);
            (box_w, shortcut_w, layout.size().height.max(lh))
        }
    }
}

impl Widget for Tooltip {
    fn kind(&self) -> &'static str {
        "Tooltip"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        let theme = cx.theme();
        let width = match p.width {
            Some(w) if w.is_finite() => w.min(self.max_width),
            _ => self.max_width,
        };
        let (label, shortcut, height) = self.boxes(&theme, width);
        let gap = if self.shortcut.is_some() {
            metrics::SHORTCUT_GAP
        } else {
            0.0
        };
        Size::new(
            metrics::PAD_X * 2.0 + label + gap + shortcut,
            height + metrics::PAD_Y * 2.0,
        )
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let r = cx.bounds();
        let fade = self.opacity.get();
        if fade < 1.0 {
            cx.rec.push_opacity(fade);
        }
        cx.rec.layer(
            "Surface",
            r,
            Shape::rounded(metrics::RADIUS),
            theme.material_style("Glass/Thick"),
        );
        let (label_w, shortcut_w, height) = self.boxes(&theme, r.width);
        let msg_style = theme.text(&typography::CALLOUT, theme.colors.text_primary);
        let layout = self.message_text.layout(
            &self.message,
            &msg_style,
            Align::Start,
            label_w,
            if height > msg_style.line_height.unwrap_or(16.0) + 0.5 {
                None
            } else {
                Some(1)
            },
        );
        cx.rec.text(
            "Label",
            Rect::new(r.x + metrics::PAD_X, r.y + metrics::PAD_Y, label_w, height),
            layout,
            Some(typography::CALLOUT.name),
            msg_style.color,
        );
        if let Some(s) = self.shortcut.clone() {
            let style = theme.text(&typography::CALLOUT, theme.colors.text_secondary);
            let lh = style.line_height.unwrap_or(16.0);
            let layout = self
                .shortcut_text
                .layout(&s, &style, Align::Start, shortcut_w, Some(1));
            cx.rec.text(
                "Shortcut",
                Rect::new(
                    r.x + metrics::PAD_X + label_w + metrics::SHORTCUT_GAP,
                    r.y + metrics::PAD_Y,
                    shortcut_w,
                    lh,
                ),
                layout,
                Some(typography::CALLOUT.name),
                style.color,
            );
        }
        if fade < 1.0 {
            cx.rec.pop();
        }
    }

    fn tick(&mut self, _cx: &mut crate::core::widget::EventCx<'_>, dt: f32) -> bool {
        self.opacity.tick(dt)
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match (prop, value) {
            (Prop::Title | Prop::Text, Value::Str(s)) => {
                self.message.clone_from(s);
                PropChange::Layout
            }
            (Prop::Shortcut, Value::Str(s)) => {
                self.shortcut = Some(s.clone()).filter(|s| !s.is_empty());
                PropChange::Layout
            }
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::Title | Prop::Text => Some(Value::Str(self.message.clone())),
            Prop::Shortcut => Some(self.shortcut.clone().map(Value::Str).unwrap_or_default()),
            _ => None,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Tooltip;
        a.label = Some(match &self.shortcut {
            Some(s) => format!("{}, {s}", self.message),
            None => self.message.clone(),
        });
    }

    fn hit(&self, _p: Point, _size: Size) -> bool {
        false
    }
}

/// On which side of its target a tooltip goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Placement {
    /// Under the target (the default).
    #[default]
    Below,
    /// Over the target.
    Above,
    /// Before the target (left in left-to-right).
    Leading,
    /// After the target.
    Trailing,
}

/// Where a tooltip of `size` goes next to `target` inside `bounds` (silica_ui's
/// `amalfiTooltipOffset`): 8 from the target on the placement side (flipped when it does not
/// fit and the opposite side does), centred across and floored to the 4 pt grid, kept inside
/// `bounds`.
///
/// The grid rule is read from the kit: the 116 x 24 tooltip over a 28 pt target at x 46 sits
/// at x 0, not 2. "Centred, then floored" is an assumption (no case tells floor from round).
pub fn place(target: Rect, size: Size, bounds: Size, placement: Placement, rtl: bool) -> Point {
    #[derive(Clone, Copy, PartialEq)]
    enum Side {
        Down,
        Up,
        Left,
        Right,
    }
    let side = match (placement, rtl) {
        (Placement::Below, _) => Side::Down,
        (Placement::Above, _) => Side::Up,
        (Placement::Leading, false) | (Placement::Trailing, true) => Side::Left,
        (Placement::Leading, true) | (Placement::Trailing, false) => Side::Right,
    };
    let gap = metrics::DISTANCE;
    let along = |s: Side| match s {
        Side::Down => target.bottom() + gap,
        Side::Up => target.y - gap - size.height,
        Side::Right => target.right() + gap,
        Side::Left => target.x - gap - size.width,
    };
    let fits = |s: Side| {
        let v = along(s);
        match s {
            Side::Down | Side::Up => v >= 0.0 && v + size.height <= bounds.height,
            Side::Left | Side::Right => v >= 0.0 && v + size.width <= bounds.width,
        }
    };
    let opposite = |s: Side| match s {
        Side::Down => Side::Up,
        Side::Up => Side::Down,
        Side::Left => Side::Right,
        Side::Right => Side::Left,
    };
    let chosen = if fits(side) || !fits(opposite(side)) {
        side
    } else {
        opposite(side)
    };
    let grid = |v: f32| (v / metrics::STEP).floor() * metrics::STEP;
    let clamp = |v: f32, extent: f32, room: f32| v.min(room - extent).max(0.0);
    let vertical = matches!(chosen, Side::Down | Side::Up);
    if vertical {
        Point::new(
            clamp(
                grid(target.center().x - size.width / 2.0),
                size.width,
                bounds.width,
            ),
            clamp(along(chosen), size.height, bounds.height),
        )
    } else {
        Point::new(
            clamp(along(chosen), size.width, bounds.width),
            clamp(
                grid(target.center().y - size.height / 2.0),
                size.height,
                bounds.height,
            ),
        )
    }
}

/// Shows the tooltip of whatever the pointer rests on: after [`APPEAR_DELAY`] the first
/// time, at once while moving between views with tooltips (within [`SWITCH_WINDOW`]); the
/// bubble fades out in [`LEAVE_FADE`] when the pointer leaves. A view has a tooltip when
/// `Prop::Tooltip` is set; `Prop::Shortcut` on a widget that has it adds the shortcut.
#[derive(Debug, Default)]
pub struct Presenter {
    target: Option<ViewId>,
    pending: Option<TimerId>,
    shown: Option<(WindowId, ViewId)>,
    closing: Vec<WindowId>,
    last_hidden: Option<f64>,
}

impl Presenter {
    /// The window showing a tooltip, if any.
    pub fn window(&self) -> Option<WindowId> {
        self.shown.map(|(w, _)| w)
    }
}

/// The pointer's hover chain in `window` changed.
pub(crate) fn hover_changed(ui: &mut Ui, window: WindowId) {
    let target = ui.window(window).and_then(|w| {
        w.hover
            .iter()
            .rev()
            .copied()
            .find(|v| ui.tree.get(*v).is_some_and(|n| n.tooltip.is_some()))
    });
    if target == ui.tooltips.target {
        return;
    }
    if let Some(t) = ui.tooltips.pending.take() {
        ui.cancel_timer(t);
    }
    let had_popup = ui.tooltips.shown.is_some();
    hide(ui, target.is_none());
    ui.tooltips.target = target;
    let Some(t) = target else { return };
    let recent = ui
        .tooltips
        .last_hidden
        .is_some_and(|at| ui.now - at <= SWITCH_WINDOW);
    if had_popup || recent {
        show(ui, window, t);
    } else {
        let id = ui.after(APPEAR_DELAY, move |ui| {
            ui.tooltips.pending = None;
            if ui.tooltips.target == Some(t) {
                show(ui, window, t);
            }
        });
        ui.tooltips.pending = Some(id);
    }
}

/// Removes the bubble: it fades out (`fade`), or goes at once.
fn hide(ui: &mut Ui, fade: bool) {
    let Some((window, view)) = ui.tooltips.shown.take() else {
        return;
    };
    ui.tooltips.last_hidden = Some(ui.now);
    if fade {
        ui.with_widget::<Tooltip, _>(view, Tooltip::begin_leave);
        ui.start_animating(view);
        ui.tooltips.closing.push(window);
        ui.after(f64::from(LEAVE_FADE), move |ui| {
            ui.tooltips.closing.retain(|w| *w != window);
            ui.close_window(window);
        });
    } else {
        ui.close_window(window);
    }
}

fn show(ui: &mut Ui, parent: WindowId, target: ViewId) {
    let Some(text) = ui.tree.get(target).and_then(|n| n.tooltip.clone()) else {
        return;
    };
    let shortcut = ui
        .tree
        .widget(target)
        .and_then(|w| w.prop(Prop::Shortcut))
        .and_then(|v| v.as_str().map(str::to_string))
        .and_then(|s| Shortcut::parse(&s))
        .map(|s| s.display());
    let mut bubble = Tooltip::new(&text);
    bubble.shortcut = shortcut;
    let view = ui.create(bubble);
    let size = ui.measure(view, Proposal::UNSPECIFIED);
    let rect = ui.tree.window_rect(target);
    let bounds = ui.window(parent).map(|w| w.size).unwrap_or_default();
    let at = place(rect, size, bounds, Placement::Below, false);
    let window = ui.create_window(
        WindowKind::Popup {
            parent,
            anchor: Rect::from_origin_size(at, size),
            grab: false,
        },
        size,
    );
    ui.set_content(window, view);
    ui.release(view);
    ui.show_window(window);
    ui.tooltips.shown = Some((window, view));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::event::Mods;

    #[test]
    fn placement_floors_to_the_grid_and_flips() {
        let size = Size::new(116.0, 24.0);
        let bounds = Size::new(400.0, 300.0);
        let target = Rect::new(146.0, 100.0, 28.0, 28.0);
        assert_eq!(
            place(target, size, bounds, Placement::Below, false),
            Point::new(100.0, 136.0)
        );
        assert_eq!(
            place(target, size, bounds, Placement::Above, false),
            Point::new(100.0, 68.0)
        );
        assert_eq!(
            place(target, size, bounds, Placement::Leading, false),
            Point::new(22.0, 100.0)
        );
        assert_eq!(
            place(target, size, bounds, Placement::Trailing, true),
            Point::new(22.0, 100.0)
        );
        // Below the bottom edge: it flips above.
        let low = Rect::new(146.0, 270.0, 28.0, 28.0);
        assert_eq!(place(low, size, bounds, Placement::Below, false).y, 238.0);
        // Kept inside the area.
        let corner = Rect::new(0.0, 0.0, 28.0, 28.0);
        assert_eq!(place(corner, size, bounds, Placement::Below, false).x, 0.0);
    }

    #[test]
    fn bubble_sizes() {
        let mut ui = Ui::new_headless();
        let one = ui.create(Tooltip::new("Add to favourites"));
        let s = ui.measure(one, Proposal::UNSPECIFIED);
        assert_eq!(s.height, 24.0);
        assert_eq!((s.width - 16.0) % 4.0, 0.0);
        let long = ui.create(Tooltip::new(
            "Shows the recipe in a new window so you can keep this one open.",
        ));
        let s = ui.measure(long, Proposal::UNSPECIFIED);
        assert_eq!(s.width, 224.0);
        assert_eq!(s.height, 40.0);
    }

    fn scene() -> (Ui, WindowId, ViewId, ViewId) {
        use crate::amalfi::button::PushButton;
        use crate::core::layout::Stack;
        let mut ui = Ui::new_headless();
        let win = ui.create_window(WindowKind::Normal, Size::new(300.0, 200.0));
        let row = ui.create(Stack::row(40.0));
        let a = ui.create(PushButton::new("One"));
        let b = ui.create(PushButton::new("Two"));
        let _ = ui.set(a, Prop::Tooltip, Value::from("Add to favourites"));
        let _ = ui.set(b, Prop::Tooltip, Value::from("Remove"));
        ui.add_child(row, a);
        ui.add_child(row, b);
        ui.set_content(win, row);
        ui.show_window(win);
        ui.prepare_frame(win);
        (ui, win, a, b)
    }

    fn run(ui: &mut Ui, seconds: f64) {
        let mut t = 0.0;
        while t < seconds {
            ui.advance(0.05);
            t += 0.05;
        }
    }

    #[test]
    fn appears_after_the_delay_and_leaves_with_a_fade() {
        let (mut ui, win, a, b) = scene();
        let over_a = ui.tree.window_rect(a).center();
        ui.pointer_move(win, over_a, Mods::empty());
        run(&mut ui, 1.3);
        assert!(ui.tooltips.window().is_none());
        run(&mut ui, 0.4);
        let Some(popup) = ui.tooltips.window() else {
            panic!("no tooltip after the delay");
        };
        assert!(matches!(
            ui.window(popup).map(|w| w.kind.clone()),
            Some(WindowKind::Popup { grab: false, .. })
        ));
        // Moving to the other button shows its tooltip at once.
        let over_b = ui.tree.window_rect(b).center();
        ui.pointer_move(win, over_b, Mods::empty());
        let Some(second) = ui.tooltips.window() else {
            panic!("no tooltip while moving between buttons");
        };
        assert_ne!(second, popup);
        assert!(ui.window(popup).is_none());
        // Leaving: it fades for 450 ms, then goes.
        ui.pointer_leave(win);
        run(&mut ui, 0.2);
        assert!(ui.window(second).is_some());
        run(&mut ui, 0.4);
        assert!(ui.window(second).is_none());
        // A pointer that passes through without resting shows nothing.
        ui.pointer_move(win, over_a, Mods::empty());
        run(&mut ui, 0.5);
        ui.pointer_leave(win);
        run(&mut ui, 2.0);
        assert!(ui.tooltips.window().is_none());
    }
}
