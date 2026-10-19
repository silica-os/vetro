//! Sidebars (kit `Sidebars`): rows (plain, with a count badge, a volume with an eject button),
//! section headers (plain or collapsible), groups, the footer and the panel that stacks them.

use crate::amalfi::buttons::IconButton;
use crate::amalfi::foundation::{ControlSize, Interaction as Transitions, Label};
use crate::amalfi::tokens::typography;
use crate::core::a11y::{A11y, Action, Role};
use crate::core::event::{Event, Handled, Key};
use crate::core::layout::Proposal;
use crate::core::tree::{NodeFlags, ViewId};
use crate::core::ui::Ui;
use crate::core::value::{Prop, Signal, Value};
use crate::core::widget::{EventCx, FocusKind, Interaction, LayoutCx, PaintCx, PropChange, Widget};
use crate::geom::{Point, Rect, Size};
use crate::shape::Shape;
use crate::style::{Border, Style};
use crate::text::Align;

/// Fixed measures of the kit.
pub mod metrics {
    /// A row's width in the kit.
    pub const ROW_WIDTH: f32 = 216.0;
    /// A row's height.
    pub const ROW_HEIGHT: f32 = 32.0;
    /// A header's height.
    pub const HEADER_HEIGHT: f32 = 28.0;
    /// The footer's height.
    pub const FOOTER_HEIGHT: f32 = 36.0;
    /// A row's selection and hover radius.
    pub const ROW_RADIUS: f32 = 8.0;
    /// The row's icon box and its inset.
    pub const ICON: f32 = 20.0;
    /// Where a row's label starts.
    pub const LABEL_X: f32 = 40.0;
    /// A row's padding at its sides.
    pub const PADDING: f32 = 12.0;
    /// Gap between a row's label and its trailing part.
    pub const GAP: f32 = 8.0;
    /// A badge's height.
    pub const BADGE_HEIGHT: f32 = 16.0;
    /// An eject button's size.
    pub const EJECT: f32 = 24.0;
    /// Space before a section header.
    pub const SECTION_GAP: f32 = 8.0;
    /// The panel's padding.
    pub const PANEL_PADDING: f32 = 12.0;
}

// ---------------------------------------------------------------------------------------
// Badge.
// ---------------------------------------------------------------------------------------

/// A count in a pill.
#[derive(Debug, Default)]
pub struct Badge {
    /// The count.
    pub count: String,
    text: Label,
}

impl Badge {
    /// A badge with `count`.
    pub fn new(count: &str) -> Badge {
        Badge {
            count: count.to_string(),
            text: Label::default(),
        }
    }

    fn text_width(&mut self, theme: &crate::amalfi::theme::Theme) -> f32 {
        let style = theme.text(&typography::CAPTION, theme.colors.text_secondary);
        self.text.width(&self.count, &style).ceil()
    }
}

impl Widget for Badge {
    fn kind(&self) -> &'static str {
        "SidebarBadge"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        let theme = cx.theme();
        let w = self.text_width(&theme).max(16.0) + 16.0;
        Size::new(w, metrics::BADGE_HEIGHT)
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let c = theme.colors;
        let r = cx.bounds();
        cx.rec.layer(
            "Pill",
            r,
            Shape::rounded(8.0),
            Style::fill(c.surface_raised).with_border(Border::inside(c.line_border, 1.0)),
        );
        let style = theme.text(&typography::CAPTION, c.text_secondary);
        let w = (r.width - 16.0).max(0.0);
        let layout = self
            .text
            .layout(&self.count, &style, Align::Center, w, Some(1));
        cx.rec.text(
            "Count",
            Rect::new(r.x + 8.0, r.y + 1.0, w, 14.0),
            layout,
            Some(typography::CAPTION.name),
            c.text_secondary,
        );
    }

    fn hit(&self, _p: Point, _size: Size) -> bool {
        false
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match (prop, value.as_str()) {
            (Prop::Text | Prop::Badge, Some(s)) => {
                self.count = s.to_string();
                PropChange::Layout
            }
            _ => PropChange::Unknown,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Label;
        a.label = Some(self.count.clone());
    }
}

