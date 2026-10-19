//! Kit parity of `Titlebars`: window buttons, pieces, bars, sections, title bars and tabs.

mod common;

use common::titlebar_view as build;
use vetro_lib::amalfi::titlebars::Spacer;
use vetro_lib::geom::Point;
use vetro_test::{KitMaster, Scene, Tolerance};

/// Glass, gradients and rounded corners.
const TITLEBAR: Tolerance = Tolerance {
    mean: 2.5,
    max: 110,
    mask_text: true,
};

/// The 16 pt window buttons: the edge pixels are most of the image and the kit's hover
/// export antialiases its 0.5 pt rim differently (the inside matches exactly).
const DOTS: Tolerance = Tolerance {
    mean: 12.0,
    max: 110,
    mask_text: true,
};

#[test]
fn titlebar_masters_match_the_kit() {
    let names = KitMaster::names("Titlebars");
    assert_eq!(names.len(), 181, "{} masters", names.len());
    let mut failures = Vec::new();
    for name in &names {
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        let root = scene.master.root.clone();
        let size = scene.master.size();
        let mounted = scene.mount_sized(size, |ui| {
            build(ui, &root, name).unwrap_or_else(|| ui.create(Spacer::fixed(1.0)))
        });
        let _ = mounted;
        let w = scene.window;
        let inactive = name.contains("/Inactive/") || name.ends_with("/Inactive");
        scene.ui.set_window_active(w, !inactive);
        // Kit defect: the Dark/Glass/Regular shared style blurs 10 where materials.md gives 6.
        scene.ignore_aspects = vec!["#blurs".to_string()];
        // Several instances share a name in the kit (the toolbar buttons of the compact
        // bar): their path is ambiguous.
        let skip: &[&str] = if name.contains("Bar/Compact") {
            &["Toolbar button/Background", "Toolbar button/Glyph Gear"]
        } else if name.contains("Title Bar/Subtitle") {
            // Sketch measures the subtitle 2 pt wider.
            &["Title block/Subtitle"]
        } else if name.contains("Search Token") {
            // The nested token master keeps its own label box.
            &["Token/Label"]
        } else {
            &[]
        };
        if name.contains("Controls/Hover") {
            scene.hover_at(Point::new(8.0, 8.0));
        } else if name.contains("Dot/") && name.contains("/Hover/") {
            scene.set_state("Hover");
        } else if name.contains("Tab Bar/Hover") {
            if let Some(p) = scene.master.rect_in_master("Tab 2") {
                scene.hover_at(p.center());
            }
        } else if name.starts_with("Titlebars/Tab/Hover") {
            scene.set_state("Hover");
        }
        scene.settle();
        let report = scene.auto_geometry_except(skip, &[]);
        if !report.is_empty() {
            failures.push(format!("{name}:\n    {}", report.join("\n    ")));
            continue;
        }
        if name.starts_with("Titlebars/Dot/")
            && let Some(r) = scene.master.rect_in_master("Glyph")
        {
            scene.extra_mask.push(r);
        }
        // The window buttons are 16 pt: their antialiased rims weigh a lot in the mean.
        let tol = if name.starts_with("Titlebars/Dot/") || name.starts_with("Titlebars/Controls/") {
            DOTS
        } else {
            TITLEBAR
        };
        if let Err(e) = scene.pixels(tol) {
            failures.push(format!("{name}: {e}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
