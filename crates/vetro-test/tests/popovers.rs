//! Kit parity of `Popovers`: the three contents and the popovers on every edge.

mod common;

use common::{row_view, text_at};
use vetro_lib::amalfi::button::{PushButton, PushVariant};
use vetro_lib::amalfi::buttons::LinkButton;
use vetro_lib::amalfi::fields::TextField;
use vetro_lib::amalfi::foundation::ControlSize;
use vetro_lib::amalfi::popovers::{
    Align, Edge, PopoverShape, content_column, metrics, popover, text_label,
};
use vetro_lib::core::{Alignment, Prop, Spacer, Stack, Ui, Value, ViewId};
use vetro_test::{KitLayer, KitMaster, Scene, Tolerance};

/// Glass, rounded corners and the arrow's antialiasing.
const POPOVER: Tolerance = Tolerance {
    mean: 3.0,
    max: 140,
    mask_text: true,
};

fn child<'a>(kit: &'a KitLayer, name: &str) -> Option<&'a KitLayer> {
    kit.children.iter().find(|c| c.name == name)
}

fn set_id(ui: &mut Ui, v: ViewId, name: &str) {
    let _ = ui.set(v, Prop::Identifier, Value::from(name));
}

fn set_width(ui: &mut Ui, v: ViewId, w: f32) {
    let _ = ui.set(v, Prop::Width, Value::from(w));
}

/// The content of a popover from a kit content master (or an instance of it).
fn content(ui: &mut Ui, kit: &KitLayer, master: &str) -> Option<ViewId> {
    if master.contains("/Content/Form/") {
        let width = kit.frame.width;
        let title = text_label(ui, &text_at(kit, "Title")?, false, "Title");
        set_width(ui, title, width);
        let field_kit = child(kit, "Name field")?;
        let field = ui.create(TextField::text().with_text(&text_at(field_kit, "Value")?));
        set_id(ui, field, "Name field");
        set_width(ui, field, width);
        let row = ui.create(Stack::row(8.0));
        set_width(ui, row, width);
        let spacer = ui.create(Spacer::default());
        ui.add_child(row, spacer);
        let buttons = child(kit, "Buttons")?;
        let mut kids: Vec<&KitLayer> = buttons.children.iter().collect();
        kids.sort_by(|a, b| a.frame.x.total_cmp(&b.frame.x));
        for k in kids {
            let primary = k.master.as_deref().is_some_and(|m| m.contains("/Primary/"));
            let b = ui.create(
                PushButton::new(&text_at(k, "Label").unwrap_or_default())
                    .variant(if primary {
                        PushVariant::Primary
                    } else {
                        PushVariant::Secondary
                    })
                    .size(ControlSize::Small),
            );
            set_id(ui, b, &k.name);
            ui.add_child(row, b);
        }
        set_id(ui, row, "Buttons");
        Some(content_column(ui, 8.0, 2.0, &[title, field, row]))
    } else if master.contains("/Content/List/") {
        let mut rows: Vec<&KitLayer> = kit.children.iter().collect();
        rows.sort_by(|a, b| a.frame.y.total_cmp(&b.frame.y));
        let mut views = Vec::new();
        for r in rows {
            let m = r.master.clone().unwrap_or_default();
            let v = row_view(ui, r, &m);
            set_width(ui, v, kit.frame.width);
            set_id(ui, v, &r.name);
            views.push(v);
        }
        let col = ui.create(Stack::column(0.0).aligned(Alignment::LEADING));
        for v in views {
            ui.add_child(col, v);
        }
        Some(col)
    } else if master.contains("/Content/Text/") {
        let width = kit.frame.width;
        let title = text_label(ui, &text_at(kit, "Title")?, false, "Title");
        let body = text_label(ui, &text_at(kit, "Body")?, true, "Body");
        set_width(ui, body, width);
        let link = ui.create(LinkButton::new(&text_at(child(kit, "Link")?, "Label")?));
        set_id(ui, link, "Link");
        Some(content_column(ui, 4.0, 2.0, &[title, body, link]))
    } else {
        None
    }
}

fn shape_of(name: &str) -> PopoverShape {
    let parts: Vec<&str> = name.split('/').collect();
    let place = parts.get(3).copied().unwrap_or("");
    let mut words = place.split(' ');
    let edge = match words.next() {
        Some("Top") => Some(Edge::Top),
        Some("Bottom") => Some(Edge::Bottom),
        Some("Leading") => Some(Edge::Leading),
        Some("Trailing") => Some(Edge::Trailing),
        _ => None,
    };
    let align = match words.next() {
        Some("Start") => Align::Start,
        Some("End") => Align::End,
        _ => Align::Center,
    };
    match edge {
        Some(e) => PopoverShape::arrow(e, align),
        None => PopoverShape::DETACHED,
    }
}

#[test]
fn popover_masters_match_the_kit() {
    let names = KitMaster::names("Popovers");
    assert_eq!(names.len(), 84, "{} masters", names.len());
    let mut failures = Vec::new();
    for name in &names {
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        let root = scene.master.root.clone();
        let size = scene.master.size();
        let is_content = name.starts_with("Popovers/Content/");
        scene.mount_sized(size, |ui| {
            if is_content {
                content(ui, &root, name).unwrap_or_else(|| ui.create(Spacer::default()))
            } else {
                let c = child(&root, "Content");
                let m = c.and_then(|c| c.master.clone()).unwrap_or_default();
                let inner = c
                    .and_then(|c| content(ui, c, &m))
                    .unwrap_or_else(|| ui.create(Spacer::default()));
                set_id(ui, inner, "Content");
                let padding = if name.contains("/Popover/List/") {
                    metrics::LIST_PADDING
                } else {
                    metrics::PADDING
                };
                popover(ui, shape_of(name), padding, inner)
            }
        });
        // Kit defect: the Dark/Glass/Thick shared style blurs 10 where materials.md gives 6.
        scene.ignore_aspects = vec!["#blurs".to_string()];
        scene.png_margin = true;
        let report = scene.auto_geometry(&[]);
        if !report.is_empty() {
            failures.push(format!("{name}:\n    {}", report.join("\n    ")));
            continue;
        }
        if let Err(e) = scene.pixels(POPOVER) {
            failures.push(format!("{name}: {e}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
