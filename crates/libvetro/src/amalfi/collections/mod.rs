//! Collections on one provider model: views are made by a provider when they come into
//! sight and dropped when they leave, so a collection of a million items costs what is on
//! screen.
//!
//! - [`ListView`]: a virtualised list of rows (source lists, plain lists, list boxes).
//! - [`TableView`]: columns with a header (sorting, resizing), cells made per row and
//!   column, in-place editing, and an outline mode (rows with disclosure levels over a
//!   [`TreeSource`]).
//! - [`GridView`]: tiles in rows that reflow with the width (icon views).
//!
//! They share selection (click, ⌘ toggles, ⇧ extends), the keyboard (arrows, Home/End,
//! Page Up/Down, ⌘A, Return), double click and the overlay scroller. Selected rows are
//! drawn on the accent with their content emphasized (`Text/On Accent`) while the
//! collection has the focus, and on `Line/Separator` without it (kit: `Apps/Mustache/List
//! Row`, `Column Row`, `Tile Selected`).

mod grid;
mod header;
mod table;

pub use grid::GridView;
pub use header::{Column, TableColumn, TableHeader};
pub use table::{CellProvider, TableView, TreeSource};

use std::collections::BTreeMap;

use crate::amalfi::scrollbars::{BarLook, ScrollbarStyle, paint_bar};
use crate::core::a11y::{A11y, Action, Role};
use crate::core::event::{Button, Event, Handled, Key, KeyEvent, Mods};
use crate::core::layout::{Axis, Proposal};
use crate::core::tree::{NodeFlags, ViewId};
use crate::core::ui::Ui;
use crate::core::value::{Prop, Signal, Value};
use crate::core::widget::{EventCx, FocusKind, LayoutCx, PaintCx, PropChange, Widget};
use crate::geom::{Insets, Point, Rect, Size};
use crate::shape::Shape;
use crate::style::{Border, Style};

/// Makes the view of a row (or of an item of a grid).
pub type RowProvider = Box<dyn FnMut(&mut Ui, usize) -> Option<ViewId>>;

/// Measures of the collections (`docs/design/amalfi/components.md` "Lists and tables",
/// the kit's `Apps/Mustache/List Row`, `List Header`, `Column Row` and `Tile` masters).
pub mod metrics {
    /// Regular row.
    pub const ROW_HEIGHT: f32 = 28.0;
    /// Compact row.
    pub const COMPACT_ROW_HEIGHT: f32 = 24.0;
    /// Two-line row.
    pub const TWO_LINE_ROW_HEIGHT: f32 = 44.0;
    /// The selection's inset from the row's sides.
    pub const SELECTION_INSET: f32 = 8.0;
    /// The selection's corner radius.
    pub const SELECTION_RADIUS: f32 = 8.0;
    /// Table header height.
    pub const HEADER_HEIGHT: f32 = 28.0;
    /// Header titles' top.
    pub const HEADER_TITLE_Y: f32 = 6.0;
    /// The first column's left edge in a table.
    pub const LEADING: f32 = 20.0;
    /// Space between columns.
    pub const COLUMN_SPACING: f32 = 12.0;
    /// Space after the last column.
    pub const TRAILING: f32 = 12.0;
    /// The narrowest a column gets by resizing.
    pub const MIN_COLUMN: f32 = 32.0;
    /// Half the width of the zone where a column edge is grabbed.
    pub const RESIZE_SLOP: f32 = 4.0;
    /// A list box's corner radius (as a group box).
    pub const BOX_RADIUS: f32 = 12.0;
    /// A list box's padding around the rows.
    pub const BOX_PADDING: f32 = 4.0;
    /// Indentation per outline level (token: nothing measured).
    pub const INDENT: f32 = 16.0;
    /// The disclosure chevron's slot in an outline row.
    pub const DISCLOSURE: f32 = 16.0;
    /// The disclosure chevron's glyph size.
    pub const DISCLOSURE_GLYPH: f32 = 12.0;
    /// Icon-view tile (`components.md` "Icon grids").
    pub const TILE: f32 = 104.0;
    /// The icon in a tile.
    pub const TILE_ICON: f32 = 64.0;
    /// Space between tiles.
    pub const TILE_GAP: f32 = 16.0;
    /// The label capsule's height under a selected tile's icon.
    pub const TILE_LABEL_HEIGHT: f32 = 20.0;
}

