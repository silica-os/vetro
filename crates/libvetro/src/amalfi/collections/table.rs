//! The table view: columns under a header, cells made per row and column by a provider when
//! their row comes into sight, in-place editing, and an outline mode where rows are the
//! visible nodes of a tree with disclosure chevrons.
//!
//! Rows are known to the app by their *key*: the row index in a table, the node id in an
//! outline. The cell provider, `SelectionChanged`, `DoubleClicked`, `CellEdited` and
//! `Prop::SelectedRows` all use keys.

use std::collections::{BTreeMap, HashSet};

use super::header::{
    Header, HeaderHit, TableColumn, column_frames, natural_width, sort_from, sort_value,
};
use super::{
    Body, ListLook, Rows, drop_view, emphasized, make_body, metrics, paint_box, paint_box_border,
    paint_scroller, selection_fill,
};
use crate::amalfi::fields::{FieldKind, TextField};
use crate::core::a11y::{A11y, Action, Role};
use crate::core::event::{Button, Event, Handled, Key, Mods};
use crate::core::layout::Proposal;
use crate::core::tree::{NodeFlags, ViewId};
use crate::core::ui::Ui;
use crate::core::value::{Prop, Signal, Value};
use crate::core::widget::{EventCx, FocusKind, LayoutCx, PaintCx, PropChange, Widget};
use crate::geom::{Insets, Point, Rect, Size, Transform};
use crate::shape::Shape;
use crate::style::Style;

/// Makes the view of a cell: (row key, column).
pub type CellProvider = Box<dyn FnMut(&mut Ui, u64, usize) -> Option<ViewId>>;

/// The tree an outline shows. Node ids are the app's; `None` is the invisible root.
pub trait TreeSource {
    /// Number of children of `parent`.
    fn child_count(&mut self, ui: &mut Ui, parent: Option<u64>) -> usize;
    /// The id of child `index` of `parent`.
    fn child(&mut self, ui: &mut Ui, parent: Option<u64>, index: usize) -> u64;
    /// Whether `node` can be expanded (shows a chevron).
    fn is_expandable(&mut self, ui: &mut Ui, node: u64) -> bool;
}

/// A tree with no nodes.
struct EmptyTree;

impl TreeSource for EmptyTree {
    fn child_count(&mut self, _ui: &mut Ui, _parent: Option<u64>) -> usize {
        0
    }

    fn child(&mut self, _ui: &mut Ui, _parent: Option<u64>, _index: usize) -> u64 {
        0
    }

    fn is_expandable(&mut self, _ui: &mut Ui, _node: u64) -> bool {
        false
    }
}

/// Deepest outline level followed (a guard against cyclic sources).
const MAX_LEVEL: u16 = 64;

/// A visible node of an outline.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Flat {
    node: u64,
    level: u16,
    expandable: bool,
}

struct Outline {
    source: Box<dyn TreeSource>,
    flat: Vec<Flat>,
    expanded: HashSet<u64>,
    /// The flat rows are built again at the next layout.
    stale: bool,
}

impl Outline {
    fn flatten(&mut self, ui: &mut Ui, parent: Option<u64>, level: u16, out: &mut Vec<Flat>) {
        if level > MAX_LEVEL {
            return;
        }
        let n = self.source.child_count(ui, parent);
        for i in 0..n {
            let node = self.source.child(ui, parent, i);
            let expandable = self.source.is_expandable(ui, node);
            out.push(Flat {
                node,
                level,
                expandable,
            });
            if expandable && self.expanded.contains(&node) {
                self.flatten(ui, Some(node), level + 1, out);
            }
        }
    }

    /// The visible descendants of `node` (expanded), as flat rows.
    fn subtree(&mut self, ui: &mut Ui, node: u64, level: u16) -> Vec<Flat> {
        let mut out = Vec::new();
        self.flatten(ui, Some(node), level + 1, &mut out);
        out
    }
}

/// A live row: its host view and its cells.
#[derive(Debug)]
struct LiveRow {
    host: ViewId,
    key: u64,
    cells: Vec<Option<ViewId>>,
}

/// A cell being edited.
#[derive(Debug, Clone, Copy)]
struct Editing {
    row: usize,
    column: usize,
    key: u64,
    field: ViewId,
    cell: Option<ViewId>,
}

/// A table (or an outline) of rows with columns.
pub struct TableView {
    header: Header,
    show_header: bool,
    rows: Rows,
    look: ListLook,
    alternating: bool,
    provider: Option<CellProvider>,
    outline: Option<Outline>,
    body: Option<ViewId>,
    live: BTreeMap<usize, LiveRow>,
    dirty: bool,
    hovered: bool,
    emphasis: bool,
    editing: Option<Editing>,
    /// An edit asked for before its row was made (Return on a row out of sight).
    pending_edit: Option<(usize, usize)>,
    /// The table's own view (known from its first layout).
    me: Option<ViewId>,
    /// Keys to select once their rows exist (set before the tree was read).
    pending_keys: Option<Vec<i64>>,
}

