//! Kit parity of `Sliders`: thumbs, linear sliders, the dial.

use vetro_lib::amalfi::sliders::{Dial, LensThumb, Slider, SliderThumb, ThumbLook};
use vetro_lib::core::{Prop, Value};
use vetro_lib::geom::{Insets, Point};
use vetro_test::{KitMaster, Scene, Tolerance, center_of};

/// Thin shapes (a 4 pt track, 20 pt discs): edges are most of their pixels.
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

/// Puts the mounted slider in a kit state: the pointer over the thumb for Hover and Pressed.
fn put_in_state(scene: &mut Scene, state: &str) {
    let thumb =
        center_of(&scene.master, "Thumb").or_else(|| center_of(&scene.master, "Slider/Thumb"));
    match (state, thumb) {
        ("Hover", Some(p)) => scene.hover_at(p),
        ("Pressed", Some(p)) => scene.press_at(p),
        ("Rest", _) => {}
        _ => scene.set_state(state),
    }
}

fn run(name: &str, scene: &mut Scene, devs: &[&str], tol: Tolerance) -> Option<String> {
    let report = scene.auto_geometry(devs);
    if !report.is_empty() {
        return Some(format!("{name}:\n    {}", report.join("\n    ")));
    }
    scene.pixels(tol).err().map(|e| format!("{name}: {e}"))
}

#[test]
fn thumbs_match_the_kit() {
    let names = KitMaster::names("Sliders/Thumb");
    assert_eq!(names.len(), 12, "{names:?}");
    let mut failures = Vec::new();
    for name in &names {
        let parts: Vec<&str> = name.split('/').collect();
        let [_, _, kind, state, _app] = parts[..] else {
            continue;
        };
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        let mut devs: Vec<&str> = Vec::new();
        if kind == "Lens" {
            scene.mount(|ui| ui.create(LensThumb));
            // The lens rule takes half the height minus 1, the kit half.
            devs.push("Lens#radii");
            if let Some(r) = scene.master.rect_in_master("Lens") {
                scene.extra_mask.push(r.inflate(1.0, 1.0));
            }
        } else {
            let look = ThumbLook {
                pressed: state == "Pressed",
                hovered: state == "Hover",
                focused: false,
            };
            scene.mount(|ui| ui.create(SliderThumb { look }));
            match state {
                "Focused" => {
                    // The halo is the focus ring: drawn by the look in a standalone thumb.
                    let look = ThumbLook {
                        focused: true,
                        ..ThumbLook::default()
                    };
                    scene.mount(|ui| ui.create(SliderThumb { look }));
                }
                "Disabled" => scene.set_state("Disabled"),
                _ => {}
            }
        }
        if let Some(f) = run(name, &mut scene, &devs, SMALL) {
            failures.push(f);
        }
    }
    finish(failures, names.len());
}

#[test]
fn horizontal_and_vertical_sliders_match_the_kit() {
    let mut names = KitMaster::names("Sliders/Horizontal");
    names.extend(KitMaster::names("Sliders/Vertical"));
    assert_eq!(names.len(), 42, "{} masters", names.len());
    let mut failures = Vec::new();
    for name in &names {
        let parts: Vec<&str> = name.split('/').collect();
        let [_, axis, kind, state, _app] = parts[..] else {
            continue;
        };
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        let mut slider = Slider::new().value(0.6);
        if axis == "Vertical" {
            slider = slider.vertical();
        }
        match kind {
            "Ticks" => slider = slider.divisions(10).ticks(),
            "Glyphs" => slider = slider.glyphs("volume-0", "volume-2"),
            _ => {}
        }
        let mut devs: Vec<&str> = Vec::new();
        if kind == "Dragging" {
            // Room for the lens: the track sits in the middle of the 28 pt box.
            let track = scene.master.rect_in_master("Track").unwrap_or_default();
            let ms = scene.master.size();
            let pad = Insets::new(track.y - 8.0, 0.0, ms.height - (track.y - 8.0) - 20.0, 0.0);
            let v = scene.mount(|ui| {
                let v = ui.create(slider);
                let _ = ui.set(v, Prop::Padding, Value::Insets(pad));
                v
            });
            // Press the thumb, drag away and back: the lens follows the pointer.
            let r = scene.ui.tree.window_rect(v);
            let start = Point::new(r.x + 118.0, r.y + track.y - 8.0 + 10.0);
            let w = scene.window;
            let m = vetro_lib::core::Mods::empty();
            scene.ui.pointer_move(w, start, m);
            scene
                .ui
                .pointer_down(w, start, vetro_lib::core::Button::Left, m);
            scene
                .ui
                .pointer_move(w, Point::new(start.x + 4.0, start.y), m);
            scene.ui.pointer_move(w, start, m);
            scene.settle();
            devs.push("Lens thumb/Lens#radii");
            if let Some(l) = scene.master.rect_in_master("Lens thumb/Lens") {
                scene.extra_mask.push(l.inflate(1.0, 1.0));
            }
        } else {
            scene.mount(|ui| ui.create(slider));
            if kind == "Glyphs" {
                // The glyph boxes are rasterised by CoreText in the kit.
                for g in ["Minimum glyph", "Maximum glyph"] {
                    if let Some(r) = scene.master.rect_in_master(g) {
                        scene.extra_mask.push(r);
                    }
                }
            }
            put_in_state(&mut scene, state);
        }
        if let Some(f) = run(name, &mut scene, &devs, SMALL) {
            failures.push(f);
        }
    }
    finish(failures, names.len());
}

#[test]
fn dials_match_the_kit() {
    let names = KitMaster::names("Sliders/Circular");
    assert_eq!(names.len(), 10, "{names:?}");
    let mut failures = Vec::new();
    for name in &names {
        let parts: Vec<&str> = name.split('/').collect();
        let [_, _, _, state, _app] = parts[..] else {
            continue;
        };
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        scene.mount(|ui| ui.create(Dial::new().value(0.625)));
        put_in_state(&mut scene, state);
        if let Some(f) = run(name, &mut scene, &[], SMALL) {
            failures.push(f);
        }
    }
    finish(failures, names.len());
}
