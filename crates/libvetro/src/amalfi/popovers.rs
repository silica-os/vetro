//! Popovers (kit `Popovers`): the Glass/Thick surface with an arrow on one edge (or none,
//! detached), its content layouts, and [`present`], which shows a popover against an anchor.
//!
//! The arrow is 8 deep and 20 wide at its base (flanks at atan(8 / 10)), its base corners
//! rounded 4 and its tip 3; its centre is 30 from the start or the end of its edge, or in the
//! middle. The body's edge is 8 from the frame on the arrow's side.

use crate::amalfi::foundation::Label;
use crate::amalfi::tokens::typography;
use crate::core::a11y::{A11y, Role};
use crate::core::layout::{Alignment, Proposal, Stack};
use crate::core::tree::ViewId;
use crate::core::ui::Ui;
use crate::core::value::{Prop, Value};
use crate::core::widget::{LayoutCx, PaintCx, PropChange, Widget};
use crate::core::window::{WindowId, WindowKind};
use crate::geom::{Corners, Insets, Point, Rect, Size};
use crate::shape::{AMALFI_SMOOTHING, Shape, smooth_rect_path};
use crate::style::Style;
use skia_safe::{Matrix, Path, PathBuilder, PathOp};

/// Fixed measures of the kit.
pub mod metrics {
    /// The corner radius.
    pub const RADIUS: f32 = 16.0;
    /// How far the arrow sticks out of the body.
    pub const ARROW_DEPTH: f32 = 8.0;
    /// Half the width of the arrow's base.
    pub const ARROW_HALF_BASE: f32 = 10.0;
    /// The radius of the corners where the arrow meets the body.
    pub const ARROW_BASE_RADIUS: f32 = 4.0;
    /// The radius of the arrow's tip.
    pub const ARROW_TIP_RADIUS: f32 = 3.0;
    /// The arrow's distance from the start or end of its edge.
    pub const ARROW_INSET: f32 = 30.0;
    /// The space between the body's edge and a form or text content.
    pub const PADDING: f32 = 16.0;
    /// The space around a list.
    pub const LIST_PADDING: f32 = 8.0;
}

/// The edge of a popover that carries the arrow (the side facing what it points at).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    /// The popover is below its anchor.
    Top,
    /// The popover is above its anchor.
    Bottom,
    /// The popover is after (right of) its anchor.
    Leading,
    /// The popover is before (left of) its anchor.
    Trailing,
}

/// Where the arrow sits along its edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Align {
    /// 30 pt from the start of the edge.
    Start,
    /// In the middle.
    #[default]
    Center,
    /// 30 pt from the end of the edge.
    End,
}

/// The outline of a popover: the body and, unless `edge` is `None` (detached), the arrow.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PopoverShape {
    /// The edge with the arrow.
    pub edge: Option<Edge>,
    /// Where the arrow sits.
    pub align: Align,
    /// The arrow's centre from the start of its edge (overrides `align`).
    pub offset: Option<f32>,
}

impl PopoverShape {
    /// A detached popover (no arrow).
    pub const DETACHED: PopoverShape = PopoverShape {
        edge: None,
        align: Align::Center,
        offset: None,
    };

    /// An arrow on `edge`, at `align`.
    pub fn arrow(edge: Edge, align: Align) -> PopoverShape {
        PopoverShape {
            edge: Some(edge),
            align,
            offset: None,
        }
    }

    /// The space the arrow adds on each side: left, top, right, bottom.
    pub fn insets(&self) -> [f32; 4] {
        let d = metrics::ARROW_DEPTH;
        match self.edge {
            None => [0.0; 4],
            Some(Edge::Top) => [0.0, d, 0.0, 0.0],
            Some(Edge::Bottom) => [0.0, 0.0, 0.0, d],
            Some(Edge::Leading) => [d, 0.0, 0.0, 0.0],
            Some(Edge::Trailing) => [0.0, 0.0, d, 0.0],
        }
    }

    /// The body's rectangle in `rect`.
    pub fn body(&self, rect: Rect) -> Rect {
        let [l, t, r, b] = self.insets();
        Rect::new(
            rect.x + l,
            rect.y + t,
            (rect.width - l - r).max(0.0),
            (rect.height - t - b).max(0.0),
        )
    }

