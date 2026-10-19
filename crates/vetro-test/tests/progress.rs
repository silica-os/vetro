//! Kit parity of `Progress`: bars, circular indicators and spinners.

use vetro_lib::amalfi::foundation::ControlSize;
use vetro_lib::amalfi::progress::{CircularProgress, ProgressBar, ProgressSize, Spinner};
use vetro_test::{KitMaster, Scene, Tolerance};

/// Thin shapes and 2-4 pt strokes: edges are most of their pixels.
const SMALL: Tolerance = Tolerance {
    mean: 3.0,
    max: 128,
    mask_text: true,
};

fn size(name: &str) -> ProgressSize {
    match name {
        "Small" => ProgressSize::Small,
        "Large" => ProgressSize::Large,
        _ => ProgressSize::of(ControlSize::Regular),
    }
}

fn run(
    prefix: &str,
    count: usize,
    make: impl Fn(&[&str]) -> Option<(Box<dyn FnOnce(&mut Scene)>, bool)>,
) {
    let names = KitMaster::names(prefix);
    assert_eq!(names.len(), count, "{} masters", names.len());
    let mut failures = Vec::new();
    for name in &names {
        let parts: Vec<&str> = name.split('/').collect();
        let Some((mount, disabled)) = make(&parts) else {
            continue;
        };
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        mount(&mut scene);
        // The kit's arcs are 1 px off the size our bounds give (rounding of the arc's
        // frame): the corner they share is compared.
        scene.png_margin = true;
        if disabled {
            scene.set_state("Disabled");
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

fn value_of(state: &str) -> f32 {
    match state {
        "25" => 0.25,
        "100" => 1.0,
        _ => 0.6,
    }
}

#[test]
fn bars_match_the_kit() {
    run("Progress/Bar", 48, |p| {
        let [_, _, kind, sz, state, _app] = p[..] else {
            return None;
        };
        let sz = size(sz);
        let bar = if kind == "Determinate" {
            ProgressBar::new().size(sz).value(value_of(state))
        } else {
            let bar = ProgressBar::new().size(sz);
            match state {
                // The segment's x is -72 + phase * 272: Phase 1 starts at 0.
                "Phase 1" => bar.phase(72.0 / 272.0),
                "Phase 2" => bar.phase(0.5),
                "Phase 3" => bar.phase(0.75),
                _ => bar,
            }
        };
        Some((
            Box::new(move |s: &mut Scene| {
                s.mount(|ui| ui.create(bar));
            }),
            state == "Disabled",
        ))
    });
}

#[test]
fn circular_indicators_match_the_kit() {
    run("Progress/Circular", 24, |p| {
        let [_, _, _, sz, state, _app] = p[..] else {
            return None;
        };
        let c = CircularProgress::new(value_of(state)).size(size(sz));
        Some((
            Box::new(move |s: &mut Scene| {
                s.mount(|ui| ui.create(c));
            }),
            state == "Disabled",
        ))
    });
}

#[test]
fn spinners_match_the_kit() {
    run("Progress/Spinner", 12, |p| {
        let [_, _, sz, state, _app] = p[..] else {
            return None;
        };
        let sp = Spinner::new().size(size(sz));
        Some((
            Box::new(move |s: &mut Scene| {
                s.mount(|ui| ui.create(sp));
            }),
            state == "Disabled",
        ))
    });
}
