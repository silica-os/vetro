//! Kit parity of `Disclosure`: the triangle, the button, the group header and the group.

use vetro_lib::amalfi::disclosure::{
    Disclosure, DisclosureGroup, DisclosureStyle, disclosure_group,
};
use vetro_lib::amalfi::text::{Text, TextRole};
use vetro_lib::amalfi::tokens::typography;
use vetro_lib::geom::Point;
use vetro_test::{KitMaster, Scene, Tolerance};

/// Small round controls: the edge pixels are a large share of the canvas.
const SMALL: Tolerance = Tolerance {
    mean: 3.0,
    max: 100,
    mask_text: true,
};

fn finish(failures: Vec<String>, count: usize) {
    assert!(
        failures.is_empty(),
        "{} of {count} masters differ:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

fn check(scene: &mut Scene, name: &str, tol: Tolerance) -> Option<String> {
    let report = scene.auto_geometry(&[]);
    if !report.is_empty() {
        return Some(format!("{name}:\n    {}", report.join("\n    ")));
    }
    scene.pixels(tol).err().map(|e| format!("{name}: {e}"))
}

#[test]
fn triangles_and_buttons_match_the_kit() {
    let mut names = KitMaster::names("Disclosure/Triangle");
    names.extend(KitMaster::names("Disclosure/Button"));
    assert_eq!(names.len(), 32, "{} masters", names.len());
    let mut failures = Vec::new();
    for name in &names {
        let parts: Vec<&str> = name.split('/').collect();
        let [_, kind, value, state, _app] = parts[..] else {
            continue;
        };
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        let style = if kind == "Button" {
            DisclosureStyle::Button
        } else {
            DisclosureStyle::Triangle
        };
        let d = Disclosure::new().style(style).expanded(value == "Expanded");
        scene.mount(|ui| ui.create(d));
        if state != "Rest" {
            scene.set_state(state);
        }
        if let Some(f) = check(&mut scene, name, SMALL) {
            failures.push(f);
        }
    }
    finish(failures, names.len());
}

#[test]
fn group_headers_match_the_kit() {
    let names = KitMaster::names("Disclosure/Group Header");
    assert_eq!(names.len(), 8, "{names:?}");
    let mut failures = Vec::new();
    for name in &names {
        let parts: Vec<&str> = name.split('/').collect();
        let [_, _, value, state, _app] = parts[..] else {
            continue;
        };
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        let title = scene
            .master
            .layer("Title")
            .and_then(|l| l.text.clone())
            .unwrap_or_default();
        scene.mount(|ui| ui.create(DisclosureGroup::new(&title).expanded(value == "Open")));
        if state == "Hover" {
            scene.hover_at(Point::new(150.0, 14.0));
        }
        if let Some(f) = check(&mut scene, name, SMALL) {
            failures.push(f);
        }
    }
    finish(failures, names.len());
}

#[test]
fn groups_match_the_kit() {
    let names = KitMaster::names("Disclosure/Group/");
    assert_eq!(names.len(), 4, "{names:?}");
    let mut failures = Vec::new();
    for name in &names {
        let parts: Vec<&str> = name.split('/').collect();
        let [_, _, value, _state, _app] = parts[..] else {
            continue;
        };
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        let text = |layer: &str| scene.master.layer(layer).and_then(|l| l.text.clone());
        let title = scene
            .master
            .layer("Header/Title")
            .and_then(|l| l.text.clone())
            .unwrap_or_default();
        let body = text("Content").unwrap_or_default();
        let open = value == "Open";
        scene.mount(|ui| {
            let content = ui.create(
                Text::new(&body)
                    .token(&typography::CALLOUT)
                    .role(TextRole::Secondary)
                    .layer("Content")
                    .wrapping(),
            );
            disclosure_group(ui, &title, open, content)
        });
        // A closed group keeps its content out of the picture.
        let skip: &[&str] = if open { &[] } else { &["Content"] };
        let report = scene.auto_geometry_except(skip, &[]);
        if !report.is_empty() {
            failures.push(format!("{name}:\n    {}", report.join("\n    ")));
            continue;
        }
        if let Err(e) = scene.pixels(SMALL) {
            failures.push(format!("{name}: {e}"));
        }
    }
    finish(failures, names.len());
}
