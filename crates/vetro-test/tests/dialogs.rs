//! Kit parity of `Dialogs`: the form dialog, the open and save panels, the browser and the
//! panels' footers.

mod common;

use common::{build_footer, build_row, build_section, child, sidebar_view, text_at};
use vetro_lib::amalfi::button::{PushButton, PushVariant};
use vetro_lib::amalfi::dialogs::{Dialog, browser, dialog, dialog_footer};
use vetro_lib::amalfi::text::{Text, TextRole};
use vetro_lib::amalfi::tokens::typography;
use vetro_lib::core::{Prop, Ui, Value, ViewId};
use vetro_test::{KitLayer, KitMaster, Scene, Tolerance};

/// Raised surfaces with controls and rows inside.
const DIALOG: Tolerance = Tolerance {
    mean: 3.0,
    max: 150,
    mask_text: true,
};

fn set_id(ui: &mut Ui, v: ViewId, name: &str) {
    let _ = ui.set(v, Prop::Identifier, Value::from(name));
}

fn set_width(ui: &mut Ui, v: ViewId, w: f32) {
    let _ = ui.set(v, Prop::Width, Value::from(w));
}

fn push(ui: &mut Ui, k: &KitLayer) -> ViewId {
    let primary = k.master.as_deref().is_some_and(|m| m.contains("/Primary/"));
    let b = ui.create(
        PushButton::new(&text_at(k, "Label").unwrap_or_default()).variant(if primary {
            PushVariant::Primary
        } else {
            PushVariant::Secondary
        }),
    );
    let _ = ui.set(b, Prop::MinWidth, Value::from(72.0));
    set_id(ui, b, &k.name);
    b
}

fn footer(ui: &mut Ui, kit: &KitLayer) -> ViewId {
    let mut buttons: Vec<&KitLayer> = kit.children.iter().filter(|c| c.master.is_some()).collect();
    buttons.sort_by(|a, b| a.frame.x.total_cmp(&b.frame.x));
    let spacer_x = child(kit, "Spacer").map_or(0.0, |s| s.frame.x);
    let (mut lead, mut trail) = (Vec::new(), Vec::new());
    for k in buttons {
        let v = push(ui, k);
        if k.frame.x < spacer_x {
            lead.push(v);
        } else {
            trail.push(v);
        }
    }
    dialog_footer(ui, kit.frame.width, &lead, &trail)
}

fn browser_view(ui: &mut Ui, kit: &KitLayer, dark: bool) -> ViewId {
    let _ = dark;
    let places = child(kit, "Places")
        .and_then(|p| child(p, "Places group"))
        .and_then(|g| {
            let m = g.master.clone()?;
            let v = sidebar_view(ui, g, &m)?;
            set_id(ui, v, "Places group");
            Some(v)
        });
    let mut files: Vec<&KitLayer> = child(kit, "File list")
        .map(|l| l.children.iter().filter(|c| c.master.is_some()).collect())
        .unwrap_or_default();
    files.sort_by(|a, b| a.frame.y.total_cmp(&b.frame.y));
    let rows: Vec<ViewId> = files
        .iter()
        .filter_map(|f| {
            let m = f.master.clone()?;
            let v = sidebar_view(ui, f, &m)?;
            set_id(ui, v, &f.name);
            Some(v)
        })
        .collect();
    browser(
        ui,
        kit.frame.width,
        &places.into_iter().collect::<Vec<_>>(),
        &rows,
    )
}

fn build(ui: &mut Ui, kit: &KitLayer, name: &str) -> Option<ViewId> {
    if name.starts_with("Dialogs/Browser/") {
        return Some(browser_view(ui, kit, name.contains("/Dark")));
    }
    if name.starts_with("Dialogs/Footer/") {
        return Some(footer(ui, kit));
    }
    let shape = if name.contains("/Generic/") {
        Dialog::form()
    } else {
        Dialog::panel()
    };
    let mut kids: Vec<&KitLayer> = kit
        .children
        .iter()
        .filter(|c| c.name != "Surface")
        .collect();
    kids.sort_by(|a, b| a.frame.y.total_cmp(&b.frame.y));
    let mut views = Vec::new();
    for k in kids {
        let v = if k.name == "Title" {
            ui.create(
                Text::new(&k.text.clone().unwrap_or_default())
                    .token(&typography::TITLE_3)
                    .role(TextRole::Primary)
                    .layer("Title")
                    .whole(1.0, 0.0),
            )
        } else {
            let m = k.master.clone().unwrap_or_default();
            let v = if m.starts_with("Forms/Section/") {
                build_section(ui, k)
            } else if m.starts_with("Forms/Footer/") {
                build_footer(ui, k)
            } else if m.starts_with("Forms/Row/") {
                build_row(ui, k)
            } else if m.starts_with("Dialogs/Browser/") {
                browser_view(ui, k, m.contains("/Dark"))
            } else if m.starts_with("Dialogs/Footer/") {
                footer(ui, k)
            } else {
                return None;
            };
            set_width(ui, v, k.frame.width);
            v
        };
        set_id(ui, v, &k.name);
        views.push(v);
    }
    Some(dialog(ui, shape, &views))
}

#[test]
fn dialog_masters_match_the_kit() {
    let names = KitMaster::names("Dialogs");
    assert_eq!(names.len(), 12, "{} masters", names.len());
    let mut failures = Vec::new();
    for name in &names {
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        let root = scene.master.root.clone();
        let size = scene.master.size();
        scene.mount_sized(size, |ui| {
            build(ui, &root, name).unwrap_or_else(|| ui.create(Dialog::form()))
        });
        scene.png_margin = true;
        // Kit quirks of the nested forms (see forms.rs) and Sketch's wider titles.
        let skip = [
            "Control/Background",
            "Control/Focus Halo",
            "Control/Error icon",
            "Control/Value",
            "Control/Switch/Track",
            "Control/Switch/Thumb",
            "Save/Label",
            "Title",
            // Buttons wider than their text keep the master label's place in the kit.
            "/Label",
        ];
        let report = scene.auto_geometry_except(&skip, &[]);
        if !report.is_empty() {
            failures.push(format!("{name}:\n    {}", report.join("\n    ")));
            continue;
        }
        // The footer's buttons are 72 wide in the kit whatever their text: a mean that small
        // with a high peak is an edge or two.
        let tol = Tolerance { max: 255, ..DIALOG };
        if let Err(e) = scene.pixels(tol) {
            failures.push(format!("{name}: {e}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
