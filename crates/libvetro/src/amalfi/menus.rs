//! Menus (kit `Menus`): the rows (items, section headers, separators), the panel that stacks
//! them and tracks the highlight, and [`present`], which shows a menu as a popup.

use crate::amalfi::foundation::Label;
use crate::amalfi::tokens::typography;
use crate::core::a11y::{A11y, Action, Role};
use crate::core::event::{Button, Event, Handled, Key};
use crate::core::layout::Proposal;
use crate::core::tree::ViewId;
use crate::core::ui::Ui;
use crate::core::value::{Prop, Signal, Value};
use crate::core::widget::{EventCx, FocusKind, Interaction, LayoutCx, PaintCx, PropChange, Widget};
use crate::core::window::{WindowId, WindowKind};
use crate::geom::{Point, Rect, Size};
use crate::shape::Shape;
use crate::style::Style;
use crate::text::Align;

/// Fixed measures (silica_ui's `AmalfiMenuMetrics`).
pub mod metrics {
    /// An item's height.
    pub const ITEM_HEIGHT: f32 = 24.0;
    /// A separator's height.
    pub const SEPARATOR_HEIGHT: f32 = 8.0;
    /// The panel's padding.
    pub const PADDING: f32 = 4.0;
    /// The panel's corner radius.
    pub const RADIUS: f32 = 12.0;
    /// The highlight's corner radius.
    pub const HIGHLIGHT_RADIUS: f32 = 7.0;
    /// The item's padding at its sides.
    pub const ITEM_PADDING: f32 = 8.0;
    /// The check's slot.
    pub const CHECK_SLOT: f32 = 12.0;
    /// An icon's slot.
    pub const ICON_SLOT: f32 = 20.0;
    /// Gap between an item's parts.
    pub const GAP: f32 = 4.0;
    /// The chevron's size.
    pub const CHEVRON: f32 = 16.0;
    /// A header's padding at its start.
    pub const HEADER_LEFT: f32 = 12.0;
    /// A header's padding at its end.
    pub const HEADER_RIGHT: f32 = 8.0;
    /// A separator's inset at its sides.
    pub const SEPARATOR_INSET: f32 = 12.0;
    /// The narrowest menu.
    pub const MIN_WIDTH: f32 = 160.0;
    /// Space between a pull-down button and its menu (`docs/spec/menus.md`).
    pub const PULL_DOWN_GAP: f32 = 3.0;
}

/// What a row is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RowKind {
    /// A command.
    #[default]
    Item,
    /// A section header.
    Header,
    /// A line between groups.
    Separator,
}

/// A row of a menu.
#[derive(Debug)]
pub struct MenuItem {
    /// What it is.
    pub kind: RowKind,
    /// The label (or the header).
    pub label: String,
    /// The shortcut, as displayed (`⌘N`).
    pub shortcut: Option<String>,
    /// An icon (Silica Symbols name) in place of the check slot.
    pub icon: Option<String>,
    /// Checked.
    pub checked: bool,
    /// Opens a submenu.
    pub submenu: bool,
    /// Terracotta when highlighted.
    pub destructive: bool,
    /// Highlighted by itself (the menu highlights its rows as the pointer moves).
    pub highlighted: bool,
    label_text: Label,
    shortcut_text: Label,
}

impl MenuItem {
    fn new(kind: RowKind, label: &str) -> MenuItem {
        MenuItem {
            kind,
            label: label.to_string(),
            shortcut: None,
            icon: None,
            checked: false,
            submenu: false,
            destructive: false,
            highlighted: false,
            label_text: Label::default(),
            shortcut_text: Label::default(),
        }
    }

    /// A command.
    pub fn item(label: &str) -> MenuItem {
        MenuItem::new(RowKind::Item, label)
    }

    /// A section header.
    pub fn header(label: &str) -> MenuItem {
        MenuItem::new(RowKind::Header, label)
    }

    /// A separator.
    pub fn separator() -> MenuItem {
        MenuItem::new(RowKind::Separator, "")
    }

    /// With a shortcut.
    pub fn shortcut(mut self, s: &str) -> MenuItem {
        self.shortcut = Some(s.to_string());
        self
    }

    /// With an icon.
    pub fn icon(mut self, symbol: &str) -> MenuItem {
        self.icon = Some(symbol.to_string());
        self
    }

