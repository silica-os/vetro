//! Tables, outlines and grids through the C ABI: columns, cell providers, tree sources.

use std::ffi::{c_char, c_void};
use std::rc::Rc;

use vetro_sys::*;

use super::{UserData, enter, guard, req_str, set_last, view_id, with_ui};
use crate::amalfi::collections::{
    CellProvider, GridView, ListView, TableColumn, TableView, TreeSource,
};
use crate::core::tree::ViewId;
use crate::core::ui::Ui;
use crate::geom::Size;
use crate::text::Align;

fn finish(r: Result<(), VetroStatus>) -> VetroStatus {
    let s = r.err().unwrap_or(VETRO_OK);
    set_last(s);
    s
}

fn with_table<R>(
    view: *mut VetroView,
    f: impl FnOnce(&mut TableView) -> R,
) -> Result<R, VetroStatus> {
    let v = view_id(view)?;
    with_ui(move |ui| {
        ui.with_widget::<TableView, _>(v, f)
            .ok_or(VETRO_ERROR_INVALID_ARGUMENT)
    })?
}

/// A view a provider returned (a new reference), checked.
fn made(ui: &Ui, p: *mut VetroView) -> Option<ViewId> {
    if p.is_null() {
        None
    } else {
        view_id(p).ok().filter(|id| ui.tree.contains(*id))
    }
}

fn cell_provider(
    f: unsafe extern "C" fn(u64, u32, *mut c_void) -> *mut VetroView,
    data: Rc<UserData>,
) -> CellProvider {
    Box::new(move |ui, key, column| {
        let user = data.user;
        let column = u32::try_from(column).unwrap_or(u32::MAX);
        // SAFETY: the caller's provider with its own data; it returns a new reference.
        let p = enter(ui, || unsafe { f(key, column, user) });
        made(ui, p)
    })
}

/// Adds a column to a table or an outline.
#[unsafe(no_mangle)]
pub extern "C" fn vetro_table_add_column(
    view: *mut VetroView,
    title: *const c_char,
    width: f32,
    min_width: f32,
    max_width: f32,
    flags: u32,
) -> VetroStatus {
    guard(VETRO_ERROR_PLATFORM, || {
        finish((|| {
            let title = req_str(title)?;
            if !width.is_finite() || width <= 0.0 {
                return Err(VETRO_ERROR_INVALID_ARGUMENT);
            }
            let mut c = TableColumn::new(title, width)
                .editable(flags & VETRO_COLUMN_EDITABLE != 0)
                .sortable(flags & VETRO_COLUMN_NOT_SORTABLE == 0);
            let min = if min_width.is_finite() && min_width > 0.0 {
                min_width
            } else {
                c.min_width.min(width)
            };
            let max = if max_width.is_nan() || max_width <= 0.0 {
                f32::INFINITY
            } else {
                max_width
            };
            c = c.limits(min, max);
            c.resizable = flags & VETRO_COLUMN_FIXED_WIDTH == 0;
            c.align = if flags & VETRO_COLUMN_ALIGN_TRAILING != 0 {
                Align::End
            } else if flags & VETRO_COLUMN_ALIGN_CENTER != 0 {
                Align::Center
            } else {
                Align::Start
            };
            with_table(view, move |t| t.add_column(c))
        })())
    })
}

/// Removes every column of a table or an outline.
#[unsafe(no_mangle)]
pub extern "C" fn vetro_table_clear_columns(view: *mut VetroView) -> VetroStatus {
    guard(VETRO_ERROR_PLATFORM, || {
        finish(with_table(view, |t| t.set_columns(Vec::new())))
    })
}

/// Table mode: `rows` rows whose cells `provider` makes (called with the row index).
#[unsafe(no_mangle)]
pub extern "C" fn vetro_table_set_cells(
    view: *mut VetroView,
    rows: u64,
    row_height: f32,
    provider: VetroCellFn,
    user: *mut c_void,
    destroy: VetroDestroy,
) -> VetroStatus {
    guard(VETRO_ERROR_PLATFORM, || {
        let data = Rc::new(UserData::new(user, destroy));
        finish((|| {
            let f = provider.ok_or(VETRO_ERROR_INVALID_ARGUMENT)?;
            let rows = usize::try_from(rows).map_err(|_| VETRO_ERROR_INVALID_ARGUMENT)?;
            let make = cell_provider(f, data);
            with_table(view, move |t| t.set_cells(rows, row_height, make))
        })())
    })
}

/// A tree source over the C callbacks.
struct CTree {
    count: unsafe extern "C" fn(u64, *mut c_void) -> u64,
    child: unsafe extern "C" fn(u64, u64, *mut c_void) -> u64,
    expandable: unsafe extern "C" fn(u64, *mut c_void) -> u8,
    data: Rc<UserData>,
}

/// Children followed per node (a guard against a broken source).
const MAX_CHILDREN: u64 = 1 << 24;

