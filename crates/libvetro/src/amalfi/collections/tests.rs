use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::*;
use crate::amalfi::label::Label as Text;
use crate::core::event::KeyEvent;
use crate::core::window::{WindowId, WindowKind};

fn fill(ui: &mut Ui, v: ViewId) {
    ui.set(v, Prop::MaxWidth, Value::from(f32::INFINITY))
        .unwrap();
    ui.set(v, Prop::MaxHeight, Value::from(f32::INFINITY))
        .unwrap();
}

fn window(ui: &mut Ui, size: Size, content: ViewId) -> WindowId {
    let w = ui.create_window(WindowKind::Normal, size);
    fill(ui, content);
    ui.set_content(w, content);
    ui.show_window(w);
    ui.prepare_frame(w);
    w
}

fn rows_of(ui: &Ui, view: ViewId) -> Vec<ViewId> {
    let body = ui.tree.children(view).first().copied().unwrap();
    ui.tree.children(body).to_vec()
}

fn text(ui: &Ui, v: ViewId) -> String {
    for d in ui.tree.descendants(v) {
        if let Some(Value::Str(s)) = ui.get(d, Prop::Text) {
            return s;
        }
    }
    String::new()
}

fn scroll_y(ui: &Ui, v: ViewId) -> f32 {
    match ui.get(v, Prop::ScrollOffset) {
        Some(Value::Point(p)) => p.y,
        _ => f32::NAN,
    }
}

fn click(ui: &mut Ui, w: WindowId, p: Point, mods: Mods) {
    ui.pointer_down(w, p, Button::Left, mods);
    ui.pointer_up(w, p, Button::Left, mods);
}

fn key(ui: &mut Ui, w: WindowId, k: Key, mods: Mods) {
    ui.key(w, KeyEvent::press(k, mods));
    ui.flush();
}

#[test]
fn a_million_rows_cost_what_is_shown() {
    let mut ui = Ui::new_headless();
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
    let w = window(&mut ui, Size::new(300.0, 280.0), list);
    let live = rows_of(&ui, list).len();
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
    assert!(rows_of(&ui, list).len() < 30);
    assert!(made.get() < 80);
    let first = rows_of(&ui, list).first().copied().unwrap();
    assert!(text(&ui, first).starts_with("Row 49999"));
    // Keyboard selection.
    ui.set_focus(list, false);
    ui.key(w, KeyEvent::press(Key::Down, Mods::empty()));
    ui.key(w, KeyEvent::press(Key::Down, Mods::SHIFT));
    let sel = ui.get(list, Prop::SelectedRows).unwrap();
    assert!(matches!(sel, Value::IntList(ref l) if l.len() == 2));
}

#[test]
fn a_list_fills_its_frame_and_its_rows_span_it() {
    let mut ui = Ui::new_headless();
    let list = ui.create(
        ListView::new(
            10_000,
            24.0,
            Box::new(|ui, i| Some(ui.create(Text::new(&format!("Row {}", i + 1))))),
        )
        .look(ListLook::Bordered),
    );
    let w = window(&mut ui, Size::new(400.0, 200.0), list);
    let frame = ui.tree.window_rect(list);
    assert_eq!(frame.size(), Size::new(400.0, 200.0));
    let rows = rows_of(&ui, list);
    assert!(rows.len() >= 8, "{} rows", rows.len());
    let r0 = ui.tree.window_rect(rows[0]);
    assert_eq!(r0.x, frame.x + 1.0 + metrics::BOX_PADDING);
    assert_eq!(r0.width, 400.0 - 2.0 - 2.0 * metrics::BOX_PADDING);
    assert_eq!(r0.y, frame.y + 1.0 + metrics::BOX_PADDING);
    let names: Vec<String> = ui
        .inspect_layers(list)
        .into_iter()
        .map(|l| l.name)
        .collect();
    assert!(names.iter().any(|n| n == "Box"), "{names:?}");
    assert!(names.iter().any(|n| n == "Border"), "{names:?}");
    // The wheel scrolls it.
    ui.scroll(
        w,
        Point::new(frame.x + 50.0, frame.y + 50.0),
        0.0,
        240.0,
        true,
        crate::core::event::Phase::Changed,
    );
    ui.prepare_frame(w);
    assert_eq!(scroll_y(&ui, list), 240.0);
    let mut shown: Vec<(f32, String)> = rows_of(&ui, list)
        .into_iter()
        .map(|r| (ui.tree.window_rect(r).y, text(&ui, r)))
        .filter(|(y, _)| *y >= frame.y)
        .collect();
    shown.sort_by(|a, b| a.0.total_cmp(&b.0));
    assert_eq!(shown[0].1, "Row 11");
}

