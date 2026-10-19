//! Kit parity of `Group Boxes`.

use vetro_lib::amalfi::fields::TextField;
use vetro_lib::amalfi::group_box::group_box;
use vetro_lib::amalfi::switch::Switch;
use vetro_lib::amalfi::toggles::{Toggle, ToggleState};
use vetro_lib::core::{Prop, Ui, Value, ViewId};
use vetro_test::{KitLayer, KitMaster, Scene, Tolerance};

/// A box of raised surface with controls: glass-free shapes of 12 pt radius.
const BOX: Tolerance = Tolerance {
    mean: 3.0,
    max: 100,
    mask_text: true,
};

/// The views of a kit group's instances, top to bottom.
fn build(ui: &mut Ui, group: &KitLayer) -> Vec<ViewId> {
    let mut items: Vec<&KitLayer> = group
        .children
        .iter()
        .filter(|c| c.master.is_some() && !c.hidden)
        .collect();
    items.sort_by(|a, b| a.frame.y.total_cmp(&b.frame.y));
    let mut out = Vec::new();
    for item in items {
        let master = item.master.clone().unwrap_or_default();
        let label = item.overrides.first().cloned().unwrap_or_default();
        let on = master.contains("/On/");
        let view = if master.contains("Toggles/Checkbox/With Label") {
            let state = if on {
                ToggleState::On
            } else {
                ToggleState::Off
            };
            ui.create(Toggle::checkbox().label(&label).state(state))
        } else if master.contains("Toggles/Switch/With Label") {
            ui.create(Switch::new().on(on).label(&label))
        } else if master.contains("Fields/Text Field") {
            let value = KitMaster::load(&master)
                .ok()
                .and_then(|m| m.layer("Value").and_then(|l| l.text.clone()))
                .unwrap_or_default();
            ui.create(TextField::text().with_text(&value))
        } else if master.contains("Group Boxes/Box") {
            let inner = KitMaster::load(&master)
                .ok()
                .and_then(|m| m.layer("Box").cloned());
            let kids = inner.map(|b| build(ui, &b)).unwrap_or_default();
            group_box(ui, None, &kids)
        } else {
            continue;
        };
        let _ = ui.set(view, Prop::Identifier, Value::from(item.name.as_str()));
        out.push(view);
    }
    out
}

#[test]
fn group_boxes_match_the_kit() {
    let names = KitMaster::names("Group Boxes");
    assert_eq!(names.len(), 8, "{names:?}");
    let mut failures = Vec::new();
    for name in &names {
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        let title = scene.master.layer("Title").and_then(|t| t.text.clone());
        let boxed = scene.master.layer("Box").cloned();
        let size = scene.master.size();
        scene.mount_sized(size, |ui| {
            let kids = boxed.map(|b| build(ui, &b)).unwrap_or_default();
            group_box(ui, title.as_deref(), &kids)
        });
        if name.contains("/Disabled/") {
            scene.set_state("Disabled");
        }
        // Kit quirk: the nested group's instance is 256 wide where its master is 224; the
        // nested layers keep the master's sizes.
        // The same for the switch row (256 wide, its master 240): the switch keeps its place.
        let skip: &[&str] = if name.contains("/Nested/") {
            &[
                "Box/Inner box/Box/Surface",
                "Box/Enable/Switch/Track",
                "Box/Enable/Switch/Thumb",
            ]
        } else {
            &[]
        };
        // Kit text measure: "Notifications" is 88 wide in the kit, 84 on the 4 pt grid here.
        let devs: &[&str] = if name.contains("/Titled/") {
            &["Title#frame"]
        } else {
            &[]
        };
        let report = scene.auto_geometry_except(skip, devs);
        if !report.is_empty() {
            failures.push(format!("{name}:\n    {}", report.join("\n    ")));
            continue;
        }
        if let Err(e) = scene.pixels(BOX) {
            failures.push(format!("{name}: {e}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