    /// The arrow's tip in `rect` (`None` when detached).
    pub fn tip(&self, rect: Rect) -> Option<Point> {
        let edge = self.edge?;
        let b = self.body(rect);
        let horizontal = matches!(edge, Edge::Top | Edge::Bottom);
        let extent = if horizontal { b.width } else { b.height };
        let from_start = self.offset.unwrap_or(match self.align {
            Align::Start => metrics::ARROW_INSET,
            Align::Center => extent / 2.0,
            Align::End => extent - metrics::ARROW_INSET,
        });
        let room = metrics::RADIUS + metrics::ARROW_HALF_BASE;
        let along = if extent < 2.0 * room {
            extent / 2.0
        } else {
            from_start.clamp(room, extent - room)
        };
        Some(match edge {
            Edge::Top => Point::new(b.x + along, rect.y),
            Edge::Bottom => Point::new(b.x + along, rect.bottom()),
            Edge::Leading => Point::new(rect.x, b.y + along),
            Edge::Trailing => Point::new(rect.right(), b.y + along),
        })
    }

    /// The outline in `rect`.
    pub fn path(&self, rect: Rect) -> Path {
        let body = smooth_rect_path(
            self.body(rect),
            Corners::all(metrics::RADIUS),
            AMALFI_SMOOTHING,
        );
        let (Some(edge), Some(tip)) = (self.edge, self.tip(rect)) else {
            return body;
        };
        // The arrow in a canonical frame (base on y = 0, pointing to +y, centred on x = 0),
        // turned onto its side.
        let angle = match edge {
            Edge::Bottom => 0.0_f32,
            Edge::Leading => std::f32::consts::FRAC_PI_2,
            Edge::Top => std::f32::consts::PI,
            Edge::Trailing => -std::f32::consts::FRAC_PI_2,
        };
        let d = metrics::ARROW_DEPTH;
        let base = Point::new(tip.x + angle.sin() * d, tip.y - angle.cos() * d);
        let mut m = Matrix::translate((base.x, base.y));
        m.pre_rotate(angle.to_degrees(), None);
        let arrow = arrow_path().make_transform(&m);
        body.op(&arrow, PathOp::Union).unwrap_or(body)
    }
}

/// The arrow in its canonical frame: base on y = 0 with a little overlap above it so the union
/// with the body is clean.
fn arrow_path() -> Path {
    let h = metrics::ARROW_HALF_BASE;
    let d = metrics::ARROW_DEPTH;
    let mut b = PathBuilder::new();
    let wing = 6.0;
    b.move_to((-h - wing, -1.0));
    b.line_to((-h - wing, 0.0));
    b.arc_to_tangent((-h, 0.0), (0.0, d), metrics::ARROW_BASE_RADIUS);
    b.arc_to_tangent((0.0, d), (h, 0.0), metrics::ARROW_TIP_RADIUS);
    b.arc_to_tangent((h, 0.0), (h + wing, 0.0), metrics::ARROW_BASE_RADIUS);
    b.line_to((h + wing, -1.0));
    b.close();
    b.detach()
}

/// A popover: a Glass/Thick surface in the outline of its [`PopoverShape`], its single child
/// (the content) inside the body.
#[derive(Debug)]
pub struct Popover {
    /// The outline.
    pub shape: PopoverShape,
    /// The space between the body's edge and the content.
    pub padding: f32,
}

impl Popover {
    /// A popover with `shape` and the padding of a form or text content.
    pub fn new(shape: PopoverShape) -> Popover {
        Popover {
            shape,
            padding: metrics::PADDING,
        }
    }

    /// With another padding (8 around a list).
    pub fn padded(mut self, padding: f32) -> Popover {
        self.padding = padding;
        self
    }

    fn layer_name(&self) -> &'static str {
        if self.shape.edge.is_some() {
            "Surface"
        } else {
            "Body"
        }
    }
}

