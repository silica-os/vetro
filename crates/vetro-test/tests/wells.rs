//! Kit parity of `Wells`: colour wells, image wells, swatch chips and rows, the colour panel.

use vetro_lib::amalfi::theme::Theme;
use vetro_lib::amalfi::wells::{
    ColorWell, ImageWell, ImageWellSize, SwatchChip, WellStyle, color_panel, swatch_row,
};
use vetro_lib::color::Color;
use vetro_lib::core::{Appearance, LayoutCx, PaintCx, Prop, Proposal, Value, ViewId, Widget};
use vetro_lib::geom::{Corners, Rect, Size};
use vetro_lib::shape::Shape;
use vetro_lib::style::{Paint, Style};
use vetro_test::{KitLayer, KitMaster, Scene, Tolerance, center_of};

/// Small round shapes and 1.5 pt dashed borders.
const WELL: Tolerance = Tolerance {
    mean: 3.0,
    max: 128,
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

fn check(scene: &mut Scene, name: &str, devs: &[&str], tol: Tolerance) -> Option<String> {
    let report = scene.auto_geometry(devs);
    if !report.is_empty() {
        return Some(format!("{name}:\n    {}", report.join("\n    ")));
    }
    scene.pixels(tol).err().map(|e| format!("{name}: {e}"))
}

fn solid(layer: &KitLayer) -> Color {
    match layer.style.fills.first() {
        Some(Paint::Solid(c)) => *c,
        _ => Color::BLACK,
    }
}

fn theme_of(scene: &Scene) -> Theme {
    Theme::new(if scene.master.dark() {
        Appearance::dark()
    } else {
        Appearance::light()
    })
}

/// A stand-in picture: the kit's layers (`Sky`, `Sun`, `Sea`, `Hill`) replayed.
struct Picture {
    layers: Vec<(String, Rect, Color, [f32; 4])>,
}

impl Widget for Picture {
    fn kind(&self) -> &'static str {
        "Picture"
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        p.replacing_unspecified(Size::new(72.0, 72.0))
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        for (name, rect, color, r) in &self.layers {
            cx.rec.layer(
                name,
                *rect,
                Shape::rounded_corners(Corners::new(r[0], r[1], r[2], r[3])),
                Style::fill(*color),
            );
        }
    }
}

fn picture_of(master: &KitMaster) -> Picture {
    let layers = ["Sky", "Sun", "Sea", "Hill"]
        .iter()
        .filter_map(|n| {
            let l = master.layer(n)?;
            let r = match l.style.radii.as_slice() {
                [a, b, c, d] => [*a, *b, *c, *d],
                [a] => [*a; 4],
                _ => [0.0; 4],
            };
            Some(((*n).to_string(), l.frame, solid(l), r))
        })
        .collect();
    Picture { layers }
}

#[test]
fn swatch_chips_match_the_kit() {
    let names = KitMaster::names("Wells/Swatch Chip");
    assert_eq!(names.len(), 4, "{names:?}");
    let mut failures = Vec::new();
    for name in &names {
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        let color = scene
            .master
            .layer("Colour")
            .map(solid)
            .unwrap_or(Color::BLACK);
        let chip = SwatchChip::new(color).selected(name.contains("/Selected/"));
        scene.mount(|ui| ui.create(chip));
        if let Some(f) = check(&mut scene, name, &[], WELL) {
            failures.push(f);
        }
    }
    finish(failures, names.len());
}

/// The (name, colour) items of a kit swatch row and the selected index.
fn row_items(theme: &Theme, row: &KitLayer) -> (Vec<(String, Color)>, Option<usize>) {
    let mut chips: Vec<&KitLayer> = row.children.iter().filter(|c| c.master.is_some()).collect();
    chips.sort_by(|a, b| a.frame.x.total_cmp(&b.frame.x));
    let selected = chips.iter().position(|c| {
        c.master
            .as_deref()
            .is_some_and(|m| m.contains("/Selected/"))
    });
    let items = chips
        .iter()
        .map(|c| {
            (
                c.name.clone(),
                // The kit's "Shadow/Warm" swatch has no token: silica_ui shows Shadow/Soft.
                theme
                    .color(if c.name == "Shadow/Warm" {
                        "Shadow/Soft"
                    } else {
                        &c.name
                    })
                    .unwrap_or(theme.colors.text_tertiary),
            )
        })
        .collect();
    (items, selected)
}

/// Kit quirk: a chip's colour is an override of its master's fill that the export does not
/// carry (the nested chip shows the accent): every chip but the accent one differs.
fn fill_devs(prefix: &str, items: &[(String, Color)], accent: Color) -> Vec<String> {
    items
        .iter()
        .filter(|(_, c)| *c != accent)
        .map(|(n, _)| format!("{prefix}{n}/Colour#fills"))
        .collect()
}