// ---------------------------------------------------------------------------------------
// Eject button.
// ---------------------------------------------------------------------------------------

/// The eject button of a volume row: a 24 pt hit area with a 16 pt glyph.
#[derive(Debug, Default)]
pub struct EjectButton {
    transitions: Transitions,
}

impl Widget for EjectButton {
    fn kind(&self) -> &'static str {
        "EjectButton"
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        Size::new(metrics::EJECT, metrics::EJECT)
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let c = cx.theme().colors;
        let r = cx.bounds();
        let hover = self.transitions.hover.get();
        if hover > 0.0 {
            cx.rec.layer(
                "Eject hover",
                r,
                Shape::rounded(12.0),
                Style::fill(c.line_separator).with_opacity(hover),
            );
        }
        cx.rec.glyph(
            "Eject",
            Rect::new(r.x + 4.0, r.y + 4.0, 16.0, 16.0),
            "eject",
            400,
            c.text_secondary,
        );
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        if *event == Event::Activate {
            cx.emit(Signal::Activated, Value::None);
            Handled::Yes
        } else {
            Handled::No
        }
    }

    fn state_changed(&mut self, cx: &mut EventCx<'_>, _old: NodeFlags, new: NodeFlags) {
        let reduce = cx.theme().appearance.reduce_motion;
        if self
            .transitions
            .update(new, new.contains(NodeFlags::DISABLED), reduce)
        {
            cx.animate();
        }
    }

    fn tick(&mut self, _cx: &mut EventCx<'_>, dt: f32) -> bool {
        self.transitions.tick(dt)
    }

    fn interaction(&self) -> Interaction {
        Interaction::Button
    }

    fn focus(&self) -> FocusKind {
        FocusKind::Control
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Button;
        a.label = Some("Eject".to_string());
        a.actions = vec![Action::Click, Action::Focus];
    }
}

// ---------------------------------------------------------------------------------------
// Row.
// ---------------------------------------------------------------------------------------

/// A sidebar row: an icon, a label, an optional badge or eject button (as children named
/// `Badge` and `Eject button`).
#[derive(Debug)]
pub struct Row {
    /// The label.
    pub label: String,
    /// The icon (a symbol name).
    pub icon: String,
    /// Selected.
    pub selected: bool,
    /// Whether the window is active (`None`: follows the window).
    pub active: Option<bool>,
    transitions: Transitions,
    text: Label,
    trailing: f32,
}

impl Row {
    /// A row with `icon` and `label`.
    pub fn new(icon: &str, label: &str) -> Row {
        Row {
            label: label.to_string(),
            icon: icon.to_string(),
            selected: false,
            active: None,
            transitions: Transitions::default(),
            text: Label::default(),
            trailing: metrics::ROW_WIDTH - metrics::PADDING,
        }
    }

    /// Selected.
    pub fn selected(mut self, selected: bool) -> Row {
        self.selected = selected;
        self
    }
}