impl Widget for Popover {
    fn kind(&self) -> &'static str {
        "Popover"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        let [l, t, r, b] = self.shape.insets();
        let pad = self.padding;
        let extra_w = l + r + 2.0 * pad;
        let extra_h = t + b + 2.0 * pad;
        let inner = Proposal::new(
            p.width
                .filter(|w| w.is_finite())
                .map(|w| (w - extra_w).max(0.0)),
            p.height
                .filter(|h| h.is_finite())
                .map(|h| (h - extra_h).max(0.0)),
        );
        let size = cx
            .children()
            .first()
            .map_or(Size::ZERO, |c| cx.measure(*c, inner));
        Size::new(size.width + extra_w, size.height + extra_h)
    }

    fn place(&mut self, cx: &mut LayoutCx<'_>, size: Size) {
        let body = self.shape.body(Rect::from_size(size));
        let pad = self.padding;
        if let Some(c) = cx.children().first().copied() {
            let w = (body.width - 2.0 * pad).max(0.0);
            let s = cx.measure(c, Proposal::new(Some(w), None));
            cx.place(c, Rect::new(body.x + pad, body.y + pad, w, s.height));
        }
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let r = cx.bounds();
        let local = Rect::from_size(r.size());
        let path = self.shape.path(local);
        let style: Style = theme.material_style("Glass/Thick");
        // Detached: the plain smooth rectangle (the arrowless path is the same outline).
        let shape = if self.shape.edge.is_some() {
            Shape::Path(path)
        } else {
            Shape::rounded(metrics::RADIUS)
        };
        cx.rec.layer(self.layer_name(), r, shape, style);
    }

    fn set_prop(&mut self, _prop: Prop, _value: &Value) -> PropChange {
        PropChange::Unknown
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Group;
    }
}

/// Makes a popover around `content`.
pub fn popover(ui: &mut Ui, shape: PopoverShape, padding: f32, content: ViewId) -> ViewId {
    let p = ui.create(Popover::new(shape).padded(padding));
    ui.add_child(p, content);
    p
}

/// A content column: children from the top left, `gap` apart, `top` pt below the top.
pub fn content_column(ui: &mut Ui, gap: f32, top: f32, children: &[ViewId]) -> ViewId {
    let c = ui.create(Stack::column(gap).aligned(Alignment::LEADING));
    if top > 0.0 {
        let _ = ui.set(
            c,
            Prop::Padding,
            Value::Insets(Insets {
                top,
                ..Insets::ZERO
            }),
        );
    }
    for v in children {
        ui.add_child(c, *v);
    }
    c
}

/// Where a popover sits against its anchor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// Below the anchor (the arrow on the popover's top edge).
    Below,
    /// Above the anchor.
    Above,
    /// After (right of) the anchor.
    After,
    /// Before (left of) the anchor.
    Before,
}

/// Shows `content` as a popover popup of `parent` against `anchor` (window coordinates): on
/// `side` when it fits, on the opposite side when it does not; the arrow points at the anchor's
/// centre and the popover is kept inside the window. Returns the popup.
pub fn present(
    ui: &mut Ui,
    parent: WindowId,
    anchor: Rect,
    content: ViewId,
    side: Side,
    padding: f32,
) -> WindowId {
    let bounds = ui.window(parent).map(|w| w.size).unwrap_or_default();
    let natural = ui.measure(content, Proposal::new(None, None));
    let measure = |_: &mut Ui, shape: PopoverShape, _: ViewId| -> Size {
        let [l, t, r, b] = shape.insets();
        Size::new(
            natural.width + l + r + 2.0 * padding,
            natural.height + t + b + 2.0 * padding,
        )
    };
    let fits = |side: Side, size: Size| match side {
        Side::Below => anchor.bottom() + size.height <= bounds.height,
        Side::Above => anchor.y - size.height >= 0.0,
        Side::After => anchor.right() + size.width <= bounds.width,
        Side::Before => anchor.x - size.width >= 0.0,
    };
    let edge_of = |side: Side| match side {
        Side::Below => Edge::Top,
        Side::Above => Edge::Bottom,
        Side::After => Edge::Leading,
        Side::Before => Edge::Trailing,
    };
    let opposite = |side: Side| match side {
        Side::Below => Side::Above,
        Side::Above => Side::Below,
        Side::After => Side::Before,
        Side::Before => Side::After,
    };
    let mut side = side;
    let mut shape = PopoverShape::arrow(edge_of(side), Align::Center);
    let mut size = measure(ui, shape, content);
    if !fits(side, size) && fits(opposite(side), size) {
        side = opposite(side);
        shape = PopoverShape::arrow(edge_of(side), Align::Center);
        size = measure(ui, shape, content);
    }
    let margin = 6.0;
    let centre = anchor.center();
    let mut at = match side {
        Side::Below => Point::new(centre.x - size.width / 2.0, anchor.bottom()),
        Side::Above => Point::new(centre.x - size.width / 2.0, anchor.y - size.height),
        Side::After => Point::new(anchor.right(), centre.y - size.height / 2.0),
        Side::Before => Point::new(anchor.x - size.width, centre.y - size.height / 2.0),
    };
    at.x = at.x.min(bounds.width - margin - size.width).max(margin);
    at.y = at.y.min(bounds.height - margin - size.height).max(margin);
    // The arrow points at the anchor's centre.
    let offset = match side {
        Side::Below | Side::Above => centre.x - at.x - shape.body(Rect::from_size(size)).x,
        Side::After | Side::Before => centre.y - at.y - shape.body(Rect::from_size(size)).y,
    };
    shape.offset = Some(offset);
    let p = ui.create(Popover::new(shape).padded(padding));
    ui.add_child(p, content);
    let window = ui.create_window(
        WindowKind::Popup {
            parent,
            anchor: Rect::from_origin_size(at, size),
            grab: true,
        },
        size,
    );
    ui.set_content(window, p);
    ui.show_window(window);
    window
}

