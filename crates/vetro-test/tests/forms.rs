//! Kit parity of `Forms`: rows, sections, the footer and the layout.

use vetro_lib::amalfi::button::{PushButton, PushVariant};
use vetro_lib::amalfi::buttons::MenuButton;
use vetro_lib::amalfi::fields::TextField;
use vetro_lib::amalfi::forms::{MessageKind, form_footer, form_layout, form_row, form_section};
use vetro_lib::amalfi::switch::Switch;
use vetro_lib::amalfi::toggles::{Toggle, ToggleState};
use vetro_lib::core::{Prop, Ui, Value, ViewId};
use vetro_test::{KitLayer, KitMaster, Scene, Tolerance};

/// Raised surfaces with controls inside: edges of small controls and fields.
const FORM: Tolerance = Tolerance {
    mean: 3.0,
    max: 150,
    mask_text: true,
};

fn text_at(layer: &KitLayer, path: &str) -> String {
    let mut cur = layer;
    for part in path.split('/') {
        match cur.children.iter().find(|c| c.name == part) {
            Some(c) => cur = c,
            None => return String::new(),
        }
    }
    cur.text.clone().unwrap_or_default()
}

/// A form row from a kit row instance (expanded).
fn build_row(ui: &mut Ui, row: &KitLayer) -> ViewId {
    let master = row.master.clone().unwrap_or_default();
    let label = text_at(row, "Label cell/Label");
    let control = row
        .children
        .iter()
        .find(|c| c.name == "Value")
        .and_then(|v| v.children.iter().find(|c| c.name == "Control"));
    let cm = control.and_then(|c| c.master.clone()).unwrap_or_default();
    let on = cm.contains("/On/");
    let (view, expand) = if cm.contains("Fields/Text Field") {
        let value = control.map(|c| text_at(c, "Value")).unwrap_or_default();
        let mut f = TextField::text().with_text(&value);
        if cm.contains("/Error/") {
            f = f.with_error("Invalid");
        }
        (ui.create(f), true)
    } else if cm.contains("Toggles/Checkbox") {
        let text = control.map(|c| text_at(c, "Label")).unwrap_or_default();
        let state = if on {
            ToggleState::On
        } else {
            ToggleState::Off
        };
        (
            ui.create(Toggle::checkbox().label(&text).state(state)),
            false,
        )
    } else if cm.contains("Toggles/Switch") {
        let text = control.map(|c| text_at(c, "Label")).unwrap_or_default();
        (ui.create(Switch::new().on(on).label(&text)), true)
    } else {
        let text = control.map(|c| text_at(c, "Label")).unwrap_or_default();
        (
            ui.create(MenuButton::pop_up(&[text.as_str(), "Other"], 0)),
            false,
        )
    };
    let message = row
        .children
        .iter()
        .find(|c| c.name == "Value")
        .map(|v| text_at(v, "Message"))
        .filter(|m| !m.is_empty());
    let kind = if master.contains("/Error/") {
        MessageKind::Error
    } else {
        MessageKind::Help
    };
    let v = form_row(
        ui,
        &label,
        view,
        message.as_deref().map(|m| (m, kind)),
        expand,
    );
    let _ = ui.set(v, Prop::Identifier, Value::from(row.name.as_str()));
    v
}

fn build_section(ui: &mut Ui, section: &KitLayer) -> ViewId {
    let title = section
        .children
        .iter()
        .find(|c| c.name == "Title")
        .and_then(|t| t.text.clone());
    let mut rows: Vec<&KitLayer> = section
        .children
        .iter()
        .find(|c| c.name == "Group")
        .map(|g| g.children.iter().filter(|c| c.master.is_some()).collect())
        .unwrap_or_default();
    rows.sort_by(|a, b| a.frame.y.total_cmp(&b.frame.y));
    let views: Vec<ViewId> = rows.iter().map(|r| build_row(ui, r)).collect();
    let v = form_section(ui, title.as_deref(), &views);
    let _ = ui.set(v, Prop::Identifier, Value::from(section.name.as_str()));
    v
}

fn build_footer(ui: &mut Ui, footer: &KitLayer) -> ViewId {
    let mut buttons: Vec<&KitLayer> = footer
        .children
        .iter()
        .filter(|c| c.master.is_some())
        .collect();
    buttons.sort_by(|a, b| a.frame.x.total_cmp(&b.frame.x));
    let views: Vec<ViewId> = buttons
        .iter()
        .map(|b| {
            let label = text_at(b, "Label");
            let primary = b.master.as_deref().is_some_and(|m| m.contains("/Primary/"));
            let v = ui.create(PushButton::new(&label).variant(if primary {
                PushVariant::Primary
            } else {
                PushVariant::Secondary
            }));
            let _ = ui.set(v, Prop::Identifier, Value::from(b.name.as_str()));
            v
        })
        .collect();
    let f = form_footer(ui, &views);
    let _ = ui.set(f, Prop::Identifier, Value::from(footer.name.as_str()));
    f
}

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
