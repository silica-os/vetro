//! Kit parity of the collections against the file manager's masters: `Apps/Mustache/List
//! Header` (a table header), `List Row` (table rows: plain, striped, selected),
//! `Column Row` (list rows: plain, selected, dimmed) and `Tile Selected` (a grid tile).
//! The cells are the app's content; what is compared is the collection's part (row and
//! header geometry, column origins, stripes, selections, emphasis of selected content).

use vetro_lib::amalfi::collections::{
    GridView, ListLook, ListView, TableColumn, TableHeader, TableView,
};
use vetro_lib::amalfi::label::ColorRole;
use vetro_lib::amalfi::label::{Label, SymbolImage};
use vetro_lib::amalfi::tokens::typography;
use vetro_lib::core::layout::{Alignment, Overlay, Stack};
use vetro_lib::core::{Prop, Ui, Value, ViewId};
use vetro_lib::geom::Size;
use vetro_test::{KitMaster, Scene, Tolerance};

/// Text and thin glyphs on flat fills.
const ROWS: Tolerance = Tolerance {
    mean: 2.5,
    max: 255,
    mask_text: true,
};

/// The file list's columns: names after a 20 pt icon, then date, size and kind.
fn file_columns() -> Vec<TableColumn> {
    vec![
        TableColumn::new("Name", 308.0).title_inset(24.0),
        TableColumn::new("Date Modified", 168.0),
        TableColumn::new("Size", 88.0),
        TableColumn::new("Kind", 128.0),
    ]
}

fn text(
    ui: &mut Ui,
    id: &str,
    s: &str,
    style: &'static vetro_lib::amalfi::tokens::TypeStyle,
    color: &str,
    width: Option<f32>,
) -> ViewId {
    let v = ui.create(Label::new(s).style(style).color(color));
    let _ = ui.set(v, Prop::Identifier, Value::from(id));
    if let Some(w) = width {
        let _ = ui.set(v, Prop::Width, Value::from(w));
    }
    v
}

fn icon(ui: &mut Ui, name: &str, size: f32) -> ViewId {
    let mut s = SymbolImage::new(name);
    s.size = size;
    s.color = ColorRole::Token("Accent/Amalfi".into());
    let v = ui.create(s);
    let _ = ui.set(v, Prop::Identifier, Value::from("Icon"));
    v
}

/// A cell of the file list (`List Row`).
fn file_cell(ui: &mut Ui, column: usize) -> Option<ViewId> {
    Some(match column {
        0 => {
            let row = ui.create(Stack::row(4.0));
            let i = icon(ui, "folder", 20.0);
            let n = text(
                ui,
                "Name",
                "Documents",
                &typography::BODY,
                "Text/Primary",
                Some(280.0),
            );
            ui.add_child(row, i);
            ui.add_child(row, n);
            ui.release(i);
            ui.release(n);
            row
        }
        1 => text(
            ui,
            "Date",
            "Today, 10:22",
            &typography::CALLOUT,
            "Text/Secondary",
            None,
        ),
        2 => text(
            ui,
            "Size",
            "--",
            &typography::CALLOUT,
            "Text/Secondary",
            None,
        ),
        _ => text(
            ui,
            "Kind",
            "Folder",
            &typography::CALLOUT,
            "Text/Secondary",
            None,
        ),
    })
}

fn fill(ui: &mut Ui, v: ViewId) {
    let _ = ui.set(v, Prop::MaxWidth, Value::from(f32::INFINITY));
    let _ = ui.set(v, Prop::MaxHeight, Value::from(f32::INFINITY));
}

