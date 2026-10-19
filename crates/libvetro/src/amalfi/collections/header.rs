//! Table columns and their header (kit: `Apps/Mustache/List Header`): titles in `Label`
//! type on `Text/Tertiary`, a separator under them, click to sort (the indicator toggles),
//! drag a column's edge to resize it.

use super::metrics;
use crate::amalfi::foundation::Label;
use crate::amalfi::tokens;
use crate::core::a11y::{A11y, Role};
use crate::core::event::{Button, Event, Handled};
use crate::core::layout::Proposal;
use crate::core::ui::Cursor;
use crate::core::value::{Prop, Signal, Value};
use crate::core::widget::{EventCx, LayoutCx, PaintCx, PropChange, Widget};
use crate::geom::{Rect, Size};
use crate::shape::Shape;
use crate::style::Style;
use crate::text::{Align, TextStyle};

/// A column of a table.
#[derive(Debug, Clone, PartialEq)]
pub struct TableColumn {
    /// Title in the header.
    pub title: String,
    /// Width (the last column also takes the rest of the table's width).
    pub width: f32,
    /// The narrowest it gets by resizing.
    pub min_width: f32,
    /// The widest it gets by resizing.
    pub max_width: f32,
    /// Its cells can be edited in place (double click, Return).
    pub editable: bool,
    /// Clicking its title sorts by it.
    pub sortable: bool,
    /// Its right edge can be dragged.
    pub resizable: bool,
    /// Alignment of its title.
    pub align: Align,
    /// The title's inset from the column's left edge (the first column of a file list
    /// aligns its title with the names after the icons: 24).
    pub title_inset: f32,
}

impl TableColumn {
    /// A sortable, resizable column.
    pub fn new(title: &str, width: f32) -> TableColumn {
        TableColumn {
            title: title.to_string(),
            width: width.max(1.0),
            min_width: metrics::MIN_COLUMN,
            max_width: f32::INFINITY,
            editable: false,
            sortable: true,
            resizable: true,
            align: Align::Start,
            title_inset: 0.0,
        }
    }

    /// Its cells can be edited.
    pub fn editable(mut self, editable: bool) -> Self {
        self.editable = editable;
        self
    }

    /// Clicking its title sorts.
    pub fn sortable(mut self, sortable: bool) -> Self {
        self.sortable = sortable;
        self
    }

    /// Limits for resizing.
    pub fn limits(mut self, min: f32, max: f32) -> Self {
        self.min_width = min.max(1.0);
        self.max_width = max.max(self.min_width);
        self
    }

    /// The title's alignment.
    pub fn align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }

    /// The title's inset.
    pub fn title_inset(mut self, inset: f32) -> Self {
        self.title_inset = inset.max(0.0);
        self
    }
}

/// The old name of [`TableColumn`].
pub type Column = TableColumn;

/// Each column's left edge and width in a table `width` wide.
pub(crate) fn column_frames(columns: &[TableColumn], width: f32) -> Vec<(f32, f32)> {
    let mut x = metrics::LEADING;
    let n = columns.len();
    let mut out = Vec::with_capacity(n);
    for (i, c) in columns.iter().enumerate() {
        let w = if i + 1 == n {
            (width - metrics::TRAILING - x).max(c.width)
        } else {
            c.width
        };
        out.push((x, w));
        x += w + metrics::COLUMN_SPACING;
    }
    out
}

/// The natural width of a table with these columns.
pub(crate) fn natural_width(columns: &[TableColumn]) -> f32 {
    let w: f32 = columns.iter().map(|c| c.width).sum();
    metrics::LEADING
        + w
        + metrics::COLUMN_SPACING * columns.len().saturating_sub(1) as f32
        + metrics::TRAILING
}

/// The sort as one integer: the column when ascending, `-column - 1` when descending.
pub(crate) fn sort_value(sort: Option<(usize, bool)>) -> Value {
    match sort {
        Some((c, true)) => Value::Int(c as i64),
        Some((c, false)) => Value::Int(-(c as i64) - 1),
        None => Value::None,
    }
}

/// The sort from its integer form.
pub(crate) fn sort_from(v: &Value) -> Option<Option<(usize, bool)>> {
    match v {
        Value::None => Some(None),
        other => {
            let i = other.as_int()?;
            Some(Some(if i >= 0 {
                (usize::try_from(i).ok()?, true)
            } else {
                (usize::try_from(-(i + 1)).ok()?, false)
            }))
        }
    }
}

