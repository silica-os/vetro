//! Dialogs (kit `Dialogs`): the dialog panel (a title and its parts, stacked), the file browser
//! (places and file list panes) and the footer row of the open and save panels.

use crate::amalfi::sidebars;
use crate::core::a11y::{A11y, Role};
use crate::core::event::{Event, Handled, Key};
use crate::core::layout::{Alignment, Proposal, Spacer, Stack};
use crate::core::tree::ViewId;
use crate::core::ui::Ui;
use crate::core::value::{Prop, Signal, Value};
use crate::core::widget::{EventCx, FocusKind, LayoutCx, PaintCx, Widget};
use crate::geom::{Insets, Rect, Size};
use crate::shape::Shape;
use crate::style::{Border, Style};

/// Fixed measures of the kit.
pub mod metrics {
    /// The dialog's corner radius.
    pub const RADIUS: f32 = 20.0;
    /// Gap between the parts of a form dialog.
    pub const FORM_GAP: f32 = 20.0;
    /// Gap between the parts of a file panel.
    pub const PANEL_GAP: f32 = 12.0;
    /// A form dialog's width.
    pub const FORM_WIDTH: f32 = 400.0;
    /// A file panel's width.
    pub const PANEL_WIDTH: f32 = 600.0;
    /// The browser's height.
    pub const BROWSER_HEIGHT: f32 = 152.0;
    /// The browser's gap between its panes.
    pub const BROWSER_GAP: f32 = 16.0;
    /// The places pane's width.
    pub const PLACES_WIDTH: f32 = 216.0;
    /// The padding inside a browser pane.
    pub const PANE_PADDING: f32 = 8.0;
    /// The places pane's corner radius.
    pub const PLACES_RADIUS: f32 = 14.0;
    /// The file list's corner radius.
    pub const LIST_RADIUS: f32 = 12.0;
}

/// The dialog panel: a raised surface with a level 3 shadow; its children, top to bottom, sit
/// at the left padding, each at its own width.
#[derive(Debug)]
pub struct Dialog {
    /// Space around the content.
    pub padding: Insets,
    /// Gap between the children.
    pub gap: f32,
    /// The dialog's width.
    pub width: f32,
}

impl Dialog {
    /// A form dialog (a title, sections and a footer): 26 above, 24 at the other sides.
    pub fn form() -> Dialog {
        Dialog {
            padding: Insets {
                top: 26.0,
                left: 24.0,
                bottom: 24.0,
                right: 24.0,
            },
            gap: metrics::FORM_GAP,
            width: metrics::FORM_WIDTH,
        }
    }

    /// A file panel (open or save): 22 above, 20 at the other sides.
    pub fn panel() -> Dialog {
        Dialog {
            padding: Insets {
                top: 22.0,
                left: 20.0,
                bottom: 20.0,
                right: 20.0,
            },
            gap: metrics::PANEL_GAP,
            width: metrics::PANEL_WIDTH,
        }
    }
}

impl Widget for Dialog {
    fn kind(&self) -> &'static str {
        "Dialog"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        let w = match p.width {
            Some(w) if w.is_finite() => w,
            _ => self.width,
        };
        let inner = (w - self.padding.left - self.padding.right).max(0.0);
        let mut h = self.padding.top + self.padding.bottom;
        for (i, c) in cx.children().into_iter().enumerate() {
            if i > 0 {
                h += self.gap;
            }
            h += cx.measure(c, Proposal::new(Some(inner), None)).height;
        }
        Size::new(w, h)
    }

    fn place(&mut self, cx: &mut LayoutCx<'_>, size: Size) {
        let inner = (size.width - self.padding.left - self.padding.right).max(0.0);
        let mut y = self.padding.top;
        for c in cx.children() {
            let s = cx.measure(c, Proposal::new(Some(inner), None));
            cx.place(c, Rect::new(self.padding.left, y, s.width, s.height));
            y += s.height + self.gap;
        }
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let c = theme.colors;
        let mut style =
            Style::fill(c.surface_raised).with_border(Border::inside(c.line_border, 1.0));
        if let Some(s) = theme.shadow(3) {
            style.shadows.push(s);
        }
        cx.rec.layer(
            "Surface",
            cx.bounds(),
            Shape::rounded(metrics::RADIUS),
            style,
        );
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        match event {
            Event::Key(k) if k.pressed && k.key == Key::Escape => {
                cx.emit(Signal::CloseRequested, Value::None);
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn focus(&self) -> FocusKind {
        FocusKind::Always
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Dialog;
    }
}

/// What a browser pane is filled with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneFill {
    /// The places pane: `Surface/Sidebar`, radius 14.
    Places,
    /// The file list: `Surface/Raised`, radius 12.
    Files,
}

/// A browser pane: a bordered surface with its children stacked 8 pt in.
#[derive(Debug)]
pub struct Pane {
    /// What it is filled with.
    pub fill: PaneFill,
}

impl Widget for Pane {
    fn kind(&self) -> &'static str {
        "BrowserPane"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        let pad = metrics::PANE_PADDING;
        let w = p
            .width
            .filter(|w| w.is_finite())
            .unwrap_or(metrics::PLACES_WIDTH);
        let inner = (w - 2.0 * pad).max(0.0);
        let mut h = 2.0 * pad;
        for c in cx.children() {
            h += cx.measure(c, Proposal::new(Some(inner), None)).height;
        }
        Size::new(w, p.height.filter(|h| h.is_finite()).unwrap_or(h))
    }

    fn place(&mut self, cx: &mut LayoutCx<'_>, size: Size) {
        let pad = metrics::PANE_PADDING;
        let inner = (size.width - 2.0 * pad).max(0.0);
        let mut y = pad;
        for c in cx.children() {
            let s = cx.measure(c, Proposal::new(Some(inner), None));
            cx.place(c, Rect::new(pad, y, s.width, s.height));
            y += s.height;
        }
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let c = cx.theme().colors;
        let (fill, radius) = match self.fill {
            PaneFill::Places => (c.surface_sidebar, metrics::PLACES_RADIUS),
            PaneFill::Files => (c.surface_raised, metrics::LIST_RADIUS),
        };
        cx.rec.layer(
            "Surface",
            cx.bounds(),
            Shape::rounded(radius),
            Style::fill(fill).with_border(Border::inside(c.line_border, 1.0)),
        );
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::List;
    }
}