    /// Checked.
    pub fn checked(mut self, checked: bool) -> MenuItem {
        self.checked = checked;
        self
    }

    /// With a submenu.
    pub fn submenu(mut self) -> MenuItem {
        self.submenu = true;
        self
    }

    /// Destructive.
    pub fn destructive(mut self) -> MenuItem {
        self.destructive = true;
        self
    }

    /// Whether it can be chosen.
    pub fn selectable(&self) -> bool {
        self.kind == RowKind::Item
    }

    fn height(&self) -> f32 {
        if self.kind == RowKind::Separator {
            metrics::SEPARATOR_HEIGHT
        } else {
            metrics::ITEM_HEIGHT
        }
    }

    fn natural_width(&mut self, theme: &crate::amalfi::theme::Theme) -> f32 {
        let body = theme.text(&typography::BODY, theme.colors.text_primary);
        match self.kind {
            RowKind::Separator => 0.0,
            RowKind::Header => {
                let st = theme.text(&typography::LABEL, theme.colors.text_tertiary);
                metrics::HEADER_LEFT
                    + self.label_text.width(&self.label, &st)
                    + metrics::HEADER_RIGHT
            }
            RowKind::Item => {
                let lead = if self.icon.is_some() {
                    metrics::ICON_SLOT
                } else {
                    metrics::CHECK_SLOT
                };
                let mut w = 2.0 * metrics::ITEM_PADDING
                    + lead
                    + metrics::GAP
                    // Whole points: the label is laid out again in the width it gets.
                    + self.label_text.width(&self.label, &body).ceil();
                if let Some(s) = self.shortcut.clone() {
                    w += metrics::GAP + self.shortcut_text.width(&s, &body).ceil();
                }
                if self.submenu {
                    w += metrics::GAP + metrics::CHEVRON;
                }
                w
            }
        }
    }
}

