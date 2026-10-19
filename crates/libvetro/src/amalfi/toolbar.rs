//! A window's toolbar (kit `Titlebars/Bar/Unified`): 52 tall on `Surface/Window`, its items
//! laid out as the kit's pieces — the window controls' piece first (96: Ossido draws the
//! buttons over the app's full-size content), then each item with 4 pt on either side (so
//! capsules are 8 apart), flexible spaces sharing what is left, and 12 more after the last
//! piece (16 in all, `components.md` "Toolbar items").

use crate::amalfi::titlebars;
use crate::core::a11y::{A11y, Role};
use crate::core::layout::Proposal;
use crate::core::tree::ViewId;
use crate::core::ui::Ui;
use crate::core::value::{Prop, Value};
use crate::core::widget::{LayoutCx, PaintCx, PropChange, Widget};
use crate::geom::{Point, Rect, Size};
use crate::shape::Shape;
use crate::style::Style;

/// Measures of the toolbar.
pub mod metrics {
    /// The bar's height.
    pub const HEIGHT: f32 = 52.0;
    /// The window controls' piece.
    pub const CONTROLS: f32 = 96.0;
    /// A piece's padding on either side of its item.
    pub const PIECE_PAD: f32 = 4.0;
    /// After the last piece (16 from the edge with its padding).
    pub const TRAILING: f32 = 12.0;
    /// Before the first piece when there are no window controls (a sheet's toolbar).
    pub const LEADING: f32 = 12.0;
}

/// The toolbar.
#[derive(Debug)]
pub struct Toolbar {
    /// Room is kept for the window controls.
    pub controls: bool,
}

impl Default for Toolbar {
    fn default() -> Self {
        Toolbar { controls: true }
    }
}

fn flexible(ui: &Ui, id: ViewId) -> bool {
    match ui.tree.get(id).map(|n| n.kind()) {
        Some("Spacer") => ui
            .widget::<titlebars::Spacer>(id)
            .is_none_or(|s| s.flexible),
        _ => false,
    }
}

impl Toolbar {
    fn leading(&self) -> f32 {
        if self.controls {
            metrics::CONTROLS
        } else {
            metrics::LEADING
        }
    }

    /// Each item, its natural size and whether it flexes.
    fn items(&self, cx: &mut LayoutCx<'_>) -> Vec<(ViewId, Size, bool)> {
        let h = metrics::HEIGHT;
        cx.children()
            .into_iter()
            .map(|c| {
                let flex = flexible(cx.ui, c);
                let s = if flex {
                    Size::new(0.0, 0.0)
                } else {
                    cx.measure(c, Proposal::new(None, Some(h)))
                };
                (c, s, flex)
            })
            .collect()
    }
}

impl Widget for Toolbar {
    fn kind(&self) -> &'static str {
        "Toolbar"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        let items = self.items(cx);
        let natural = self.leading()
            + metrics::TRAILING
            + items
                .iter()
                .map(|(_, s, _)| s.width + 2.0 * metrics::PIECE_PAD)
                .sum::<f32>();
        Size::new(
            p.width.filter(|w| w.is_finite()).unwrap_or(natural),
            metrics::HEIGHT,
        )
    }

    fn place(&mut self, cx: &mut LayoutCx<'_>, size: Size) {
        let items = self.items(cx);
        let pads = 2.0 * metrics::PIECE_PAD;
        let fixed: f32 = items
            .iter()
            .filter(|i| !i.2)
            .map(|i| i.1.width + pads)
            .sum();
        let flex = items.iter().filter(|i| i.2).count();
        let free = (size.width - self.leading() - metrics::TRAILING - fixed).max(0.0);
        let share = if flex > 0 { free / flex as f32 } else { 0.0 };
        let mut x = self.leading();
        for (c, s, f) in items {
            if f {
                cx.place(c, Rect::new(x, 0.0, share, size.height));
                x += share;
                continue;
            }
            let h = s.height.min(size.height);
            cx.place(
                c,
                Rect::new(
                    x + metrics::PIECE_PAD,
                    ((size.height - h) / 2.0).round(),
                    s.width,
                    h,
                ),
            );
            x += s.width + pads;
        }
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let c = cx.theme().colors;
        let r = cx.bounds();
        cx.rec
            .layer("Surface", r, Shape::RECT, Style::fill(c.surface_window));
    }

    fn hit(&self, _p: Point, _size: Size) -> bool {
        false
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match prop {
            // 0: the window's toolbar (room for the controls), 1: no controls.
            Prop::Style => match value.as_int() {
                Some(s) => {
                    self.controls = s == 0;
                    PropChange::Layout
                }
                None => PropChange::Unknown,
            },
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::Style => Some(Value::Int(if self.controls { 0 } else { 1 })),
            _ => None,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Toolbar;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::amalfi::buttons::ToolbarButton;
    use crate::amalfi::segments::{ActionSegments, Segment};
    use crate::core::window::WindowKind;

    #[test]
    fn items_sit_as_the_kits_pieces() {
        let mut ui = Ui::new_headless();
        let bar = ui.create(Toolbar::default());
        let nav = ui.create(ActionSegments::new(vec![
            Segment::icon("chevron-left", "Back"),
            Segment::icon("chevron-right", "Forward"),
        ]));
        let space = ui.create(titlebars::Spacer::flexible(8.0));
        let share = ui.create(ToolbarButton::new("square-and-arrow-up"));
        for v in [nav, space, share] {
            ui.add_child(bar, v);
        }
        ui.set(bar, Prop::MaxWidth, Value::from(f32::INFINITY))
            .unwrap();
        let w = ui.create_window(WindowKind::Normal, Size::new(760.0, 300.0));
        ui.set_content(w, bar);
        ui.show_window(w);
        ui.prepare_frame(w);
        let r = |ui: &Ui, v| ui.tree.window_rect(v);
        let b = r(&ui, bar);
        assert_eq!(b.height, 52.0);
        // Back and forward: 96 + 4, 8 down, 73 × 36 (kit `Piece/Nav`).
        assert_eq!(r(&ui, nav).x - b.x, 100.0);
        assert_eq!(r(&ui, nav).y - b.y, 8.0);
        assert_eq!(r(&ui, nav).size(), Size::new(73.0, 36.0));
        // The last item ends 16 from the edge.
        let s = r(&ui, share);
        assert_eq!(b.right() - s.right(), 16.0);
    }
}