/// Makes the file browser: the places pane (a sidebar group, 216 wide) and the file list (the
/// rest of `width`), `BROWSER_GAP` apart, `BROWSER_HEIGHT` high. The panes are named `Places`
/// and `File list`.
pub fn browser(ui: &mut Ui, width: f32, places: &[ViewId], files: &[ViewId]) -> ViewId {
    let row = ui.create(Stack::row(metrics::BROWSER_GAP).aligned(Alignment::TOP_LEADING));
    let _ = ui.set(row, Prop::Width, Value::from(width));
    let list_width = width - metrics::PLACES_WIDTH - metrics::BROWSER_GAP;
    for (name, fill, w, kids) in [
        ("Places", PaneFill::Places, metrics::PLACES_WIDTH, places),
        ("File list", PaneFill::Files, list_width, files),
    ] {
        let pane = ui.create(Pane { fill });
        let _ = ui.set(pane, Prop::Width, Value::from(w));
        let _ = ui.set(pane, Prop::Height, Value::from(metrics::BROWSER_HEIGHT));
        let _ = ui.set(pane, Prop::Identifier, Value::from(name));
        for k in kids {
            let _ = ui.set(
                *k,
                Prop::Width,
                Value::from(w - 2.0 * metrics::PANE_PADDING),
            );
            ui.add_child(pane, *k);
        }
        ui.add_child(row, pane);
    }
    row
}

/// Makes a file row of the browser's list (an icon and a name).
pub fn file_row(ui: &mut Ui, icon: &str, name: &str, selected: bool) -> ViewId {
    let r = sidebars::row(ui, icon, name, None, false);
    ui.with_widget::<sidebars::Row, _>(r, |row| row.selected = selected);
    r
}

/// Makes the footer row of the open and save panels: `leading` buttons at the start, a
/// flexible gap, `trailing` buttons at the end, 8 pt apart, `width` wide.
pub fn dialog_footer(ui: &mut Ui, width: f32, leading: &[ViewId], trailing: &[ViewId]) -> ViewId {
    let row = ui.create(Stack::row(8.0));
    let _ = ui.set(row, Prop::Width, Value::from(width));
    for v in leading {
        ui.add_child(row, *v);
    }
    let spacer = ui.create(Spacer::default());
    ui.add_child(row, spacer);
    for v in trailing {
        ui.add_child(row, *v);
    }
    row
}

/// Makes a dialog around `children` (top to bottom).
pub fn dialog(ui: &mut Ui, shape: Dialog, children: &[ViewId]) -> ViewId {
    let d = ui.create(shape);
    for c in children {
        ui.add_child(d, *c);
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::amalfi::text::Text;
    use crate::core::event::{KeyEvent, Mods};
    use crate::core::window::WindowKind;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn a_form_dialog_stacks_its_parts_26_in_with_20_between() {
        let mut ui = Ui::new_headless();
        let win = ui.create_window(WindowKind::Normal, Size::new(400.0, 300.0));
        let a = ui.create(Text::new("Share recipe"));
        let b = ui.create(Text::new("Body"));
        let d = dialog(&mut ui, Dialog::form(), &[a, b]);
        ui.set_content(win, d);
        ui.show_window(win);
        ui.prepare_frame(win);
        let origin = ui.tree.window_rect(d);
        let ra = ui.tree.window_rect(a);
        let rb = ui.tree.window_rect(b);
        assert_eq!(ra.x - origin.x, 24.0);
        assert_eq!(ra.y - origin.y, 26.0);
        assert_eq!(rb.y - origin.y, 26.0 + ra.height + 20.0);
        assert_eq!(origin.width, 400.0);
    }

    #[test]
    fn escape_asks_the_dialog_to_close() {
        let mut ui = Ui::new_headless();
        let win = ui.create_window(WindowKind::Normal, Size::new(400.0, 300.0));
        let d = dialog(&mut ui, Dialog::panel(), &[]);
        let log = Rc::new(RefCell::new(0));
        let l = log.clone();
        ui.connect(d, Signal::CloseRequested, move |_, _, _| {
            *l.borrow_mut() += 1;
        });
        ui.set_content(win, d);
        ui.show_window(win);
        ui.set_focus(d, true);
        ui.key(win, KeyEvent::press(Key::Escape, Mods::empty()));
        assert_eq!(*log.borrow(), 1);
    }
}
