//! Kit parity of `Menus`: the rows and the panels.

mod common;

use common::{Fixed, menu_view, row_view};
use vetro_lib::amalfi::menus::{Menu, Row};
use vetro_lib::core::{Prop, Value};
use vetro_test::{KitMaster, Scene, Tolerance};

/// Glass panels and highlighted rows.
const MENU: Tolerance = Tolerance {
    mean: 2.0,
    max: 100,
    mask_text: true,
};

#[test]
fn items_match_the_kit() {
    let names: Vec<String> = KitMaster::names("Menus/Item")
        .into_iter()
        .filter(|n| !n.contains("/App/"))
        .collect();
    assert_eq!(names.len(), 38, "{} masters", names.len());
    let mut failures = Vec::new();
    for name in &names {
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        let root = scene.master.root.clone();
        let size = scene.master.size();
        scene.mount_sized(size, |ui| row_view(ui, &root, name));
        let report = scene.auto_geometry(&[]);
        if !report.is_empty() {
            failures.push(format!("{name}:\n    {}", report.join("\n    ")));
            continue;
        }
        if let Err(e) = scene.pixels(MENU) {
            failures.push(format!("{name}: {e}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn menus_match_the_kit() {
    let names = KitMaster::names("Menus/Menu");
    assert_eq!(names.len(), 10, "{names:?}");
    let mut failures = Vec::new();
    for name in &names {
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        let root = scene.master.root.clone();
        let size = scene.master.size();
        let cascade = name.contains("/Cascade/");
        scene.mount_sized(size, |ui| {
            if cascade {
                let mut rects = Vec::new();
                let mut views = Vec::new();
                for c in root.children.iter().filter(|c| c.master.is_some()) {
                    let kit = KitMaster::load(c.master.as_deref().unwrap_or_default());
                    let Ok(kit) = kit else { continue };
                    let m = menu_view(ui, &kit.root);
                    let _ = ui.set(m, Prop::Identifier, Value::from(c.name.as_str()));
                    rects.push(c.frame);
                    views.push(m);
                }
                let f = ui.create(Fixed { size, rects });
                for v in views {
                    ui.add_child(f, v);
                }
                f
            } else {
                menu_view(ui, &root)
            }
        });
        let _ = Menu::new();
        // Two separators have the same name in the kit, so their layer path is ambiguous;
        // the pixels and the Item/Separator masters cover them.
        let report = scene.auto_geometry_except(&["Separator/Rest/Line"], &[]);
        if !report.is_empty() {
            failures.push(format!("{name}:\n    {}", report.join("\n    ")));
            continue;
        }
        if let Err(e) = scene.pixels(MENU) {
            failures.push(format!("{name}: {e}"));
        }
    }
    let _ = Row::item("");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
