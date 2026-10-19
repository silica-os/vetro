//! The group box (kit `Group Boxes/Box`): related controls on a raised surface, with an
//! optional title above it. [`group_box`] makes the two views it is built of: the group (the
//! title) and its `Box` (the surface and the column of controls).

use crate::amalfi::foundation::{Label, whole};
use crate::amalfi::tokens::typography;
use crate::core::a11y::{A11y, Role};
use crate::core::layout::Proposal;
use crate::core::tree::ViewId;
use crate::core::ui::Ui;
use crate::core::value::{Prop, Value};
use crate::core::widget::{LayoutCx, PaintCx, PropChange, Widget};
use crate::geom::{Point, Rect, Size};
use crate::shape::Shape;
use crate::style::{Border, Style};
use crate::text::Align;

/// Fixed measures (silica_ui's `AmalfiGroupBoxMetrics`).
pub mod metrics {
    /// Space between the title and the box.
    pub const TITLE_GAP: f32 = 6.0;
    /// The box's padding.
    pub const PADDING: f32 = 12.0;
    /// Space between the controls.
    pub const SPACING: f32 = 8.0;
    /// The box's corner radius.
    pub const RADIUS: f32 = 12.0;
    /// The title's height.
    pub const TITLE_HEIGHT: f32 = 18.0;
}

/// The surface of a group box and its column of controls.
#[derive(Debug, Default)]
pub struct BoxSurface;

impl Widget for BoxSurface {
    fn kind(&self) -> &'static str {
        "BoxSurface"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        let pad = metrics::PADDING;
        let inner = p
            .width
            .filter(|w| w.is_finite())
            .map(|w| (w - 2.0 * pad).max(0.0));
        let mut width = 0.0f32;
        let mut height = 0.0f32;
        let children = cx.children();
        for (i, c) in children.iter().enumerate() {
            let s = cx.measure(*c, Proposal::new(inner, None));
            width = width.max(s.width);
            height += s.height;
            if i > 0 {
                height += metrics::SPACING;
            }
        }
        Size::new(inner.unwrap_or(width) + 2.0 * pad, height + 2.0 * pad)
    }

    fn place(&mut self, cx: &mut LayoutCx<'_>, size: Size) {
        let pad = metrics::PADDING;
        let inner = (size.width - 2.0 * pad).max(0.0);
        let mut y = pad;
        for c in cx.children() {
            let s = cx.measure(c, Proposal::new(Some(inner), None));
            cx.place(
                c,
                Rect::new(pad, y, s.width.min(inner.max(s.width)), s.height),
            );
            y += s.height + metrics::SPACING;
        }
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let c = cx.theme().colors;
        let b = cx.bounds();
        cx.rec.layer(
            "Surface",
            b,
            Shape::rounded(metrics::RADIUS),
            Style::fill(c.surface_raised).with_border(Border::inside(c.line_border, 1.0)),
        );
    }

    fn hit(&self, _p: Point, _size: Size) -> bool {
        false
    }
}

/// A group box: an optional title over a [`BoxSurface`] (its only child, `Box`).
#[derive(Debug, Default)]
pub struct GroupBox {
    /// The title.
    pub title: Option<String>,
    label: Label,
}

impl GroupBox {
    /// A group box with a title.
    pub fn new(title: Option<&str>) -> GroupBox {
        GroupBox {
            title: title.map(str::to_string),
            label: Label::default(),
        }
    }

    fn top(&self) -> f32 {
        if self.title.is_some() {
            metrics::TITLE_HEIGHT + metrics::TITLE_GAP
        } else {
            0.0
        }
    }
}

impl Widget for GroupBox {
    fn kind(&self) -> &'static str {
        "GroupBox"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        let top = self.top();
        let mut size = Size::ZERO;
        if let Some(c) = cx.children().first().copied() {
            size = cx.measure(c, p);
        }
        Size::new(size.width, size.height + top)
    }

    fn place(&mut self, cx: &mut LayoutCx<'_>, size: Size) {
        let top = self.top();
        if let Some(c) = cx.children().first().copied() {
            let w = size.width;
            let s = cx.measure(c, Proposal::new(Some(w), None));
            cx.place(c, Rect::new(0.0, top, w, s.height));
        }
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let Some(title) = self.title.clone() else {
            return;
        };
        let theme = *cx.theme();
        let b = cx.bounds();
        let style = theme.text(&typography::HEADLINE, theme.colors.text_secondary);
        let w = whole(self.label.width(&title, &style), 4.0).min(b.width);
        let layout = self.label.layout(&title, &style, Align::Start, w, Some(1));
        cx.rec.text(
            "Title",
            Rect::new(b.x, b.y, w, metrics::TITLE_HEIGHT),
            layout,
            Some(typography::HEADLINE.name),
            style.color,
        );
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match (prop, value) {
            (Prop::Title, Value::Str(s)) => {
                self.title = Some(s.clone()).filter(|s| !s.is_empty());
                PropChange::Layout
            }
            (Prop::Title, Value::None) => {
                self.title = None;
                PropChange::Layout
            }
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::Title => Some(self.title.clone().map(Value::Str).unwrap_or_default()),
            _ => None,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::GroupBox;
        a.label.clone_from(&self.title);
    }

    fn hit(&self, _p: Point, _size: Size) -> bool {
        false
    }
}

/// Makes a group box over `children` (views already made, in order).
pub fn group_box(ui: &mut Ui, title: Option<&str>, children: &[ViewId]) -> ViewId {
    let group = ui.create(GroupBox::new(title));
    let surface = ui.create(BoxSurface);
    let _ = ui.set(surface, Prop::Identifier, Value::from("Box"));
    for c in children {
        ui.add_child(surface, *c);
    }
    ui.add_child(group, surface);
    group
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::amalfi::toggles::Toggle;

    #[test]
    fn rows_are_12_in_and_8_apart() {
        let mut ui = Ui::new_headless();
        let rows: Vec<_> = ["Show banners", "Play sound", "Lock screen"]
            .iter()
            .map(|l| ui.create(Toggle::checkbox().label(l)))
            .collect();
        let g = group_box(&mut ui, Some("Notifications"), &rows);
        let s = ui.measure(g, Proposal::UNSPECIFIED);
        // 18 + 6 + 12 + 3 x 20 + 2 x 8 + 12
        assert_eq!(s.height, 124.0);
    }
}
