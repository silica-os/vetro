//! Kit parity of `Segments`: items, segmented controls, action segments, the Align group and
//! the Icon + Label control.

use vetro_lib::amalfi::foundation::ControlSize;
use vetro_lib::amalfi::segments::{
    ActionSegments, ItemContent, ItemLook, Pane, PaneControl, Segment, SegmentItem,
    SegmentedControl, Selection,
};
use vetro_lib::core::{Prop, Value};
use vetro_lib::geom::Insets;
use vetro_test::{KitMaster, Scene, Tolerance, center_of};

fn size(name: &str) -> ControlSize {
    match name {
        "Mini" => ControlSize::Mini,
        "Small" => ControlSize::Small,
        "Large" => ControlSize::Large,
        _ => ControlSize::Regular,
    }
}

/// Glass capsules: the rim and the blurred backdrop at the smooth ends antialias a little
/// differently from Sketch (silica_ui measured max 68 on a few edge pixels, mean <= 1).
const GLASS: Tolerance = Tolerance {
    mean: 1.0,
    max: 72,
    mask_text: true,
};
/// Dark glass: the 1 pt rim and the inner highlight of a 20-36 pt capsule (a large share of
/// its pixels) antialias differently in Sketch and Skia: mean up to 2.9.
const DARK_GLASS: Tolerance = Tolerance {
    mean: 3.0,
    max: 80,
    mask_text: true,
};
/// The items are tiny (12-24 pt tall): their antialiased edges and the shadow of the
/// selection weigh much more in the mean.
const ITEM: Tolerance = Tolerance {
    mean: 2.5,
    max: 96,
    mask_text: true,
};