impl Widget for MenuItem {
    fn kind(&self) -> &'static str {
        "MenuItem"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        let theme = cx.theme();
        let natural = self.natural_width(&theme);
        let w = match p.width {
            Some(w) if w.is_finite() => w,
            _ => natural,
        };
        Size::new(w, self.height())
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let c = theme.colors;
        let r = cx.bounds();
        if self.kind == RowKind::Separator {
            cx.rec.layer(
                "Line",
                Rect::new(
                    r.x + metrics::SEPARATOR_INSET,
                    r.y + 4.0,
                    (r.width - 2.0 * metrics::SEPARATOR_INSET).max(0.0),
                    1.0,
                ),
                Shape::RECT,
                Style::fill(c.line_separator),
            );
            return;
        }
        if self.kind == RowKind::Header {
            let style = theme.text(&typography::LABEL, c.text_tertiary);
            let lh = style.line_height.unwrap_or(14.0);
            let w = (r.width - metrics::HEADER_LEFT - metrics::HEADER_RIGHT).max(0.0);
            let layout = self
                .label_text
                .layout(&self.label, &style, Align::Start, w, Some(1));
            cx.rec.text(
                "Label",
                Rect::new(
                    r.x + metrics::HEADER_LEFT,
                    r.y + 4.0 + (20.0 - lh) / 2.0,
                    w,
                    lh,
                ),
                layout,
                Some(typography::LABEL.name),
                style.color,
            );
            return;
        }
        let enabled = !cx.is_disabled();
        let parent_lit = cx.ui.tree.parent(cx.id).is_some_and(|p| {
            cx.ui
                .widget::<Menu>(p)
                .and_then(|m| m.highlight)
                .is_some_and(|h| cx.ui.tree.children(p).get(h) == Some(&cx.id))
        });
        let lit = (self.highlighted || parent_lit) && enabled;
        let text = if lit {
            c.text_on_accent
        } else {
            c.text_primary
        };
        let secondary = if lit {
            c.text_on_accent
        } else {
            c.text_secondary
        };
        if lit {
            cx.rec.layer(
                "Highlight",
                r,
                Shape::rounded(metrics::HIGHLIGHT_RADIUS),
                Style::fill(if self.destructive {
                    c.palette_terracotta
                } else {
                    c.accent_amalfi
                }),
            );
        }
        let pad = metrics::ITEM_PADDING;
        let lead = if let Some(icon) = &self.icon {
            cx.rec.glyph(
                "Icon",
                Rect::new(r.x + pad, r.y + 2.0, metrics::ICON_SLOT, metrics::ICON_SLOT),
                icon,
                400,
                text,
            );
            metrics::ICON_SLOT
        } else {
            if self.checked {
                cx.rec.glyph(
                    "Check",
                    Rect::new(
                        r.x + pad,
                        r.y + 6.0,
                        metrics::CHECK_SLOT,
                        metrics::CHECK_SLOT,
                    ),
                    "checkmark",
                    400,
                    text,
                );
            }
            metrics::CHECK_SLOT
        };
        let body = theme.text(&typography::BODY, text);
        let lh = body.line_height.unwrap_or(18.0);
        let mut right = r.right() - pad;
        if self.submenu {
            cx.rec.glyph(
                "Chevron",
                Rect::new(
                    right - metrics::CHEVRON,
                    r.y + 4.0,
                    metrics::CHEVRON,
                    metrics::CHEVRON,
                ),
                "chevron-right",
                400,
                secondary,
            );
            right -= metrics::CHEVRON + metrics::GAP;
        }
        if let Some(s) = self.shortcut.clone() {
            let st = theme.text(&typography::BODY, secondary);
            let w = self.shortcut_text.width(&s, &st).ceil();
            let layout = self.shortcut_text.layout(&s, &st, Align::End, w, Some(1));
            cx.rec.text(
                "Shortcut",
                Rect::new(right - w, r.y + 3.0, w, lh),
                layout,
                Some(typography::BODY.name),
                secondary,
            );
            right -= w + metrics::GAP;
        }
        let x = r.x + pad + lead + metrics::GAP;
        let w = (right - x).max(0.0);
        let layout = self
            .label_text
            .layout(&self.label, &body, Align::Start, w, Some(1));
        cx.rec.text(
            "Label",
            Rect::new(x, r.y + 3.0, w, lh),
            layout,
            Some(typography::BODY.name),
            text,
        );
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match prop {
            Prop::Title => value.as_str().map_or(PropChange::Unknown, |s| {
                self.label = s.to_string();
                PropChange::Layout
            }),
            Prop::Shortcut => {
                self.shortcut = value.as_str().filter(|s| !s.is_empty()).map(str::to_string);
                PropChange::Layout
            }
            Prop::Symbol => {
                self.icon = value.as_str().filter(|s| !s.is_empty()).map(str::to_string);
                PropChange::Layout
            }
            Prop::State => value.as_bool().map_or(PropChange::Unknown, |b| {
                self.checked = b;
                PropChange::Paint
            }),
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::Title => Some(Value::Str(self.label.clone())),
            Prop::Shortcut => Some(self.shortcut.clone().map(Value::Str).unwrap_or_default()),
            Prop::Symbol => Some(self.icon.clone().map(Value::Str).unwrap_or_default()),
            Prop::State => Some(Value::Bool(self.checked)),
            _ => None,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = match self.kind {
            RowKind::Item => Role::MenuItem,
            RowKind::Header => Role::Heading,
            RowKind::Separator => Role::Group,
        };
        a.label = Some(self.label.clone()).filter(|s| !s.is_empty());
        a.shortcut.clone_from(&self.shortcut);
        a.selected = Some(self.checked).filter(|c| *c);
        a.actions = vec![Action::Click];
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        // A click from an assistive technology chooses the row as the menu would.
        if *event == Event::Activate && self.selectable() {
            let id = cx.id();
            let ui = cx.ui();
            if let Some(menu) = ui.tree.parent(id)
                && let Some(i) = ui.tree.children(menu).iter().position(|c| *c == id)
            {
                ui.emit(menu, Signal::Activated, Value::Int(i as i64));
                ui.emit(menu, Signal::SelectionChanged, Value::Int(i as i64));
                return Handled::Yes;
            }
        }
        Handled::No
    }
}

/// The panel of a menu: `Glass/Thick`, radius 12, its rows (children) 4 in.
#[derive(Debug, Default)]
pub struct Menu {
    /// The highlighted row (index among the children).
    pub highlight: Option<usize>,
    /// The panel is at least this wide.
    pub min_width: Option<f32>,
}

impl Menu {
    /// An empty menu.
    pub fn new() -> Menu {
        Menu::default()
    }

    fn rows(cx_children: &[ViewId], ui: &Ui) -> Vec<(ViewId, bool)> {
        cx_children
            .iter()
            .map(|c| {
                (
                    *c,
                    ui.widget::<MenuItem>(*c).is_some_and(MenuItem::selectable)
                        && !ui.is_disabled(*c),
                )
            })
            .collect()
    }

