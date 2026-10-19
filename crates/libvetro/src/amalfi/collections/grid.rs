//! The grid view: tiles in rows that reflow with the width (icon views), made by a provider
//! when their row comes into sight. Selection as the kit's `Apps/Mustache/Tile Selected`:
//! a `Surface/Sunken` plate behind the icon and an accent capsule behind the label.

use std::collections::BTreeMap;

use super::{
    RowProvider, Rows, drop_view, emphasized, make_body, metrics, paint_scroller, selection_fill,
};
use crate::core::a11y::{A11y, Action, Role};
use crate::core::event::{Button, Event, Handled, Key, Mods};
use crate::core::layout::Proposal;
use crate::core::tree::{NodeFlags, ViewId};
use crate::core::ui::Ui;
use crate::core::value::{Prop, Signal, Value};
use crate::core::widget::{EventCx, FocusKind, LayoutCx, PaintCx, PropChange, Widget};
use crate::geom::{Point, Rect, Size};
use crate::shape::Shape;
use crate::style::Style;

/// A virtualised grid of tiles.
pub struct GridView {
    count: usize,
    tile: Size,
    /// The icon's height in a tile (the selection plate covers it).
    icon: f32,
    gap: f32,
    padding: f32,
    columns: usize,
    /// Scrolling by rows of tiles.
    lines: Rows,
    /// Selection of items.
    items: Rows,
    provider: Option<RowProvider>,
    body: Option<ViewId>,
    live: BTreeMap<usize, ViewId>,
    dirty: bool,
    hovered: bool,
    emphasis: bool,
}

impl std::fmt::Debug for GridView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GridView")
            .field("count", &self.count)
            .field("columns", &self.columns)
            .field("live", &self.live.len())
            .finish()
    }
}

impl GridView {
    /// A grid of `count` tiles of the kit's size made by `provider`.
    pub fn new(count: usize, provider: RowProvider) -> GridView {
        let tile = Size::new(metrics::TILE, metrics::TILE);
        GridView {
            count,
            tile,
            icon: metrics::TILE_ICON,
            gap: metrics::TILE_GAP,
            padding: metrics::TILE_GAP,
            columns: 1,
            lines: Rows::new(0, tile.height + metrics::TILE_GAP),
            items: Rows::new(count, 1.0),
            provider: Some(provider),
            body: None,
            live: BTreeMap::new(),
            dirty: false,
            hovered: false,
            emphasis: false,
        }
    }

    /// The tile size and the icon's height in it.
    pub fn tile(mut self, tile: Size, icon: f32) -> Self {
        self.set_tile(tile);
        self.icon = icon.clamp(0.0, self.tile.height);
        self
    }

    /// The padding around the tiles and the gap between them (the kit's: 16 and 16).
    pub fn spacing(mut self, padding: f32, gap: f32) -> Self {
        self.padding = padding.max(0.0);
        self.gap = gap.max(0.0);
        self.lines.height = self.tile.height + self.gap;
        self
    }

    fn set_tile(&mut self, tile: Size) {
        self.tile = Size::new(tile.width.max(1.0), tile.height.max(1.0));
        self.icon = self.icon.min(self.tile.height);
        self.lines.height = self.tile.height + self.gap;
        self.dirty = true;
    }

    /// Replaces the items: their number and provider.
    pub fn reset(&mut self, count: usize, provider: RowProvider) {
        self.count = count;
        self.items.set_count(count);
        self.provider = Some(provider);
        self.dirty = true;
    }

    /// Makes the tiles again (the data changed).
    pub fn reload(&mut self) {
        self.dirty = true;
    }

    /// Number of items.
    pub fn count(&self) -> usize {
        self.count
    }

    /// Tiles per row at the last layout.
    pub fn columns(&self) -> usize {
        self.columns
    }

    /// The selected items.
    pub fn selection(&self) -> &[usize] {
        &self.items.selected
    }

    /// The view of item `i` when it is in sight.
    pub fn item_view(&self, i: usize) -> Option<ViewId> {
        self.live.get(&i).copied()
    }

    fn columns_for(&self, width: f32) -> usize {
        let usable = width - 2.0 * self.padding + self.gap;
        ((usable / (self.tile.width + self.gap)).floor() as usize).max(1)
    }