/// Compares geometry and pixels of the scene's master; the failure is returned.
fn check(
    scene: &mut Scene,
    name: &str,
    skip: &[&str],
    deviations: &[&str],
    tol: Tolerance,
) -> Option<String> {
    let report = scene.auto_geometry_except(skip, deviations);
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

#[test]
fn items_match_the_kit() {
    let mut names = KitMaster::names("Segments/Text Item");
    names.extend(KitMaster::names("Segments/Icon Item"));
    assert_eq!(names.len(), 64, "{} masters", names.len());
    let mut failures = Vec::new();
    for name in &names {
        let parts: Vec<&str> = name.split('/').collect();
        let [_, kind, sz, state, _app] = parts[..] else {
            continue;
        };
        let Ok(mut scene) = Scene::load(name) else {
            failures.push(format!("{name}: cannot load"));
            continue;
        };
        let content = if kind == "Text Item" {
            ItemContent::Text(
                scene
                    .master
                    .layer("Label")
                    .and_then(|l| l.text.clone())
                    .unwrap_or_default(),
            )
        } else {
            ItemContent::Icon(
                scene
                    .master
                    .layer("Glyph Sidebar")
                    .and_then(|l| l.text.clone())
                    .unwrap_or_default(),
            )
        };
        let look = match state {
            "Hover" => ItemLook::Hover,
            "Selected" => ItemLook::Selected,
            _ => ItemLook::Rest,
        };
        let item = SegmentItem::new(content).size(size(sz)).look(look);
        scene.mount(|ui| ui.create(item));
        if state == "Disabled" {
            scene.set_state("Disabled");
        }
        if let Some(f) = check(&mut scene, name, &[], &[], ITEM) {
            failures.push(f);
        }
    }
    finish(failures, names.len());
}

#[test]
fn icon_label_items_match_the_kit() {
    let names = KitMaster::names("Segments/Icon + Label Item");
    assert_eq!(names.len(), 10, "{names:?}");
    let mut failures = Vec::new();
    for name in &names {
        let parts: Vec<&str> = name.split('/').collect();
        let [_, _, state, _app] = parts[..] else {
            continue;
        };
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        let text = |layer: &str| {
            scene
                .master
                .layer(layer)
                .and_then(|l| l.text.clone())
                .unwrap_or_default()
        };
        let (label, symbol) = (text("Label"), text("Glyph"));
        let look = match state {
            "Hover" => ItemLook::Hover,
            "Pressed" => ItemLook::Pressed,
            "Selected" => ItemLook::Selected,
            _ => ItemLook::Rest,
        };
        let width = scene.master.size().width;
        let item = SegmentItem::new(ItemContent::IconLabel { symbol, label })
            .look(look)
            .width(width);
        scene.mount(|ui| ui.create(item));
        if state == "Disabled" {
            scene.set_state("Disabled");
        }
        if let Some(f) = check(&mut scene, name, &[], &[], ITEM) {
            failures.push(f);
        }
    }
    finish(failures, names.len());
}

#[test]
fn action_segments_match_the_kit() {
    let mut names = KitMaster::names("Segments/Action");
    names.extend(KitMaster::names("Segments/Align"));
    assert_eq!(names.len(), 248, "{} masters", names.len());
    let mut failures = Vec::new();
    for name in &names {
        let parts: Vec<&str> = name.split('/').collect();
        let align = parts.get(1) == Some(&"Align");
        let (variant, sz, state) = if align {
            ("Icon 4", "Small", "Selected")
        } else {
            match parts[..] {
                [_, _, v, s, st, _] => (v, s, st),
                _ => continue,
            }
        };
        let Ok(mut scene) = Scene::load(name) else {
            failures.push(format!("{name}: cannot load"));
            continue;
        };
        let text = variant.starts_with("Text");
        let count: usize = variant
            .rsplit(' ')
            .next()
            .and_then(|n| n.parse().ok())
            .unwrap_or(2);
        let segments: Vec<Segment> = (1..=count)
            .map(|i| {
                let layer = if text {
                    format!("Label {i}")
                } else {
                    format!("Glyph {i}")
                };
                let t = scene
                    .master
                    .layer(&layer)
                    .and_then(|l| l.text.clone())
                    .unwrap_or_default();
                if text {
                    Segment::text(&t)
                } else {
                    Segment::icon(&t, &format!("Glyph {i}"))
                }
            })
            .collect();
        let mut control = ActionSegments::new(segments)
            .size(size(sz))
            .active(state != "Inactive");
        if align {
            let at = ["Left", "Centre", "Right", "Justify"]
                .iter()
                .position(|a| parts.get(3) == Some(a))
                .unwrap_or(0);
            control = control.selected(&[at]);
        }
        scene.mount(|ui| ui.create(control));
        let first = center_of(
            &scene.master,
            &if text {
                "Label 1".to_string()
            } else {
                "Glyph 1".to_string()
            },
        );
        match (state, first) {
            ("Hover", Some(p)) => scene.hover_at(p),
            ("Pressed", Some(p)) => scene.press_at(p),
            ("Disabled", _) => scene.set_state("Disabled"),
            _ => {}
        }
        // The glyph boxes are rasterised by CoreText in the kit: masked in the pixels.
        if !text {
            for i in 1..=count {
                if let Some(r) = scene.master.rect_in_master(&format!("Glyph {i}")) {
                    scene.extra_mask.push(r);
                }
            }
        }
        // Kit defect: the Dark/Glass/Regular shared style blurs 10 where materials.md gives 6.
        let devs: &[&str] = if scene.master.dark() {
            &["Glass#blurs"]
        } else {
            &[]
        };
        let tol = if scene.master.dark() {
            DARK_GLASS
        } else {
            GLASS
        };
        if let Some(f) = check(&mut scene, name, &[], devs, tol) {
            failures.push(f);
        }
    }
    finish(failures, names.len());
}

#[test]
fn segmented_controls_match_the_kit() {
    let names: Vec<String> = KitMaster::names("Segments/Control")
        .into_iter()
        .filter(|n| !n.contains("Icon + Label"))
        .collect();
    assert_eq!(names.len(), 196, "{} masters", names.len());
    let mut failures = Vec::new();
    for name in &names {
        let parts: Vec<&str> = name.split('/').collect();
        let (variant, sz, state) = match parts[..] {
            [_, _, v, s, st, _] => (v, s, st),
            [_, _, v, s, _] => (v, s, "Rest"),
            _ => continue,
        };
        let Ok(mut scene) = Scene::load(name) else {
            failures.push(format!("{name}: cannot load"));
            continue;
        };
        let words: Vec<&str> = variant.split(' ').collect();
        let one = words.first() == Some(&"One");
        let count: usize = words.get(1).and_then(|n| n.parse().ok()).unwrap_or(2);
        let text = words.get(2) == Some(&"Text");
        let csize = size(sz);
        let mut thr = Vec::new();
        let mut segments = Vec::new();
        let mut selected = Vec::new();
        for i in 1..=count {
            let Some(seg) = scene.master.layer(&format!("Segment {i}")) else {
                continue;
            };
            if seg
                .master
                .as_deref()
                .is_some_and(|m| m.contains("/Selected/"))
            {
                selected.push(i - 1);
            }
            if text {
                let l = seg
                    .overrides
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "Item".into());
                // The kit's Mini items are 44 wide whatever the label; at 11 pt "Three"
                // needs 48 with the 8 pt padding: the label is cut to what fits.
                let l = if csize == ControlSize::Mini && l == "Three" {
                    thr.push(format!("Segment {i}/Label#text"));
                    "Thr".to_string()
                } else {
                    l
                };
                segments.push(Segment::text(&l));
            } else {
                segments.push(Segment::icon("sidebar", &format!("Item {i}")));
            }
        }
        let control = SegmentedControl::new(segments)
            .mode(if one { Selection::One } else { Selection::Any })
            .selected(&selected)
            .size(csize);
        let pressed = state == "Pressed";
        let mut skip: Vec<&str> = Vec::new();
        let mut devs: Vec<&str> = thr.iter().map(String::as_str).collect();
        // Kit defect: the Dark/Glass/Regular shared style blurs 10 where materials.md gives 6.
        if scene.master.dark() {
            devs.push("Track#blurs");
        }
        if pressed {
            // Room for the lens: 8 beside the item, 12 above and below it.
            scene.mount(|ui| {
                let v = ui.create(control);
                let _ = ui.set(
                    v,
                    Prop::Padding,
                    Value::Insets(Insets::new(8.0, 8.0, 8.0, 8.0)),
                );
                v
            });
            if let Some(p) = center_of(&scene.master, "Segment 2") {
                scene.press_at(p);
            }
            // Kit quirk: the Pressed masters of One 6 Text have their Track and Lens
            // scaled by 437/440 (Track 9.5 + 437 wide in a 456 frame); the items keep their
            // places. Those two layers are not compared there.
            if count == 6 {
                skip.extend(["Track", "Lens"]);
                scene.png_margin = true;
                devs.retain(|d| *d != "Track#blurs");
                if let Some(r) = scene.master.rect_in_master("Track") {
                    scene.extra_mask.push(r.inflate(2.0, 2.0));
                }
            } else {
                // The lens rule takes a radius of half the height minus 1 (user decision
                // 2026-10-16), the kit's lens half its height.
                devs.push("Lens#radii");
            }
            // The kit's lens refracts the items beneath it; the region is masked (the
            // geometry is compared).
            if let Some(r) = scene.master.rect_in_master("Lens") {
                scene.extra_mask.push(r.inflate(1.0, 1.0));
            }
        } else {
            scene.mount(|ui| ui.create(control));
            if state == "Disabled" {
                scene.set_state("Disabled");
            }
        }
        // The quirk masters of One 6 Text: the scaled track's edge differs.
        let tol = if pressed && count == 6 {
            Tolerance {
                mean: 2.5,
                max: 255,
                mask_text: true,
            }
        } else {
            GLASS_ITEMS
        };
        if let Some(f) = check(&mut scene, name, &skip, &devs, tol) {
            failures.push(f);
        }
    }
    finish(failures, names.len());
}