    fn row_at(&self, cx: &EventCx<'_>, p: Point) -> Option<usize> {
        let id = cx.id();
        let frame = cx.size();
        if p.x < 0.0 || p.x > frame.width || p.y < 0.0 {
            return None;
        }
        let kids = cx.ui.tree.children(id);
        kids.iter().position(|c| {
            cx.ui
                .tree
                .get(*c)
                .is_some_and(|n| n.frame.y <= p.y && p.y < n.frame.bottom())
        })
    }

    fn move_highlight(&mut self, cx: &mut EventCx<'_>, step: isize) {
        let id = cx.id();
        let kids = cx.ui().tree.children(id).to_vec();
        let rows = Menu::rows(&kids, cx.ui());
        let n = rows.len() as isize;
        if n == 0 {
            return;
        }
        let mut i = self
            .highlight
            .map_or(if step > 0 { -1 } else { n }, |h| h as isize);
        for _ in 0..n {
            i = (i + step).rem_euclid(n);
            if rows.get(i as usize).is_some_and(|(_, ok)| *ok) {
                self.highlight = Some(i as usize);
                cx.request_paint();
                return;
            }
        }
    }

    fn choose(&mut self, cx: &mut EventCx<'_>, i: usize) {
        let id = cx.id();
        let kids = cx.ui().tree.children(id).to_vec();
        let rows = Menu::rows(&kids, cx.ui());
        if rows.get(i).is_some_and(|(_, ok)| *ok) {
            cx.emit(Signal::Activated, Value::Int(i as i64));
            cx.emit(Signal::SelectionChanged, Value::Int(i as i64));
        }
    }
}

impl Widget for Menu {
    fn kind(&self) -> &'static str {
        "Menu"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        let pad = metrics::PADDING;
        let mut width = self.min_width.unwrap_or(metrics::MIN_WIDTH) - 2.0 * pad;
        let mut height = 0.0;
        for c in cx.children() {
            let s = cx.measure(c, Proposal::UNSPECIFIED);
            width = width.max(s.width);
            height += s.height;
        }
        let w = match p.width {
            Some(w) if w.is_finite() => w,
            _ => width + 2.0 * pad,
        };
        Size::new(w, height + 2.0 * pad)
    }

    fn place(&mut self, cx: &mut LayoutCx<'_>, size: Size) {
        let pad = metrics::PADDING;
        let w = (size.width - 2.0 * pad).max(0.0);
        let mut y = pad;
        for c in cx.children() {
            let s = cx.measure(c, Proposal::new(Some(w), None));
            cx.place(c, Rect::new(pad, y, w, s.height));
            y += s.height;
        }
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let b = cx.bounds();
        cx.rec.layer(
            "Surface",
            b,
            Shape::rounded(metrics::RADIUS),
            theme.material_style("Glass/Thick"),
        );
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        let origin = cx.bounds().origin();
        let local = |p: &Point| Point::new(p.x - origin.x, p.y - origin.y);
        match event {
            Event::PointerMove { pos, .. } => {
                let id = cx.id();
                let kids = cx.ui().tree.children(id).to_vec();
                let rows = Menu::rows(&kids, cx.ui());
                let at = self
                    .row_at(cx, local(pos))
                    .filter(|i| rows.get(*i).is_some_and(|(_, ok)| *ok));
                if at != self.highlight {
                    self.highlight = at;
                    cx.request_paint();
                }
                Handled::Yes
            }
            Event::PointerLeave => {
                if self.highlight.take().is_some() {
                    cx.request_paint();
                }
                Handled::No
            }
            Event::PointerDown {
                button: Button::Left,
                ..
            } => Handled::Yes,
            Event::PointerUp {
                pos,
                button: Button::Left,
                ..
            } => {
                if let Some(i) = self.row_at(cx, local(pos)) {
                    self.choose(cx, i);
                }
                Handled::Yes
            }
            Event::Key(k) if k.pressed => match k.key {
                Key::Down => {
                    self.move_highlight(cx, 1);
                    Handled::Yes
                }
                Key::Up => {
                    self.move_highlight(cx, -1);
                    Handled::Yes
                }
                Key::Enter | Key::Space => {
                    if let Some(h) = self.highlight {
                        self.choose(cx, h);
                    }
                    Handled::Yes
                }
                Key::Escape => {
                    cx.emit(Signal::CloseRequested, Value::None);
                    Handled::Yes
                }
                _ => Handled::No,
            },
            _ => Handled::No,
        }
    }

    fn interaction(&self) -> Interaction {
        Interaction::Hover
    }

    fn focus(&self) -> FocusKind {
        FocusKind::Always
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match prop {
            Prop::SelectedIndex => {
                self.highlight = value.as_int().and_then(|i| usize::try_from(i).ok());
                PropChange::Paint
            }
            Prop::MinWidth => value.as_f32().map_or(PropChange::Unknown, |w| {
                self.min_width = Some(w);
                PropChange::Layout
            }),
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::SelectedIndex => Some(Value::Int(self.highlight.map_or(-1, |h| h as i64))),
            _ => None,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Menu;
    }
}