/// A static title or body text of a popover's content.
pub fn text_label(ui: &mut Ui, text: &str, body: bool, layer: &str) -> ViewId {
    use crate::amalfi::text::{Text, TextRole};
    let t = if body {
        Text::new(text)
            .token(&typography::CALLOUT)
            .role(TextRole::Secondary)
            .layer(layer)
            .wrapping()
    } else {
        Text::new(text)
            .token(&typography::HEADLINE)
            .role(TextRole::Primary)
            .layer(layer)
            .whole(1.0, 0.0)
    };
    ui.create(t)
}

/// Keeps the label cache type reachable for content authors.
#[doc(hidden)]
pub type TextCache = Label;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::amalfi::text::Text;
    use crate::core::window::WindowKind;

    #[test]
    fn the_arrow_takes_8_on_its_side_and_sits_30_in_from_the_start() {
        let shape = PopoverShape::arrow(Edge::Top, Align::Start);
        let rect = Rect::new(0.0, 0.0, 260.0, 128.0);
        assert_eq!(shape.body(rect), Rect::new(0.0, 8.0, 260.0, 120.0));
        assert_eq!(shape.tip(rect), Some(Point::new(30.0, 0.0)));
        let end = PopoverShape::arrow(Edge::Leading, Align::End);
        assert_eq!(end.tip(rect), Some(Point::new(0.0, 8.0 + 120.0 - 30.0)));
        assert_eq!(PopoverShape::DETACHED.tip(rect), None);
    }

    #[test]
    fn the_tip_stays_clear_of_the_corners() {
        let mut shape = PopoverShape::arrow(Edge::Bottom, Align::Center);
        shape.offset = Some(2.0);
        let rect = Rect::new(0.0, 0.0, 260.0, 128.0);
        assert_eq!(shape.tip(rect), Some(Point::new(26.0, 128.0)));
    }

    fn parent() -> (Ui, WindowId) {
        let mut ui = Ui::new_headless();
        let win = ui.create_window(WindowKind::Normal, Size::new(400.0, 300.0));
        ui.show_window(win);
        (ui, win)
    }

    #[test]
    fn present_puts_the_popover_under_its_anchor_with_the_arrow_on_it() {
        let (mut ui, win) = parent();
        let content = ui.create(Text::new("Hello"));
        let anchor = Rect::new(100.0, 40.0, 40.0, 20.0);
        let popup = present(&mut ui, win, anchor, content, Side::Below, 16.0);
        let WindowKind::Popup { anchor: at, .. } = ui
            .window(popup)
            .map(|w| w.kind.clone())
            .unwrap_or(WindowKind::Normal)
        else {
            panic!("not a popup");
        };
        assert_eq!(at.y, anchor.bottom());
        let centre = at.x + at.width / 2.0;
        assert!((centre - anchor.center().x).abs() < 1.0);
    }

    #[test]
    fn present_flips_to_the_other_side_when_it_does_not_fit() {
        let (mut ui, win) = parent();
        let content = ui.create(Text::new("Hello"));
        let anchor = Rect::new(100.0, 280.0, 40.0, 20.0);
        let popup = present(&mut ui, win, anchor, content, Side::Below, 16.0);
        let WindowKind::Popup { anchor: at, .. } = ui
            .window(popup)
            .map(|w| w.kind.clone())
            .unwrap_or(WindowKind::Normal)
        else {
            panic!("not a popup");
        };
        assert!(at.bottom() <= anchor.y + 0.01);
    }
}