/// Rows made beyond each edge of the viewport (smooth scrolling).
const OVERSCAN: usize = 4;

/// How a list looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ListLook {
    /// A source list: inset rows with rounded selections on the window's background.
    #[default]
    Inset,
    /// Full-width rows with square selections.
    Plain,
    /// A list box: rows in a bordered, raised box (as a group box).
    Bordered,
}

impl ListLook {
    fn from_style(s: i64) -> ListLook {
        match s {
            1 => ListLook::Plain,
            2 => ListLook::Bordered,
            _ => ListLook::Inset,
        }
    }

    fn style(self) -> i64 {
        match self {
            ListLook::Inset => 0,
            ListLook::Plain => 1,
            ListLook::Bordered => 2,
        }
    }

    /// The rows' inset from the sides, and the padding above and below them.
    fn insets(self) -> (f32, f32) {
        match self {
            ListLook::Inset => (metrics::SELECTION_INSET, 0.0),
            ListLook::Plain => (0.0, 0.0),
            ListLook::Bordered => (metrics::BOX_PADDING, metrics::BOX_PADDING),
        }
    }
}

/// The scroll position, viewport and selection of a run of equal rows: the engine shared
/// by the collections.
#[derive(Debug, Clone, Default)]
pub(crate) struct Rows {
    /// Number of rows.
    pub count: usize,
    /// Height of every row.
    pub height: f32,
    /// Scroll offset of the content.
    pub scroll: f32,
    /// Height of the rows' viewport.
    pub viewport: f32,
    /// Padding above the first row and below the last.
    pub pad: f32,
    /// Selected rows, sorted.
    pub selected: Vec<usize>,
    /// Where ⇧ extends from.
    pub anchor: Option<usize>,
    /// The moving end of the selection (where the arrow keys go from).
    pub cursor: Option<usize>,
    /// Whether more than one row can be selected.
    pub multiple: bool,
    /// A selection set before its rows existed (applied when the rows come).
    pending: Option<Vec<i64>>,
}

impl Rows {
    pub fn new(count: usize, height: f32) -> Rows {
        Rows {
            count,
            height: height.max(1.0),
            multiple: true,
            ..Rows::default()
        }
    }

    pub fn content(&self) -> f32 {
        self.count as f32 * self.height + 2.0 * self.pad
    }

    pub fn max_scroll(&self) -> f32 {
        (self.content() - self.viewport).max(0.0)
    }

    pub fn clamp(&mut self) {
        let max = self.max_scroll();
        if !self.scroll.is_finite() {
            self.scroll = 0.0;
        }
        self.scroll = self.scroll.clamp(0.0, max);
    }

    /// Rows in sight, with overscan.
    pub fn visible(&self) -> std::ops::Range<usize> {
        if self.count == 0 {
            return 0..0;
        }
        let top = self.scroll - self.pad;
        let first = (top / self.height).floor().max(0.0) as usize;
        let last = ((top + self.viewport) / self.height).ceil().max(0.0) as usize;
        first.saturating_sub(OVERSCAN).min(self.count)..(last + OVERSCAN).min(self.count)
    }

    /// Top of row `i` in the viewport.
    pub fn y(&self, i: usize) -> f32 {
        self.pad + i as f32 * self.height - self.scroll
    }

    /// The row at `y` in the viewport.
    pub fn at(&self, y: f32) -> Option<usize> {
        let c = y + self.scroll - self.pad;
        if c < 0.0 {
            return None;
        }
        let i = (c / self.height).floor() as usize;
        (i < self.count).then_some(i)
    }

    /// The row the keyboard is on.
    pub fn current(&self) -> Option<usize> {
        self.cursor
            .or(self.anchor)
            .or_else(|| self.selected.last().copied())
    }

    pub fn is_selected(&self, i: usize) -> bool {
        self.selected.binary_search(&i).is_ok()
    }

