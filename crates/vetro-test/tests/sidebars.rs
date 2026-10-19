//! Kit parity of `Sidebars`: rows, headers, groups, the footer and the panel.

mod common;

use common::sidebar_view as build;
use vetro_lib::amalfi::sidebars::{Panel, metrics};
use vetro_lib::geom::Point;
use vetro_test::{KitMaster, Scene, Tolerance};

/// Pills, rounded selections and thin glyphs.
const SIDEBAR: Tolerance = Tolerance {
    mean: 2.5,
    max: 110,
    mask_text: true,
};

#[test]
fn sidebar_masters_match_the_kit() {
    let names = KitMaster::names("Sidebars");
    assert_eq!(names.len(), 44, "{} masters", names.len());
    let _ = metrics::ROW_WIDTH;
    let mut failures = Vec::new();
    for name in &names {
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        let root = scene.master.root.clone();
        let size = scene.master.size();
        scene.mount_sized(size, |ui| {
            build(ui, &root, name).unwrap_or_else(|| ui.create(Panel))
        });
        if name.contains("Volume/Hover/") {
            // The kit shows the eject button hovered too.
            scene.hover_at(Point::new(196.0, 16.0));
        } else if name.contains("/Hover/") {
            scene.set_state("Hover");
        }
        let report = scene.auto_geometry(&[]);
        if !report.is_empty() {
            failures.push(format!("{name}:\n    {}", report.join("\n    ")));
            continue;
        }
        if let Err(e) = scene.pixels(SIDEBAR) {
            failures.push(format!("{name}: {e}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