#[test]
fn selected_rows_are_emphasized_while_focused() {
    let mut ui = Ui::new_headless();
    let list = ui.create(ListView::new(
        20,
        28.0,
        Box::new(|ui, i| Some(ui.create(Text::new(&format!("Row {i}"))))),
    ));
    let w = window(&mut ui, Size::new(300.0, 200.0), list);
    ui.set_window_active(w, true);
    let r = ui.tree.window_rect(list);
    click(
        &mut ui,
        w,
        Point::new(r.x + 50.0, r.y + 28.0 + 10.0),
        Mods::empty(),
    );
    ui.prepare_frame(w);
    let row1 = rows_of(&ui, list)
        .into_iter()
        .find(|v| text(&ui, *v) == "Row 1")
        .unwrap();
    assert!(ui.tree.flags(row1).contains(NodeFlags::EMPHASIZED));
    assert!(ui.theme_of(row1).emphasized);
    assert_eq!(
        ui.theme_of(row1).colors.text_primary,
        ui.theme_of(list).colors.text_on_accent
    );
    // In an inactive window the selection dims and the row is drawn as usual.
    ui.set_window_active(w, false);
    ui.prepare_frame(w);
    assert!(!ui.tree.flags(row1).contains(NodeFlags::EMPHASIZED));
}

#[test]
fn header_sorts_and_resizes() {
    let mut ui = Ui::new_headless();
    let h = ui.create(TableHeader::new(vec![
        TableColumn::new("Name", 200.0),
        TableColumn::new("Size", 100.0),
    ]));
    let w = ui.create_window(WindowKind::Normal, Size::new(400.0, 100.0));
    ui.set(h, Prop::MaxWidth, Value::from(f32::INFINITY))
        .unwrap();
    ui.set_content(w, h);
    ui.show_window(w);
    ui.prepare_frame(w);
    let got = Rc::new(Cell::new(99i64));
    let g = got.clone();
    ui.connect(h, Signal::SortChanged, move |_, _, v| {
        g.set(v.as_int().unwrap_or(0))
    });
    let r = ui.tree.window_rect(h);
    // Size starts at 20 + 200 + 12.
    let size_x = r.x + 260.0;
    click(&mut ui, w, Point::new(size_x, r.y + 10.0), Mods::empty());
    ui.flush();
    assert_eq!(got.get(), 1);
    click(&mut ui, w, Point::new(size_x, r.y + 10.0), Mods::empty());
    ui.flush();
    assert_eq!(got.get(), -2, "descending");
    // Dragging Name's edge (at 20 + 200 + 6) resizes it.
    let edge = Point::new(r.x + 226.0, r.y + 10.0);
    ui.pointer_down(w, edge, Button::Left, Mods::empty());
    ui.pointer_move(w, Point::new(edge.x + 40.0, edge.y), Mods::empty());
    ui.pointer_up(
        w,
        Point::new(edge.x + 40.0, edge.y),
        Button::Left,
        Mods::empty(),
    );
    assert_eq!(
        ui.get(h, Prop::ColumnWidths),
        Some(Value::IntList(vec![240, 100]))
    );
    assert_eq!(ui.get(h, Prop::SortDescriptor), Some(Value::Int(-2)));
}

/// Cells: text labels "r{row} c{column}".
fn table(ui: &mut Ui, rows: usize, editable: bool) -> ViewId {
    let mut t = TableView::new(vec![
        TableColumn::new("Name", 200.0).editable(editable),
        TableColumn::new("Size", 80.0),
        TableColumn::new("Kind", 100.0),
    ])
    .alternating(true);
    t.set_cells(
        rows,
        metrics::ROW_HEIGHT,
        Box::new(|ui, r, c| Some(ui.create(Text::new(&format!("r{r} c{c}"))))),
    );
    ui.create(t)
}

#[test]
fn a_table_places_cells_in_columns() {
    let mut ui = Ui::new_headless();
    let t = table(&mut ui, 100_000, false);
    let w = window(&mut ui, Size::new(500.0, 300.0), t);
    let f = ui.tree.window_rect(t);
    let rows = rows_of(&ui, t);
    assert!(rows.len() < 20, "{} rows", rows.len());
    let r0 = rows[0];
    let rr = ui.tree.window_rect(r0);
    assert_eq!(rr.y, f.y + metrics::HEADER_HEIGHT);
    assert_eq!(rr.width, 500.0);
    let cells = ui.tree.children(r0).to_vec();
    assert_eq!(cells.len(), 3);
    let xs: Vec<f32> = cells
        .iter()
        .map(|c| ui.tree.window_rect(*c).x - f.x)
        .collect();
    assert_eq!(xs, vec![20.0, 232.0, 324.0]);
    // The last column takes the rest.
    assert_eq!(ui.tree.window_rect(cells[2]).width, 500.0 - 324.0 - 12.0);
    assert_eq!(text(&ui, cells[1]), "r0 c1");
    let layers = ui.inspect_layers(t);
    assert!(layers.iter().any(|l| l.name == "Stripe"));
    assert_eq!(layers.iter().filter(|l| l.name == "Title").count(), 3);
    // Selection by keyboard, ⇧ extends.
    ui.set_focus(t, true);
    key(&mut ui, w, Key::Down, Mods::empty());
    key(&mut ui, w, Key::Down, Mods::SHIFT);
    key(&mut ui, w, Key::Down, Mods::SHIFT);
    assert_eq!(
        ui.get(t, Prop::SelectedRows),
        Some(Value::IntList(vec![0, 1, 2]))
    );
    key(&mut ui, w, Key::End, Mods::empty());
    ui.prepare_frame(w);
    assert_eq!(
        ui.get(t, Prop::SelectedRows),
        Some(Value::IntList(vec![99_999]))
    );
    let last = rows_of(&ui, t)
        .into_iter()
        .map(|r| text(&ui, r))
        .any(|s| s == "r99999 c0");
    assert!(last, "the last row is in sight");
}

