//! Forms (kit `Forms`): rows (a right-aligned label, a control, a help or error message),
//! sections (a titled group box of rows), the footer of actions and the layout that stacks
//! them in a window.

use crate::amalfi::foundation::{Label, whole};
use crate::amalfi::group_box::group_box_with;
use crate::amalfi::text::{Text, TextRole};
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

/// Fixed measures (silica_ui's `AmalfiFormMetrics`).
pub mod metrics {
    /// The label column's width.
    pub const LABEL_WIDTH: f32 = 112.0;
    /// Gap between the label and the control.
    pub const LABEL_GAP: f32 = 12.0;
    /// The value column's width.
    pub const VALUE_WIDTH: f32 = 204.0;
    /// A row's width.
    pub const ROW_WIDTH: f32 = LABEL_WIDTH + LABEL_GAP + VALUE_WIDTH;
    /// A row's line height.
    pub const LINE_HEIGHT: f32 = 28.0;
    /// Gap between a control and its message.
    pub const MESSAGE_GAP: f32 = 4.0;
    /// Space between rows.
    pub const ROW_GAP: f32 = 12.0;
    /// A section's padding.
    pub const SECTION_PADDING: f32 = 12.0;
    /// A section's width.
    pub const SECTION_WIDTH: f32 = ROW_WIDTH + 2.0 * SECTION_PADDING;
    /// Gap between the footer's buttons.
    pub const FOOTER_GAP: f32 = 8.0;
    /// The footer's buttons are at least this wide.
    pub const BUTTON_MIN_WIDTH: f32 = 72.0;
    /// The layout's padding: left, top, right, bottom.
    pub const LAYOUT_PADDING: (f32, f32, f32, f32) = (24.0, 26.0, 24.0, 24.0);
    /// Space between the layout's parts.
    pub const LAYOUT_GAP: f32 = 20.0;
    /// The layout's width.
    pub const LAYOUT_WIDTH: f32 = 400.0;
    /// The layout's corner radius.
    pub const LAYOUT_RADIUS: f32 = 18.0;
    /// The layout title's height.
    pub const LAYOUT_TITLE: f32 = 22.0;
}

/// What a message under a control says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MessageKind {
    /// Help (tertiary text).
    #[default]
    Help,
    /// An error (secondary text).
    Error,
}

/// The value column of a row: the control, and under it the message as wide as the control.
#[derive(Debug, Default)]
pub struct FormValue {
    /// The control fills the 204 pt column.
    pub expand: bool,
}

impl FormValue {
    fn control_size(&self, cx: &mut LayoutCx<'_>, control: ViewId) -> Size {
        if self.expand {
            let s = cx.measure(control, Proposal::new(Some(metrics::VALUE_WIDTH), None));
            Size::new(metrics::VALUE_WIDTH, s.height)
        } else {
            cx.measure(control, Proposal::UNSPECIFIED)
        }
    }
}

impl Widget for FormValue {
    fn kind(&self) -> &'static str {
        "FormValue"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        let kids = cx.children();
        let Some(control) = kids.first().copied() else {
            return Size::ZERO;
        };
        let cs = self.control_size(cx, control);
        let top = 0.0;
        let mut h = metrics::LINE_HEIGHT.max(top + cs.height);
        if let Some(m) = kids.get(1).copied() {
            let ms = cx.measure(m, Proposal::new(Some(cs.width), None));
            h = h.max(top + cs.height + metrics::MESSAGE_GAP + ms.height);
        }
        Size::new(cs.width, h)
    }

    fn place(&mut self, cx: &mut LayoutCx<'_>, _size: Size) {
        let kids = cx.children();
        let Some(control) = kids.first().copied() else {
            return;
        };
        let cs = self.control_size(cx, control);
        let top = 0.0;
        cx.place(control, Rect::new(0.0, top, cs.width, cs.height));
        if let Some(m) = kids.get(1).copied() {
            let ms = cx.measure(m, Proposal::new(Some(cs.width), None));
            cx.place(
                m,
                Rect::new(
                    0.0,
                    top + cs.height + metrics::MESSAGE_GAP,
                    cs.width,
                    ms.height,
                ),
            );
        }
    }

    fn hit(&self, _p: Point, _size: Size) -> bool {
        false
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match prop {
            // The control fills the value column.
            Prop::Expanded => value.as_bool().map_or(PropChange::Unknown, |b| {
                self.expand = b;
                PropChange::Layout
            }),
            _ => PropChange::Unknown,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Group;
    }
}

/// A form row: the label right-aligned in a 112 pt column, then the value column.
#[derive(Debug)]
pub struct FormRow {
    /// The label.
    pub label: String,
    cache: Label,
}

impl FormRow {
    /// A row labelled `label`.
    pub fn new(label: &str) -> FormRow {
        FormRow {
            label: label.to_string(),
            cache: Label::default(),
        }
    }
}

