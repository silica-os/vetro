//! Collections: a virtualised list (rows made on demand by a provider and dropped when they
//! scroll out of view, so a list of a million rows costs what is on screen) and a table (a
//! header with sortable columns over a list whose rows are cells). Selection with the mouse
//! (⌘ toggles, ⇧ extends) and the keyboard, double click, an overlay scroller.

use std::collections::BTreeMap;

use crate::amalfi::foundation::Label;
use crate::amalfi::scrollbars::{BarLook, ScrollbarStyle, paint_bar};
use crate::amalfi::tokens;
use crate::core::a11y::{A11y, Action, Role};
use crate::core::event::{Button, Event, Handled, Key, Mods};
use crate::core::layout::{Axis, Proposal};
use crate::core::tree::ViewId;
use crate::core::ui::Ui;
use crate::core::value::{Prop, Signal, Value};
use crate::core::widget::{EventCx, FocusKind, LayoutCx, PaintCx, PropChange, Widget};
use crate::geom::{Point, Rect, Size};
use crate::shape::Shape;
use crate::style::Style;
use crate::text::{Align, TextStyle};

/// Makes the view of a row (or of a cell: row, column).
pub type RowProvider = Box<dyn FnMut(&mut Ui, usize) -> Option<ViewId>>;

/// Rows made beyond each edge of the viewport (smooth scrolling).
const OVERSCAN: usize = 4;

/// A virtualised list.
pub struct ListView {
    /// Number of rows.
    pub rows: usize,
    /// Height of every row.
    pub row_height: f32,
    /// Inset rows with rounded selections (source lists, menus-like lists).
    pub inset: bool,
    provider: Option<RowProvider>,
    live: BTreeMap<usize, ViewId>,
    scroll: f32,
    height: f32,
    width: f32,
    selected: Vec<usize>,
    anchor: Option<usize>,
    /// Rows are made again (the data changed).
    dirty: bool,
    /// The pointer is over the list (the overlay scroller shows).
    hovered: bool,
}

impl std::fmt::Debug for ListView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ListView")
            .field("rows", &self.rows)
            .field("live", &self.live.len())
            .field("selected", &self.selected)
            .finish()
    }
}

impl ListView {
    /// A list of `rows` rows of `row_height`, made by `provider`.
    pub fn new(rows: usize, row_height: f32, provider: RowProvider) -> Self {
        ListView {
            rows,
            row_height: row_height.max(1.0),
            inset: true,
            provider: Some(provider),
            live: BTreeMap::new(),
            scroll: 0.0,
            height: 0.0,
            width: 0.0,
            selected: Vec::new(),
            anchor: None,
            dirty: false,
            hovered: false,
        }
    }

    /// The selected rows, sorted.
    pub fn selection(&self) -> &[usize] {
        &self.selected
    }

    /// Replaces the rows: their number, height and provider (the rows are made again and the
    /// selection is kept where it still exists).
    pub fn reset(&mut self, rows: usize, row_height: f32, provider: RowProvider) {
        self.rows = rows;
        if row_height.is_finite() {
            self.row_height = row_height.max(1.0);
        }
        self.selected.retain(|i| *i < rows);
        self.set_provider(provider);
    }

    /// Replaces the provider (and remakes the rows).
    pub fn set_provider(&mut self, provider: RowProvider) {
        self.provider = Some(provider);
        self.dirty = true;
    }

    fn content_height(&self) -> f32 {
        self.rows as f32 * self.row_height
    }

    fn clamp(&mut self) {
        let max = (self.content_height() - self.height).max(0.0);
        self.scroll = self.scroll.clamp(0.0, max);
    }

    fn visible(&self) -> std::ops::Range<usize> {
        if self.rows == 0 {
            return 0..0;
        }
        let first = (self.scroll / self.row_height).floor().max(0.0) as usize;
        let last = ((self.scroll + self.height) / self.row_height)
            .ceil()
            .max(0.0) as usize;
        first.saturating_sub(OVERSCAN)..(last + OVERSCAN).min(self.rows)
    }