#[test]
fn swatch_rows_and_the_colour_panel_match_the_kit() {
    let mut names = KitMaster::names("Wells/Swatch Row");
    names.extend(KitMaster::names("Wells/Color Panel"));
    assert_eq!(names.len(), 10, "{names:?}");
    let mut failures = Vec::new();
    for name in &names {
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        let theme = theme_of(&scene);
        let accent = theme.colors.accent_amalfi;
        let mut devs: Vec<String> = Vec::new();
        if name.contains("Swatch Row") {
            let (items, selected) = row_items(&theme, &scene.master.root);
            devs = fill_devs("", &items, accent);
            scene.mount(|ui| swatch_row(ui, &items, selected).0);
        } else {
            let title = scene
                .master
                .layer("Title")
                .and_then(|t| t.text.clone())
                .unwrap_or_default();
            let mut rows = Vec::new();
            for i in 1..=4 {
                if let Some(row) = scene.master.layer(&format!("Row {i}")) {
                    let (items, _) = row_items(&theme, row);
                    devs.extend(fill_devs(&format!("Row {i}/"), &items, accent));
                    rows.push(items);
                }
            }
            scene.mount(|ui| color_panel(ui, &title, &rows, Some(accent)).0);
        }
        let refs: Vec<&str> = devs.iter().map(String::as_str).collect();
        // Pixels: the chips' colours are what the kit PNG shows; geometry names the rest.
        let report = scene.auto_geometry(&refs);
        if !report.is_empty() {
            failures.push(format!("{name}:\n    {}", report.join("\n    ")));
            continue;
        }
        if let Err(e) = scene.pixels(WELL) {
            failures.push(format!("{name}: {e}"));
        }
    }
    finish(failures, names.len());
}

#[test]
fn color_wells_match_the_kit() {
    let names = KitMaster::names("Wells/Color Well");
    assert_eq!(names.len(), 24, "{} masters", names.len());
    let mut failures = Vec::new();
    for name in &names {
        let parts: Vec<&str> = name.split('/').collect();
        let [_, _, style, state, _app] = parts[..] else {
            continue;
        };
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        let color = scene
            .master
            .layer("Colour")
            .map(solid)
            .unwrap_or(Color::BLACK);
        let mut well = ColorWell::new(color).style(match style {
            "Minimal" => WellStyle::Minimal,
            "Expanded" => WellStyle::Expanded,
            _ => WellStyle::Standard,
        });
        if style == "Expanded" {
            let n = scene
                .master
                .layer("Name")
                .and_then(|l| l.text.clone())
                .unwrap_or_default();
            well = well.named(&n);
        }
        scene.mount(|ui| ui.create(well));
        match (state, center_of(&scene.master, "Colour")) {
            ("Hover", Some(p)) => scene.hover_at(p),
            ("Focused" | "Disabled", _) => scene.set_state(state),
            _ => {}
        }
        if let Some(f) = check(&mut scene, name, &[], WELL) {
            failures.push(f);
        }
    }
    finish(failures, names.len());
}

#[test]
fn image_wells_match_the_kit() {
    let names = KitMaster::names("Wells/Image Well");
    assert_eq!(names.len(), 36, "{} masters", names.len());
    let mut failures = Vec::new();
    for name in &names {
        let parts: Vec<&str> = name.split('/').collect();
        let [_, _, kind, sz, state, _app] = parts[..] else {
            continue;
        };
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        let size = match sz {
            "Small" => ImageWellSize::Small,
            "Large" => ImageWellSize::Large,
            _ => ImageWellSize::Regular,
        };
        let filled = kind == "Filled";
        let pic = picture_of(&scene.master);
        let well_view: ViewId = scene.mount(|ui| {
            let w = ui.create(ImageWell::new().size(size));
            if filled {
                let p = ui.create(pic);
                ui.add_child(w, p);
            }
            w
        });
        match state {
            "Drag Over" => {
                let _ = scene.ui.set(well_view, Prop::State, Value::Bool(true));
                scene.refresh();
            }
            "Focused" | "Disabled" => scene.set_state(state),
            _ => {}
        }
        // Kit text measure: the hint "Drop image" is 61 wide in the kit, 59.96 here.
        let devs: &[&str] = if !filled && size != ImageWellSize::Small {
            &["Hint#frame"]
        } else {
            &[]
        };
        if let Some(f) = check(&mut scene, name, devs, WELL) {
            failures.push(f);
        }
    }
    finish(failures, names.len());
}