#[test]
fn a_table_sorts_by_its_header() {
    let mut ui = Ui::new_headless();
    let t = table(&mut ui, 10, false);
    let w = window(&mut ui, Size::new(500.0, 300.0), t);
    let f = ui.tree.window_rect(t);
    let got = Rc::new(Cell::new(99i64));
    let g = got.clone();
    ui.connect(t, Signal::SortChanged, move |_, _, v| {
        g.set(v.as_int().unwrap_or(99))
    });
    click(
        &mut ui,
        w,
        Point::new(f.x + 250.0, f.y + 10.0),
        Mods::empty(),
    );
    ui.flush();
    ui.prepare_frame(w);
    assert_eq!(got.get(), 1);
    let layers = ui.inspect_layers(t);
    assert!(layers.iter().any(|l| l.name == "Sort"));
}

#[test]
fn cells_are_edited_in_place() {
    let mut ui = Ui::new_headless();
    let t = table(&mut ui, 10, true);
    let w = window(&mut ui, Size::new(500.0, 300.0), t);
    let f = ui.tree.window_rect(t);
    let edits = Rc::new(RefCell::new(Vec::new()));
    let e2 = edits.clone();
    ui.connect(t, Signal::CellEdited, move |_, _, v| {
        if let Value::Cell { row, column, text } = v {
            e2.borrow_mut().push((*row, *column, text.clone()));
        }
    });
    // Select row 2, Return edits its first editable cell.
    let p = Point::new(f.x + 60.0, f.y + 28.0 + 2.0 * 28.0 + 14.0);
    click(&mut ui, w, p, Mods::empty());
    key(&mut ui, w, Key::Enter, Mods::empty());
    ui.prepare_frame(w);
    assert!(ui.widget::<TableView>(t).unwrap().is_editing());
    ui.text_input(w, "X");
    key(&mut ui, w, Key::Enter, Mods::empty());
    ui.prepare_frame(w);
    let got = edits.borrow().clone();
    assert_eq!(got.len(), 1, "{got:?}");
    assert_eq!((got[0].0, got[0].1), (2, 0));
    assert!(got[0].2.contains('X'), "{got:?}");
    assert!(got[0].2.contains("r2 c0"), "{got:?}");
    assert!(!ui.widget::<TableView>(t).unwrap().is_editing());
    // Escape cancels.
    key(&mut ui, w, Key::Enter, Mods::empty());
    ui.prepare_frame(w);
    assert!(ui.widget::<TableView>(t).unwrap().is_editing());
    key(&mut ui, w, Key::Escape, Mods::empty());
    ui.prepare_frame(w);
    assert!(!ui.widget::<TableView>(t).unwrap().is_editing());
    assert_eq!(edits.borrow().len(), 1);
}

/// A tree: node n has children 10n+1 ..= 10n+3 while n < 100; roots 1, 2, 3.
struct Tens;

impl TreeSource for Tens {
    fn child_count(&mut self, _ui: &mut Ui, parent: Option<u64>) -> usize {
        match parent {
            None => 3,
            Some(n) if n < 100 => 3,
            Some(_) => 0,
        }
    }

    fn child(&mut self, _ui: &mut Ui, parent: Option<u64>, index: usize) -> u64 {
        parent.unwrap_or(0) * 10 + index as u64 + 1
    }

    fn is_expandable(&mut self, _ui: &mut Ui, node: u64) -> bool {
        node < 100
    }
}