impl TreeSource for CTree {
    fn child_count(&mut self, ui: &mut Ui, parent: Option<u64>) -> usize {
        let (f, user) = (self.count, self.data.user);
        let p = parent.unwrap_or(VETRO_OUTLINE_ROOT);
        // SAFETY: the caller's callback with its own data.
        let n = enter(ui, || unsafe { f(p, user) });
        usize::try_from(n.min(MAX_CHILDREN)).unwrap_or(0)
    }

    fn child(&mut self, ui: &mut Ui, parent: Option<u64>, index: usize) -> u64 {
        let (f, user) = (self.child, self.data.user);
        let p = parent.unwrap_or(VETRO_OUTLINE_ROOT);
        // SAFETY: as above.
        enter(ui, || unsafe { f(p, index as u64, user) })
    }

    fn is_expandable(&mut self, ui: &mut Ui, node: u64) -> bool {
        let (f, user) = (self.expandable, self.data.user);
        // SAFETY: as above.
        enter(ui, || unsafe { f(node, user) }) != 0
    }
}

/// Outline mode: the rows are the visible nodes of the tree the callbacks describe; the
/// cells of a node are made by `provider` (called with the node id).
#[unsafe(no_mangle)]
pub extern "C" fn vetro_outline_set_tree(
    view: *mut VetroView,
    row_height: f32,
    count: VetroTreeCountFn,
    child: VetroTreeChildFn,
    expandable: VetroTreeExpandableFn,
    provider: VetroCellFn,
    user: *mut c_void,
    destroy: VetroDestroy,
) -> VetroStatus {
    guard(VETRO_ERROR_PLATFORM, || {
        let data = Rc::new(UserData::new(user, destroy));
        finish((|| {
            let tree = CTree {
                count: count.ok_or(VETRO_ERROR_INVALID_ARGUMENT)?,
                child: child.ok_or(VETRO_ERROR_INVALID_ARGUMENT)?,
                expandable: expandable.ok_or(VETRO_ERROR_INVALID_ARGUMENT)?,
                data: data.clone(),
            };
            let make = cell_provider(provider.ok_or(VETRO_ERROR_INVALID_ARGUMENT)?, data);
            with_table(view, move |t| t.set_tree(Box::new(tree), row_height, make))
        })())
    })
}

/// Grid: `count` tiles of `tile_width` × `tile_height` (0: the kit's 104) made by
/// `provider`.
#[unsafe(no_mangle)]
pub extern "C" fn vetro_grid_set_items(
    view: *mut VetroView,
    count: u64,
    tile_width: f32,
    tile_height: f32,
    provider: VetroRowFn,
    user: *mut c_void,
    destroy: VetroDestroy,
) -> VetroStatus {
    guard(VETRO_ERROR_PLATFORM, || {
        let data = UserData::new(user, destroy);
        finish((|| {
            let v = view_id(view)?;
            let f = provider.ok_or(VETRO_ERROR_INVALID_ARGUMENT)?;
            let count = usize::try_from(count).map_err(|_| VETRO_ERROR_INVALID_ARGUMENT)?;
            let make: crate::amalfi::collections::RowProvider = Box::new(move |ui, i| {
                let d = &data;
                let user = d.user;
                // SAFETY: the caller's provider with its own data; it returns a new reference.
                let p = enter(ui, || unsafe { f(i as u64, user) });
                made(ui, p)
            });
            let tile = |v: f32| {
                if v.is_finite() && v > 0.0 {
                    v
                } else {
                    crate::amalfi::collections::metrics::TILE
                }
            };
            let size = Size::new(tile(tile_width), tile(tile_height));
            with_ui(move |ui| {
                ui.with_widget::<GridView, _>(v, move |g| {
                    g.reset(count, make);
                    let _ = crate::core::widget::Widget::set_prop(
                        g,
                        crate::core::value::Prop::TileSize,
                        &crate::core::value::Value::Size(size),
                    );
                })
                .ok_or(VETRO_ERROR_INVALID_ARGUMENT)
            })?
        })())
    })
}

/// Makes the rows, cells or tiles of a list, table, outline or grid again (the data
/// changed; an outline reads its tree again).
#[unsafe(no_mangle)]
pub extern "C" fn vetro_collection_reload(view: *mut VetroView) -> VetroStatus {
    guard(VETRO_ERROR_PLATFORM, || {
        finish((|| {
            let v = view_id(view)?;
            with_ui(move |ui| {
                let done = ui.with_widget::<ListView, _>(v, ListView::reload).is_some()
                    || ui
                        .with_widget::<TableView, _>(v, TableView::reload)
                        .is_some()
                    || ui.with_widget::<GridView, _>(v, GridView::reload).is_some();
                if done {
                    Ok(())
                } else {
                    Err(VETRO_ERROR_INVALID_ARGUMENT)
                }
            })?
        })())
    })
}
