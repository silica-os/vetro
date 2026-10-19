//! Windows (kit `Windows`): the window frame (shadow, clip, surface and rim), the filled blocks
//! the kit draws inside it (sidebar and inspector regions, separators, the sheet's scrim) and
//! the container the toolbars, sidebars and content sit in.

use crate::core::a11y::{A11y, Role};
use crate::core::layout::Proposal;
use crate::core::widget::{LayoutCx, PaintCx, Widget};
use crate::geom::{Corners, Point, Rect, Size};
use crate::shape::{AMALFI_SMOOTHING, Shape};
use crate::style::{Border, Style};

/// Fixed measures of the kit.
pub mod metrics {
    /// A window's corner radius.
    pub const RADIUS: f32 = 18.0;
    /// A utility window's corner radius.
    pub const UTILITY_RADIUS: f32 = 14.0;
}

/// A window's frame: four layers under the content (the shadow, the clipping shape, the
/// surface) and the rim over it. The children keep the places `Prop::Offset` gives them from
/// the frame's origin and their own sizes.
#[derive(Debug)]
pub struct WindowFrame {
    /// The corner radius.
    pub radius: f32,
    /// The frame's size when nothing proposes one.
    pub size: Size,
}

impl WindowFrame {
    /// A frame of `size` with the standard radius.
    pub fn new(size: Size) -> WindowFrame {
        WindowFrame {
            radius: metrics::RADIUS,
            size,
        }
    }

    /// A utility window's frame (radius 14).
    pub fn utility(size: Size) -> WindowFrame {
        WindowFrame {
            radius: metrics::UTILITY_RADIUS,
            size,
        }
    }
}

impl Widget for WindowFrame {
    fn kind(&self) -> &'static str {
        "WindowFrame"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        for c in cx.children() {
            let _ = cx.measure(c, Proposal::new(None, None));
        }
        Size::new(
            p.width.filter(|w| w.is_finite()).unwrap_or(self.size.width),
            p.height
                .filter(|h| h.is_finite())
                .unwrap_or(self.size.height),
        )
    }

    fn place(&mut self, cx: &mut LayoutCx<'_>, _size: Size) {
        for c in cx.children() {
            let s = cx.measure(c, Proposal::new(None, None));
            cx.place(c, Rect::from_size(s));
        }
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let c = theme.colors;
        let r = cx.bounds();
        let round = Shape::rounded(self.radius);
        let mut shadow = Style::fill(c.surface_window);
        if let Some(s) = theme.shadow(3) {
            shadow.shadows.push(s);
        }
        cx.rec.layer("Window shadow", r, round.clone(), shadow);
        cx.rec
            .layer("Window clip", r, round, Style::fill(c.surface_window));
        cx.rec.layer(
            "Window surface",
            r,
            Shape::RECT,
            Style::fill(c.surface_window),
        );
    }

    fn paint_over(&mut self, cx: &mut PaintCx<'_>) {
        let c = cx.theme().colors;
        cx.rec.layer(
            "Window rim",
            cx.bounds(),
            Shape::rounded(self.radius),
            Style::default().with_border(Border::inside(c.line_border, 1.0)),
        );
    }

    fn hit(&self, _p: Point, _size: Size) -> bool {
        false
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Window;
    }
}

/// What a block is filled with.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BlockFill {
    /// `Surface/Window`.
    Window,
    /// `Surface/Sidebar` (the sidebar and inspector regions).
    Sidebar,
    /// `Line/Separator` (a 1 pt rule).
    Separator,
    /// The sheet's scrim: `Text/Primary` of the Light appearance at this opacity, over the
    /// window's content, rounded at the bottom.
    Scrim(f32),
}

/// A filled rectangle with a layer name of the kit's choosing; its size is `Prop::Width` and
/// `Prop::Height`.
#[derive(Debug)]
pub struct Block {
    /// The layer's name (`Sidebar region`, `Separator`, `Scrim` …).
    pub layer: String,
    /// What it is filled with.
    pub fill: BlockFill,
    /// The corner radius at the bottom (the scrim).
    pub bottom_radius: f32,
}

impl Block {
    /// A block named `layer`.
    pub fn new(layer: &str, fill: BlockFill) -> Block {
        Block {
            layer: layer.to_string(),
            fill,
            bottom_radius: 0.0,
        }
    }

    /// Rounded at the bottom corners.
    pub fn rounded_bottom(mut self, radius: f32) -> Block {
        self.bottom_radius = radius;
        self
    }
}

impl Widget for Block {
    fn kind(&self) -> &'static str {
        "Block"
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        Size::new(
            p.width.filter(|w| w.is_finite()).unwrap_or(0.0),
            p.height.filter(|h| h.is_finite()).unwrap_or(0.0),
        )
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let c = theme.colors;
        let style = match self.fill {
            BlockFill::Window => Style::fill(c.surface_window),
            BlockFill::Sidebar => Style::fill(c.surface_sidebar),
            BlockFill::Separator => Style::fill(c.line_separator),
            BlockFill::Scrim(opacity) => {
                Style::fill(crate::amalfi::tokens::LIGHT.text_primary).with_opacity(opacity)
            }
        };
        let shape = if self.bottom_radius > 0.0 {
            Shape::Rect {
                corners: Corners {
                    top_left: 0.0,
                    top_right: 0.0,
                    bottom_right: self.bottom_radius,
                    bottom_left: self.bottom_radius,
                },
                smoothing: AMALFI_SMOOTHING,
            }
        } else {
            Shape::RECT
        };
        cx.rec.layer(&self.layer, cx.bounds(), shape, style);
    }

    fn hit(&self, _p: Point, _size: Size) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ui::Ui;
    use crate::core::value::{Prop, Value};
    use crate::core::window::WindowKind;

    #[test]
    fn the_frame_paints_shadow_clip_and_surface_under_the_children_and_the_rim_over() {
        let mut ui = Ui::new_headless();
        let win = ui.create_window(WindowKind::Normal, Size::new(200.0, 100.0));
        let frame = ui.create(WindowFrame::new(Size::new(200.0, 100.0)));
        let b = ui.create(Block::new("Separator", BlockFill::Separator));
        let _ = ui.set(b, Prop::Width, Value::from(200.0));
        let _ = ui.set(b, Prop::Height, Value::from(1.0));
        let _ = ui.set(b, Prop::Offset, Value::Point(Point::new(0.0, 40.0)));
        ui.add_child(frame, b);
        ui.set_content(win, frame);
        ui.show_window(win);
        ui.prepare_frame(win);
        let names: Vec<String> = ui
            .inspect_layers(frame)
            .into_iter()
            .map(|l| l.path)
            .collect();
        assert_eq!(
            names,
            [
                "Window shadow",
                "Window clip",
                "Window surface",
                "Separator",
                "Window rim"
            ]
        );
    }
}
