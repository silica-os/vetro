//! Kit parity of `Fields`: text, secure, search and token fields, text areas, tokens, the
//! clear and combo buttons, combo boxes and fields with an inline button.

mod common;

use common::{Fixed, menu_view};
use vetro_lib::amalfi::button::PushButton;
use vetro_lib::amalfi::fields::{ClearButton, ComboButton, TextField, Token};
use vetro_lib::amalfi::foundation::ControlSize;
use vetro_lib::core::event::{Key, KeyEvent, Mods};
use vetro_lib::core::{Prop, Value};
use vetro_test::{KitMaster, Scene, Tolerance, center_of};

/// Fields: a 1 pt border and the soft corner of 7-10 pt radii; the glass of search fields
/// adds its rim.
const FIELD: Tolerance = Tolerance {
    mean: 2.0,
    max: 180,
    mask_text: true,
};
/// The round parts and the masters of 24 pt or less.
const SMALL: Tolerance = Tolerance {
    mean: 3.5,
    max: 150,
    mask_text: true,
};

fn size(name: &str) -> ControlSize {
    match name {
        "Small" => ControlSize::Small,
        "Large" => ControlSize::Large,
        _ => ControlSize::Regular,
    }
}

fn text_of(scene: &Scene, layer: &str) -> String {
    scene
        .master
        .layer(layer)
        .and_then(|l| l.text.clone())
        .unwrap_or_default()
}

/// Puts the mounted field in a kit state; masks the caret (it blinks and sits after the
/// text, whose width is CoreText's in the kit) in the kit and in ours.
fn finish_state(scene: &mut Scene, state: &str) {
    match state {
        "Focused" | "Token Focused" => scene.set_state("Focused"),
        "Disabled" => scene.set_state("Disabled"),
        _ => {}
    }
    if let Some(r) = scene.master.rect_in_master("Caret") {
        scene.extra_mask.push(r.inflate(1.0, 1.0));
    }
    let origin = scene.origin;
    let ours: Vec<_> = scene
        .layers()
        .into_iter()
        .filter(|l| l.path == "Caret")
        .map(|l| l.frame)
        .collect();
    for f in ours {
        // Layer frames are relative to the view, which sits at the master's origin.
        let _ = origin;
        scene.extra_mask.push(f.inflate(1.0, 1.0));
    }
}

fn check(
    scene: &mut Scene,
    name: &str,
    skip: &[&str],
    devs: &[&str],
    tol: Tolerance,
) -> Option<String> {
    let report = scene.auto_geometry_except(skip, devs);
    if !report.is_empty() {
        return Some(format!("{name}:\n    {}", report.join("\n    ")));
    }
    scene.pixels(tol).err().map(|e| format!("{name}: {e}"))
}