impl std::fmt::Debug for TableView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TableView")
            .field("columns", &self.header.columns.len())
            .field("rows", &self.rows.count)
            .field("live", &self.live.len())
            .field("outline", &self.outline.is_some())
            .finish()
    }
}

impl TableView {
    /// A table with `columns` and no rows.
    pub fn new(columns: Vec<TableColumn>) -> TableView {
        TableView {
            header: Header::new(columns),
            show_header: true,
            rows: Rows::new(0, metrics::ROW_HEIGHT),
            look: ListLook::Inset,
            alternating: false,
            provider: None,
            outline: None,
            body: None,
            live: BTreeMap::new(),
            dirty: false,
            hovered: false,
            emphasis: false,
            editing: None,
            pending_edit: None,
            me: None,
            pending_keys: None,
        }
    }

    /// An outline with `columns` and an empty tree (see [`TableView::set_tree`]).
    pub fn outline(columns: Vec<TableColumn>) -> TableView {
        let mut t = TableView::new(columns);
        t.set_tree(
            Box::new(EmptyTree),
            metrics::ROW_HEIGHT,
            Box::new(|_, _, _| None),
        );
        t
    }

    /// The look (inset selections by default, as the kit's file list).
    pub fn look(mut self, look: ListLook) -> Self {
        self.look = look;
        self
    }

    /// Alternating row backgrounds (kit: `List Row/Odd`).
    pub fn alternating(mut self, on: bool) -> Self {
        self.alternating = on;
        self
    }

    /// Shows or hides the header.
    pub fn header_visible(mut self, on: bool) -> Self {
        self.show_header = on;
        self
    }

    /// The columns.
    pub fn columns(&self) -> &[TableColumn] {
        &self.header.columns
    }

    /// Replaces the columns (the rows are made again).
    pub fn set_columns(&mut self, columns: Vec<TableColumn>) {
        self.header = Header::new(columns);
        self.dirty = true;
    }

    /// Adds a column (the rows are made again).
    pub fn add_column(&mut self, column: TableColumn) {
        self.header.columns.push(column);
        self.dirty = true;
    }

    /// The sorted column and whether ascending.
    pub fn sort(&self) -> Option<(usize, bool)> {
        self.header.sort
    }

    /// Number of rows (visible nodes in an outline).
    pub fn row_count(&self) -> usize {
        self.rows.count
    }

    /// The key of row `i`.
    pub fn key_of(&self, i: usize) -> Option<u64> {
        match &self.outline {
            Some(o) => o.flat.get(i).map(|f| f.node),
            None => (i < self.rows.count).then_some(i as u64),
        }
    }

    /// The row showing `key`.
    pub fn row_of(&self, key: u64) -> Option<usize> {
        match &self.outline {
            Some(o) => o.flat.iter().position(|f| f.node == key),
            None => usize::try_from(key).ok().filter(|i| *i < self.rows.count),
        }
    }

    /// The outline level of row `i` (0 in a table).
    pub fn level_of(&self, i: usize) -> u16 {
        self.outline
            .as_ref()
            .and_then(|o| o.flat.get(i))
            .map_or(0, |f| f.level)
    }

    /// The selected rows' keys.
    pub fn selection(&self) -> Vec<u64> {
        self.rows
            .selected
            .iter()
            .filter_map(|i| self.key_of(*i))
            .collect()
    }

    /// The cell view of (row, column) when its row is in sight.
    pub fn cell_view(&self, row: usize, column: usize) -> Option<ViewId> {
        self.live.get(&row)?.cells.get(column).copied().flatten()
    }

    /// Whether a cell is being edited.
    pub fn is_editing(&self) -> bool {
        self.editing.is_some()
    }

    /// Table mode: `rows` rows of `row_height` whose cells `provider` makes.
    pub fn set_cells(&mut self, rows: usize, row_height: f32, provider: CellProvider) {
        self.outline = None;
        if row_height.is_finite() {
            self.rows.height = row_height.max(1.0);
        }
        self.rows.set_count(rows);
        self.provider = Some(provider);
        self.dirty = true;
    }

    /// Outline mode: the rows are the visible nodes of `source`; `provider` makes the cells
    /// of a node.
    pub fn set_tree(
        &mut self,
        source: Box<dyn TreeSource>,
        row_height: f32,
        provider: CellProvider,
    ) {
        let expanded = self.outline.take().map(|o| o.expanded).unwrap_or_default();
        if row_height.is_finite() {
            self.rows.height = row_height.max(1.0);
        }
        self.outline = Some(Outline {
            source,
            flat: Vec::new(),
            expanded,
            stale: true,
        });
        self.provider = Some(provider);
        self.dirty = true;
    }

