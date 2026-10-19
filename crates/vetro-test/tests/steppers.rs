//! Kit parity of `Steppers` (the stepper; the value field and the stepper with a field are
//! in `fields.rs`).

use vetro_lib::amalfi::foundation::ControlSize;
use vetro_lib::amalfi::steppers::Stepper;
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