/// The header's state: columns, sort and a resize in progress. Shared by [`TableHeader`]
/// and the table view.
#[derive(Debug, Default)]
pub(crate) struct Header {
    pub columns: Vec<TableColumn>,
    pub sort: Option<(usize, bool)>,
    labels: Vec<Label>,
    /// The column being resized: (column, pointer x at the start, width at the start).
    resizing: Option<(usize, f32, f32)>,
}

/// What a pointer press on the header did.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum HeaderHit {
    /// The sort changed.
    Sorted,
    /// A resize began.
    Resizing,
    /// Nothing.
    None,
}

impl Header {
    pub fn new(columns: Vec<TableColumn>) -> Header {
        Header {
            columns,
            ..Header::default()
        }
    }

    /// The column whose right edge is at `x` (resizable columns only, not the last).
    fn edge_at(&self, x: f32, width: f32) -> Option<usize> {
        let frames = column_frames(&self.columns, width);
        let n = frames.len();
        frames.iter().enumerate().find_map(|(i, (l, w))| {
            let edge = l + w + metrics::COLUMN_SPACING / 2.0;
            (i + 1 < n
                && self.columns.get(i).is_some_and(|c| c.resizable)
                && (x - edge).abs() <= metrics::RESIZE_SLOP)
                .then_some(i)
        })
    }

    /// The column under `x`.
    pub fn column_at(&self, x: f32, width: f32) -> Option<usize> {
        column_frames(&self.columns, width)
            .iter()
            .position(|(l, w)| {
                x >= l - metrics::COLUMN_SPACING / 2.0 && x < l + w + metrics::COLUMN_SPACING / 2.0
            })
    }

    /// A press at `x` in a header `width` wide.
    pub fn press(&mut self, x: f32, width: f32) -> HeaderHit {
        if let Some(i) = self.edge_at(x, width) {
            let w = self.columns.get(i).map_or(0.0, |c| c.width);
            self.resizing = Some((i, x, w));
            return HeaderHit::Resizing;
        }
        let Some(i) = self.column_at(x, width) else {
            return HeaderHit::None;
        };
        if !self.columns.get(i).is_some_and(|c| c.sortable) {
            return HeaderHit::None;
        }
        self.sort = Some(match self.sort {
            Some((s, asc)) if s == i => (i, !asc),
            _ => (i, true),
        });
        HeaderHit::Sorted
    }

    /// A drag to `x`: resizes; returns the column and its new width.
    pub fn drag(&mut self, x: f32) -> Option<(usize, f32)> {
        let (i, x0, w0) = self.resizing?;
        let c = self.columns.get_mut(i)?;
        let w = (w0 + x - x0).clamp(c.min_width, c.max_width);
        if w == c.width {
            return None;
        }
        c.width = w;
        Some((i, w))
    }

    /// The press ended.
    pub fn release(&mut self) -> bool {
        self.resizing.take().is_some()
    }

    pub fn is_resizing(&self) -> bool {
        self.resizing.is_some()
    }

    /// The cursor over `x`.
    pub fn cursor(&self, x: f32, width: f32) -> Cursor {
        if self.resizing.is_some() || self.edge_at(x, width).is_some() {
            Cursor::ColResize
        } else {
            Cursor::Default
        }
    }

    /// Records the header in `rect`.
    pub fn paint(&mut self, cx: &mut PaintCx<'_>, rect: Rect) {
        let theme = *cx.theme();
        let c = theme.colors;
        cx.rec.layer(
            "Separator",
            Rect::new(rect.x, rect.bottom() - 1.0, rect.width, 1.0),
            Shape::RECT,
            Style::fill(c.line_separator),
        );
        if self.labels.len() != self.columns.len() {
            self.labels = self.columns.iter().map(|_| Label::default()).collect();
        }
        let style: TextStyle = theme.text(&tokens::typography::LABEL, c.text_tertiary);
        let frames = column_frames(&self.columns, rect.width);
        for (i, (col, (x, w))) in self.columns.iter().zip(frames).enumerate() {
            let sorted = self.sort.filter(|(s, _)| *s == i);
            let indicator = if sorted.is_some() {
                metrics::DISCLOSURE
            } else {
                0.0
            };
            let tw = (w - col.title_inset - indicator).max(1.0);
            if let Some(label) = self.labels.get_mut(i) {
                let layout = label.layout(&col.title, &style, col.align, tw, Some(1));
                let lh = layout.size().height;
                cx.rec.text(
                    "Title",
                    Rect::new(
                        rect.x + x + col.title_inset,
                        rect.y + metrics::HEADER_TITLE_Y,
                        tw,
                        lh,
                    ),
                    layout,
                    Some(tokens::typography::LABEL.name),
                    c.text_tertiary,
                );
            }
            if let Some((_, asc)) = sorted {
                let g = metrics::DISCLOSURE_GLYPH;
                let sym = if asc { "chevron-up" } else { "chevron-down" };
                cx.rec.glyph(
                    "Sort",
                    Rect::new(
                        rect.x + x + w - g,
                        rect.y + (metrics::HEADER_HEIGHT - g) / 2.0,
                        g,
                        g,
                    ),
                    sym,
                    500,
                    c.text_tertiary,
                );
            }
        }
    }
}

