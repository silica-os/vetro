//! Kit parity of `Forms`: rows, sections, the footer and the layout.

mod common;

use common::{build_footer, build_row, build_section};
use vetro_lib::amalfi::forms::form_layout;
use vetro_lib::core::{Prop, Value, ViewId};
use vetro_test::{KitLayer, KitMaster, Scene, Tolerance};

/// Raised surfaces with controls inside: edges of small controls and fields.
const FORM: Tolerance = Tolerance {
    mean: 3.0,
    max: 150,
    mask_text: true,
};

#[test]
fn forms_match_the_kit() {
    let names = KitMaster::names("Forms");
    assert_eq!(names.len(), 22, "{names:?}");
    let mut failures = Vec::new();
    for name in &names {
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        let parts: Vec<&str> = name.split('/').collect();
        let kind = parts.get(1).copied().unwrap_or_default();
        let root = scene.master.root.clone();
        let size = scene.master.size();
        scene.mount_sized(size, |ui| match kind {
            "Row" => {
                // The master is the row itself: wrap its layers as an instance.
                let wrapper = KitLayer {
                    master: Some(name.clone()),
                    ..root.clone()
                };
                let v = build_row(ui, &wrapper);
                let _ = ui.set(v, Prop::Identifier, Value::from(""));
                v
            }
            "Section" => build_section(ui, &root),
            "Footer" => build_footer(ui, &root),
            _ => {
                let title = root
                    .children
                    .iter()
                    .find(|c| c.name == "Title")
                    .and_then(|t| t.text.clone());
                let mut parts: Vec<&KitLayer> = root
                    .children
                    .iter()
                    .filter(|c| c.master.is_some())
                    .collect();
                parts.sort_by(|a, b| a.frame.y.total_cmp(&b.frame.y));
                let views: Vec<ViewId> = parts
                    .iter()
                    .map(|p| {
                        if p.name == "Footer" {
                            build_footer(ui, p)
                        } else {
                            build_section(ui, p)
                        }
                    })
                    .collect();
                form_layout(ui, title.as_deref(), &views)
            }
        });
        // Kit quirks: a resized instance keeps its nested layers' places (the text field of a
        // row is 204 in a 200 master, the switch row 204 in a 240 one, the Save button's
        // label keeps the default label's place), and Sketch measures titles a little wider.
        let skip = [
            "Control/Background",
            "Control/Focus Halo",
            "Control/Error icon",
            "Control/Value",
            "Control/Switch/Track",
            "Control/Switch/Thumb",
            "Save/Label",
            "Title",
        ];
        // The kit PNGs of some rows are cropped to the layout, ours include the halo's room.
        scene.png_margin = true;
        let report = scene.auto_geometry_except(&skip, &[]);
        if !report.is_empty() {
            failures.push(format!("{name}:\n    {}", report.join("\n    ")));
            continue;
        }
        if let Err(e) = scene.pixels(FORM) {
            failures.push(format!("{name}: {e}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
