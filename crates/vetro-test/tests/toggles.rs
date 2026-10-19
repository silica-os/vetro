//! Kit parity of `Toggles`: checkboxes, radio buttons, their labelled rows and groups.

use vetro_lib::amalfi::foundation::ControlSize;
use vetro_lib::amalfi::switch::{Switch, SwitchMetrics};
use vetro_lib::amalfi::toggles::{Toggle, ToggleKind, ToggleState, toggle_group};
use vetro_lib::core::{Button, Mods, Prop, Value};
use vetro_test::{KitMaster, Scene, Tolerance};

/// Small controls: Sketch and Skia antialias tight smooth corners a little differently (the
/// corner of a 1 pt accent border on white differs by up to ~100 on one channel, on a few
/// pixels; the means stay at 1-2), as silica_ui measured on the same masters.
const SMALL: Tolerance = Tolerance {
    mean: 3.5,
    max: 100,
    mask_text: true,
};

/// A 1 pt accent border meeting white (hovered or pressed off boxes): the edge pixels under
/// the border are antialiased by the fill and again by the border.
const ACCENT_RIM: Tolerance = Tolerance {
    mean: 4.0,
    max: 128,
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

fn state_of(name: &str) -> ToggleState {
    match name {
        "On" => ToggleState::On,
        "Mixed" => ToggleState::Mixed,
        _ => ToggleState::Off,
    }
}

fn finish(failures: Vec<String>, count: usize) {
    assert!(
        failures.is_empty(),
        "{} of {count} masters differ:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

fn run(kind: ToggleKind, prefix: &str, min: usize) {
    let names = KitMaster::names(prefix);
    assert!(names.len() >= min, "{} masters", names.len());
    let label = match kind {
        ToggleKind::Checkbox => "Show hidden files",
        ToggleKind::Radio => "Sort by name",
    };
    let mut failures = Vec::new();
    for name in &names {
        let parts: Vec<&str> = name.split('/').collect();
        let [_, _, a, b, c, _app] = parts[..] else {
            continue;
        };
        // `Checkbox/<value>/<size>/<state>` or `Checkbox/With Label/<value>/<state>`.
        let (toggle, kit_state, sized) = if a == "With Label" {
            (
                match kind {
                    ToggleKind::Checkbox => Toggle::checkbox(),
                    ToggleKind::Radio => Toggle::radio(),
                }
                .label(label)
                .state(state_of(b)),
                c,
                false,
            )
        } else {
            (
                match kind {
                    ToggleKind::Checkbox => Toggle::checkbox(),
                    ToggleKind::Radio => Toggle::radio(),
                }
                .size(size(b))
                .state(state_of(a)),
                c,
                true,
            )
        };
        let Ok(mut scene) = Scene::load(name) else {
            failures.push(format!("{name}: cannot load"));
            continue;
        };
        scene.mount(|ui| ui.create(toggle));
        scene.set_state(kit_state);
        // Kit defect: the Small radio is a circle (radius 8) but its hover wash and focus
        // halo keep the old radius 7 (docs/kit-issues.md).
        let stale = kind == ToggleKind::Radio && sized && b == "Small";
        let deviations: &[&str] = match (stale, kit_state, a) {
            (true, "Focused", _) => &["Focus Halo#radii"],
            (true, "Hover", "On") => &["State#radii"],
            _ => &[],
        };
        let report = scene.auto_geometry(deviations);
        if !report.is_empty() {
            failures.push(format!("{name}:\n    {}", report.join("\n    ")));
            continue;
        }
        let off = if sized { a == "Off" } else { b == "Off" };
        let tol = if kit_state == "Hover" && off {
            ACCENT_RIM
        } else if !deviations.is_empty() {
            Tolerance {
                mean: 5.5,
                max: 100,
                mask_text: true,
            }
        } else {
            SMALL
        };
        if let Err(e) = scene.pixels(tol) {
            failures.push(format!("{name}: {e}"));
        }
    }
    finish(failures, names.len());
}

#[test]
fn checkboxes_match_the_kit() {
    run(ToggleKind::Checkbox, "Toggles/Checkbox", 120);
}

#[test]
fn radios_match_the_kit() {
    run(ToggleKind::Radio, "Toggles/Radio", 80);
}

#[test]
fn checkbox_group_matches_the_kit() {
    let mut failures = Vec::new();
    for name in KitMaster::names("Toggles/Checkbox Group") {
        let Ok(mut scene) = Scene::load(&name) else {
            continue;
        };
        scene.mount(|ui| {
            let rows: Vec<_> = [
                ("Select all", ToggleState::Mixed),
                ("Show hidden files", ToggleState::On),
                ("Open folders in tabs", ToggleState::Off),
            ]
            .into_iter()
            .map(|(l, s)| {
                let r = ui.create(Toggle::checkbox().label(l).state(s));
                let _ = ui.set(r, Prop::Identifier, Value::from(l));
                r
            })
            .collect();
            toggle_group(ui, Some("Browsing"), &rows)
        });
        // Kit defect: the PNG has a 6 pt empty margin below the group.
        scene.png_margin = true;
        let report = scene.auto_geometry(&[]);
        if !report.is_empty() {
            failures.push(format!("{name}:\n    {}", report.join("\n    ")));
            continue;
        }
        // The kit's last label overflows its text frame, which the text mask leaves out.
        if let Err(e) = scene.pixels(Tolerance {
            mean: 3.5,
            max: 100,
            mask_text: true,
        }) {
            failures.push(format!("{name}: {e}"));
        }
    }
    finish(failures, 2);
}

#[test]
fn radio_group_matches_the_kit() {
    let mut failures = Vec::new();
    for name in KitMaster::names("Toggles/Radio Group") {
        let Ok(mut scene) = Scene::load(&name) else {
            continue;
        };
        scene.mount(|ui| {
            let rows: Vec<_> = [
                ("Sort by name", ToggleState::Off),
                ("Sort by date", ToggleState::On),
                ("Sort by size", ToggleState::Off),
            ]
            .into_iter()
            .map(|(l, s)| {
                let r = ui.create(Toggle::radio().label(l).state(s));
                let _ = ui.set(r, Prop::Identifier, Value::from(l));
                r
            })
            .collect();
            toggle_group(ui, Some("Order"), &rows)
        });
        // Kit defects: the rows keep stale instance widths (140; the widest row is 124, as
        // `Toggles/Radio/With Label`), and the heading's frame is a fixed 40 for "Order"
        // (36 on the 8 pt grid). Labels sit at the same x either way.
        let report = scene.auto_geometry(&["#frame", "Heading#frame"]);
        if !report.is_empty() {
            failures.push(format!("{name}:\n    {}", report.join("\n    ")));
        }
    }
    finish(failures, 2);
}

/// Hover and press of switches: the same edge antialiasing as small controls.
const SWITCH: Tolerance = Tolerance {
    mean: 3.5,
    max: 100,
    mask_text: true,
};

fn switch_for(sz: ControlSize, on: bool) -> Switch {
    Switch::new().size(sz).on(on)
}

#[test]
fn switches_match_the_kit() {
    let names = KitMaster::names("Toggles/Switch");
    assert!(names.len() >= 92, "{} masters", names.len());
    let mut failures = Vec::new();
    for name in &names {
        let parts: Vec<&str> = name.split('/').collect();
        let [_, _, a, b, c, _app] = parts[..] else {
            continue;
        };
        let Ok(mut scene) = Scene::load(name) else {
            failures.push(format!("{name}: cannot load"));
            continue;
        };
        if a == "With Label" {
            let sw = Switch::new().on(b == "On").label("Wi-Fi");
            let master_size = scene.master.size();
            scene.mount_sized(master_size, |ui| ui.create(sw));
            scene.set_state(c);
        } else if c == "Dragging" {
            let sz = size(b);
            let m = SwitchMetrics::of(sz);
            let on = a == "On";
            let track = scene.master.rect_in_master("Track").unwrap_or_default();
            let ms = scene.master.size();
            let pad = vetro_lib::geom::Insets::new(
                track.y,
                track.x,
                ms.height - track.y - track.height,
                ms.width - track.x - track.width,
            );
            let v = scene.mount(|ui| {
                let v = ui.create(switch_for(sz, on));
                let _ = ui.set(v, Prop::Padding, Value::Insets(pad));
                v
            });
            // Press the thumb and drag away and back: the lens follows the pointer 1:1.
            let r = scene.ui.tree.window_rect(v);
            let start = vetro_lib::geom::Point::new(
                r.x + track.x + m.rest_x(on) + m.thumb / 2.0,
                r.y + track.y + m.track.height / 2.0,
            );
            let w = scene.window;
            scene.ui.pointer_move(w, start, Mods::empty());
            scene.ui.pointer_down(w, start, Button::Left, Mods::empty());
            let away = vetro_lib::geom::Point::new(start.x + 4.0, start.y);
            scene.ui.pointer_move(w, away, Mods::empty());
            scene.ui.pointer_move(w, start, Mods::empty());
            scene.settle();
            // The kit's lens refracts the track beneath it; the region is left out of the
            // pixels (the layer's geometry is checked).
            if let Some(l) = scene.master.rect_in_master("Lens") {
                scene.extra_mask.push(l.inflate(1.0, 1.0));
            }
        } else {
            let sz = size(b);
            let sw = switch_for(sz, a == "On");
            scene.mount(|ui| ui.create(sw));
            scene.set_state(c);
        }
        let report = scene.auto_geometry(&[]);
        if !report.is_empty() {
            failures.push(format!("{name}:\n    {}", report.join("\n    ")));
            continue;
        }
        if let Err(e) = scene.pixels(SWITCH) {
            failures.push(format!("{name}: {e}"));
        }
    }
    finish(failures, names.len());
}