    /// Selects row `i` as a click with `mods` would. Returns whether the selection changed.
    pub fn select(&mut self, i: usize, mods: Mods) -> bool {
        if i >= self.count {
            return false;
        }
        let before = self.selected.clone();
        if self.multiple && mods.contains(Mods::LOGO) {
            match self.selected.binary_search(&i) {
                Ok(p) => {
                    self.selected.remove(p);
                }
                Err(p) => self.selected.insert(p, i),
            }
            self.anchor = Some(i);
        } else if self.multiple && mods.contains(Mods::SHIFT) {
            let a = self.anchor.unwrap_or(i).min(self.count - 1);
            self.anchor = Some(a);
            self.selected = (a.min(i)..=a.max(i)).collect();
        } else {
            self.selected = vec![i];
            self.anchor = Some(i);
        }
        self.cursor = Some(i);
        self.selected != before
    }

    pub fn select_all(&mut self) -> bool {
        if !self.multiple || self.selected.len() == self.count {
            return false;
        }
        self.selected = (0..self.count).collect();
        true
    }

    /// Replaces the selection (rows beyond the count are selected when they come).
    pub fn set_selection(&mut self, rows: &[i64]) {
        let beyond = rows
            .iter()
            .any(|i| usize::try_from(*i).is_ok_and(|i| i >= self.count));
        self.pending = beyond.then(|| rows.to_vec());
        let mut v: Vec<usize> = rows
            .iter()
            .filter_map(|i| usize::try_from(*i).ok())
            .filter(|i| *i < self.count)
            .collect();
        v.sort_unstable();
        v.dedup();
        if !self.multiple {
            v.truncate(1);
        }
        self.anchor = v.last().copied();
        self.cursor = self.anchor;
        self.selected = v;
    }

    pub fn selection_value(&self) -> Value {
        Value::IntList(self.selected.iter().map(|x| *x as i64).collect())
    }

    /// Keeps the selection inside `count` rows.
    pub fn set_count(&mut self, count: usize) {
        self.count = count;
        if let Some(p) = self.pending.take() {
            self.set_selection(&p);
        }
        self.selected.retain(|i| *i < count);
        if self.anchor.is_some_and(|a| a >= count) {
            self.anchor = self.selected.last().copied();
        }
        if self.cursor.is_some_and(|a| a >= count) {
            self.cursor = self.anchor;
        }
        self.clamp();
    }

    /// Rows were inserted (`n > 0`) or removed (`n < 0`) after row `at`: the selection
    /// follows its rows.
    pub fn shift_after(&mut self, at: usize, n: isize) {
        let moved = |i: usize| -> Option<usize> {
            if i <= at {
                Some(i)
            } else if n >= 0 {
                Some(i + n.unsigned_abs())
            } else {
                let k = n.unsigned_abs();
                // Rows at + 1 ..= at + k were removed.
                if i <= at + k { None } else { Some(i - k) }
            }
        };
        self.selected = self.selected.iter().filter_map(|i| moved(*i)).collect();
        self.anchor = self.anchor.and_then(moved);
        self.cursor = self.cursor.and_then(moved).or(self.anchor);
        if n >= 0 {
            self.count += n.unsigned_abs();
        } else {
            self.count = self.count.saturating_sub(n.unsigned_abs());
        }
        self.clamp();
    }

    /// Scrolls so that row `i` is in sight.
    pub fn scroll_to(&mut self, i: usize) {
        let top = self.pad + i as f32 * self.height;
        if top < self.scroll {
            self.scroll = top;
        } else if top + self.height > self.scroll + self.viewport {
            self.scroll = top + self.height - self.viewport;
        }
        self.clamp();
    }

    /// The row a navigation key moves to.
    pub fn navigate(&self, key: &Key) -> Option<usize> {
        if self.count == 0 {
            return None;
        }
        let last = self.count - 1;
        let current = self.current();
        let page = ((self.viewport / self.height).floor() as usize).max(1);
        Some(match key {
            Key::Down => current.map_or(0, |c| (c + 1).min(last)),
            Key::Up => current.map_or(0, |c| c.saturating_sub(1)),
            Key::Home => 0,
            Key::End => last,
            Key::PageDown => current.map_or(0, |c| (c + page).min(last)),
            Key::PageUp => current.map_or(0, |c| c.saturating_sub(page)),
            _ => return None,
        })
    }

