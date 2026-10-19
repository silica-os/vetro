//! Kit parity of the buttons besides `Buttons/Push`: icon, help, link, gradient, toolbar,
//! pop-up and pull-down.

use vetro_lib::amalfi::buttons::{
    GradientButtons, HelpButton, IconButton, LinkButton, MenuButton, ToolbarButton,
};
use vetro_lib::amalfi::foundation::ControlSize;
use vetro_test::{KitMaster, Scene, Tolerance, center_of};

/// Small controls: Sketch antialiases the edges of small controls a little wider than Skia.
const CONTROLS: Tolerance = Tolerance {
    mean: 2.5,
    max: 100,
    mask_text: true,
};

fn size(name: &str) -> ControlSize {
    match name {
        "Mini" => ControlSize::Mini,
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

fn state(scene: &mut Scene, state: &str, at: &str) {
    match (state, center_of(&scene.master, at)) {
        ("Hover", Some(p)) => scene.hover_at(p),
        ("Pressed", Some(p)) => scene.press_at(p),
        ("Focused" | "Disabled", _) => scene.set_state(state),
        _ => {}
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

/// Kit defect: the Dark/Glass/Regular shared style blurs 10 where materials.md gives 6.
fn glass_devs(scene: &Scene) -> &'static [&'static str] {
    if scene.master.dark() {
        &["Background#blurs"]
    } else {
        &[]
    }
}

#[test]
fn icon_and_help_buttons_match_the_kit() {
    let mut names = KitMaster::names("Buttons/Icon");
    names.extend(KitMaster::names("Buttons/Help"));
    assert_eq!(names.len(), 90, "{} masters", names.len());
    let mut failures = Vec::new();
    for name in &names {
        let parts: Vec<&str> = name.split('/').collect();
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        let st;
        let at;
        if parts.get(1) == Some(&"Help") {
            st = parts.get(2).copied().unwrap_or("Rest");
            at = "Help";
            scene.mount(|ui| ui.create(HelpButton::default()));
        } else {
            let [_, _, shape, sz, s, _app] = parts[..] else {
                continue;
            };
            st = s;
            at = "Glyph Plus";
            let mut b = IconButton::new("plus").size(size(sz));
            if shape == "Square" {
                b = b.square();
            }
            scene.mount(|ui| ui.create(b));
        }
        state(&mut scene, st, at);
        if let Some(f) = check(&mut scene, name, &[], &[], CONTROLS) {
            failures.push(f);
        }
    }
    finish(failures, names.len());
}

#[test]
fn links_match_the_kit() {
    let names = KitMaster::names("Buttons/Link");
    assert_eq!(names.len(), 10, "{names:?}");
    let mut failures = Vec::new();
    for name in &names {
        let parts: Vec<&str> = name.split('/').collect();
        let [_, _, _, st, _app] = parts[..] else {
            continue;
        };
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        let label = text_of(&scene, "Label");
        scene.mount(|ui| ui.create(LinkButton::new(&label)));
        state(&mut scene, st, "Label");
        if let Some(f) = check(&mut scene, name, &[], &[], CONTROLS) {
            failures.push(f);
        }
    }
    finish(failures, names.len());
}

#[test]
fn gradient_buttons_match_the_kit() {
    let names = KitMaster::names("Buttons/Gradient");
    assert_eq!(names.len(), 10, "{names:?}");
    let mut failures = Vec::new();
    for name in &names {
        let parts: Vec<&str> = name.split('/').collect();
        let [_, _, _, st, _app] = parts[..] else {
            continue;
        };
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        scene.mount(|ui| ui.create(GradientButtons::new()));
        state(&mut scene, st, "Glyph Plus");
        // Each half is a button of its own: the hover wash and the pressed fill cover the "+" half (outer corners rounded), where the kit draws them over the
        // whole control.
        let (skip, devs): (&[&str], &[&str]) = match st {
            "Hover" => (&[], &["State#frame", "State#radii"]),
            "Pressed" => (&["State"], &["Background#fills"]),
            _ => (&[], &[]),
        };
        if st != "Rest" && st != "Disabled" {
            // The "-" half shows the rest look.
            scene
                .extra_mask
                .push(vetro_lib::geom::Rect::new(28.0, -4.0, 32.0, 32.0));
        }
        if let Some(f) = check(&mut scene, name, skip, devs, CONTROLS) {
            failures.push(f);
        }
    }
    finish(failures, names.len());
}

#[test]
fn toolbar_buttons_match_the_kit() {
    let names = KitMaster::names("Buttons/Toolbar");
    assert_eq!(names.len(), 24, "{} masters", names.len());
    let mut failures = Vec::new();
    for name in &names {
        let parts: Vec<&str> = name.split('/').collect();
        let [_, _, kind, st, _app] = parts[..] else {
            continue;
        };
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        let symbol = text_of(&scene, "Glyph Gear");
        let mut b = ToolbarButton::new(&symbol).active(true);
        match kind {
            "Icon + Label" => b = b.labelled(&text_of(&scene, "Label")),
            "Icon Small" => b = b.small(),
            _ => {}
        }
        scene.mount(|ui| ui.create(b));
        state(&mut scene, st, "Glyph Gear");
        let devs = glass_devs(&scene);
        if let Some(f) = check(&mut scene, name, &[], devs, CONTROLS) {
            failures.push(f);
        }
    }
    finish(failures, names.len());
}

#[test]
fn pop_up_and_pull_down_buttons_match_the_kit() {
    let names: Vec<String> = KitMaster::names("Buttons/Pop-up")
        .into_iter()
        .chain(KitMaster::names("Buttons/Pull-down"))
        .filter(|n| !n.contains("/Open/") && !n.contains("/App/"))
        .collect();
    assert_eq!(names.len(), 80, "{} masters", names.len());
    let mut failures = Vec::new();
    for name in &names {
        let parts: Vec<&str> = name.split('/').collect();
        let [_, kind, sz, st, _app] = parts[..] else {
            continue;
        };
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        let label = text_of(&scene, "Label");
        let b = if kind == "Pop-up" {
            MenuButton::pop_up(&[label.as_str(), "Orange"], 0)
        } else {
            MenuButton::pull_down(&label, &["Copy", "Paste"])
        }
        .size(size(sz));
        scene.mount(|ui| ui.create(b));
        state(&mut scene, st, "Label");
        if let Some(f) = check(&mut scene, name, &[], &[], CONTROLS) {
            failures.push(f);
        }
    }
    finish(failures, names.len());
}
