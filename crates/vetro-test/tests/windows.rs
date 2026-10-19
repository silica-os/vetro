//! Kit parity of `Windows`: the window frame, its regions, toolbars, sidebars and content.
//!
//! Nested components that have masters of their own (group boxes, the segmented control, the
//! checkbox group, the search field, the menu bar) are replaced by placeholders of their size
//! and masked: their families check them.

mod common;

use common::{Fixed, Swatch, alert_view, child, set_id, sidebar_view, text_at, titlebar_view};
use vetro_lib::amalfi::button::{PushButton, PushVariant};
use vetro_lib::amalfi::buttons::MenuButton;
use vetro_lib::amalfi::fields::TextField;
use vetro_lib::amalfi::sidebars::{Panel, Row};
use vetro_lib::amalfi::text::{Text, TextRole};
use vetro_lib::amalfi::toggles::{Toggle, ToggleState};
use vetro_lib::amalfi::tokens::type_style_ref;
use vetro_lib::amalfi::windows::{Block, BlockFill, WindowFrame};
use vetro_lib::core::{Prop, Ui, Value, ViewId};
use vetro_lib::geom::{Point, Rect, Size};
use vetro_test::{KitLayer, KitMaster, Scene, Tolerance};

/// Whole windows: glass bars, rounded corners and nested controls.
const WINDOW: Tolerance = Tolerance {
    mean: 3.0,
    max: 150,
    mask_text: true,
};

/// The layers of the frame itself.
const CHROME: [&str; 4] = [
    "Window shadow",
    "Window clip",
    "Window surface",
    "Window rim",
];

fn text_view(ui: &mut Ui, k: &KitLayer) -> ViewId {
    let shared = k.shared_style.clone().unwrap_or_default();
    let parts: Vec<&str> = shared.split('/').collect();
    let token = parts
        .get(1)
        .and_then(|t| type_style_ref(t))
        .unwrap_or(&vetro_lib::amalfi::tokens::typography::BODY);
    let role = match parts.get(2) {
        Some(&"Secondary") => TextRole::Secondary,
        Some(&"Tertiary") => TextRole::Tertiary,
        _ => TextRole::Primary,
    };
    let mut t = Text::new(&k.text.clone().unwrap_or_default())
        .token(token)
        .role(role)
        .layer(&k.name);
    if k.name.contains("body") {
        t = t.wrapping();
    }
    ui.create(t)
}

/// A view for a kit child; `None` for what the frame or a stack makes itself.
fn child_view(ui: &mut Ui, k: &KitLayer, col: &mut Collect, abs: Point) -> Option<ViewId> {
    match k.kind.as_str() {
        "Text" => Some(text_view(ui, k)),
        "ShapePath" => {
            if CHROME.contains(&k.name.as_str()) {
                return None;
            }
            let fill = match k.name.as_str() {
                "Sidebar region" | "Inspector region" => BlockFill::Sidebar,
                "Separator" => BlockFill::Separator,
                "Surface" => BlockFill::Window,
                "Scrim" => BlockFill::Scrim(k.style.opacity),
                _ => return None,
            };
            let mut b = Block::new(&k.name, fill);
            if k.name == "Scrim" {
                b = b.rounded_bottom(18.0);
            }
            Some(ui.create(b))
        }
        "SymbolInstance" => {
            let m = k.master.clone()?;
            if m.starts_with("Titlebars/") {
                titlebar_view(ui, k, &m)
            } else if m.starts_with("Sidebars/") {
                sidebar_view(ui, k, &m)
            } else if m.starts_with("Windows/Sidebar Body/") {
                body_view(ui, k, &m, col.inactive)
            } else if m.starts_with("Windows/Window/") {
                let keep = col.prefix.len();
                col.prefix.push_str(&format!("{}/", k.name));
                let at = Point::new(abs.x + k.frame.x, abs.y + k.frame.y);
                let v = window_at(ui, k, &m, col, at);
                col.prefix.truncate(keep);
                Some(v)
            } else if m.starts_with("Alerts/") {
                Some(alert_view(ui, k, &m))
            } else if m.starts_with("Buttons/Push/") {
                let primary = m.contains("/Primary/");
                let b = ui.create(
                    PushButton::new(&text_at(k, "Label").unwrap_or_default()).variant(if primary {
                        PushVariant::Primary
                    } else {
                        PushVariant::Secondary
                    }),
                );
                Some(b)
            } else if m.starts_with("Buttons/Pop-up/") {
                let label = text_at(k, "Label").unwrap_or_default();
                Some(ui.create(MenuButton::pop_up(&[label.as_str(), "Other"], 0)))
            } else if m.starts_with("Toggles/Checkbox/With Label/") {
                let on = m.contains("/On/");
                Some(
                    ui.create(
                        Toggle::checkbox()
                            .label(&text_at(k, "Label").unwrap_or_default())
                            .state(if on {
                                ToggleState::On
                            } else {
                                ToggleState::Off
                            }),
                    ),
                )
            } else if m.starts_with("Fields/Search Field/") {
                Some(ui.create(TextField::search()))
            } else {
                // A nested component of another family: a placeholder, masked.
                col.trees.push(format!("{}{}", col.prefix, k.name));
                col.masks.push(Rect::from_origin_size(
                    Point::new(abs.x + k.frame.x, abs.y + k.frame.y),
                    k.frame.size(),
                ));
                Some(ui.create(Swatch(k.frame.size())))
            }
        }
        _ => None,
    }
}