    /// The data changed: rows are made again (an outline reads its tree again, keeping the
    /// expanded nodes and the selected keys).
    pub fn reload(&mut self) {
        if let Some(o) = self.outline.as_mut() {
            o.stale = true;
        }
        self.dirty = true;
    }

    /// The expanded nodes of an outline.
    pub fn expanded(&self) -> Vec<u64> {
        let mut v: Vec<u64> = self
            .outline
            .as_ref()
            .map(|o| o.expanded.iter().copied().collect())
            .unwrap_or_default();
        v.sort_unstable();
        v
    }

    fn header_height(&self) -> f32 {
        if self.show_header {
            metrics::HEADER_HEIGHT
        } else {
            0.0
        }
    }

    /// The box (bordered look) in the content box.
    fn frame(&self, size: Size) -> Rect {
        Rect::from_size(size)
    }

    /// The header's rect.
    fn header_rect(&self, size: Size) -> Rect {
        let f = self.inner(size);
        Rect::new(f.x, f.y, f.width, self.header_height())
    }

    /// Inside the box's border.
    fn inner(&self, size: Size) -> Rect {
        match self.look {
            ListLook::Bordered => self.frame(size).inset(Insets::all(1.0)),
            _ => self.frame(size),
        }
    }

    /// The rows' viewport.
    fn viewport(&self, size: Size) -> Rect {
        let f = self.inner(size);
        let h = self.header_height();
        Rect::new(f.x, f.y + h, f.width, (f.height - h).max(0.0))
    }

    /// The left edge of the first column's content in row `i` (after the outline's
    /// indentation and chevron).
    fn content_indent(&self, i: usize) -> f32 {
        match &self.outline {
            Some(_) => self.level_of(i) as f32 * metrics::INDENT + metrics::DISCLOSURE,
            None => 0.0,
        }
    }

    /// The chevron's rect of row `i`, in viewport coordinates.
    fn chevron_rect(&self, i: usize, width: f32) -> Rect {
        let frames = column_frames(&self.header.columns, width);
        let x0 = frames.first().map_or(metrics::LEADING, |f| f.0);
        let x = x0 + self.level_of(i) as f32 * metrics::INDENT;
        let g = metrics::DISCLOSURE_GLYPH;
        Rect::new(
            x + (metrics::DISCLOSURE - g) / 2.0 - 2.0,
            self.rows.y(i) + (self.rows.height - g) / 2.0,
            g,
            g,
        )
    }

    fn rebuild_outline(&mut self, ui: &mut Ui) {
        let Some(o) = self.outline.as_mut() else {
            return;
        };
        if !o.stale {
            return;
        }
        let keys: Vec<u64> = self
            .rows
            .selected
            .iter()
            .filter_map(|i| o.flat.get(*i).map(|f| f.node))
            .collect();
        let mut flat = Vec::new();
        o.flatten(ui, None, 0, &mut flat);
        // Expanded nodes that are gone are forgotten.
        let present: HashSet<u64> = flat.iter().map(|f| f.node).collect();
        o.expanded.retain(|n| present.contains(n));
        let selected: Vec<i64> = flat
            .iter()
            .enumerate()
            .filter(|(_, f)| keys.contains(&f.node))
            .map(|(i, _)| i as i64)
            .collect();
        o.flat = flat;
        o.stale = false;
        self.rows.set_count(o.flat.len());
        self.rows.set_selection(&selected);
        self.dirty = true;
    }

    /// Expands or collapses row `i` of an outline (with its descendants when `deep`).
    /// Returns whether anything changed.
    fn set_row_expanded(&mut self, ui: &mut Ui, i: usize, expand: bool, deep: bool) -> bool {
        let Some(o) = self.outline.as_mut() else {
            return false;
        };
        let Some(f) = o.flat.get(i).copied() else {
            return false;
        };
        if !f.expandable || o.expanded.contains(&f.node) == expand && !deep {
            return false;
        }
        // Rows of the subtree that are shown now.
        let shown = o
            .flat
            .get(i + 1..)
            .unwrap_or_default()
            .iter()
            .take_while(|r| r.level > f.level)
            .count();
        if expand {
            o.expanded.insert(f.node);
            if deep {
                Self::expand_deep(o, ui, f.node, f.level);
            }
            let sub = o.subtree(ui, f.node, f.level);
            let n = sub.len() as isize - shown as isize;
            o.flat.splice(i + 1..i + 1 + shown, sub);
            self.rows.shift_after(i, n);
        } else {
            o.expanded.remove(&f.node);
            if deep {
                let gone: Vec<u64> = o
                    .flat
                    .get(i + 1..i + 1 + shown)
                    .unwrap_or_default()
                    .iter()
                    .map(|r| r.node)
                    .collect();
                for n in gone {
                    o.expanded.remove(&n);
                }
            }
            o.flat.drain(i + 1..i + 1 + shown);
            self.rows.shift_after(i, -(shown as isize));
        }
        self.rows.count = o.flat.len();
        self.rows.clamp();
        // Rows below moved: their views are made again for their new keys.
        let stale: Vec<usize> = self.live.keys().copied().filter(|r| *r > i).collect();
        for r in stale {
            if let Some(row) = self.live.remove(&r) {
                drop_view(ui, row.host);
            }
        }
        if self.editing.is_some_and(|e| e.row > i) {
            self.cancel_edit(ui);
        }
        true
    }

