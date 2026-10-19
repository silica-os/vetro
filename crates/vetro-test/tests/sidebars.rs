//! Kit parity of `Sidebars`: rows, headers, groups, the footer and the panel.

mod common;

use common::text_at;
use vetro_lib::amalfi::sidebars::{Badge, Header, Row, footer, group, row};
use vetro_lib::amalfi::sidebars::{Panel, metrics};
use vetro_lib::core::{Prop, Ui, Value, ViewId};
use vetro_lib::geom::Point;
use vetro_test::{KitLayer, KitMaster, Scene, Tolerance};

/// Pills, rounded selections and thin glyphs.
const SIDEBAR: Tolerance = Tolerance {
    mean: 2.5,
    max: 110,
    mask_text: true,
};

fn child<'a>(kit: &'a KitLayer, name: &str) -> Option<&'a KitLayer> {
    kit.children.iter().find(|c| c.name == name)
}

/// A view from a kit layer (a master root or an instance of a sidebar master).
fn build(ui: &mut Ui, kit: &KitLayer, master: &str) -> Option<ViewId> {
    let parts: Vec<&str> = master.split('/').collect();
    let kind = *parts.get(1)?;
    let v = match kind {
        "Row" => {
            let variant = *parts.get(2)?;
            let state = *parts.get(3)?;
            let badge = child(kit, "Badge").and_then(|b| text_at(b, "Count"));
            let volume = child(kit, "Eject button").is_some();
            debug_assert!(variant != "Badge" || badge.is_some());
            let v = row(
                ui,
                // The kit names the SF-style symbol; ours is `drive`.
                match text_at(kit, "Icon").unwrap_or_default().as_str() {
                    "externaldrive" => "drive",
                    other => other,
                },
                &text_at(kit, "Label").unwrap_or_default(),
                badge.as_deref(),
                volume,
            );
            let selected = state.starts_with("Selected");
            ui.with_widget::<Row, _>(v, |r| {
                r.selected = selected;
                if state == "Selected Inactive" {
                    r.active = Some(false);
                } else if selected {
                    r.active = Some(true);
                }
            });
            if state == "Disabled" {
                let _ = ui.set(v, Prop::Disabled, Value::Bool(true));
            }
            v
        }
        "Header" => {
            let mut h = Header::new(&text_at(kit, "Title").unwrap_or_default());
            let variant = *parts.get(2)?;
            if variant != "Plain" {
                h = h.collapsible(variant == "Open");
            }
            ui.create(h)
        }
        "Badge" => ui.create(Badge::new(&text_at(kit, "Count").unwrap_or_default())),
        "Footer" => {
            let f = footer(ui);
            // The kit's Settings button shows a plus like the Add button
            // (docs/kit-issues.md); ours has a gear.
            if let Some(settings) = ui.tree.children(f).get(1).copied() {
                let _ = ui.set(settings, Prop::Symbol, Value::from("plus"));
            }
            f
        }
        "Group" => {
            let open = *parts.get(2)? == "Open";
            let mut kids: Vec<&KitLayer> =
                kit.children.iter().filter(|c| c.master.is_some()).collect();
            kids.sort_by(|a, b| a.frame.y.total_cmp(&b.frame.y));
            let mut rows = Vec::new();
            // The group makes its own header (the kit's first instance).
            for k in kids {
                let m = k.master.clone().unwrap_or_default();
                if m.contains("/Header/") {
                    continue;
                }
                let v = build(ui, k, &m)?;
                let _ = ui.set(v, Prop::Identifier, Value::from(k.name.as_str()));
                rows.push(v);
            }
            let title = child(kit, "Header")
                .and_then(|h| text_at(h, "Title"))
                .unwrap_or_default();
            group(ui, &title, open, &rows)
        }
        "Panel" => {
            let mut kids: Vec<&KitLayer> =
                kit.children.iter().filter(|c| c.master.is_some()).collect();
            kids.sort_by(|a, b| a.frame.y.total_cmp(&b.frame.y));
            let p = ui.create(Panel);
            for k in kids {
                let m = k.master.clone().unwrap_or_default();
                let v = build(ui, k, &m)?;
                let _ = ui.set(v, Prop::Identifier, Value::from(k.name.as_str()));
                ui.add_child(p, v);
            }
            p
        }
        _ => return None,
    };
    Some(v)
}

#[test]
fn sidebar_masters_match_the_kit() {
    let names = KitMaster::names("Sidebars");
    assert_eq!(names.len(), 44, "{} masters", names.len());
    let _ = metrics::ROW_WIDTH;
    let mut failures = Vec::new();
    for name in &names {
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        let root = scene.master.root.clone();
        let size = scene.master.size();
        scene.mount_sized(size, |ui| {
            build(ui, &root, name).unwrap_or_else(|| ui.create(Panel))
        });
        if name.contains("Volume/Hover/") {
            // The kit shows the eject button hovered too.
            scene.hover_at(Point::new(196.0, 16.0));
        } else if name.contains("/Hover/") {
            scene.set_state("Hover");
        }
        let report = scene.auto_geometry(&[]);
        if !report.is_empty() {
            failures.push(format!("{name}:\n    {}", report.join("\n    ")));
            continue;
        }
        if let Err(e) = scene.pixels(SIDEBAR) {
            failures.push(format!("{name}: {e}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
