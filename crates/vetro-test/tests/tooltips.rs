//! Kit parity of `Tooltips`: the bubbles and their placement.

use vetro_lib::amalfi::buttons::IconButton;
use vetro_lib::amalfi::tooltips::{Placement, Tooltip, place};
use vetro_lib::core::{LayoutCx, Prop, Proposal, Value, Widget};
use vetro_lib::geom::{Rect, Size};
use vetro_test::{KitMaster, Scene, Tolerance};

/// Children at fixed rectangles.
struct Fixed {
    size: Size,
    rects: Vec<Rect>,
}

impl Widget for Fixed {
    fn kind(&self) -> &'static str {
        "Fixed"
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        self.size
    }

    fn place(&mut self, cx: &mut LayoutCx<'_>, _size: Size) {
        for (c, r) in cx.children().into_iter().zip(self.rects.clone()) {
            let _ = cx.measure(c, Proposal::fixed(r.size()));
            cx.place(c, r);
        }
    }
}

/// The dark bubble's rim (a 1 pt border and an inner highlight on a 24 pt shape) antialiases
/// a little differently in Sketch and Skia: edge pixels reach 61, the mean 1.8.
const DARK_SHAPES: Tolerance = Tolerance {
    mean: 2.0,
    max: 64,
    mask_text: true,
};
/// With text: two lines of dark text rasterise (CoreText vs FreeType) with a mean of 3.3.
const DARK_WITH_TEXT: Tolerance = Tolerance {
    mean: 3.5,
    max: 255,
    mask_text: false,
};

#[test]
fn tooltips_match_the_kit() {
    let names = KitMaster::names("Tooltips/Tooltip");
    assert_eq!(names.len(), 6, "{names:?}");
    let mut failures = Vec::new();
    for name in &names {
        let tip = if name.contains("/One Line/") {
            Tooltip::new("Add to favourites")
        } else if name.contains("/Two Lines/") {
            Tooltip::new("Shows the recipe in a new window so you can keep this one open.")
        } else {
            Tooltip::new("Duplicate").shortcut("⌘D")
        };
        let Ok(mut scene) = Scene::load(name) else {
            failures.push(format!("{name}: cannot load"));
            continue;
        };
        scene.mount(|ui| ui.create(tip));
        let dark = scene.master.dark();
        let report = scene.auto_geometry(&[]);
        if !report.is_empty() {
            failures.push(format!("{name}:\n    {}", report.join("\n    ")));
            continue;
        }
        if let Err(e) = scene.pixels(if dark { DARK_SHAPES } else { Tolerance::SHAPES }) {
            failures.push(format!("{name}: {e}"));
        }
        // With text: the same bubble, text included (Sketch rasterises with CoreText).
        if let Err(e) = scene.pixels(if dark {
            DARK_WITH_TEXT
        } else {
            Tolerance::WITH_TEXT
        }) {
            failures.push(format!("{name} (text): {e}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn placement_matches_the_kit() {
    let names = KitMaster::names("Tooltips/Placement");
    assert_eq!(names.len(), 8, "{names:?}");
    let mut failures = Vec::new();
    for name in &names {
        let Ok(master) = KitMaster::load(name) else {
            failures.push(format!("{name}: cannot load"));
            continue;
        };
        let placement = if name.contains("/Below/") {
            Placement::Below
        } else if name.contains("/Above/") {
            Placement::Above
        } else if name.contains("/Leading/") {
            Placement::Leading
        } else {
            Placement::Trailing
        };
        let (Some(target), Some(tip)) = (
            master.rect_in_master("Target"),
            master.rect_in_master("Tooltip"),
        ) else {
            failures.push(format!("{name}: no Target or Tooltip layer"));
            continue;
        };
        let at = place(target, tip.size(), master.size(), placement, false);
        if (at.x - tip.x).abs() > 0.01 || (at.y - tip.y).abs() > 0.01 {
            failures.push(format!("{name}: tooltip at {at:?}, kit {:?}", tip.origin()));
        }
        // The scene: a round icon button and the tooltip placed beside it.
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        let master_size = master.size();
        let rect_tip = Rect::from_origin_size(at, tip.size());
        scene.mount(|ui| {
            let f = ui.create(Fixed {
                size: master_size,
                rects: vec![target, rect_tip],
            });
            let b = ui.create(IconButton::new("plus"));
            let _ = ui.set(b, Prop::Identifier, Value::from("Target"));
            let t = ui.create(Tooltip::new("Add to favourites"));
            let _ = ui.set(t, Prop::Identifier, Value::from("Tooltip"));
            ui.add_child(f, b);
            ui.add_child(f, t);
            f
        });
        let report = scene.auto_geometry(&[]);
        if !report.is_empty() {
            failures.push(format!("{name}:\n    {}", report.join("\n    ")));
            continue;
        }
        let tol = Tolerance {
            mean: 3.0,
            max: 128,
            mask_text: true,
        };
        if let Err(e) = scene.pixels(tol) {
            failures.push(format!("{name}: {e}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
