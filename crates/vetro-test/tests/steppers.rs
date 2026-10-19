//! Kit parity of `Steppers` (the stepper; the value field and the stepper with a field are
//! in `fields.rs`).

use vetro_lib::amalfi::fields::TextField;
use vetro_lib::amalfi::foundation::ControlSize;
use vetro_lib::amalfi::steppers::{Stepper, stepper_with_field};
use vetro_test::{KitMaster, Scene, Tolerance, center_of};

/// Small bezels: the edge pixels are a large share of the canvas.
const SMALL: Tolerance = Tolerance {
    mean: 3.0,
    max: 100,
    mask_text: true,
};

#[test]
fn steppers_match_the_kit() {
    let names = KitMaster::names("Steppers/Stepper");
    assert_eq!(names.len(), 56, "{} masters", names.len());
    let mut failures = Vec::new();
    for name in &names {
        let parts: Vec<&str> = name.split('/').collect();
        let [_, _, sz, state, _app] = parts[..] else {
            continue;
        };
        let size = match sz {
            "Mini" => ControlSize::Mini,
            "Small" => ControlSize::Small,
            "Large" => ControlSize::Large,
            _ => ControlSize::Regular,
        };
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        scene.mount(|ui| ui.create(Stepper::new().range(0.0, 1e9).value(5e8).size(size)));
        let up = center_of(&scene.master, "Glyph Chevron Up");
        let down = center_of(&scene.master, "Glyph Chevron Down");
        match (state, up, down) {
            ("Hover Up", Some(p), _) => scene.hover_at(p),
            ("Hover Down", _, Some(p)) => scene.hover_at(p),
            ("Pressed Up", Some(p), _) => scene.press_at(p),
            ("Pressed Down", _, Some(p)) => scene.press_at(p),
            ("Focused" | "Disabled", _, _) => scene.set_state(state),
            _ => {}
        }
        let report = scene.auto_geometry(&[]);
        if !report.is_empty() {
            failures.push(format!("{name}:\n    {}", report.join("\n    ")));
            continue;
        }
        if let Err(e) = scene.pixels(SMALL) {
            failures.push(format!("{name}: {e}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn value_fields_and_steppers_with_fields_match_the_kit() {
    let mut names = KitMaster::names("Steppers/Value Field");
    names.extend(KitMaster::names("Steppers/With Field"));
    assert_eq!(names.len(), 12, "{} masters", names.len());
    let mut failures = Vec::new();
    for name in &names {
        let parts: Vec<&str> = name.split('/').collect();
        let [_, kind, _sz, state, _app] = parts[..] else {
            continue;
        };
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };

        let focus = if kind == "Value Field" {
            scene.mount(|ui| ui.create(TextField::value().with_text("12")))
        } else {
            let row = scene
                .mount(|ui| stepper_with_field(ui, Stepper::new().range(0.0, 1e9).value(12.0)));
            scene.ui.find(row, "Value field").unwrap_or(row)
        };
        match state {
            "Focused" => {
                let mut a = scene.ui.appearance;
                a.full_keyboard_access = true;
                scene.ui.set_appearance(a);
                scene.ui.set_focus(focus, true);
                scene.settle();
            }
            "Disabled" => scene.set_state("Disabled"),
            _ => {}
        }
        // The caret blinks and sits after the text: masked in the kit and in ours.
        for l in scene.layers() {
            if l.path.ends_with("Caret") {
                scene.extra_mask.push(l.frame.inflate(1.0, 1.0));
            }
        }
        let report = scene.auto_geometry_except(&["Caret"], &[]);
        if !report.is_empty() {
            failures.push(format!("{name}:\n    {}", report.join("\n    ")));
            continue;
        }
        if let Err(e) = scene.pixels(Tolerance {
            mean: 2.0,
            max: 180,
            mask_text: true,
        }) {
            failures.push(format!("{name}: {e}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