    fn expand_deep(o: &mut Outline, ui: &mut Ui, node: u64, level: u16) {
        if level > MAX_LEVEL {
            return;
        }
        let n = o.source.child_count(ui, Some(node));
        for k in 0..n {
            let c = o.source.child(ui, Some(node), k);
            if o.source.is_expandable(ui, c) {
                o.expanded.insert(c);
                Self::expand_deep(o, ui, c, level + 1);
            }
        }
    }

    fn sync_rows(&mut self, ui: &mut Ui, body: ViewId) {
        if self.dirty {
            if self.editing.is_some() {
                self.cancel_edit(ui);
            }
            for (_, row) in std::mem::take(&mut self.live) {
                drop_view(ui, row.host);
            }
            self.dirty = false;
        }
        let range = self.rows.visible();
        let gone: Vec<usize> = self
            .live
            .keys()
            .copied()
            .filter(|i| !range.contains(i))
            .collect();
        for i in gone {
            if self.editing.is_some_and(|e| e.row == i) {
                self.commit_edit(ui);
            }
            if let Some(row) = self.live.remove(&i) {
                drop_view(ui, row.host);
            }
        }
        let columns = self.header.columns.len();
        for i in range {
            if self.live.contains_key(&i) {
                continue;
            }
            let Some(key) = self.key_of(i) else {
                continue;
            };
            let host = ui.create(Body {
                clips: false,
                kind: "Row",
            });
            ui.add_child(body, host);
            ui.release(host);
            let mut cells = Vec::with_capacity(columns);
            if let Some(provider) = self.provider.as_mut() {
                for c in 0..columns {
                    let cell = provider(ui, key, c);
                    if let Some(v) = cell {
                        ui.add_child(host, v);
                        ui.release(v);
                    }
                    cells.push(cell);
                }
            }
            self.live.insert(i, LiveRow { host, key, cells });
        }
    }

    fn sync_emphasis(&self, ui: &mut Ui) {
        for (i, row) in &self.live {
            ui.set_emphasized(row.host, self.emphasis && self.rows.is_selected(*i));
        }
    }

    fn selection_changed(&mut self, cx: &mut EventCx<'_>) {
        cx.emit(Signal::SelectionChanged, self.selection_value());
        self.sync_emphasis(cx.ui());
        cx.request_paint();
    }

    fn selection_value(&self) -> Value {
        Value::IntList(self.selection().into_iter().map(|k| k as i64).collect())
    }

    fn set_selection_keys(&mut self, keys: &[i64]) {
        if self.outline.as_ref().is_some_and(|o| o.stale) {
            self.pending_keys = Some(keys.to_vec());
            return;
        }
        if self.outline.is_none() {
            // Keys are rows: the engine keeps rows that do not exist yet.
            self.rows.set_selection(keys);
            return;
        }
        let rows: Vec<i64> = keys
            .iter()
            .filter_map(|k| u64::try_from(*k).ok())
            .filter_map(|k| self.row_of(k))
            .map(|i| i as i64)
            .collect();
        self.rows.set_selection(&rows);
    }

    /// The first text found in a view's subtree (what an edit starts from).
    fn text_of(ui: &Ui, v: ViewId) -> String {
        for d in ui.tree.descendants(v) {
            if let Some(Value::Str(s)) = ui.get(d, Prop::Text) {
                return s;
            }
        }
        String::new()
    }

    /// Starts editing cell (row, column) when its column is editable.
    fn begin_edit(&mut self, ui: &mut Ui, table: ViewId, row: usize, column: usize) -> bool {
        if !self.header.columns.get(column).is_some_and(|c| c.editable) {
            return false;
        }
        self.me = Some(table);
        if self.editing.is_some() {
            self.commit_edit(ui);
        }
        let Some(live) = self.live.get(&row) else {
            // Made at the next layout, then edited.
            self.pending_edit = Some((row, column));
            self.rows.scroll_to(row);
            ui.request_layout(table);
            return true;
        };
        let (host, key) = (live.host, live.key);
        let cell = live.cells.get(column).copied().flatten();
        let text = cell.map(|c| Self::text_of(ui, c)).unwrap_or_default();
        let field = ui.create(TextField::new(FieldKind::Text));
        let _ = ui.set(field, Prop::Text, Value::Str(text));
        ui.add_child(host, field);
        ui.release(field);
        if let Some(c) = cell {
            let _ = ui.set(c, Prop::Hidden, Value::Bool(true));
        }
        for (signal, refocus) in [(Signal::Submitted, true), (Signal::EditingEnded, false)] {
            ui.connect(field, signal, move |ui, f, v| {
                let text = v.as_str().map(str::to_string);
                if let Some((e, text)) = ui
                    .with_widget::<TableView, _>(table, |t| t.end_from_field(f, text))
                    .flatten()
                {
                    finish_edit(ui, table, e, text, refocus);
                }
            });
        }
        self.editing = Some(Editing {
            row,
            column,
            key,
            field,
            cell,
        });
        ui.set_focus(field, true);
        ui.emit(
            table,
            Signal::EditingBegan,
            Value::Cell {
                row: key,
                column: column as u32,
                text: String::new(),
            },
        );
        ui.request_layout(table);
        true
    }