impl Widget for FormRow {
    fn kind(&self) -> &'static str {
        "FormRow"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        let mut size = Size::new(
            metrics::LABEL_WIDTH + metrics::LABEL_GAP,
            metrics::LINE_HEIGHT,
        );
        if let Some(v) = cx.children().first().copied() {
            let s = cx.measure(v, Proposal::UNSPECIFIED);
            size = Size::new(size.width + s.width, size.height.max(s.height));
        }
        size
    }

    fn place(&mut self, cx: &mut LayoutCx<'_>, _size: Size) {
        if let Some(v) = cx.children().first().copied() {
            let s = cx.measure(v, Proposal::UNSPECIFIED);
            cx.place(
                v,
                Rect::new(
                    metrics::LABEL_WIDTH + metrics::LABEL_GAP,
                    0.0,
                    s.width,
                    s.height,
                ),
            );
        }
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        if self.label.is_empty() {
            return;
        }
        let theme = *cx.theme();
        let b = cx.bounds();
        let style = theme.text(&typography::BODY, theme.colors.text_primary);
        let lh = style.line_height.unwrap_or(18.0);
        let layout = self.cache.layout(
            &self.label,
            &style,
            Align::End,
            metrics::LABEL_WIDTH,
            Some(1),
        );
        cx.rec.text(
            "Label cell/Label",
            Rect::new(
                b.x,
                b.y + (metrics::LINE_HEIGHT - lh) / 2.0,
                metrics::LABEL_WIDTH,
                lh,
            ),
            layout,
            Some(typography::BODY.name),
            style.color,
        );
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match (prop, value) {
            (Prop::Title, Value::Str(s)) => {
                self.label.clone_from(s);
                PropChange::Paint
            }
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::Title => Some(Value::Str(self.label.clone())),
            _ => None,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Group;
        a.label = Some(self.label.clone()).filter(|s| !s.is_empty());
    }

    fn hit(&self, _p: Point, _size: Size) -> bool {
        false
    }
}

/// Makes a form row over `control` (a view already made); `expand` fills the 204 pt column
/// (text fields and labelled switches; pop-up buttons and checkboxes keep their own width).
pub fn form_row(
    ui: &mut Ui,
    label: &str,
    control: ViewId,
    message: Option<(&str, MessageKind)>,
    expand: bool,
) -> ViewId {
    let row = ui.create(FormRow::new(label));
    let value = ui.create(FormValue { expand });
    let _ = ui.set(value, Prop::Identifier, Value::from("Value"));
    let _ = ui.set(control, Prop::Identifier, Value::from("Control"));
    ui.add_child(value, control);
    if let Some((text, kind)) = message {
        let t = ui.create(
            Text::new(text)
                .token(&typography::CAPTION)
                .role(if kind == MessageKind::Error {
                    TextRole::Secondary
                } else {
                    TextRole::Tertiary
                })
                .layer("Message")
                .wrapping(),
        );
        ui.add_child(value, t);
    }
    ui.add_child(row, value);
    row
}

/// Makes a form section: a titled group box of rows, 12 apart.
pub fn form_section(ui: &mut Ui, title: Option<&str>, rows: &[ViewId]) -> ViewId {
    group_box_with(ui, title, rows, metrics::ROW_GAP, "Group")
}

/// The footer of actions: the buttons at the end, 8 apart, each at least 72 wide.
#[derive(Debug, Default)]
pub struct FormFooter;

impl Widget for FormFooter {
    fn kind(&self) -> &'static str {
        "FormFooter"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        let kids = cx.children();
        let mut w = 0.0f32;
        let mut h = 0.0f32;
        for (i, c) in kids.iter().enumerate() {
            let s = cx.measure(*c, Proposal::UNSPECIFIED);
            w += s.width.max(metrics::BUTTON_MIN_WIDTH);
            if i > 0 {
                w += metrics::FOOTER_GAP;
            }
            h = h.max(s.height);
        }
        Size::new(p.width.filter(|x| x.is_finite()).unwrap_or(w), h)
    }

    fn place(&mut self, cx: &mut LayoutCx<'_>, size: Size) {
        let mut x = size.width;
        let kids = cx.children();
        for c in kids.iter().rev() {
            let s = cx.measure(*c, Proposal::UNSPECIFIED);
            let w = s.width.max(metrics::BUTTON_MIN_WIDTH);
            x -= w;
            cx.place(
                *c,
                Rect::new(x, (size.height - s.height) / 2.0, w, s.height),
            );
            x -= metrics::FOOTER_GAP;
        }
    }

    fn hit(&self, _p: Point, _size: Size) -> bool {
        false
    }
}

/// Makes a footer over `buttons` (in reading order, the default action last).
pub fn form_footer(ui: &mut Ui, buttons: &[ViewId]) -> ViewId {
    let f = ui.create(FormFooter);
    for b in buttons {
        ui.add_child(f, *b);
    }
    f
}