/// A row to put in a menu.
#[derive(Debug, Clone, Default)]
pub struct Row {
    /// The row.
    pub kind: RowKind,
    /// The label.
    pub label: String,
    /// The shortcut.
    pub shortcut: Option<String>,
    /// The icon.
    pub icon: Option<String>,
    /// Checked.
    pub checked: bool,
    /// Has a submenu.
    pub submenu: bool,
    /// Destructive.
    pub destructive: bool,
    /// Disabled.
    pub disabled: bool,
}

impl Row {
    /// A command.
    pub fn item(label: &str) -> Row {
        Row {
            label: label.to_string(),
            ..Row::default()
        }
    }
}

/// Makes a menu over `rows`; returns it and its row views.
pub fn menu(ui: &mut Ui, rows: &[Row]) -> (ViewId, Vec<ViewId>) {
    let m = ui.create(Menu::new());
    let mut views = Vec::new();
    for r in rows {
        let mut item = MenuItem::new(r.kind, &r.label);
        item.shortcut.clone_from(&r.shortcut);
        item.icon.clone_from(&r.icon);
        item.checked = r.checked;
        item.submenu = r.submenu;
        item.destructive = r.destructive;
        let v = ui.create(item);
        if r.disabled {
            let _ = ui.set(v, Prop::Disabled, Value::Bool(true));
        }
        ui.add_child(m, v);
        views.push(v);
    }
    (m, views)
}

/// How a presented menu sits against its anchor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Placement {
    /// Under the anchor, 3 pt away (a pull-down button).
    PullDown,
    /// Over the anchor, the row `row` of the given height on it (a pop-up button).
    PopUp {
        /// The row that lands on the button.
        row: usize,
    },
    /// At a point (a context menu).
    At,
    /// Under the anchor, 4 pt away, as wide as it (a combo box's list).
    List,
}