    /// The field `field` ended the edit with `text`: the edit is taken out to be finished.
    fn end_from_field(
        &mut self,
        field: ViewId,
        text: Option<String>,
    ) -> Option<(Editing, Option<String>)> {
        let e = self.editing.filter(|e| e.field == field)?;
        self.editing = None;
        Some((e, text))
    }

    /// Ends the edit keeping the field's text.
    fn commit_edit(&mut self, ui: &mut Ui) {
        let Some(e) = self.editing.take() else {
            return;
        };
        let text = match ui.get(e.field, Prop::Text) {
            Some(Value::Str(s)) => Some(s),
            _ => None,
        };
        remove_editor(ui, &e);
        if let (Some(t), Some(text)) = (self.me, text) {
            ui.emit(
                t,
                Signal::CellEdited,
                Value::Cell {
                    row: e.key,
                    column: e.column as u32,
                    text,
                },
            );
        }
    }

    /// Ends the edit dropping the field's text.
    fn cancel_edit(&mut self, ui: &mut Ui) {
        if let Some(e) = self.editing.take() {
            remove_editor(ui, &e);
        }
    }

    fn place_editor(&mut self, ui: &mut Ui, frames: &[(f32, f32)]) {
        let Some(e) = self.editing else { return };
        if !ui.tree.contains(e.field) {
            self.editing = None;
            return;
        }
        let Some((x, w)) = frames.get(e.column).copied() else {
            return;
        };
        let indent = if e.column == 0 {
            self.content_indent(e.row)
        } else {
            0.0
        };
        let x = x + indent - 4.0;
        let w = (w - indent + 4.0).max(1.0);
        let s = ui.measure(e.field, Proposal::new(Some(w), None));
        let h = s.height.min(self.rows.height);
        ui.place(e.field, Rect::new(x, (self.rows.height - h) / 2.0, w, h));
    }

    fn column_at(&self, x: f32, width: f32) -> Option<usize> {
        self.header.column_at(x, width)
    }

    fn current(&self) -> Option<usize> {
        self.rows.current()
    }

    fn expanded_changed(&self, cx: &mut EventCx<'_>, i: usize) {
        if let (Some(key), Some(o)) = (self.key_of(i), self.outline.as_ref()) {
            let open = o.expanded.contains(&key);
            cx.emit(
                Signal::ExpandedChanged,
                Value::IntList(vec![key as i64, i64::from(open)]),
            );
        }
    }
}

/// Removes an edit's field and shows its cell again.
fn remove_editor(ui: &mut Ui, e: &Editing) {
    if let Some(c) = e.cell {
        let _ = ui.set(c, Prop::Hidden, Value::Bool(false));
    }
    if ui.tree.contains(e.field) {
        drop_view(ui, e.field);
    }
}

/// Finishes an edit the field ended: the field goes, `CellEdited` is sent with the text
/// and, after Return, the table takes the focus back.
fn finish_edit(ui: &mut Ui, table: ViewId, e: Editing, text: Option<String>, refocus: bool) {
    let field_focused = ui
        .tree
        .get(e.field)
        .is_some_and(|n| n.flags.contains(NodeFlags::FOCUSED));
    remove_editor(ui, &e);
    if let Some(text) = text {
        ui.emit(
            table,
            Signal::CellEdited,
            Value::Cell {
                row: e.key,
                column: e.column as u32,
                text,
            },
        );
    }
    if refocus || field_focused {
        ui.set_focus(table, true);
    }
    ui.request_layout(table);
}