impl Widget for Row {
    fn kind(&self) -> &'static str {
        "SidebarRow"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        for c in cx.children() {
            let _ = cx.measure(c, Proposal::new(None, Some(metrics::ROW_HEIGHT)));
        }
        let w = match p.width {
            Some(w) if w.is_finite() => w,
            _ => metrics::ROW_WIDTH,
        };
        Size::new(w, metrics::ROW_HEIGHT)
    }

    fn place(&mut self, cx: &mut LayoutCx<'_>, size: Size) {
        let right = size.width - metrics::PADDING;
        self.trailing = right;
        for c in cx.children() {
            let s = cx.measure(c, Proposal::new(None, Some(metrics::ROW_HEIGHT)));
            let y = (metrics::ROW_HEIGHT - s.height) / 2.0;
            // The eject button hangs 4 pt further out than a badge.
            let x = if s.width == metrics::EJECT {
                right - metrics::EJECT + 4.0
            } else {
                right - s.width
            };
            cx.place(c, Rect::new(x, y, s.width, s.height));
            self.trailing = x;
        }
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let c = theme.colors;
        let r = cx.bounds();
        let disabled = cx.is_disabled();
        let active = self.active.unwrap_or_else(|| cx.window_active());
        if disabled {
            cx.rec.push_opacity(theme.disabled_opacity());
        }
        let shape = Shape::rounded(metrics::ROW_RADIUS);
        let (label_color, icon_color) = if self.selected {
            if active {
                cx.rec
                    .layer("Selection", r, shape, Style::fill(c.accent_amalfi_soft));
                (c.text_primary, c.accent_amalfi)
            } else {
                // The kit's light inactive selection is `Surface/Control Pressed`, the dark
                // one `Surface/Control` (docs/kit-issues.md).
                let fill = if theme.dark() {
                    c.surface_control
                } else {
                    c.surface_control_pressed
                };
                cx.rec.layer("Selection", r, shape, Style::fill(fill));
                (c.text_secondary, c.text_secondary)
            }
        } else {
            let hover = self.transitions.hover.get();
            if hover > 0.0 {
                cx.rec.layer(
                    "Hover",
                    r,
                    shape,
                    Style::fill(c.line_separator).with_opacity(hover),
                );
            }
            (
                c.text_primary,
                if disabled {
                    c.text_secondary
                } else {
                    c.accent_amalfi
                },
            )
        };
        cx.rec.glyph(
            "Icon",
            Rect::new(
                r.x + metrics::PADDING,
                r.y + 6.0,
                metrics::ICON,
                metrics::ICON,
            ),
            &self.icon,
            400,
            icon_color,
        );
        let mut style = theme.text(&typography::SIDEBAR, label_color);
        if self.selected {
            style.weight = 600;
        }
        let trailing = self.trailing;
        let end = if trailing < r.width - metrics::PADDING {
            trailing - metrics::GAP
        } else {
            trailing
        };
        let w = (end - metrics::LABEL_X).max(0.0);
        let layout = self
            .text
            .layout(&self.label, &style, Align::Start, w, Some(1));
        cx.rec.text(
            "Label",
            Rect::new(r.x + metrics::LABEL_X, r.y + 6.0, w, 20.0),
            layout,
            Some(typography::SIDEBAR.name),
            label_color,
        );
        if disabled {
            cx.rec.pop();
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        match event {
            Event::Activate => {
                cx.emit(Signal::Activated, Value::None);
                Handled::Yes
            }
            Event::Key(k) if k.pressed && matches!(k.key, Key::Enter | Key::Space) => {
                cx.emit(Signal::Activated, Value::None);
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn state_changed(&mut self, cx: &mut EventCx<'_>, _old: NodeFlags, new: NodeFlags) {
        let reduce = cx.theme().appearance.reduce_motion;
        if self
            .transitions
            .update(new, new.contains(NodeFlags::DISABLED), reduce)
        {
            cx.animate();
        }
    }

    fn tick(&mut self, _cx: &mut EventCx<'_>, dt: f32) -> bool {
        self.transitions.tick(dt)
    }

    fn interaction(&self) -> Interaction {
        Interaction::Button
    }

    fn focus(&self) -> FocusKind {
        FocusKind::Control
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match prop {
            Prop::Title | Prop::Text => value.as_str().map_or(PropChange::Unknown, |s| {
                self.label = s.to_string();
                PropChange::Paint
            }),
            Prop::Symbol => value.as_str().map_or(PropChange::Unknown, |s| {
                self.icon = s.to_string();
                PropChange::Paint
            }),
            Prop::State => value.as_bool().map_or(PropChange::Unknown, |b| {
                self.selected = b;
                PropChange::Paint
            }),
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::Title | Prop::Text => Some(Value::Str(self.label.clone())),
            Prop::Symbol => Some(Value::Str(self.icon.clone())),
            Prop::State => Some(Value::Bool(self.selected)),
            _ => None,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::ListItem;
        a.label = Some(self.label.clone());
        a.selected = Some(self.selected);
        a.actions = vec![Action::Click, Action::Focus];
    }
}

/// Makes a row with an optional count badge and an optional eject button.
pub fn row(ui: &mut Ui, icon: &str, label: &str, badge: Option<&str>, eject: bool) -> ViewId {
    let r = ui.create(Row::new(icon, label));
    if let Some(count) = badge {
        let b = ui.create(Badge::new(count));
        let _ = ui.set(b, Prop::Identifier, Value::from("Badge"));
        ui.add_child(r, b);
    }
    if eject {
        let e = ui.create(EjectButton::default());
        let _ = ui.set(e, Prop::Identifier, Value::from("Eject button"));
        ui.add_child(r, e);
    }
    r
}

// ---------------------------------------------------------------------------------------
// Header.
// ---------------------------------------------------------------------------------------

/// A section header; with `collapsible` it has a disclosure chevron and toggles on a click.
#[derive(Debug)]
pub struct Header {
    /// The title.
    pub title: String,
    /// Shows a chevron and toggles.
    pub collapsible: bool,
    /// Open (the chevron points down).
    pub open: bool,
    text: Label,
}

impl Header {
    /// A plain header.
    pub fn new(title: &str) -> Header {
        Header {
            title: title.to_string(),
            collapsible: false,
            open: true,
            text: Label::default(),
        }
    }

    /// A header that opens and closes its section.
    pub fn collapsible(mut self, open: bool) -> Header {
        self.collapsible = true;
        self.open = open;
        self
    }

    fn toggle(&mut self, cx: &mut EventCx<'_>) {
        self.open = !self.open;
        cx.emit(Signal::ExpandedChanged, Value::Bool(self.open));
        cx.request_paint();
    }
}

impl Widget for Header {
    fn kind(&self) -> &'static str {
        "SidebarHeader"
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        let w = match p.width {
            Some(w) if w.is_finite() => w,
            _ => metrics::ROW_WIDTH,
        };
        Size::new(w, metrics::HEADER_HEIGHT)
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let c = theme.colors;
        let r = cx.bounds();
        if self.collapsible {
            cx.rec.glyph(
                "Disclosure",
                Rect::new(r.x + r.width - 24.0, r.y + 12.0, 12.0, 12.0),
                if self.open {
                    "chevron-down"
                } else {
                    "chevron-right"
                },
                400,
                c.text_tertiary,
            );
        }
        let style = theme.text(&typography::SIDEBAR_HEADER, c.text_tertiary);
        let w = (r.width - 12.0 - if self.collapsible { 28.0 } else { 12.0 }).max(0.0);
        let layout = self
            .text
            .layout(&self.title, &style, Align::Start, w, Some(1));
        cx.rec.text(
            "Title",
            Rect::new(r.x + 12.0, r.y + 10.0, w, 16.0),
            layout,
            Some(typography::SIDEBAR_HEADER.name),
            c.text_tertiary,
        );
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        if !self.collapsible {
            return Handled::No;
        }
        match event {
            Event::Activate => {
                self.toggle(cx);
                Handled::Yes
            }
            Event::Key(k) if k.pressed && matches!(k.key, Key::Enter | Key::Space) => {
                self.toggle(cx);
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn interaction(&self) -> Interaction {
        if self.collapsible {
            Interaction::Button
        } else {
            Interaction::None
        }
    }

    fn focus(&self) -> FocusKind {
        if self.collapsible {
            FocusKind::Control
        } else {
            FocusKind::None
        }
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match prop {
            Prop::Title | Prop::Text => value.as_str().map_or(PropChange::Unknown, |s| {
                self.title = s.to_string();
                PropChange::Paint
            }),
            Prop::Expanded => value.as_bool().map_or(PropChange::Unknown, |b| {
                self.open = b;
                PropChange::Paint
            }),
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::Title | Prop::Text => Some(Value::Str(self.title.clone())),
            Prop::Expanded => Some(Value::Bool(self.open)),
            _ => None,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Heading;
        a.label = Some(self.title.clone());
        if self.collapsible {
            a.expanded = Some(self.open);
            a.actions = vec![Action::Click, Action::Focus];
        }
    }
}

// ---------------------------------------------------------------------------------------
// Group and panel.
// ---------------------------------------------------------------------------------------

/// A header with the rows of its section below it; the rows are hidden while it is closed.
#[derive(Debug, Default)]
pub struct Group {
    /// Open.
    pub open: bool,
}

impl Widget for Group {
    fn kind(&self) -> &'static str {
        "SidebarGroup"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        let w = match p.width {
            Some(w) if w.is_finite() => w,
            _ => metrics::ROW_WIDTH,
        };
        let mut h = 0.0;
        for (i, c) in cx.children().into_iter().enumerate() {
            if i > 0 && !self.open {
                break;
            }
            h += cx.measure(c, Proposal::new(Some(w), None)).height;
        }
        Size::new(w, h)
    }

    fn place(&mut self, cx: &mut LayoutCx<'_>, size: Size) {
        let mut y = 0.0;
        for (i, c) in cx.children().into_iter().enumerate() {
            if i > 0 && !self.open {
                break;
            }
            let s = cx.measure(c, Proposal::new(Some(size.width), None));
            cx.place(c, Rect::new(0.0, y, size.width, s.height));
            y += s.height;
        }
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match (prop, value.as_bool()) {
            (Prop::Expanded, Some(b)) => {
                self.open = b;
                PropChange::Layout
            }
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        (prop == Prop::Expanded).then_some(Value::Bool(self.open))
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Group;
    }
}

/// Makes a group: a collapsible header and its rows.
pub fn group(ui: &mut Ui, title: &str, open: bool, rows: &[ViewId]) -> ViewId {
    let g = ui.create(Group { open });
    let h = ui.create(Header::new(title).collapsible(open));
    let _ = ui.set(h, Prop::Identifier, Value::from("Header"));
    ui.add_child(g, h);
    for r in rows {
        ui.add_child(g, *r);
    }
    let rows: Vec<ViewId> = rows.to_vec();
    ui.connect(h, Signal::ExpandedChanged, move |ui, _, v| {
        let open = v.as_bool().unwrap_or(false);
        let _ = ui.set(g, Prop::Expanded, Value::Bool(open));
        for r in &rows {
            let _ = ui.set(*r, Prop::Hidden, Value::Bool(!open));
        }
    });
    g
}

/// The bar at the bottom of the panel: an edge line, add and settings buttons.
#[derive(Debug, Default)]
pub struct Footer;

impl Widget for Footer {
    fn kind(&self) -> &'static str {
        "SidebarFooter"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        for c in cx.children() {
            let _ = cx.measure(c, Proposal::new(None, None));
        }
        let w = match p.width {
            Some(w) if w.is_finite() => w,
            _ => metrics::ROW_WIDTH,
        };
        Size::new(w, metrics::FOOTER_HEIGHT)
    }

    fn place(&mut self, cx: &mut LayoutCx<'_>, _size: Size) {
        // Children: Add (first), Settings (second).
        for (i, c) in cx.children().into_iter().enumerate() {
            let s = cx.measure(c, Proposal::new(None, None));
            cx.place(c, Rect::new(12.0 + 28.0 * i as f32, 6.0, s.width, s.height));
        }
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let c = cx.theme().colors;
        let r = cx.bounds();
        cx.rec.layer(
            "Edge",
            Rect::new(r.x, r.y, r.width, 1.0),
            Shape::RECT,
            Style::fill(c.line_separator),
        );
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Toolbar;
    }
}

/// Makes the footer with its add and settings buttons (children `Add` and `Settings`).
pub fn footer(ui: &mut Ui) -> ViewId {
    let f = ui.create(Footer);
    for (symbol, name, title) in [("plus", "Add", "Add"), ("gear", "Settings", "Settings")] {
        let b = ui.create(
            IconButton::new(symbol)
                .square()
                .size(ControlSize::Small)
                .titled(title),
        );
        let _ = ui.set(b, Prop::Identifier, Value::from(name));
        ui.add_child(f, b);
    }
    f
}

/// The sidebar panel: `Surface/Sidebar`, its children stacked 12 in from the top and the
/// sides, 8 pt before every header but the first, and a trailing [`Footer`] at the bottom.
#[derive(Debug, Default)]
pub struct Panel;

impl Panel {
    fn is_header(ui: &Ui, id: ViewId) -> bool {
        ui.widget::<Header>(id).is_some()
    }
}

impl Widget for Panel {
    fn kind(&self) -> &'static str {
        "SidebarPanel"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        let w = match p.width {
            Some(w) if w.is_finite() => w,
            _ => metrics::ROW_WIDTH + 2.0 * metrics::PANEL_PADDING,
        };
        let inner = (w - 2.0 * metrics::PANEL_PADDING).max(0.0);
        let mut h = metrics::PANEL_PADDING;
        for c in cx.children() {
            h += cx.measure(c, Proposal::new(Some(inner), None)).height;
        }
        Size::new(w, p.height.filter(|h| h.is_finite()).unwrap_or(h))
    }

    fn place(&mut self, cx: &mut LayoutCx<'_>, size: Size) {
        let inner = (size.width - 2.0 * metrics::PANEL_PADDING).max(0.0);
        let kids = cx.children();
        let last = kids.len().checked_sub(1);
        let mut y = metrics::PANEL_PADDING;
        let mut first = true;
        for (i, c) in kids.into_iter().enumerate() {
            let s = cx.measure(c, Proposal::new(Some(inner), None));
            if Some(i) == last && cx.ui.widget::<Footer>(c).is_some() {
                cx.place(
                    c,
                    Rect::new(
                        metrics::PANEL_PADDING,
                        size.height - s.height,
                        inner,
                        s.height,
                    ),
                );
                continue;
            }
            if !first && Panel::is_header(cx.ui, c) {
                y += metrics::SECTION_GAP;
            }
            first = false;
            cx.place(c, Rect::new(metrics::PANEL_PADDING, y, inner, s.height));
            y += s.height;
        }
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let c = cx.theme().colors;
        cx.rec.layer(
            "Surface",
            cx.bounds(),
            Shape::RECT,
            Style::fill(c.surface_sidebar),
        );
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::List;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::event::{Button, Mods};
    use crate::core::window::WindowKind;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn a_closed_header_hides_its_rows_and_a_click_reopens_them() {
        let mut ui = Ui::new_headless();
        let win = ui.create_window(WindowKind::Normal, Size::new(300.0, 300.0));
        let a = row(&mut ui, "folder", "Documents", None, false);
        let b = row(&mut ui, "folder", "Downloads", Some("3"), false);
        let g = group(&mut ui, "Favourites", true, &[a, b]);
        ui.set_content(win, g);
        ui.show_window(win);
        ui.prepare_frame(win);
        assert_eq!(ui.tree.window_rect(g).height, 28.0 + 64.0);
        let h = ui.tree.window_rect(g);
        let p = Point::new(h.x + 30.0, h.y + 14.0);
        ui.pointer_move(win, p, Mods::empty());
        ui.pointer_down(win, p, Button::Left, Mods::empty());
        ui.pointer_up(win, p, Button::Left, Mods::empty());
        ui.prepare_frame(win);
        assert_eq!(ui.get(g, Prop::Expanded), Some(Value::Bool(false)));
        assert_eq!(ui.tree.window_rect(g).height, 28.0);
    }

    #[test]
    fn a_row_activates_and_selects() {
        let mut ui = Ui::new_headless();
        let win = ui.create_window(WindowKind::Normal, Size::new(300.0, 300.0));
        let r = row(&mut ui, "folder", "Documents", None, false);
        let log = Rc::new(RefCell::new(0));
        let l = log.clone();
        ui.connect(r, Signal::Activated, move |_, _, _| *l.borrow_mut() += 1);
        ui.set_content(win, r);
        ui.show_window(win);
        ui.prepare_frame(win);
        let b = ui.tree.window_rect(r);
        let p = Point::new(b.x + 100.0, b.y + 16.0);
        ui.pointer_move(win, p, Mods::empty());
        ui.pointer_down(win, p, Button::Left, Mods::empty());
        ui.pointer_up(win, p, Button::Left, Mods::empty());
        assert_eq!(*log.borrow(), 1);
        let _ = ui.set(r, Prop::State, Value::Bool(true));
        assert_eq!(ui.get(r, Prop::State), Some(Value::Bool(true)));
    }
}