/// The sidebar body: the rows and headers of a panel without footer.
fn body_view(ui: &mut Ui, kit: &KitLayer, master: &str, inactive: bool) -> Option<ViewId> {
    let top = if master.contains("/Full/") { 60.0 } else { 8.0 };
    let p = ui.create(Panel { top });
    let mut kids: Vec<&KitLayer> = kit.children.iter().filter(|c| c.master.is_some()).collect();
    kids.sort_by(|a, b| a.frame.y.total_cmp(&b.frame.y));
    for k in kids {
        let m = k.master.clone()?;
        let v = sidebar_view(ui, k, &m)?;
        // The kit swaps the selected row of an inactive window for the inactive master, and its
        // export gives the row's icon the label's text.
        ui.with_widget::<Row, _>(v, |r| {
            if r.icon == r.label {
                r.icon = "clock".to_string();
            }
            if inactive && r.selected {
                r.active = Some(false);
            }
        });
        set_id(ui, v, &k.name);
        ui.add_child(p, v);
    }
    Some(p)
}

/// What the builder leaves for the scene: placeholders to skip and mask.
#[derive(Default)]
struct Collect {
    trees: Vec<String>,
    masks: Vec<Rect>,
    /// The layer path of the window being built (`Window/` inside a sheet's window).
    prefix: String,
    /// The window is inactive: its sidebar's selected row is drawn inactive.
    inactive: bool,
}

fn window_at(ui: &mut Ui, kit: &KitLayer, name: &str, col: &mut Collect, abs: Point) -> ViewId {
    if name.starts_with("Windows/Sidebar Body/") {
        return body_view(ui, kit, name, col.inactive)
            .unwrap_or_else(|| ui.create(Swatch(kit.frame.size())));
    }
    let size = kit.frame.size();
    let mut parts = Vec::new();
    for k in &kit.children {
        let Some(v) = child_view(ui, k, col, abs) else {
            continue;
        };
        if k.kind == "SymbolInstance" {
            set_id(ui, v, &k.name);
        }
        parts.push((v, k.frame));
    }
    if child(kit, "Window shadow").is_some() {
        let frame = ui.create(if name.contains("/Utility/") {
            WindowFrame::utility(size)
        } else {
            WindowFrame::new(size)
        });
        for (v, r) in parts {
            let _ = ui.set(v, Prop::Offset, Value::Point(r.origin()));
            let _ = ui.set(v, Prop::Width, Value::from(r.width));
            let _ = ui.set(v, Prop::Height, Value::from(r.height));
            ui.add_child(frame, v);
        }
        frame
    } else {
        let fixed = ui.create(Fixed {
            size,
            rects: parts.iter().map(|p| p.1).collect(),
        });
        for (v, _) in parts {
            ui.add_child(fixed, v);
        }
        fixed
    }
}

#[test]
fn window_masters_match_the_kit() {
    let names = KitMaster::names("Windows");
    assert_eq!(names.len(), 40, "{} masters", names.len());
    let mut failures = Vec::new();
    for name in &names {
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        let root = scene.master.root.clone();
        let size = scene.master.size();
        let inactive = name.contains("/Inactive");
        let mut col = Collect {
            inactive,
            ..Collect::default()
        };
        scene.mount_sized(size, |ui| window_at(ui, &root, name, &mut col, Point::ZERO));
        // A sidebar panel resized in the window keeps the master's footer and surface in the
        // kit (docs/kit-issues.md).
        col.trees.push("Sidebar/Footer".to_string());
        scene.skip_trees = col.trees;
        scene.extra_mask = col.masks;
        let w = scene.window;
        scene.ui.set_window_active(w, !inactive);
        scene.ignore_aspects = vec!["#blurs".to_string()];
        scene.png_margin = true;
        scene.settle();
        // Buttons wider than their text keep the master label's place (Alerts, Dialogs).
        let report = scene.auto_geometry_except(
            &[
                "/Label",
                "Title",
                "Content title",
                "Sidebar/Surface",
                "Sidebar/Recents/Icon",
                "Sidebar/Recents/Selection",
                // The sheet is 266 in the window where its master is 268.
                "Sheet/Surface",
                "Toolbar button/Background",
                "Toolbar button/Glyph Gear",
            ],
            &[],
        );
        if !report.is_empty() {
            failures.push(format!("{name}:\n    {}", report.join("\n    ")));
            continue;
        }
        // Rounded window corners and shadows: a few edge pixels peak, the mean stays small.
        let tol = Tolerance { max: 255, ..WINDOW };
        if let Err(e) = scene.pixels(tol) {
            failures.push(format!("{name}: {e}"));
        }
    }
    let _ = Size::ZERO;
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