impl Widget for TableView {
    fn kind(&self) -> &'static str {
        if self.outline.is_some() {
            "Outline"
        } else {
            "Table"
        }
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        p.replacing_unspecified(Size::new(
            natural_width(&self.header.columns).max(240.0),
            (self.header_height() + self.rows.content()).min(320.0),
        ))
    }

    fn place(&mut self, cx: &mut LayoutCx<'_>, size: Size) {
        self.me = Some(cx.id());
        let body = match self.body.filter(|b| cx.ui.tree.contains(*b)) {
            Some(b) => b,
            None => {
                let b = make_body(cx.ui, cx.id());
                self.body = Some(b);
                b
            }
        };
        self.rebuild_outline(cx.ui);
        if let Some(keys) = self.pending_keys.take() {
            self.set_selection_keys(&keys);
        }
        let area = self.viewport(size);
        cx.place(body, area);
        self.rows.viewport = area.height;
        self.rows.pad = self.look.insets().1;
        self.rows.clamp();
        self.sync_rows(cx.ui, body);
        self.sync_emphasis(cx.ui);
        let frames = column_frames(&self.header.columns, area.width);
        let rh = self.rows.height;
        let entries: Vec<(usize, ViewId, Vec<Option<ViewId>>)> = self
            .live
            .iter()
            .map(|(i, r)| (*i, r.host, r.cells.clone()))
            .collect();
        for (i, host, cells) in entries {
            cx.ui
                .place(host, Rect::new(0.0, self.rows.y(i), area.width, rh));
            let indent = self.content_indent(i);
            for (c, cell) in cells.into_iter().enumerate() {
                let (Some(cell), Some((x, w))) = (cell, frames.get(c).copied()) else {
                    continue;
                };
                let (x, w) = if c == 0 {
                    (x + indent, (w - indent).max(0.0))
                } else {
                    (x, w)
                };
                let s = cx.ui.measure(cell, Proposal::new(Some(w), Some(rh)));
                let h = s.height.min(rh);
                cx.ui
                    .place(cell, Rect::new(x, ((rh - h) / 2.0).round(), w, h));
            }
        }
        if let Some((row, column)) = self.pending_edit
            && self.live.contains_key(&row)
        {
            self.pending_edit = None;
            let id = cx.id();
            self.begin_edit(cx.ui, id, row, column);
        }
        if self.editing.is_some_and(|e| self.live.contains_key(&e.row)) {
            self.place_editor(cx.ui, &frames);
        }
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let r = cx.bounds();
        let size = r.size();
        let theme = *cx.theme();
        let c = theme.colors;
        if self.look == ListLook::Bordered {
            paint_box(cx, self.frame(size).translate(r.origin()));
        }
        let area = self.viewport(size).translate(r.origin());
        cx.rec.push_clip(area, Shape::RECT);
        let range = self.rows.visible();
        if self.alternating {
            for i in range.clone().filter(|i| i % 2 == 1) {
                let y = area.y + self.rows.y(i);
                cx.rec.layer(
                    "Stripe",
                    Rect::new(area.x, y, area.width, self.rows.height),
                    Shape::RECT,
                    Style::fill(c.surface_sidebar),
                );
            }
        }
        let fill = selection_fill(cx, self.emphasis);
        let (inset, shape) = match self.look {
            ListLook::Plain => (0.0, Shape::RECT),
            _ => (
                metrics::SELECTION_INSET,
                Shape::rounded(metrics::SELECTION_RADIUS),
            ),
        };
        for i in &self.rows.selected {
            let y = self.rows.y(*i);
            if y + self.rows.height < 0.0 || y > area.height {
                continue;
            }
            cx.rec.layer(
                "Selection",
                Rect::new(
                    area.x + inset,
                    area.y + y,
                    (area.width - 2.0 * inset).max(0.0),
                    self.rows.height,
                ),
                shape.clone(),
                Style::fill(fill),
            );
        }
        if let Some(o) = &self.outline {
            for i in range {
                let Some(f) = o.flat.get(i) else { continue };
                if !f.expandable {
                    continue;
                }
                let open = o.expanded.contains(&f.node);
                let g = self.chevron_rect(i, area.width).translate(area.origin());
                let on = self.emphasis && self.rows.is_selected(i);
                let color = if on {
                    c.text_on_accent
                } else {
                    c.text_secondary
                };
                let centre = g.center();
                if open {
                    cx.rec.push_transform(
                        Transform::translate(-centre.x, -centre.y)
                            .then(Transform::rotate(std::f32::consts::FRAC_PI_2))
                            .then(Transform::translate(centre.x, centre.y)),
                    );
                }
                cx.rec.glyph(
                    if open {
                        "Disclosure Expanded"
                    } else {
                        "Disclosure"
                    },
                    g,
                    "chevron-right",
                    500,
                    color,
                );
                if open {
                    cx.rec.pop();
                }
            }
        }
        cx.rec.pop();
        if self.show_header {
            let h = self.header_rect(size).translate(r.origin());
            self.header.paint(cx, h);
        }
    }

    fn paint_over(&mut self, cx: &mut PaintCx<'_>) {
        let r = cx.bounds();
        let area = self.viewport(r.size()).translate(r.origin());
        paint_scroller(cx, area, &self.rows, self.hovered);
        if self.look == ListLook::Bordered {
            paint_box_border(cx, self.frame(r.size()).translate(r.origin()));
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        let b = cx.bounds();
        let size = b.size();
        let area = self.viewport(size);
        match event {
            Event::Scroll { dy, .. } => {
                if self.rows.scroll_by(*dy) {
                    cx.request_layout();
                    cx.request_paint();
                    cx.emit(
                        Signal::Scrolled,
                        Value::Point(Point::new(0.0, self.rows.scroll)),
                    );
                }
                Handled::Yes
            }
            Event::PointerDown {
                pos,
                button: Button::Left,
                clicks,
                mods,
            } => {
                let local = Point::new(pos.x - b.x, pos.y - b.y);
                let header = self.header_rect(size);
                if self.show_header && header.contains(local) {
                    match self.header.press(local.x - header.x, header.width) {
                        HeaderHit::Sorted => {
                            cx.emit(Signal::SortChanged, sort_value(self.header.sort));
                            cx.request_paint();
                        }
                        HeaderHit::Resizing => cx.capture_pointer(),
                        HeaderHit::None => {}
                    }
                    return Handled::Yes;
                }
                let (x, y) = (local.x - area.x, local.y - area.y);
                if let Some(i) = self.rows.at(y) {
                    let chevron = self.chevron_rect(i, area.width).inset(Insets::all(-4.0));
                    let expandable = self
                        .outline
                        .as_ref()
                        .and_then(|o| o.flat.get(i))
                        .is_some_and(|f| f.expandable);
                    if expandable && chevron.contains(Point::new(x, y)) {
                        let open = self.key_of(i).is_some_and(|k| self.expanded().contains(&k));
                        if self.set_row_expanded(cx.ui(), i, !open, mods.contains(Mods::ALT)) {
                            self.expanded_changed(cx, i);
                            cx.request_layout();
                            cx.request_paint();
                        }
                        return Handled::Yes;
                    }
                    if *clicks >= 2 {
                        let col = self.column_at(x, area.width);
                        let id = cx.id();
                        let edited = col.is_some_and(|c| self.begin_edit(cx.ui(), id, i, c));
                        if !edited && let Some(k) = self.key_of(i) {
                            cx.emit(Signal::DoubleClicked, Value::Int(k as i64));
                        }
                        return Handled::Yes;
                    }
                    if self.editing.is_some() {
                        self.commit_edit(cx.ui());
                    }
                    if self.rows.select(i, *mods) {
                        self.selection_changed(cx);
                    }
                } else if self.editing.is_some() {
                    self.commit_edit(cx.ui());
                }
                cx.focus_self();
                Handled::Yes
            }
            Event::PointerMove { pos, .. } => {
                let header = self.header_rect(size);
                let x = pos.x - b.x - header.x;
                if let Some((i, w)) = self.header.drag(x) {
                    cx.emit(
                        Signal::ColumnResized,
                        Value::IntList(vec![i as i64, w.round() as i64]),
                    );
                    cx.request_layout();
                    cx.request_paint();
                }
                let in_header =
                    self.show_header && pos.y - b.y < header.bottom() || self.header.is_resizing();
                if in_header {
                    cx.set_cursor(self.header.cursor(x, header.width));
                }
                Handled::No
            }
            Event::PointerUp { .. } => {
                if self.header.release() {
                    Handled::Yes
                } else {
                    Handled::No
                }
            }
            Event::Key(k) if k.pressed => {
                if self.editing.is_some() {
                    if k.key == Key::Escape {
                        self.cancel_edit(cx.ui());
                        cx.focus_self();
                        cx.request_layout();
                        return Handled::Yes;
                    }
                    return Handled::No;
                }
                match k.key {
                    Key::Enter => {
                        let Some(i) = self.current() else {
                            return Handled::No;
                        };
                        let col = self.header.columns.iter().position(|c| c.editable);
                        let id = cx.id();
                        let edited = col.is_some_and(|c| self.begin_edit(cx.ui(), id, i, c));
                        if !edited && let Some(key) = self.key_of(i) {
                            cx.emit(Signal::DoubleClicked, Value::Int(key as i64));
                        }
                        return Handled::Yes;
                    }
                    Key::Right | Key::Left if self.outline.is_some() => {
                        let Some(i) = self.current() else {
                            return Handled::No;
                        };
                        let deep = k.mods.contains(Mods::ALT);
                        let open = self
                            .key_of(i)
                            .is_some_and(|key| self.expanded().contains(&key));
                        let right = k.key == Key::Right;
                        if right && !open || !right && open {
                            if self.set_row_expanded(cx.ui(), i, right, deep) {
                                self.expanded_changed(cx, i);
                            }
                        } else {
                            // Into the first child, or out to the parent.
                            let level = self.level_of(i);
                            let target = if right {
                                (i + 1 < self.rows.count && self.level_of(i + 1) > level)
                                    .then_some(i + 1)
                            } else {
                                (0..i).rev().find(|r| self.level_of(*r) < level)
                            };
                            if let Some(t) = target {
                                if self.rows.select(t, Mods::empty()) {
                                    self.selection_changed(cx);
                                }
                                self.rows.scroll_to(t);
                            }
                        }
                        cx.request_layout();
                        cx.request_paint();
                        return Handled::Yes;
                    }
                    _ => {}
                }
                match self.rows.key(k) {
                    Some(changed) => {
                        if changed {
                            self.selection_changed(cx);
                        }
                        cx.request_layout();
                        Handled::Yes
                    }
                    None => Handled::No,
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

    fn state_changed(&mut self, cx: &mut EventCx<'_>, _old: NodeFlags, new: NodeFlags) {
        let e = emphasized(new.contains(NodeFlags::FOCUSED), cx.window_active());
        if e != self.emphasis {
            self.emphasis = e;
            self.sync_emphasis(cx.ui());
            cx.request_paint();
        }
    }

    fn focus(&self) -> FocusKind {
        FocusKind::Always
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match prop {
            Prop::RowCount if self.outline.is_none() => {
                match value.as_int().and_then(|n| usize::try_from(n).ok()) {
                    Some(n) => {
                        self.rows.set_count(n);
                        self.dirty = true;
                        PropChange::Layout
                    }
                    None => PropChange::Unknown,
                }
            }
            Prop::SelectedRows => match value.as_int_list() {
                Some(l) => {
                    self.set_selection_keys(&l);
                    PropChange::Layout
                }
                _ => PropChange::Unknown,
            },
            Prop::ScrollOffset => match value {
                Value::Point(p) => {
                    self.rows.scroll = p.y;
                    self.rows.clamp();
                    PropChange::Layout
                }
                _ => PropChange::Unknown,
            },
            Prop::Style => match value.as_int() {
                Some(s) => {
                    self.look = ListLook::from_style(s);
                    PropChange::Layout
                }
                None => PropChange::Unknown,
            },
            Prop::ColumnTitles => match value.as_str_list() {
                Some(l) => {
                    let mut cols: Vec<TableColumn> = l
                        .iter()
                        .enumerate()
                        .map(|(i, t)| {
                            self.header
                                .columns
                                .get(i)
                                .cloned()
                                .map(|mut c| {
                                    c.title = t.clone();
                                    c
                                })
                                .unwrap_or_else(|| TableColumn::new(t, 160.0))
                        })
                        .collect();
                    std::mem::swap(&mut self.header.columns, &mut cols);
                    self.dirty = true;
                    PropChange::Layout
                }
                None => PropChange::Unknown,
            },
            Prop::ColumnWidths => match value.as_int_list() {
                Some(l) => {
                    for (c, w) in self.header.columns.iter_mut().zip(l) {
                        c.width = (w as f32).clamp(c.min_width, c.max_width);
                    }
                    PropChange::Layout
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
            Prop::AlternatingRows => match value.as_bool() {
                Some(b) => {
                    self.alternating = b;
                    PropChange::Paint
                }
                None => PropChange::Unknown,
            },
            Prop::HeaderVisible => match value.as_bool() {
                Some(b) => {
                    self.show_header = b;
                    PropChange::Layout
                }
                None => PropChange::Unknown,
            },
            Prop::AllowsMultipleSelection => match value.as_bool() {
                Some(b) => {
                    self.rows.multiple = b;
                    PropChange::Paint
                }
                None => PropChange::Unknown,
            },
            Prop::ExpandedItems => match (value.as_int_list(), self.outline.as_mut()) {
                (Some(l), Some(o)) => {
                    o.expanded = l.iter().filter_map(|k| u64::try_from(*k).ok()).collect();
                    o.stale = true;
                    PropChange::Layout
                }
                _ => PropChange::Unknown,
            },
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::RowCount => Some(Value::from(self.rows.count)),
            Prop::SelectedRows => Some(self.selection_value()),
            Prop::ScrollOffset => Some(Value::Point(Point::new(0.0, self.rows.scroll))),
            Prop::ContentSize => Some(Value::Size(Size::new(
                natural_width(&self.header.columns),
                self.header_height() + self.rows.content(),
            ))),
            Prop::Style => Some(Value::Int(self.look.style())),
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
            Prop::AlternatingRows => Some(Value::Bool(self.alternating)),
            Prop::HeaderVisible => Some(Value::Bool(self.show_header)),
            Prop::AllowsMultipleSelection => Some(Value::Bool(self.rows.multiple)),
            Prop::ExpandedItems => self
                .outline
                .as_ref()
                .map(|_| Value::IntList(self.expanded().into_iter().map(|k| k as i64).collect())),
            _ => None,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Table;
        a.label = Some(
            self.header
                .columns
                .iter()
                .map(|c| c.title.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        );
        a.value = Some(format!("{} rows", self.rows.count));
        a.actions = vec![Action::Focus, Action::ScrollIntoView];
    }
}
