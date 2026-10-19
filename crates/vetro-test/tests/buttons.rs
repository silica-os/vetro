//! Kit parity of `Buttons/Push`.

use vetro_lib::amalfi::button::{PushButton, PushVariant};
use vetro_lib::amalfi::foundation::ControlSize;
use vetro_test::{KitMaster, Scene, Tolerance};

fn variant(name: &str) -> PushVariant {
    match name {
        "Primary" => PushVariant::Primary,
        "Destructive" => PushVariant::Destructive,
        "Borderless" => PushVariant::Borderless,
        "Glass" => PushVariant::Glass,
        _ => PushVariant::Secondary,
    }
}

fn size(name: &str) -> ControlSize {
    match name {
        "Mini" => ControlSize::Mini,
        "Small" => ControlSize::Small,
        "Large" => ControlSize::Large,
        _ => ControlSize::Regular,
    }
}

#[test]
fn push_buttons_match_the_kit() {
    let names = KitMaster::names("Buttons/Push");
    assert!(names.len() >= 160, "{} masters", names.len());
    let mut failures = Vec::new();
    for name in &names {
        let parts: Vec<&str> = name.split('/').collect();
        let [_, _, v, s, state, _app] = parts[..] else {
            continue;
        };
        let mut scene = Scene::load(name).unwrap();
        let (v, s) = (variant(v), size(s));
        scene.mount(|ui| ui.create(PushButton::new("Button").variant(v).size(s)));
        scene.set_state(state);
        // Kit defect: the Dark/Glass/Regular shared style blurs 10 where materials.md (which
        // wins) gives Regular 6 (docs/kit-issues.md).
        let dark_regular_glass = name.contains("/Glass/")
            && name.ends_with("/Dark")
            && matches!(state, "Rest" | "Focused" | "Disabled");
        let deviations: &[&str] = if dark_regular_glass {
            &["Background#blurs"]
        } else {
            &[]
        };
        let report = scene.auto_geometry(deviations);
        if !report.is_empty() {
            failures.push(format!("{name}:\n    {}", report.join("\n    ")));
            continue;
        }
        // A hovered mini capsule stacks two antialiased edges (fill and wash) on a 20 pt
        // shape, where the edge is a large share of the pixels: Sketch's wider antialiasing
        // counts twice (mean 1.70 measured).
        let tol = if name.contains("/Mini/Hover/") {
            Tolerance {
                mean: 1.8,
                ..Tolerance::CONTROLS
            }
        } else {
            Tolerance::CONTROLS
        };
        if let Err(e) = scene.pixels(tol) {
            failures.push(format!("{name}: {e}"));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} masters differ:\n{}",
        failures.len(),
        names.len(),
        failures.join("\n")
    );
}