fn finish(failures: Vec<String>, count: usize) {
    assert!(
        failures.is_empty(),
        "{} of {count} masters differ:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// The kit's caret is 2 wide where the token says 1.5: the caret's own frame is not compared.
const SKIP: &[&str] = &["Caret"];

#[test]
fn text_fields_match_the_kit() {
    let names = KitMaster::names("Fields/Text Field");
    assert_eq!(names.len(), 30, "{} masters", names.len());
    let mut failures = Vec::new();
    for name in &names {
        let parts: Vec<&str> = name.split('/').collect();
        let [_, _, sz, state, _app] = parts[..] else {
            continue;
        };
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        let value = text_of(&scene, "Value");
        let mut f = TextField::text().with_size(size(sz));
        f = if state == "Placeholder" {
            f.with_placeholder(&value)
        } else {
            f.with_text(&value)
        };
        if state == "Error" {
            f = f.with_error("Not a place");
        }
        scene.mount(|ui| ui.create(f));
        finish_state(&mut scene, state);
        if let Some(r) = check(&mut scene, name, SKIP, &[], FIELD) {
            failures.push(r);
        }
    }
    finish(failures, names.len());
}

#[test]
fn secure_fields_and_text_areas_match_the_kit() {
    let mut names = KitMaster::names("Fields/Secure Field");
    names.extend(KitMaster::names("Fields/Text Area"));
    assert_eq!(names.len(), 16, "{} masters", names.len());
    let mut failures = Vec::new();
    for name in &names {
        let parts: Vec<&str> = name.split('/').collect();
        let [_, kind, _sz, state, _app] = parts[..] else {
            continue;
        };
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        let value = text_of(&scene, "Value");
        let mut f = if kind == "Secure Field" {
            TextField::secure()
        } else {
            TextField::area()
        };
        f = if state == "Placeholder" {
            f.with_placeholder(&value)
        } else if kind == "Secure Field" {
            // The master shows one bullet per character.
            f.with_text(&"x".repeat(value.chars().count()))
        } else {
            f.with_text(&value)
        };
        scene.mount(|ui| ui.create(f));
        finish_state(&mut scene, state);
        if let Some(r) = check(&mut scene, name, SKIP, &[], FIELD) {
            failures.push(r);
        }
    }
    finish(failures, names.len());
}

#[test]
fn search_fields_match_the_kit() {
    let names = KitMaster::names("Fields/Search Field");
    assert_eq!(names.len(), 28, "{} masters", names.len());
    let mut failures = Vec::new();
    for name in &names {
        let parts: Vec<&str> = name.split('/').collect();
        let [_, _, sz, state, _app] = parts[..] else {
            continue;
        };
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        let mut f = TextField::search().with_size(size(sz));
        if state == "Typing" {
            f = f.with_text(&text_of(&scene, "Value"));
        }
        let token_label = scene
            .master
            .layer("Token")
            .and_then(|t| t.overrides.first().cloned());
        if let Some(t) = &token_label {
            f = f.with_tokens(&[t.as_str()]);
        }
        let wide = scene.master.size();
        scene.mount_sized(wide, |ui| ui.create(f));
        finish_state(&mut scene, state);
        // Kit defect: the Dark/Glass/Regular shared style blurs 10 where materials.md gives 6.
        let devs: &[&str] = if scene.master.dark() {
            &["Background#blurs"]
        } else {
            &[]
        };
        // Kit quirk: the nested token master is 64 wide (Travel) whatever the label: its
        // instance is as wide as the label asks and the nested layers keep their places.
        let mut skip: Vec<&str> = SKIP.iter().copied().chain(["Token/Label"]).collect();
        let wide = scene
            .master
            .layer("Token")
            .is_some_and(|t| (t.frame.width - 64.0).abs() > 0.01);
        if wide {
            skip.extend(["Token/Background", "Token/Remove"]);
        }
        if let Some(r) = check(&mut scene, name, &skip, devs, FIELD) {
            failures.push(r);
        }
    }
    finish(failures, names.len());
}

#[test]
fn token_fields_and_tokens_match_the_kit() {
    let mut names = KitMaster::names("Fields/Token Field");
    names.extend(KitMaster::names("Fields/Token/"));
    assert_eq!(names.len(), 8, "{names:?}");
    let mut failures = Vec::new();
    for name in &names {
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        // Kit defect: a token's label layer is 28 tall and 4 narrower than the label box
        // (Sketch's text measure): its frame is not compared.
        let mut skip: Vec<&str> = SKIP.iter().copied().chain(["Label"]).collect();
        let narrow = scene
            .master
            .layer("Token 2")
            .is_some_and(|t| (t.frame.width - 64.0).abs() > 0.01);
        if narrow {
            // Kit quirk: the second token's instance is 60 wide, the nested master 64 (its
            // content is not rescaled).
            skip.extend(["Token 2/Background", "Token 2/Remove"]);
        }
        if name.contains("Fields/Token Field") {
            let labels: Vec<String> = (1..=2)
                .filter_map(|i| {
                    scene
                        .master
                        .layer(&format!("Token {i}"))
                        .and_then(|t| t.overrides.first().cloned())
                })
                .collect();
            let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
            let f = TextField::token_field()
                .with_placeholder("Add tag")
                .with_tokens(&refs);
            scene.mount(|ui| ui.create(f));
            scene.png_margin = true;
            let focused = name.contains("/Focused/");
            finish_state(&mut scene, if focused { "Focused" } else { "Rest" });
            if focused {
                // The focused master shows the second token selected.
                let w = scene.window;
                scene
                    .ui
                    .key(w, KeyEvent::press(Key::Backspace, Mods::empty()));
                scene.settle();
            }
        } else {
            let label = text_of(&scene, "Label");
            let selected = name.contains("/Selected/");
            scene.mount(|ui| ui.create(Token::new(&label).selected(selected)));
            // The kit PNG does not include the label layer's 28 pt frame; ours does.
            scene.png_margin = true;
        }
        if let Some(r) = check(&mut scene, name, &skip, &[], SMALL) {
            failures.push(r);
        }
    }
    finish(failures, names.len());
}

#[test]
fn clear_and_combo_buttons_match_the_kit() {
    let mut names = KitMaster::names("Fields/Clear Button");
    names.extend(KitMaster::names("Fields/Combo Button"));
    assert_eq!(names.len(), 20, "{} masters", names.len());
    let mut failures = Vec::new();
    for name in &names {
        let parts: Vec<&str> = name.split('/').collect();
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        if parts.get(1) == Some(&"Clear Button") {
            scene.mount(|ui| ui.create(ClearButton));
        } else {
            let [_, _, sz, state, _app] = parts[..] else {
                continue;
            };
            scene.mount(|ui| ui.create(ComboButton { size: size(sz) }));
            match state {
                "Pressed" => {
                    if let Some(p) = center_of(&scene.master, "Disc") {
                        scene.press_at(p);
                    }
                }
                "Disabled" => scene.set_state("Disabled"),
                _ => {}
            }
        }
        if let Some(r) = check(&mut scene, name, &[], &[], SMALL) {
            failures.push(r);
        }
    }
    finish(failures, names.len());
}

#[test]
fn combo_boxes_match_the_kit() {
    let names: Vec<String> = KitMaster::names("Fields/Combo Box")
        .into_iter()
        .filter(|n| !n.contains("/Open/"))
        .collect();
    assert_eq!(names.len(), 18, "{} masters", names.len());
    let mut failures = Vec::new();
    for name in &names {
        let parts: Vec<&str> = name.split('/').collect();
        let [_, _, sz, state, _app] = parts[..] else {
            continue;
        };
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        let value = text_of(&scene, "Value");
        let f = TextField::combo(&["Lemon", "Orange", "Pear"])
            .with_size(size(sz))
            .with_text(&value);
        scene.mount(|ui| ui.create(f));
        finish_state(&mut scene, state);
        if let Some(r) = check(&mut scene, name, SKIP, &[], FIELD) {
            failures.push(r);
        }
    }
    finish(failures, names.len());
}

#[test]
fn inline_button_fields_match_the_kit() {
    let names = KitMaster::names("Fields/Inline Button");
    assert_eq!(names.len(), 6, "{names:?}");
    let mut failures = Vec::new();
    for name in &names {
        let parts: Vec<&str> = name.split('/').collect();
        let [_, _, _sz, state, _app] = parts[..] else {
            continue;
        };
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        let value = text_of(&scene, "Value");
        let label = scene
            .master
            .layer("Inline button")
            .and_then(|b| b.overrides.first().cloned())
            .unwrap_or_default();
        let wide = scene.master.size();
        scene.mount_sized(wide, |ui| {
            let f = ui.create(TextField::text().with_text(&value));
            let b = ui.create(PushButton::new(&label).size(ControlSize::Small));
            let _ = ui.set(b, Prop::Identifier, Value::from("Inline button"));
            ui.add_child(f, b);
            f
        });
        finish_state(&mut scene, state);
        if let Some(r) = check(&mut scene, name, SKIP, &[], FIELD) {
            failures.push(r);
        }
    }
    finish(failures, names.len());
}

#[test]
fn open_combo_boxes_match_the_kit() {
    let names = KitMaster::names("Fields/Combo Box/Open");
    assert_eq!(names.len(), 2, "{names:?}");
    let mut failures = Vec::new();
    for name in &names {
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        let (Some(combo), Some(list)) = (
            scene.master.layer("Combo box").cloned(),
            scene.master.layer("List").cloned(),
        ) else {
            failures.push(format!("{name}: no combo box or list"));
            continue;
        };
        let value = combo
            .children
            .iter()
            .find(|c| c.name == "Value")
            .and_then(|c| c.text.clone())
            .unwrap_or_default();
        let master_size = scene.master.size();
        let mut field_id = None;
        scene.mount_sized(master_size, |ui| {
            let f = ui.create(Fixed {
                size: master_size,
                rects: vec![combo.frame, list.frame],
            });
            let field = ui.create(
                TextField::combo(&["Lemon", "Orange", "Pear"])
                    .with_size(ControlSize::Regular)
                    .with_text(&value),
            );
            let _ = ui.set(field, Prop::Identifier, Value::from("Combo box"));
            field_id = Some(field);
            let menu = menu_view(ui, &list);
            let _ = ui.set(menu, Prop::Identifier, Value::from("List"));
            ui.add_child(f, field);
            ui.add_child(f, menu);
            f
        });
        scene.png_margin = true;
        scene.ignore_aspects = vec!["#blurs".to_string()];
        if let Some(field) = field_id {
            let mut a = scene.ui.appearance;
            a.full_keyboard_access = true;
            scene.ui.set_appearance(a);
            scene.ui.set_focus(field, true);
        }
        scene.settle();
        // The caret blinks; the list's separators share a name (see Menus).
        let mut skip: Vec<&str> = SKIP.to_vec();
        skip.extend(["Combo box/Caret", "Separator/Rest/Line"]);
        if let Some(r) = scene.master.rect_in_master("Combo box/Caret") {
            scene.extra_mask.push(r.inflate(1.0, 1.0));
        }
        if let Some(r) = check(&mut scene, name, &skip, &[], FIELD) {
            failures.push(r);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