#[test]
fn an_outline_expands_and_collapses() {
    let mut ui = Ui::new_headless();
    let mut o = TableView::new(vec![TableColumn::new("Name", 200.0)]);
    o.set_tree(
        Box::new(Tens),
        metrics::ROW_HEIGHT,
        Box::new(|ui, node, _| Some(ui.create(Text::new(&format!("n{node}"))))),
    );
    let t = ui.create(o);
    let w = window(&mut ui, Size::new(400.0, 400.0), t);
    let f = ui.tree.window_rect(t);
    let names = |ui: &Ui| -> Vec<String> {
        let mut rows: Vec<(f32, String)> = rows_of(ui, t)
            .into_iter()
            .map(|r| (ui.tree.window_rect(r).y, text(ui, r)))
            .collect();
        rows.sort_by(|a, b| a.0.total_cmp(&b.0));
        rows.into_iter().map(|r| r.1).collect()
    };
    assert_eq!(names(&ui), vec!["n1", "n2", "n3"]);
    let opened = Rc::new(RefCell::new(Vec::new()));
    let o2 = opened.clone();
    ui.connect(t, Signal::ExpandedChanged, move |_, _, v| {
        o2.borrow_mut().push(v.clone())
    });
    // Select n2, then expand n1 with a click on its chevron: n2 stays selected.
    click(
        &mut ui,
        w,
        Point::new(f.x + 100.0, f.y + 28.0 + 28.0 + 10.0),
        Mods::empty(),
    );
    click(
        &mut ui,
        w,
        Point::new(f.x + 24.0, f.y + 28.0 + 14.0),
        Mods::empty(),
    );
    ui.flush();
    ui.prepare_frame(w);
    assert_eq!(names(&ui), vec!["n1", "n11", "n12", "n13", "n2", "n3"]);
    assert_eq!(ui.get(t, Prop::SelectedRows), Some(Value::IntList(vec![2])));
    assert_eq!(*opened.borrow(), vec![Value::IntList(vec![1, 1])]);
    // Children are indented.
    let n11 = rows_of(&ui, t)
        .into_iter()
        .find(|r| text(&ui, *r) == "n11")
        .unwrap();
    let cell = ui.tree.children(n11)[0];
    assert_eq!(
        ui.tree.window_rect(cell).x - f.x,
        metrics::LEADING + metrics::INDENT + metrics::DISCLOSURE
    );
    // On n12, ← goes to its parent.
    ui.set(t, Prop::SelectedRows, Value::IntList(vec![12]))
        .unwrap();
    key(&mut ui, w, Key::Left, Mods::empty());
    assert_eq!(ui.get(t, Prop::SelectedRows), Some(Value::IntList(vec![1])));
    // ← on the expanded n1 collapses it; ⌥→ expands it deep.
    key(&mut ui, w, Key::Left, Mods::empty());
    ui.prepare_frame(w);
    assert_eq!(names(&ui), vec!["n1", "n2", "n3"]);
    key(&mut ui, w, Key::Right, Mods::ALT);
    ui.prepare_frame(w);
    assert_eq!(
        ui.get(t, Prop::RowCount),
        Some(Value::from(1 + 3 + 9 + 2usize))
    );
    assert_eq!(
        ui.get(t, Prop::ExpandedItems),
        Some(Value::IntList(vec![1, 11, 12, 13]))
    );
}

#[test]
fn a_grid_reflows_and_selects_in_two_dimensions() {
    let mut ui = Ui::new_headless();
    let g = ui.create(GridView::new(
        1000,
        Box::new(|ui, i| Some(ui.create(Text::new(&format!("Item {i}"))))),
    ));
    // 16 + 4 × 104 + 3 × 16 + 16 = 496.
    let w = window(&mut ui, Size::new(500.0, 400.0), g);
    assert_eq!(ui.get(g, Prop::Columns), Some(Value::from(4usize)));
    let live = rows_of(&ui, g).len();
    assert!(live < 60, "{live} tiles");
    let f = ui.tree.window_rect(g);
    // Item 5: second row, second column.
    let p = Point::new(f.x + 16.0 + 120.0 + 50.0, f.y + 16.0 + 120.0 + 50.0);
    click(&mut ui, w, p, Mods::empty());
    ui.flush();
    assert_eq!(ui.get(g, Prop::SelectedRows), Some(Value::IntList(vec![5])));
    key(&mut ui, w, Key::Down, Mods::empty());
    assert_eq!(ui.get(g, Prop::SelectedRows), Some(Value::IntList(vec![9])));
    key(&mut ui, w, Key::Right, Mods::SHIFT);
    assert_eq!(
        ui.get(g, Prop::SelectedRows),
        Some(Value::IntList(vec![9, 10]))
    );
    ui.prepare_frame(w);
    let layers = ui.inspect_layers(g);
    assert_eq!(
        layers.iter().filter(|l| l.name == "Label capsule").count(),
        2
    );
    // A click between tiles clears the selection.
    click(&mut ui, w, Point::new(f.x + 4.0, f.y + 4.0), Mods::empty());
    ui.flush();
    assert_eq!(ui.get(g, Prop::SelectedRows), Some(Value::IntList(vec![])));
}