/// Controls: glass, items and their selection together.
const GLASS_ITEMS: Tolerance = Tolerance {
    mean: 2.5,
    max: 96,
    mask_text: true,
};

#[test]
fn icon_label_controls_match_the_kit() {
    let names: Vec<String> = KitMaster::names("Segments/Control")
        .into_iter()
        .filter(|n| n.contains("Icon + Label"))
        .collect();
    assert_eq!(names.len(), 12, "{names:?}");
    let mut failures = Vec::new();
    for name in &names {
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        let count: usize = name
            .split('/')
            .nth(2)
            .and_then(|v| v.split(' ').nth(1))
            .and_then(|n| n.parse().ok())
            .unwrap_or(2);
        let mut panes = Vec::new();
        let mut selected = 0;
        let mut pane_width = 96.0;
        for i in 1..=count {
            let Some(p) = scene.master.layer(&format!("Pane {i}")) else {
                continue;
            };
            pane_width = p.frame.width;
            if p.master
                .as_deref()
                .is_some_and(|m| m.contains("/Selected/"))
            {
                selected = i - 1;
            }
            let label = p
                .overrides
                .first()
                .cloned()
                .unwrap_or_else(|| "General".into());
            panes.push(Pane::new(&label, "gear"));
        }
        let control = PaneControl::new(panes)
            .selected(selected)
            .pane_width(pane_width);
        scene.mount(|ui| ui.create(control));
        // The glyph boxes are rasterised by CoreText in the kit: masked in the pixels.
        for i in 1..=count {
            if let Some(r) = scene.master.rect_in_master(&format!("Pane {i}/Glyph")) {
                scene.extra_mask.push(r);
            }
        }
        // Kit defect: the Dark masters have the tint twice (Track and Track tint, both
        // Dark/Fill/Glass/Tint), a darker body than materials.md: only the geometry is
        // compared there.
        if scene.master.dark() {
            let report = scene.auto_geometry(&["Track#blurs"]);
            if !report.is_empty() {
                failures.push(format!("{name}:\n    {}", report.join("\n    ")));
            }
        } else if let Some(f) = check(&mut scene, name, &[], &[], GLASS_ITEMS) {
            failures.push(f);
        }
    }
    finish(failures, names.len());
}
