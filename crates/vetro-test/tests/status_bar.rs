//! Kit parity of `Status Bar`: the items and the menu bar and lock screen bars.

mod common;

use common::text_at;
use vetro_lib::amalfi::statusbar::{ItemContent, StatusBar, StatusItem, status_bar};
use vetro_lib::core::{Prop, Ui, Value, ViewId};
use vetro_test::{KitLayer, KitMaster, Scene, Tolerance};

/// Glass plates and thin glyphs.
const STATUS: Tolerance = Tolerance {
    mean: 2.5,
    max: 120,
    mask_text: true,
};

fn set_id(ui: &mut Ui, v: ViewId, name: &str) {
    let _ = ui.set(v, Prop::Identifier, Value::from(name));
}

/// An item from a kit item (a master root or an instance of one).
fn item(kit: &KitLayer, master: &str, scene_dark: bool) -> StatusItem {
    let glyph = text_at(kit, "Glyph Status").unwrap_or_default();
    let content = if master.contains("/Battery/") {
        ItemContent::Battery {
            symbol: glyph,
            percentage: text_at(kit, "Percentage").unwrap_or_default(),
        }
    } else if master.contains("/Clock/") {
        ItemContent::Clock(text_at(kit, "Label").unwrap_or_default())
    } else if master.contains("/Input Source/") {
        ItemContent::InputSource {
            symbol: glyph,
            label: text_at(kit, "Label").unwrap_or_default(),
        }
    } else {
        ItemContent::Glyph(glyph)
    };
    let mut it = StatusItem::new(content);
    it.active = master.contains("/Active/");
    // The lock screen draws the dark items on a light wallpaper.
    it.on_dark = master.ends_with("/Dark") && !scene_dark;
    it
}

#[test]
fn status_bar_masters_match_the_kit() {
    let names = KitMaster::names("Status Bar");
    assert_eq!(names.len(), 22, "{} masters", names.len());
    let mut failures = Vec::new();
    for name in &names {
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        let root = scene.master.root.clone();
        let size = scene.master.size();
        let dark = scene.master.dark();
        let mut appearance = scene.ui.appearance;
        appearance.increase_contrast = name.contains("/Increase Contrast/");
        appearance.reduce_transparency = name.contains("/Reduce Transparency/");
        let is_bar = name.starts_with("Status Bar/Bar/");
        scene.ui.set_appearance(appearance);
        scene.mount_sized(size, |ui| {
            if is_bar {
                let mut kids: Vec<&KitLayer> = root
                    .children
                    .iter()
                    .filter(|c| c.master.is_some())
                    .collect();
                kids.sort_by(|a, b| a.frame.x.total_cmp(&b.frame.x));
                let views: Vec<ViewId> = kids
                    .iter()
                    .map(|k| {
                        let m = k.master.clone().unwrap_or_default();
                        let v = ui.create(item(k, &m, dark));
                        // Sketch measures the clock 4 pt wider: the instance's width stands.
                        let _ = ui.set(v, Prop::MinWidth, Value::from(k.frame.width));
                        set_id(ui, v, &k.name);
                        v
                    })
                    .collect();
                status_bar(ui, &views)
            } else {
                ui.create(item(&root, name, dark))
            }
        });
        let _ = StatusBar;
        scene.png_margin = true;
        scene.settle();
        let report = scene.auto_geometry(&[]);
        if !report.is_empty() {
            failures.push(format!("{name}:\n    {}", report.join("\n    ")));
            continue;
        }
        if let Err(e) = scene.pixels(STATUS) {
            failures.push(format!("{name}: {e}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