    /// Item `i`'s tile in the viewport.
    fn tile_rect(&self, i: usize) -> Rect {
        let c = self.columns.max(1);
        let (row, col) = (i / c, i % c);
        Rect::new(
            self.padding + col as f32 * (self.tile.width + self.gap),
            self.lines.y(row),
            self.tile.width,
            self.tile.height,
        )
    }

    fn item_at(&self, p: Point) -> Option<usize> {
        let c = self.columns.max(1);
        let row = self.lines.at(p.y)?;
        let x = p.x - self.padding;
        if x < 0.0 {
            return None;
        }
        let col = (x / (self.tile.width + self.gap)).floor() as usize;
        let i = row * c + col;
        (col < c && i < self.count && self.tile_rect(i).contains(p)).then_some(i)
    }

    fn sync_items(&mut self, ui: &mut Ui, body: ViewId) {
        if self.dirty {
            for (_, v) in std::mem::take(&mut self.live) {
                drop_view(ui, v);
            }
            self.dirty = false;
        }
        let lines = self.lines.visible();
        let c = self.columns.max(1);
        let range = (lines.start * c).min(self.count)..(lines.end * c).min(self.count);
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
                ui.release(v);
                self.live.insert(i, v);
            }
        }
    }

    fn sync_emphasis(&self, ui: &mut Ui) {
        for (i, v) in &self.live {
            ui.set_emphasized(*v, self.emphasis && self.items.is_selected(*i));
        }
    }

    fn selection_changed(&mut self, cx: &mut EventCx<'_>) {
        cx.emit(Signal::SelectionChanged, self.items.selection_value());
        self.sync_emphasis(cx.ui());
        cx.request_paint();
    }

    /// The item an arrow key moves to.
    fn navigate(&self, key: &Key) -> Option<usize> {
        if self.count == 0 {
            return None;
        }
        let last = self.count - 1;
        let c = self.columns.max(1);
        let current = self.items.current();
        let Some(cur) = current else {
            return matches!(
                key,
                Key::Left | Key::Right | Key::Up | Key::Down | Key::Home | Key::End
            )
            .then_some(0);
        };
        Some(match key {
            Key::Right => (cur + 1).min(last),
            Key::Left => cur.saturating_sub(1),
            Key::Down => {
                if cur + c <= last {
                    cur + c
                } else {
                    cur
                }
            }
            Key::Up => cur.checked_sub(c).unwrap_or(cur),
            Key::Home => 0,
            Key::End => last,
            _ => return None,
        })
    }

    fn scroll_to_item(&mut self, i: usize) {
        self.lines.scroll_to(i / self.columns.max(1));
    }
}