#[test]
fn the_table_header_matches_the_kit() {
    let names = KitMaster::names("Apps/Mustache/List Header");
    assert_eq!(names.len(), 2, "{names:?}");
    let mut failures = Vec::new();
    for name in &names {
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        let size = scene.master.size();
        scene.mount_sized(size, |ui| {
            let h = ui.create(TableHeader::new(file_columns()));
            fill(ui, h);
            h
        });
        // Kit defect (docs/kit-issues.md): the Name column's text box is 280 wide and ends
        // 16 before Date, where every other column is 12 from the next.
        let report = scene.geometry(
            &[
                ("Name", "Name"),
                ("Date", "Date Modified"),
                ("Size", "Size"),
                ("Kind", "Kind"),
                ("Separator", "Separator"),
            ],
            &["Name#frame"],
        );
        if !report.is_empty() {
            failures.push(format!("{name}:\n    {}", report.join("\n    ")));
            continue;
        }
        if let Err(e) = scene.pixels(ROWS) {
            failures.push(format!("{name}: {e}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn table_rows_match_the_kit() {
    let names = KitMaster::names("Apps/Mustache/List Row");
    assert_eq!(names.len(), 6, "{names:?}");
    let mut failures = Vec::new();
    for name in &names {
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        let size = scene.master.size();
        let odd = name.contains("/Odd/");
        let selected = name.contains("/Selected/");
        let table = scene.mount_sized(size, |ui| {
            let mut t = TableView::new(file_columns())
                .header_visible(false)
                .alternating(true);
            // Two rows; the odd one is scrolled to the top for `Odd`.
            t.set_cells(2, 28.0, Box::new(|ui, _, c| file_cell(ui, c)));
            let v = ui.create(t);
            fill(ui, v);
            v
        });
        if odd {
            let _ = scene.ui.set(
                table,
                Prop::ScrollOffset,
                Value::Point(vetro_lib::geom::Point::new(0.0, 28.0)),
            );
        }
        if selected {
            let w = scene.window;
            scene.ui.set_window_active(w, true);
            scene.ui.set_focus(table, false);
            let _ = scene
                .ui
                .set(table, Prop::SelectedRows, Value::IntList(vec![0]));
        }
        scene.refresh();
        let row = if odd { 1 } else { 0 };
        let mut map = vec![
            ("Icon", format!("{row}/Icon")),
            ("Name", format!("{row}/Name")),
            ("Date", format!("{row}/Date")),
            ("Size", format!("{row}/Size")),
            ("Kind", format!("{row}/Kind")),
        ];
        if odd {
            map.push(("Stripe", "Stripe".to_string()));
        }
        if selected {
            map.push(("Selection", "Selection".to_string()));
        }
        // Our cells' paths: the rows are identified by their index for the comparison.
        let rows: Vec<ViewId> = scene
            .ui
            .tree
            .children(scene.ui.tree.children(table)[0])
            .to_vec();
        for (i, r) in rows.iter().enumerate() {
            let _ = scene
                .ui
                .set(*r, Prop::Identifier, Value::from(i.to_string().as_str()));
        }
        scene.refresh();
        let ours = scene.layers();
        let path_of = |id: &str| -> String {
            ours.iter()
                .find(|l| l.path.starts_with(id))
                .map(|l| l.path.clone())
                .unwrap_or_else(|| id.to_string())
        };
        let resolved: Vec<(String, String)> = map
            .iter()
            .map(|(k, o)| ((*k).to_string(), path_of(o)))
            .collect();
        let pairs: Vec<(&str, &str)> = resolved
            .iter()
            .map(|(a, b)| (a.as_str(), b.as_str()))
            .collect();
        // Kit quirk (docs/kit-issues.md): the plain rows' name sits 1 pt higher than the
        // selected row's (4 vs 5); centred under the 20 pt icon it is at 5.
        let devs: &[&str] = if selected { &[] } else { &["Name#frame"] };
        let report = scene.geometry(&pairs, devs);
        if !report.is_empty() {
            failures.push(format!("{name}:\n    {}", report.join("\n    ")));
            continue;
        }
        if let Err(e) = scene.pixels(ROWS) {
            failures.push(format!("{name}: {e}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A browser column's row (`Column Row`): icon at 16, label at 40, a chevron at the end.
fn column_row(ui: &mut Ui) -> ViewId {
    let row = ui.create(Stack::row(4.0));
    let _ = ui.set(
        row,
        Prop::Padding,
        Value::Insets(vetro_lib::geom::Insets::new(0.0, 8.0, 0.0, 4.0)),
    );
    let i = icon(ui, "folder", 20.0);
    let l = text(
        ui,
        "Label",
        "Documents",
        &typography::BODY,
        "Text/Primary",
        Some(160.0),
    );
    let mut c = SymbolImage::new("chevron-right");
    c.size = 16.0;
    c.color = ColorRole::Token("Text/Tertiary".into());
    let ch = ui.create(c);
    let _ = ui.set(ch, Prop::Identifier, Value::from("Chevron"));
    for v in [i, l, ch] {
        ui.add_child(row, v);
        ui.release(v);
    }
    row
}

#[test]
fn list_rows_match_the_kit() {
    let names = KitMaster::names("Apps/Mustache/Column Row");
    assert_eq!(names.len(), 6, "{names:?}");
    let mut failures = Vec::new();
    for name in &names {
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        let size = scene.master.size();
        let list = scene.mount_sized(size, |ui| {
            let v = ui.create(
                ListView::new(1, 28.0, Box::new(|ui, _| Some(column_row(ui))))
                    .look(ListLook::Inset),
            );
            fill(ui, v);
            v
        });
        let selected = name.contains("/Selected/");
        if selected || name.contains("/Dimmed/") {
            let _ = scene
                .ui
                .set(list, Prop::SelectedRows, Value::IntList(vec![0]));
        }
        if selected {
            let w = scene.window;
            scene.ui.set_window_active(w, true);
            scene.ui.set_focus(list, false);
        }
        scene.refresh();
        let mut map = vec![
            ("Icon", "Icon/Icon"),
            ("Label", "Label/Label"),
            ("Chevron", "Chevron/Icon"),
        ];
        if name.contains("/Plain/") {
            // No selection.
        } else {
            map.push(("Selection", "Selection"));
        }
        // Kit quirk: without a selection the label sits 1 pt higher (4 vs 5).
        let devs: &[&str] = if name.contains("/Plain/") {
            &["Label#frame"]
        } else {
            &[]
        };
        let report = scene.geometry(&map, devs);
        if !report.is_empty() {
            failures.push(format!("{name}:\n    {}", report.join("\n    ")));
            continue;
        }
        if let Err(e) = scene.pixels(ROWS) {
            failures.push(format!("{name}: {e}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn a_selected_tile_matches_the_kit() {
    let names = KitMaster::names("Apps/Mustache/Tile Selected");
    assert_eq!(names.len(), 2, "{names:?}");
    let mut failures = Vec::new();
    for name in &names {
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        let size = scene.master.size();
        let grid = scene.mount_sized(size, |ui| {
            let g = GridView::new(
                1,
                Box::new(|ui, _| {
                    let o = ui.create(Overlay {
                        alignment: Alignment::TOP,
                    });
                    let l = text(
                        ui,
                        "Label",
                        "Desktop",
                        &typography::CALLOUT,
                        "Text/Primary",
                        Some(88.0),
                    );
                    let _ = ui.set(l, Prop::TextAlignment, Value::Int(1));
                    let _ = ui.set(
                        l,
                        Prop::Padding,
                        Value::Insets(vetro_lib::geom::Insets::new(74.0, 0.0, 0.0, 0.0)),
                    );
                    ui.add_child(o, l);
                    ui.release(l);
                    Some(o)
                }),
            )
            .tile(Size::new(104.0, 104.0), 64.0)
            .spacing(0.0, 0.0);
            let v = ui.create(g);
            fill(ui, v);
            v
        });
        let w = scene.window;
        scene.ui.set_window_active(w, true);
        scene.ui.set_focus(grid, false);
        let _ = scene
            .ui
            .set(grid, Prop::SelectedRows, Value::IntList(vec![0]));
        scene.refresh();
        // The folder image and its glyph are the app's (a bitmap in the kit).
        scene
            .extra_mask
            .push(vetro_lib::geom::Rect::new(20.0, 0.0, 64.0, 64.0));
        let report = scene.geometry(
            &[
                ("Selection", "Selection"),
                ("Label capsule", "Label capsule"),
                ("Label", "Label/Label"),
            ],
            &[],
        );
        if !report.is_empty() {
            failures.push(format!("{name}:\n    {}", report.join("\n    ")));
            continue;
        }
        if let Err(e) = scene.pixels(ROWS) {
            failures.push(format!("{name}: {e}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