    /// Handles the selection keys; returns whether the selection changed, or `None` when
    /// the key is not one of them.
    pub fn key(&mut self, k: &KeyEvent) -> Option<bool> {
        if let Key::Char('a') = k.key
            && k.mods.contains(Mods::LOGO)
        {
            return Some(self.select_all());
        }
        let i = self.navigate(&k.key)?;
        let changed = self.select(i, k.mods & Mods::SHIFT);
        self.scroll_to(i);
        Some(changed)
    }

    /// Scrolls by `dy`; returns whether the offset changed.
    pub fn scroll_by(&mut self, dy: f32) -> bool {
        let before = self.scroll;
        self.scroll += dy;
        self.clamp();
        self.scroll != before
    }
}

/// The colour a selection is drawn with: the accent while the collection is emphasized
/// (it has the focus in an active window), `Line/Separator` otherwise (kit: `Column
/// Row/Dimmed`).
pub(crate) fn selection_fill(cx: &PaintCx<'_>, emphasis: bool) -> crate::color::Color {
    let c = cx.theme().colors;
    if emphasis {
        c.accent_amalfi
    } else {
        c.line_separator
    }
}

/// Whether selected rows are drawn emphasized.
pub(crate) fn emphasized(focused: bool, active: bool) -> bool {
    focused && active
}

/// Draws the overlay scroller of a vertical run of rows in `area` (paint coordinates).
pub(crate) fn paint_scroller(cx: &mut PaintCx<'_>, area: Rect, rows: &Rows, hovered: bool) {
    let content = rows.content();
    if !hovered || content <= rows.viewport || area.height <= 0.0 {
        return;
    }
    let style = ScrollbarStyle::Overlay;
    let t = style.thickness();
    let track = Rect::new(area.right() - t, area.y, t, area.height);
    let extent = (area.height * area.height / content)
        .max(24.0)
        .min(area.height);
    let offset = (area.height - extent) * (rows.scroll / rows.max_scroll().max(1.0));
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

/// A list box's background (a raised box with a 1 pt border, as a group box).
pub(crate) fn paint_box(cx: &mut PaintCx<'_>, rect: Rect) {
    let c = cx.theme().colors;
    cx.rec.layer(
        "Box",
        rect,
        Shape::rounded(metrics::BOX_RADIUS),
        Style::fill(c.surface_raised),
    );
}

/// A list box's border, drawn over the rows.
pub(crate) fn paint_box_border(cx: &mut PaintCx<'_>, rect: Rect) {
    let c = cx.theme().colors;
    cx.rec.layer(
        "Border",
        rect,
        Shape::rounded(metrics::BOX_RADIUS),
        Style::default().with_border(Border::inside(c.line_border, 1.0)),
    );
}

/// The clipping container the rows of a collection live in. Its children are placed by
/// the collection (it lays nothing out itself) and events pass through it.
#[derive(Debug, Default)]
pub(crate) struct Body {
    pub clips: bool,
    pub kind: &'static str,
}

impl Widget for Body {
    fn kind(&self) -> &'static str {
        if self.kind.is_empty() {
            "Rows"
        } else {
            self.kind
        }
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        p.replacing_unspecified(Size::ZERO)
    }

    fn place(&mut self, _cx: &mut LayoutCx<'_>, _size: Size) {}

    fn clips_children(&self) -> bool {
        self.clips
    }

    fn hit(&self, _p: Point, _size: Size) -> bool {
        false
    }

    fn dims_when_disabled(&self) -> bool {
        false
    }

    fn accessibility(&self, a: &mut A11y) {
        if self.kind == "Row" {
            a.role = Role::Row;
        }
    }
}

/// Makes the body of a collection (a clipping child of `owner`).
pub(crate) fn make_body(ui: &mut Ui, owner: ViewId) -> ViewId {
    let body = ui.create(Body {
        clips: true,
        kind: "Rows",
    });
    if let Some(n) = ui.tree.get_mut(body) {
        n.flags.insert(NodeFlags::CLIPS);
    }
    ui.add_child(owner, body);
    // The owner holds the body through its parent link only.
    ui.release(body);
    body
}

/// Drops a made view.
pub(crate) fn drop_view(ui: &mut Ui, v: ViewId) {
    ui.detach_now(v);
    ui.destroy(v);
}

/// A virtualised list.
pub struct ListView {
    rows: Rows,
    look: ListLook,
    provider: Option<RowProvider>,
    body: Option<ViewId>,
    live: BTreeMap<usize, ViewId>,
    /// Rows are made again (the data changed).
    dirty: bool,
    /// The pointer is over the list (the overlay scroller shows).
    hovered: bool,
    /// Selected rows are drawn emphasized.
    emphasis: bool,
}

impl std::fmt::Debug for ListView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ListView")
            .field("rows", &self.rows.count)
            .field("live", &self.live.len())
            .field("selected", &self.rows.selected)
            .finish()
    }
}