/// Shows `menu` as a popup of `parent`: sized, placed against `anchor` (window coordinates),
/// kept inside the parent window. Returns the popup.
pub fn present(
    ui: &mut Ui,
    parent: WindowId,
    anchor: Rect,
    menu: ViewId,
    placement: Placement,
) -> WindowId {
    let bounds = ui.window(parent).map(|w| w.size).unwrap_or_default();
    let (width, min) = match placement {
        Placement::List => (Some(anchor.width), anchor.width),
        Placement::PopUp { .. } => (None, anchor.width),
        Placement::PullDown => (None, anchor.width - 4.0),
        Placement::At => (None, metrics::MIN_WIDTH),
    };
    let _ = ui.set(menu, Prop::MinWidth, Value::from(min));
    let size = ui.measure(menu, Proposal::new(width, None));
    let margin = 6.0;
    let mut at = match placement {
        Placement::PullDown => Point::new(anchor.x - 5.0, anchor.bottom() + metrics::PULL_DOWN_GAP),
        Placement::List => Point::new(anchor.x, anchor.bottom() + 4.0),
        Placement::At => Point::new(anchor.x, anchor.y),
        Placement::PopUp { row } => {
            // The row centred on the control (docs/spec/menus.md, measured), 12 left of it.
            let on = metrics::PADDING + row as f32 * metrics::ITEM_HEIGHT;
            Point::new(
                anchor.x - 12.0,
                anchor.y + (anchor.height - metrics::ITEM_HEIGHT) / 2.0 - on,
            )
        }
    };
    at.x = at.x.min(bounds.width - margin - size.width).max(margin);
    at.y = at.y.min(bounds.height - margin - size.height).max(margin);
    let window = ui.create_window(
        WindowKind::Popup {
            parent,
            anchor: Rect::from_origin_size(at, size),
            grab: true,
        },
        size,
    );
    ui.set_content(window, menu);
    ui.show_window(window);
    window
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::event::{KeyEvent, Mods};
    use crate::core::window::WindowKind;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn rows() -> Vec<Row> {
        vec![
            Row::item("Orange"),
            Row {
                kind: RowKind::Separator,
                ..Row::default()
            },
            Row {
                disabled: true,
                ..Row::item("Lemon")
            },
            Row {
                shortcut: Some("⌘F".into()),
                ..Row::item("Fig")
            },
        ]
    }

    fn scene() -> (Ui, WindowId, ViewId, Rc<RefCell<Vec<i64>>>) {
        let mut ui = Ui::new_headless();
        let win = ui.create_window(WindowKind::Normal, Size::new(300.0, 300.0));
        let (m, _) = menu(&mut ui, &rows());
        let log = Rc::new(RefCell::new(Vec::new()));
        let l = log.clone();
        ui.connect(m, Signal::Activated, move |_, _, v| {
            l.borrow_mut().push(v.as_int().unwrap_or(-1));
        });
        ui.set_content(win, m);
        ui.show_window(win);
        ui.prepare_frame(win);
        (ui, win, m, log)
    }

    #[test]
    fn rows_are_24_and_8_tall_and_the_panel_4_in() {
        let (mut ui, _win, m, _log) = scene();
        let s = ui.measure(m, Proposal::UNSPECIFIED);
        assert_eq!(s.height, 4.0 + 24.0 + 8.0 + 24.0 + 24.0 + 4.0);
        assert!(s.width >= 160.0);
    }

    #[test]
    fn keys_skip_separators_and_disabled_rows_and_enter_chooses() {
        let (mut ui, win, m, log) = scene();
        ui.set_focus(m, true);
        ui.key(win, KeyEvent::press(Key::Down, Mods::empty()));
        assert_eq!(ui.get(m, Prop::SelectedIndex), Some(Value::Int(0)));
        ui.key(win, KeyEvent::press(Key::Down, Mods::empty()));
        // The separator and the disabled Lemon are skipped.
        assert_eq!(ui.get(m, Prop::SelectedIndex), Some(Value::Int(3)));
        ui.key(win, KeyEvent::press(Key::Enter, Mods::empty()));
        assert_eq!(*log.borrow(), vec![3]);
    }

    #[test]
    fn the_pointer_highlights_and_a_release_chooses() {
        let (mut ui, win, m, log) = scene();
        let r = ui.tree.window_rect(m);
        // The first row: y 4..28.
        let p = Point::new(r.x + 40.0, r.y + 14.0);
        ui.pointer_move(win, p, Mods::empty());
        assert_eq!(ui.get(m, Prop::SelectedIndex), Some(Value::Int(0)));
        ui.pointer_down(win, p, Button::Left, Mods::empty());
        ui.pointer_up(win, p, Button::Left, Mods::empty());
        assert_eq!(*log.borrow(), vec![0]);
        // The separator and the disabled row do nothing.
        let sep = Point::new(r.x + 40.0, r.y + 32.0);
        ui.pointer_move(win, sep, Mods::empty());
        assert_eq!(ui.get(m, Prop::SelectedIndex), Some(Value::Int(-1)));
    }

    #[test]
    fn present_puts_a_popup_over_the_button_with_the_row_on_it() {
        let mut ui = Ui::new_headless();
        let win = ui.create_window(WindowKind::Normal, Size::new(400.0, 300.0));
        let (m, _) = menu(&mut ui, &rows());
        let anchor = Rect::new(100.0, 100.0, 124.0, 28.0);
        let popup = present(&mut ui, win, anchor, m, Placement::PopUp { row: 1 });
        let Some(WindowKind::Popup { anchor: a, .. }) = ui.window(popup).map(|w| w.kind.clone())
        else {
            panic!("no popup");
        };
        // Row 1 (after 4 padding and one 24 row) sits on the button.
        assert_eq!(a.y, 100.0 + 2.0 - (4.0 + 24.0));
        assert_eq!(a.x, 88.0);
        assert!(a.width >= 124.0);
    }
}