impl Widget for GridView {
    fn kind(&self) -> &'static str {
        "Grid"
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        let w = 4.0 * (self.tile.width + self.gap) - self.gap + 2.0 * self.padding;
        let c = self
            .columns_for(p.width.filter(|w| w.is_finite()).unwrap_or(w))
            .max(1);
        let lines = self.count.div_ceil(c);
        let h = lines as f32 * (self.tile.height + self.gap) - self.gap + 2.0 * self.padding;
        p.replacing_unspecified(Size::new(w, h.clamp(0.0, 480.0)))
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
        cx.place(body, Rect::from_size(size));
        let columns = self.columns_for(size.width);
        if columns != self.columns {
            // The tiles reflow: every tile moves.
            self.columns = columns;
        }
        self.lines.set_count(self.count.div_ceil(columns));
        self.lines.viewport = size.height;
        self.lines.pad = self.padding;
        self.lines.clamp();
        self.sync_items(cx.ui, body);
        self.sync_emphasis(cx.ui);
        let entries: Vec<(usize, ViewId)> = self.live.iter().map(|(i, v)| (*i, *v)).collect();
        for (i, v) in entries {
            let r = self.tile_rect(i);
            cx.ui.place(v, r);
        }
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let b = cx.bounds();
        let c = cx.theme().colors;
        let capsule = selection_fill(cx, self.emphasis);
        let inset = metrics::SELECTION_INSET;
        for i in &self.items.selected {
            let t = self.tile_rect(*i).translate(b.origin());
            if t.bottom() < b.y || t.y > b.bottom() {
                continue;
            }
            cx.rec.layer(
                "Selection",
                Rect::new(t.x + inset, t.y, t.width - 2.0 * inset, self.icon + 4.0),
                Shape::rounded(metrics::SELECTION_RADIUS),
                Style::fill(c.surface_sunken),
            );
            cx.rec.layer(
                "Label capsule",
                Rect::new(
                    t.x + inset,
                    t.y + self.icon + 8.0,
                    t.width - 2.0 * inset,
                    metrics::TILE_LABEL_HEIGHT,
                ),
                Shape::rounded(metrics::TILE_LABEL_HEIGHT / 2.0),
                Style::fill(capsule),
            );
        }
    }

    fn paint_over(&mut self, cx: &mut PaintCx<'_>) {
        let b = cx.bounds();
        paint_scroller(cx, b, &self.lines, self.hovered);
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        let b = cx.bounds();
        match event {
            Event::Scroll { dy, .. } => {
                if self.lines.scroll_by(*dy) {
                    cx.request_layout();
                    cx.request_paint();
                    cx.emit(
                        Signal::Scrolled,
                        Value::Point(Point::new(0.0, self.lines.scroll)),
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
                let p = Point::new(pos.x - b.x, pos.y - b.y);
                match self.item_at(p) {
                    Some(i) if *clicks >= 2 => {
                        cx.emit(Signal::DoubleClicked, Value::from(i));
                    }
                    Some(i) => {
                        if self.items.select(i, *mods) {
                            self.selection_changed(cx);
                        }
                    }
                    None => {
                        // A click between tiles clears the selection.
                        if !mods.intersects(Mods::LOGO | Mods::SHIFT)
                            && !self.items.selected.is_empty()
                        {
                            self.items.selected.clear();
                            self.items.anchor = None;
                            self.items.cursor = None;
                            self.selection_changed(cx);
                        }
                    }
                }
                cx.focus_self();
                Handled::Yes
            }
            Event::Key(k) if k.pressed => {
                if k.key == Key::Enter {
                    if let Some(c) = self.items.current() {
                        cx.emit(Signal::DoubleClicked, Value::from(c));
                    }
                    return Handled::Yes;
                }
                if let Key::Char('a') = k.key
                    && k.mods.contains(Mods::LOGO)
                {
                    if self.items.select_all() {
                        self.selection_changed(cx);
                    }
                    return Handled::Yes;
                }
                let Some(i) = self.navigate(&k.key) else {
                    return Handled::No;
                };
                if self.items.select(i, k.mods & Mods::SHIFT) {
                    self.selection_changed(cx);
                }
                self.scroll_to_item(i);
                cx.request_layout();
                Handled::Yes
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
                    self.count = n;
                    self.items.set_count(n);
                    self.dirty = true;
                    PropChange::Layout
                }
                None => PropChange::Unknown,
            },
            Prop::SelectedRows => match value.as_int_list() {
                Some(l) => {
                    self.items.set_selection(&l);
                    PropChange::Layout
                }
                _ => PropChange::Unknown,
            },
            Prop::ScrollOffset => match value {
                Value::Point(p) => {
                    self.lines.scroll = p.y;
                    self.lines.clamp();
                    PropChange::Layout
                }
                _ => PropChange::Unknown,
            },
            Prop::TileSize => match value.as_size() {
                Some(s) => {
                    self.set_tile(s);
                    PropChange::Layout
                }
                None => PropChange::Unknown,
            },
            Prop::AllowsMultipleSelection => match value.as_bool() {
                Some(b) => {
                    self.items.multiple = b;
                    PropChange::Paint
                }
                None => PropChange::Unknown,
            },
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::RowCount => Some(Value::from(self.count)),
            Prop::SelectedRows => Some(self.items.selection_value()),
            Prop::ScrollOffset => Some(Value::Point(Point::new(0.0, self.lines.scroll))),
            Prop::TileSize => Some(Value::Size(self.tile)),
            Prop::Columns => Some(Value::from(self.columns)),
            Prop::AllowsMultipleSelection => Some(Value::Bool(self.items.multiple)),
            _ => None,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::List;
        a.value = Some(format!("{} items", self.count));
        a.actions = vec![Action::Focus, Action::ScrollIntoView];
    }
}