impl ListView {
    /// A list of `rows` rows of `row_height`, made by `provider`.
    pub fn new(rows: usize, row_height: f32, provider: RowProvider) -> Self {
        ListView {
            rows: Rows::new(rows, row_height),
            look: ListLook::Inset,
            provider: Some(provider),
            body: None,
            live: BTreeMap::new(),
            dirty: false,
            hovered: false,
            emphasis: false,
        }
    }

    /// The look.
    pub fn look(mut self, look: ListLook) -> Self {
        self.look = look;
        self
    }

    /// Number of rows.
    pub fn row_count(&self) -> usize {
        self.rows.count
    }

    /// The selected rows, sorted.
    pub fn selection(&self) -> &[usize] {
        &self.rows.selected
    }

    /// The view of row `i` when it is in sight.
    pub fn row_view(&self, i: usize) -> Option<ViewId> {
        self.live.get(&i).copied()
    }

    /// Replaces the rows: their number, height and provider (the rows are made again and the
    /// selection is kept where it still exists).
    pub fn reset(&mut self, rows: usize, row_height: f32, provider: RowProvider) {
        if row_height.is_finite() {
            self.rows.height = row_height.max(1.0);
        }
        self.rows.set_count(rows);
        self.set_provider(provider);
    }

    /// Replaces the provider (and remakes the rows).
    pub fn set_provider(&mut self, provider: RowProvider) {
        self.provider = Some(provider);
        self.dirty = true;
    }

    /// Makes the rows again (the data changed).
    pub fn reload(&mut self) {
        self.dirty = true;
    }

    /// Makes the visible rows and drops the others.
    fn sync_rows(&mut self, ui: &mut Ui, body: ViewId) {
        if self.dirty {
            for (_, v) in std::mem::take(&mut self.live) {
                drop_view(ui, v);
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
            if let Some(v) = self.live.remove(&i) {
                drop_view(ui, v);
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
                ui.add_child(body, v);
                // The list holds the row through its parent link only.
                ui.release(v);
                self.live.insert(i, v);
            }
        }
    }

    /// Marks the live selected rows emphasized (and the others not).
    fn sync_emphasis(&self, ui: &mut Ui) {
        for (i, v) in &self.live {
            ui.set_emphasized(*v, self.emphasis && self.rows.is_selected(*i));
        }
    }

    fn selection_changed(&mut self, cx: &mut EventCx<'_>) {
        cx.emit(Signal::SelectionChanged, self.rows.selection_value());
        self.sync_emphasis(cx.ui());
        cx.request_paint();
    }

    fn box_rect(&self, size: Size) -> Rect {
        Rect::from_size(size)
    }

    /// The rows' viewport in the list's content box.
    fn viewport(&self, size: Size) -> Rect {
        match self.look {
            ListLook::Bordered => Rect::from_size(size).inset(Insets::all(1.0)),
            _ => Rect::from_size(size),
        }
    }
}

impl Widget for ListView {
    fn kind(&self) -> &'static str {
        "List"
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        p.replacing_unspecified(Size::new(240.0, self.rows.content().min(320.0)))
    }