    /// Makes the visible rows and drops the others.
    fn sync_rows(&mut self, cx: &mut LayoutCx<'_>) {
        let id = cx.id();
        let ui = &mut *cx.ui;
        if self.dirty {
            for (_, v) in std::mem::take(&mut self.live) {
                ui.detach_now(v);
                ui.destroy(v);
            }
            self.dirty = false;
        }
        let range = self.visible();
        let gone: Vec<usize> = self
            .live
            .keys()
            .copied()
            .filter(|i| !range.contains(i))
            .collect();
        for i in gone {
            if let Some(v) = self.live.remove(&i) {
                ui.detach_now(v);
                ui.destroy(v);
            }
        }
        let Some(provider) = self.provider.as_mut() else {
            return;
        };
        for i in range {
            if self.live.contains_key(&i) {
                continue;
            }
            if let Some(v) = provider(ui, i) {
                ui.add_child(id, v);
                // The list holds the row through its parent link only.
                ui.release(v);
                self.live.insert(i, v);
            }
        }
    }

    fn scroll_to(&mut self, i: usize) {
        let top = i as f32 * self.row_height;
        if top < self.scroll {
            self.scroll = top;
        } else if top + self.row_height > self.scroll + self.height {
            self.scroll = top + self.row_height - self.height;
        }
        self.clamp();
    }

    fn select(&mut self, cx: &mut EventCx<'_>, i: usize, mods: Mods) {
        if i >= self.rows {
            return;
        }
        if mods.contains(Mods::LOGO) {
            match self.selected.binary_search(&i) {
                Ok(p) => {
                    self.selected.remove(p);
                }
                Err(p) => self.selected.insert(p, i),
            }
            self.anchor = Some(i);
        } else if mods.contains(Mods::SHIFT) {
            let a = self.anchor.unwrap_or(i);
            self.selected = (a.min(i)..=a.max(i)).collect();
        } else {
            self.selected = vec![i];
            self.anchor = Some(i);
        }
        cx.emit(
            Signal::SelectionChanged,
            Value::IntList(self.selected.iter().map(|x| *x as i64).collect()),
        );
        cx.request_paint();
    }
}