/// The layout: a window-coloured surface with a Title 3 title and its parts stacked 20 apart
/// (sections, then the footer), padded 24 / 26 / 24 / 24.
#[derive(Debug)]
pub struct FormLayout {
    /// The title.
    pub title: Option<String>,
    /// The layout's width.
    pub width: f32,
    cache: Label,
}

impl FormLayout {
    /// A layout with a title.
    pub fn new(title: Option<&str>) -> FormLayout {
        FormLayout {
            title: title.map(str::to_string),
            width: metrics::LAYOUT_WIDTH,
            cache: Label::default(),
        }
    }

    fn top(&self) -> f32 {
        let (_, t, _, _) = metrics::LAYOUT_PADDING;
        if self.title.is_some() {
            t + metrics::LAYOUT_TITLE + metrics::LAYOUT_GAP
        } else {
            t
        }
    }
}

impl Widget for FormLayout {
    fn kind(&self) -> &'static str {
        "FormLayout"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        let (l, _, r, b) = metrics::LAYOUT_PADDING;
        let inner = self.width - l - r;
        let mut h = self.top() + b;
        for (i, c) in cx.children().iter().enumerate() {
            let s = cx.measure(*c, Proposal::new(Some(inner), None));
            h += s.height;
            if i > 0 {
                h += metrics::LAYOUT_GAP;
            }
        }
        Size::new(self.width, h)
    }

    fn place(&mut self, cx: &mut LayoutCx<'_>, size: Size) {
        let (l, _, r, _) = metrics::LAYOUT_PADDING;
        let inner = size.width - l - r;
        let mut y = self.top();
        for c in cx.children() {
            let s = cx.measure(c, Proposal::new(Some(inner), None));
            cx.place(c, Rect::new(l, y, inner, s.height));
            y += s.height + metrics::LAYOUT_GAP;
        }
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let c = theme.colors;
        let b = cx.bounds();
        cx.rec.layer(
            "Surface",
            b,
            Shape::rounded(metrics::LAYOUT_RADIUS),
            Style::fill(c.surface_window).with_border(Border::inside(c.line_border, 1.0)),
        );
        if let Some(title) = self.title.clone() {
            let (l, t, _, _) = metrics::LAYOUT_PADDING;
            let style = theme.text(&typography::TITLE_3, c.text_primary);
            let w = whole(self.cache.width(&title, &style), 4.0);
            let layout = self.cache.layout(&title, &style, Align::Start, w, Some(1));
            cx.rec.text(
                "Title",
                Rect::new(b.x + l, b.y + t, w, metrics::LAYOUT_TITLE),
                layout,
                Some(typography::TITLE_3.name),
                style.color,
            );
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Group;
        a.label.clone_from(&self.title);
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match (prop, value) {
            (Prop::Title, Value::Str(s)) => {
                self.title = Some(s.clone()).filter(|t| !t.is_empty());
                PropChange::Layout
            }
            (Prop::Title, Value::None) => {
                self.title = None;
                PropChange::Layout
            }
            (Prop::Width, v) => match v.as_f32() {
                Some(w) if w.is_finite() && w > 0.0 => {
                    self.width = w;
                    PropChange::Layout
                }
                _ => PropChange::Unknown,
            },
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::Title => Some(self.title.clone().map(Value::Str).unwrap_or_default()),
            _ => None,
        }
    }
}

/// Makes a form layout over `parts` (sections, then the footer).
pub fn form_layout(ui: &mut Ui, title: Option<&str>, parts: &[ViewId]) -> ViewId {
    let l = ui.create(FormLayout::new(title));
    for p in parts {
        ui.add_child(l, *p);
    }
    l
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::amalfi::fields::TextField;
    use crate::amalfi::toggles::Toggle;

    #[test]
    fn rows_lie_on_a_328_grid() {
        let mut ui = Ui::new_headless();
        let field = ui.create(TextField::text().with_text("Amalfi"));
        let text = form_row(&mut ui, "Name", field, None, true);
        assert_eq!(
            ui.measure(text, Proposal::UNSPECIFIED),
            Size::new(328.0, 28.0)
        );
        let check = ui.create(Toggle::checkbox().label("Show hidden files"));
        let c = form_row(&mut ui, "Label", check, None, false);
        assert_eq!(ui.measure(c, Proposal::UNSPECIFIED).width, 124.0 + 152.0);
        let err = ui.create(TextField::text().with_error("x"));
        let e = form_row(
            &mut ui,
            "Email",
            err,
            Some((
                "Enter an address such as name@example.com.",
                MessageKind::Error,
            )),
            true,
        );
        // 28 + 4 + two lines of 14.
        assert_eq!(ui.measure(e, Proposal::UNSPECIFIED).height, 60.0);
    }
}