    fn place(&mut self, cx: &mut LayoutCx<'_>, size: Size) {
        let body = match self.body.filter(|b| cx.ui.tree.contains(*b)) {
            Some(b) => b,
            None => {
                let b = make_body(cx.ui, cx.id());
                self.body = Some(b);
                b
            }
        };
        let area = self.viewport(size);
        cx.place(body, area);
        self.rows.viewport = area.height;
        self.rows.pad = self.look.insets().1;
        self.rows.clamp();
        self.sync_rows(cx.ui, body);
        self.sync_emphasis(cx.ui);
        let inset = self.look.insets().0;
        let entries: Vec<(usize, ViewId)> = self.live.iter().map(|(i, v)| (*i, *v)).collect();
        for (i, v) in entries {
            cx.ui.place(
                v,
                Rect::new(
                    inset,
                    self.rows.y(i),
                    (area.width - 2.0 * inset).max(0.0),
                    self.rows.height,
                ),
            );
        }
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let r = cx.bounds();
        if self.look == ListLook::Bordered {
            paint_box(cx, self.box_rect(r.size()).translate(r.origin()));
        }
        let area = self.viewport(r.size()).translate(r.origin());
        let inset = self.look.insets().0;
        let color = selection_fill(cx, self.emphasis);
        let shape = if self.look == ListLook::Plain {
            Shape::RECT
        } else {
            Shape::rounded(metrics::SELECTION_RADIUS)
        };
        cx.rec.push_clip(area, Shape::RECT);
        for i in &self.rows.selected {
            let y = self.rows.y(*i);
            if y + self.rows.height < 0.0 || y > area.height {
                continue;
            }
            let rect = Rect::new(
                area.x + inset,
                area.y + y,
                (area.width - 2.0 * inset).max(0.0),
                self.rows.height,
            );
            cx.rec
                .layer("Selection", rect, shape.clone(), Style::fill(color));
        }
        cx.rec.pop();
    }

    fn paint_over(&mut self, cx: &mut PaintCx<'_>) {
        let r = cx.bounds();
        let area = self.viewport(r.size()).translate(r.origin());
        paint_scroller(cx, area, &self.rows, self.hovered);
        if self.look == ListLook::Bordered {
            paint_box_border(cx, self.box_rect(r.size()).translate(r.origin()));
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
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
                let b = cx.bounds();
                let area = self.viewport(b.size());
                let y = pos.y - b.y - area.y;
                if let Some(i) = self.rows.at(y) {
                    if *clicks >= 2 {
                        cx.emit(Signal::DoubleClicked, Value::from(i));
                    } else if self.rows.select(i, *mods) {
                        self.selection_changed(cx);
                    }
                }
                cx.focus_self();
                Handled::Yes
            }
            Event::Key(k) if k.pressed => {
                if k.key == Key::Enter {
                    if let Some(c) = self.rows.current() {
                        cx.emit(Signal::DoubleClicked, Value::from(c));
                    }
                    return Handled::Yes;
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
            Prop::RowCount => match value.as_int().and_then(|n| usize::try_from(n).ok()) {
                Some(n) => {
                    self.rows.set_count(n);
                    self.dirty = true;
                    PropChange::Layout
                }
                None => PropChange::Unknown,
            },
            Prop::SelectedRows => match value.as_int_list() {
                Some(l) => {
                    self.rows.set_selection(&l);
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
            Prop::AllowsMultipleSelection => match value.as_bool() {
                Some(b) => {
                    self.rows.multiple = b;
                    PropChange::Paint
                }
                None => PropChange::Unknown,
            },
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::RowCount => Some(Value::from(self.rows.count)),
            Prop::SelectedRows => Some(self.rows.selection_value()),
            Prop::ScrollOffset => Some(Value::Point(Point::new(0.0, self.rows.scroll))),
            Prop::ContentSize => Some(Value::Size(Size::new(0.0, self.rows.content()))),
            Prop::Style => Some(Value::Int(self.look.style())),
            Prop::AllowsMultipleSelection => Some(Value::Bool(self.rows.multiple)),
            _ => None,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::List;
        a.value = Some(format!("{} rows", self.rows.count));
        a.actions = vec![Action::Focus, Action::ScrollIntoView];
    }
}

#[cfg(test)]
mod tests;