impl Widget for ListView {
    fn kind(&self) -> &'static str {
        "List"
    }

    fn clips_children(&self) -> bool {
        true
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        p.replacing_unspecified(Size::new(
            240.0,
            (self.rows as f32 * self.row_height).min(320.0),
        ))
    }

    fn place(&mut self, cx: &mut LayoutCx<'_>, size: Size) {
        self.width = size.width;
        self.height = size.height;
        self.clamp();
        self.sync_rows(cx);
        let inset = if self.inset { 8.0 } else { 0.0 };
        let entries: Vec<(usize, ViewId)> = self.live.iter().map(|(i, v)| (*i, *v)).collect();
        for (i, v) in entries {
            let y = i as f32 * self.row_height - self.scroll;
            cx.place(
                v,
                Rect::new(
                    inset,
                    y,
                    (size.width - 2.0 * inset).max(0.0),
                    self.row_height,
                ),
            );
        }
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let r = cx.bounds();
        let inset = if self.inset { 8.0 } else { 0.0 };
        let focused = cx.is_focused();
        let color = if focused || !self.inset {
            theme.colors.accent_amalfi
        } else {
            theme.colors.accent_amalfi_soft
        };
        for i in &self.selected {
            let y = *i as f32 * self.row_height - self.scroll;
            if y + self.row_height < 0.0 || y > r.height {
                continue;
            }
            let rect = Rect::new(
                r.x + inset,
                r.y + y,
                (r.width - 2.0 * inset).max(0.0),
                self.row_height,
            );
            let shape = if self.inset {
                Shape::rounded(tokens::radii::SMALL_CONTROL)
            } else {
                Shape::RECT
            };
            cx.rec.layer("Selection", rect, shape, Style::fill(color));
        }
    }

    fn paint_over(&mut self, cx: &mut PaintCx<'_>) {
        let content = self.content_height();
        // Overlay scrollers show while the pointer is over the list.
        if !self.hovered || content <= self.height || self.height <= 0.0 {
            return;
        }
        let r = cx.bounds();
        let style = ScrollbarStyle::Overlay;
        let t = style.thickness();
        let track = Rect::new(r.right() - t, r.y, t, r.height);
        let extent = (r.height * r.height / content).max(24.0).min(r.height);
        let offset = (r.height - extent) * (self.scroll / (content - self.height).max(1.0));
        paint_bar(
            cx,
            "Scroller/",
            track,
            &BarLook {
                axis: Axis::Vertical,
                style,
                expansion: 0.0,
                hovered: false,
                dragging: false,
                thumb: Some((offset, extent)),
            },
        );
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        match event {
            Event::Scroll { dy, .. } => {
                self.scroll += *dy;
                self.clamp();
                cx.request_layout();
                cx.request_paint();
                cx.emit(Signal::Scrolled, Value::Point(Point::new(0.0, self.scroll)));
                Handled::Yes
            }
            Event::PointerDown {
                pos,
                button: Button::Left,
                clicks,
                mods,
            } => {
                let y = pos.y - cx.bounds().y + self.scroll;
                let i = (y / self.row_height).floor().max(0.0) as usize;
                if i < self.rows {
                    if *clicks >= 2 {
                        cx.emit(Signal::DoubleClicked, Value::from(i));
                    } else {
                        self.select(cx, i, *mods);
                    }
                }
                cx.focus_self();
                Handled::Yes
            }
            Event::Key(k) if k.pressed => {
                let current = self.anchor.or_else(|| self.selected.last().copied());
                let next = match k.key {
                    Key::Down => {
                        Some(current.map_or(0, |c| (c + 1).min(self.rows.saturating_sub(1))))
                    }
                    Key::Up => Some(current.map_or(0, |c| c.saturating_sub(1))),
                    Key::Home => Some(0),
                    Key::End => Some(self.rows.saturating_sub(1)),
                    Key::Enter => {
                        if let Some(c) = current {
                            cx.emit(Signal::DoubleClicked, Value::from(c));
                        }
                        return Handled::Yes;
                    }
                    Key::Char('a') if k.mods.contains(Mods::LOGO) => {
                        self.selected = (0..self.rows).collect();
                        cx.emit(
                            Signal::SelectionChanged,
                            Value::IntList(self.selected.iter().map(|x| *x as i64).collect()),
                        );
                        cx.request_paint();
                        return Handled::Yes;
                    }
                    _ => None,
                };
                match next {
                    Some(i) if self.rows > 0 => {
                        self.select(cx, i, k.mods & Mods::SHIFT);
                        self.scroll_to(i);
                        cx.request_layout();
                        Handled::Yes
                    }
                    _ => Handled::No,
                }
            }
            Event::PointerEnter => {
                self.hovered = true;
                cx.request_paint();
                Handled::No
            }
            Event::PointerLeave => {
                self.hovered = false;
                cx.request_paint();
                Handled::No
            }
            _ => Handled::No,
        }
    }

    fn focus(&self) -> FocusKind {
        FocusKind::Always
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match prop {
            Prop::RowCount => match value.as_int().and_then(|n| usize::try_from(n).ok()) {
                Some(n) => {
                    self.rows = n;
                    self.selected.retain(|i| *i < n);
                    self.dirty = true;
                    PropChange::Layout
                }
                None => PropChange::Unknown,
            },
            Prop::Height => PropChange::Unknown,
            Prop::SelectedRows => match value {
                Value::IntList(l) => {
                    let mut v: Vec<usize> = l
                        .iter()
                        .filter_map(|i| usize::try_from(*i).ok())
                        .filter(|i| *i < self.rows)
                        .collect();
                    v.sort_unstable();
                    v.dedup();
                    self.selected = v;
                    PropChange::Paint
                }
                _ => PropChange::Unknown,
            },
            Prop::ScrollOffset => match value {
                Value::Point(p) => {
                    self.scroll = p.y;
                    self.clamp();
                    PropChange::Layout
                }
                _ => PropChange::Unknown,
            },
            Prop::Style => match value.as_int() {
                Some(s) => {
                    self.inset = s == 0;
                    PropChange::Layout
                }
                None => PropChange::Unknown,
            },
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::RowCount => Some(Value::from(self.rows)),
            Prop::SelectedRows => Some(Value::IntList(
                self.selected.iter().map(|i| *i as i64).collect(),
            )),
            Prop::ScrollOffset => Some(Value::Point(Point::new(0.0, self.scroll))),
            _ => None,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::List;
        a.value = Some(format!("{} rows", self.rows));
        a.actions = vec![Action::Focus, Action::ScrollIntoView];
    }
}