/// A table header on its own (the table view draws its own).
#[derive(Debug)]
pub struct TableHeader {
    header: Header,
}

impl TableHeader {
    /// A header.
    pub fn new(columns: Vec<TableColumn>) -> Self {
        TableHeader {
            header: Header::new(columns),
        }
    }

    /// The columns.
    pub fn columns(&self) -> &[TableColumn] {
        &self.header.columns
    }

    /// The sorted column and whether ascending.
    pub fn sort(&self) -> Option<(usize, bool)> {
        self.header.sort
    }
}

impl Widget for TableHeader {
    fn kind(&self) -> &'static str {
        "TableHeader"
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        Size::new(
            p.width
                .filter(|w| w.is_finite())
                .unwrap_or(natural_width(&self.header.columns)),
            metrics::HEADER_HEIGHT,
        )
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let r = cx.bounds();
        self.header.paint(cx, r);
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        let b = cx.bounds();
        match event {
            Event::PointerDown {
                pos,
                button: Button::Left,
                ..
            } => match self.header.press(pos.x - b.x, b.width) {
                HeaderHit::Sorted => {
                    cx.emit(Signal::ValueChanged, sort_value(self.header.sort));
                    cx.emit(Signal::SortChanged, sort_value(self.header.sort));
                    cx.request_paint();
                    Handled::Yes
                }
                HeaderHit::Resizing => {
                    cx.capture_pointer();
                    Handled::Yes
                }
                HeaderHit::None => Handled::No,
            },
            Event::PointerMove { pos, .. } => {
                if let Some((i, w)) = self.header.drag(pos.x - b.x) {
                    cx.emit(
                        Signal::ColumnResized,
                        Value::IntList(vec![i as i64, w.round() as i64]),
                    );
                    cx.request_paint();
                }
                cx.set_cursor(self.header.cursor(pos.x - b.x, b.width));
                Handled::No
            }
            Event::PointerUp { .. } => {
                if self.header.release() {
                    Handled::Yes
                } else {
                    Handled::No
                }
            }
            _ => Handled::No,
        }
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match prop {
            Prop::ColumnTitles => match value.as_str_list() {
                Some(l) => {
                    self.header.columns = l.iter().map(|t| TableColumn::new(t, 160.0)).collect();
                    PropChange::Paint
                }
                None => PropChange::Unknown,
            },
            Prop::ColumnWidths => match value.as_int_list() {
                Some(l) => {
                    for (c, w) in self.header.columns.iter_mut().zip(l) {
                        c.width = (w as f32).clamp(c.min_width, c.max_width);
                    }
                    PropChange::Paint
                }
                _ => PropChange::Unknown,
            },
            Prop::SortDescriptor => match sort_from(value) {
                Some(s) => {
                    self.header.sort = s;
                    PropChange::Paint
                }
                None => PropChange::Unknown,
            },
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::ColumnTitles => Some(Value::StrList(
                self.header
                    .columns
                    .iter()
                    .map(|c| c.title.clone())
                    .collect(),
            )),
            Prop::ColumnWidths => Some(Value::IntList(
                self.header
                    .columns
                    .iter()
                    .map(|c| c.width.round() as i64)
                    .collect(),
            )),
            Prop::SortDescriptor => Some(sort_value(self.header.sort)),
            _ => None,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Row;
        a.label = Some(
            self.header
                .columns
                .iter()
                .map(|c| c.title.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        );
    }
}