/// A column of a table.
#[derive(Debug, Clone, PartialEq)]
pub struct Column {
    /// Title.
    pub title: String,
    /// Width (the last column takes the rest).
    pub width: f32,
}

/// The header of a table: column titles, click to sort (the sort indicator toggles).
#[derive(Debug)]
pub struct TableHeader {
    /// Columns.
    pub columns: Vec<Column>,
    /// The sorted column and whether ascending.
    pub sort: Option<(usize, bool)>,
    labels: Vec<Label>,
}

/// Height of a table header.
pub const HEADER_HEIGHT: f32 = 28.0;

impl TableHeader {
    /// A header.
    pub fn new(columns: Vec<Column>) -> Self {
        TableHeader {
            columns,
            sort: None,
            labels: Vec::new(),
        }
    }

    fn column_at(&self, x: f32, width: f32) -> Option<usize> {
        let mut left = 0.0;
        for (i, c) in self.columns.iter().enumerate() {
            let w = if i + 1 == self.columns.len() {
                (width - left).max(c.width)
            } else {
                c.width
            };
            if x >= left && x < left + w {
                return Some(i);
            }
            left += w;
        }
        None
    }
}

impl Widget for TableHeader {
    fn kind(&self) -> &'static str {
        "TableHeader"
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        let natural: f32 = self.columns.iter().map(|c| c.width).sum();
        Size::new(
            p.width.filter(|w| w.is_finite()).unwrap_or(natural),
            HEADER_HEIGHT,
        )
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let theme = *cx.theme();
        let c = theme.colors;
        let r = cx.bounds();
        cx.rec.layer(
            "Separator",
            Rect::new(r.x, r.bottom() - 1.0, r.width, 1.0),
            Shape::RECT,
            Style::fill(c.line_separator),
        );
        if self.labels.len() != self.columns.len() {
            self.labels = self.columns.iter().map(|_| Label::default()).collect();
        }
        let style: TextStyle = theme.text(&tokens::typography::HEADLINE, c.text_secondary);
        let mut left = r.x;
        let n = self.columns.len();
        for (i, col) in self.columns.iter().enumerate() {
            let w = if i + 1 == n {
                (r.right() - left).max(col.width)
            } else {
                col.width
            };
            if let Some(label) = self.labels.get_mut(i) {
                let layout = label.layout(
                    &col.title,
                    &style,
                    Align::Start,
                    (w - 28.0).max(1.0),
                    Some(1),
                );
                let lh = layout.size().height;
                cx.rec.text(
                    "Title",
                    Rect::new(
                        left + 8.0,
                        r.y + (r.height - lh) / 2.0,
                        (w - 28.0).max(1.0),
                        lh,
                    ),
                    layout,
                    Some(tokens::typography::HEADLINE.name),
                    c.text_secondary,
                );
            }
            if let Some((s, asc)) = self.sort
                && s == i
            {
                let sym = if asc { "chevron-up" } else { "chevron-down" };
                cx.rec.glyph(
                    "Sort",
                    Rect::new(left + w - 20.0, r.y + (r.height - 14.0) / 2.0, 14.0, 14.0),
                    sym,
                    400,
                    c.text_secondary,
                );
            }
            if i + 1 < n {
                cx.rec.layer(
                    "Divider",
                    Rect::new(left + w - 1.0, r.y + 6.0, 1.0, r.height - 12.0),
                    Shape::RECT,
                    Style::fill(c.line_separator),
                );
            }
            left += w;
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        if let Event::PointerDown {
            pos,
            button: Button::Left,
            ..
        } = event
        {
            let x = pos.x - cx.bounds().x;
            if let Some(i) = self.column_at(x, cx.bounds().width) {
                self.sort = Some(match self.sort {
                    Some((s, asc)) if s == i => (i, !asc),
                    _ => (i, true),
                });
                let (col, asc) = self.sort.unwrap_or((i, true));
                cx.emit(
                    Signal::ValueChanged,
                    Value::Int(if asc { col as i64 } else { -(col as i64) - 1 }),
                );
                cx.request_paint();
                return Handled::Yes;
            }
        }
        Handled::No
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match prop {
            Prop::ColumnTitles => match value.as_str_list() {
                Some(l) => {
                    self.columns = l
                        .iter()
                        .map(|t| Column {
                            title: t.clone(),
                            width: 160.0,
                        })
                        .collect();
                    PropChange::Paint
                }
                None => PropChange::Unknown,
            },
            _ => PropChange::Unknown,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Row;
        a.label = Some(
            self.columns
                .iter()
                .map(|c| c.title.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::amalfi::label::Label as Text;
    use crate::core::event::KeyEvent;
    use crate::core::window::WindowKind;
    use std::cell::Cell;
    use std::rc::Rc;

    #[test]
    fn a_million_rows_cost_what_is_shown() {
        let mut ui = Ui::new_headless();
        let w = ui.create_window(WindowKind::Normal, Size::new(300.0, 280.0));
        let made = Rc::new(Cell::new(0usize));
        let m = made.clone();
        let list = ui.create(ListView::new(
            1_000_000,
            28.0,
            Box::new(move |ui, i| {
                m.set(m.get() + 1);
                Some(ui.create(Text::new(&format!("Row {i}"))))
            }),
        ));
        ui.set(list, Prop::MaxWidth, Value::from(f32::INFINITY))
            .unwrap();
        ui.set(list, Prop::MaxHeight, Value::from(f32::INFINITY))
            .unwrap();
        ui.set_content(w, list);
        ui.show_window(w);
        ui.prepare_frame(w);
        let live = ui.tree.children(list).len();
        assert!(live > 8 && live < 30, "{live} rows live");
        // Scrolling far makes new rows and drops the old ones.
        ui.scroll(
            w,
            Point::new(100.0, 100.0),
            0.0,
            28.0 * 500_000.0,
            true,
            crate::core::event::Phase::Changed,
        );
        ui.prepare_frame(w);
        assert!(ui.tree.children(list).len() < 30);
        assert!(made.get() < 80);
        let first = ui.tree.children(list).first().copied().unwrap();
        assert!(
            ui.get(first, Prop::Text)
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .unwrap()
                .starts_with("Row 49999")
        );
        // Keyboard selection.
        ui.set_focus(list, false);
        ui.key(w, KeyEvent::press(Key::Down, Mods::empty()));
        ui.key(w, KeyEvent::press(Key::Down, Mods::SHIFT));
        let sel = ui.get(list, Prop::SelectedRows).unwrap();
        assert!(matches!(sel, Value::IntList(ref l) if l.len() == 2));
    }

    #[test]
    fn header_sorts() {
        let mut ui = Ui::new_headless();
        let w = ui.create_window(WindowKind::Normal, Size::new(400.0, 100.0));
        let h = ui.create(TableHeader::new(vec![
            Column {
                title: "Name".into(),
                width: 200.0,
            },
            Column {
                title: "Size".into(),
                width: 100.0,
            },
        ]));
        ui.set(h, Prop::MaxWidth, Value::from(f32::INFINITY))
            .unwrap();
        ui.set_content(w, h);
        ui.show_window(w);
        ui.prepare_frame(w);
        let got = Rc::new(Cell::new(99i64));
        let g = got.clone();
        ui.connect(h, Signal::ValueChanged, move |_, _, v| {
            g.set(v.as_int().unwrap_or(0))
        });
        let r = ui.tree.window_rect(h);
        ui.pointer_down(
            w,
            Point::new(r.x + 250.0, r.y + 10.0),
            Button::Left,
            Mods::empty(),
        );
        ui.pointer_up(
            w,
            Point::new(r.x + 250.0, r.y + 10.0),
            Button::Left,
            Mods::empty(),
        );
        assert_eq!(got.get(), 1);
        ui.pointer_down(
            w,
            Point::new(r.x + 250.0, r.y + 10.0),
            Button::Left,
            Mods::empty(),
        );
        assert_eq!(got.get(), -2, "descending");
    }
}
