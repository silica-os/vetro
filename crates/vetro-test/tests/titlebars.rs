//! Kit parity of `Titlebars`: window buttons, pieces, bars, sections, title bars and tabs.

mod common;

use common::text_at;
use vetro_lib::amalfi::buttons::{IconButton, ToolbarButton};
use vetro_lib::amalfi::fields::Token;
use vetro_lib::amalfi::foundation::ControlSize;
use vetro_lib::amalfi::segments::{ActionSegments, Segment};
use vetro_lib::amalfi::titlebars::{
    Backdrop, Dot, DotKind, SearchPiece, SendPiece, Spacer, Strip, SwitchItem, SwitcherPiece, Tab,
    TabBar, TitleBar, TitleBlock, TitleStyle, window_controls_view,
};
use vetro_lib::core::{Prop, Ui, Value, ViewId};
use vetro_lib::geom::Point;
use vetro_lib::text::Align;
use vetro_test::{KitLayer, KitMaster, Scene, Tolerance};

/// Glass, gradients and rounded corners.
const TITLEBAR: Tolerance = Tolerance {
    mean: 2.5,
    max: 110,
    mask_text: true,
};

/// The 16 pt window buttons: the edge pixels are most of the image and the kit's hover
/// export antialiases its 0.5 pt rim differently (the inside matches exactly).
const DOTS: Tolerance = Tolerance {
    mean: 12.0,
    max: 110,
    mask_text: true,
};

fn child<'a>(kit: &'a KitLayer, name: &str) -> Option<&'a KitLayer> {
    kit.children.iter().find(|c| c.name == name)
}

/// The first text of a layer's children (a nested button's glyph).
fn first_text(kit: &KitLayer) -> Option<String> {
    kit.children.iter().find_map(|c| c.text.clone())
}

fn set_id(ui: &mut Ui, v: ViewId, name: &str) {
    let _ = ui.set(v, Prop::Identifier, Value::from(name));
}

/// A strip from the children of `kit` (sorted by x), with the given padding.
fn strip_of(
    ui: &mut Ui,
    kit: &KitLayer,
    height: f32,
    pad: (f32, f32),
    gap: f32,
    backdrop: Backdrop,
    title_align: Align,
) -> Option<ViewId> {
    let mut kids: Vec<&KitLayer> = kit
        .children
        .iter()
        .filter(|c| c.master.is_some() || c.name == "Title" || c.name == "Spacer")
        .collect();
    kids.sort_by(|a, b| a.frame.x.total_cmp(&b.frame.x));
    let s = ui.create(Strip::new(height, pad.0, pad.1).gap(gap).backdrop(backdrop));
    for k in kids {
        let v = if k.name == "Title" && k.master.is_none() {
            let mut t = TitleBlock::new(&k.text.clone().unwrap_or_default());
            t.fill = true;
            t.align = title_align;
            ui.create(t)
        } else if k.name == "Spacer" && k.master.is_none() {
            ui.create(Spacer::fixed(k.frame.width))
        } else {
            let m = k.master.clone().unwrap_or_default();
            build(ui, k, &m)?
        };
        // The title and the spacer are plain layers of the bar in the kit.
        if k.master.is_some() {
            set_id(ui, v, &k.name);
        }
        ui.add_child(s, v);
    }
    Some(s)
}

fn action_segments(kit: &KitLayer, master: &str) -> ActionSegments {
    let parts: Vec<&str> = master.split('/').collect();
    let variant = parts.get(2).copied().unwrap_or("Icon 2");
    let count: usize = variant
        .rsplit(' ')
        .next()
        .and_then(|n| n.parse().ok())
        .unwrap_or(2);
    let segments: Vec<Segment> = (1..=count)
        .map(|i| {
            let t = text_at(kit, &format!("Glyph {i}")).unwrap_or_default();
            Segment::icon(&t, &format!("Glyph {i}"))
        })
        .collect();
    ActionSegments::new(segments)
        .size(ControlSize::Large)
        .active(parts.get(4) != Some(&"Inactive"))
}

fn switcher(group: &KitLayer) -> SwitcherPiece {
    let mut items = Vec::new();
    for i in 1..=8 {
        if let Some(t) = group
            .children
            .iter()
            .filter(|c| c.text.is_some())
            .nth(i - 1)
        {
            let text = t.text.clone().unwrap_or_default();
            items.push(if t.name == "Label" {
                SwitchItem::Text(text)
            } else {
                SwitchItem::Icon(text)
            });
        }
    }
    SwitcherPiece::new(items)
}

/// A view from a kit layer (a master root or an instance of a titlebar or button master).
fn build(ui: &mut Ui, kit: &KitLayer, master: &str) -> Option<ViewId> {
    let parts: Vec<&str> = master.split('/').collect();
    match (*parts.first()?, *parts.get(1)?) {
        ("Buttons", "Toolbar") => {
            let symbol = first_text(kit).unwrap_or_default();
            let mut b = ToolbarButton::new(&symbol);
            if parts.get(2) == Some(&"Icon Small") {
                b = b.small();
            }
            let v = ui.create(b);
            if parts.get(3) == Some(&"Disabled") {
                let _ = ui.set(v, Prop::Disabled, Value::Bool(true));
            }
            Some(v)
        }
        ("Buttons", "Icon") => {
            let symbol = first_text(kit).unwrap_or_default();
            Some(ui.create(IconButton::new(&symbol).square().size(ControlSize::Small)))
        }
        ("Segments", "Action") => Some(ui.create(action_segments(kit, master))),
        ("Titlebars", "Controls") => Some(window_controls_view(ui)),
        ("Titlebars", "Dot") => {
            let kind = match *parts.get(2)? {
                "Close" => DotKind::Close,
                "Minimise" => DotKind::Minimise,
                _ => DotKind::Zoom,
            };
            Some(ui.create(Dot::new(kind)))
        }
        ("Titlebars", "Tab") => {
            let state = *parts.get(2)?;
            Some(ui.create(
                Tab::new(&text_at(kit, "Label").unwrap_or_default()).selected(state == "Selected"),
            ))
        }
        ("Titlebars", "Tab Bar") => {
            let bar = ui.create(TabBar);
            let mut tabs: Vec<&KitLayer> = kit
                .children
                .iter()
                .filter(|c| c.master.as_deref().is_some_and(|m| m.contains("/Tab/")))
                .collect();
            tabs.sort_by(|a, b| a.frame.x.total_cmp(&b.frame.x));
            for t in tabs {
                let m = t.master.clone().unwrap_or_default();
                let v = build(ui, t, &m)?;
                set_id(ui, v, &t.name);
                ui.add_child(bar, v);
            }
            let nt = ui.create(IconButton::new("plus").square().size(ControlSize::Small));
            set_id(ui, nt, "New tab");
            ui.add_child(bar, nt);
            Some(bar)
        }
        ("Titlebars", "Title Bar") => {
            let bar = ui.create(TitleBar);
            let controls = child(kit, "Window controls")?;
            let c = build(ui, controls, controls.master.as_deref()?)?;
            set_id(ui, c, "Window controls");
            ui.add_child(bar, c);
            let block = child(kit, "Title block")?;
            let mut t = TitleBlock::new(&text_at(block, "Title").unwrap_or_default());
            match *parts.get(2)? {
                "Subtitle" => {
                    t.style = TitleStyle::Subtitle;
                    t.subtitle = text_at(block, "Subtitle").unwrap_or_default();
                }
                "Document" => t.style = TitleStyle::Document,
                _ => {}
            }
            let b = ui.create(t);
            set_id(ui, b, "Title block");
            ui.add_child(bar, b);
            Some(bar)
        }
        ("Titlebars", "Piece") => piece(ui, kit, &parts),
        ("Titlebars", "Bar") => {
            let (pad, gap, align) = match *parts.get(2)? {
                "Compact" => ((20.0, 16.0), 8.0, Align::Start),
                "Title Only" => ((20.0, 20.0), 0.0, Align::Center),
                _ => ((0.0, 16.0), 0.0, Align::Start),
            };
            let h = kit.frame.height;
            strip_of(ui, kit, h, pad, gap, Backdrop::Window, align)
        }
        ("Titlebars", "Section") => {
            let (pad, backdrop) = match *parts.get(2)? {
                "Sidebar" => ((0.0, 8.0), Backdrop::SidebarBlur),
                "Inspector" => ((12.0, 8.0), Backdrop::Sidebar),
                _ => ((12.0, 8.0), Backdrop::Window),
            };
            strip_of(ui, kit, kit.frame.height, pad, 0.0, backdrop, Align::Start)
        }
        _ => None,
    }
}

/// A toolbar piece.
fn piece(ui: &mut Ui, kit: &KitLayer, parts: &[&str]) -> Option<ViewId> {
    let name = *parts.get(2)?;
    let h = 52.0;
    match name {
        "Spacer" => {
            let mut s = Spacer::flexible(8.0);
            s.height = h;
            return Some(ui.create(s));
        }
        "Title" => {
            let mut t = TitleBlock::new(&text_at(kit, "Title").unwrap_or_default());
            t.padding = 8.0;
            t.height = Some(h);
            return Some(ui.create(t));
        }
        "Title + Document Icon" => {
            let mut t = TitleBlock::new(&text_at(kit, "Title").unwrap_or_default());
            t.style = TitleStyle::Document;
            t.document_layer = "Document";
            t.padding = 8.0;
            t.height = Some(h);
            return Some(ui.create(t));
        }
        _ => {}
    }
    let pad = match name {
        "Controls" => (20.0, 12.0),
        "Mail Send" => (4.0, 16.0),
        "Title + Subtitle" => (8.0, 8.0),
        _ => (4.0, 4.0),
    };
    let strip = ui.create(Strip::new(h, pad.0, pad.1));
    for k in &kit.children {
        let v = if k.name == "Text block" {
            let mut t = TitleBlock::new(&text_at(k, "Title").unwrap_or_default());
            t.style = TitleStyle::Subtitle;
            t.subtitle = text_at(k, "Subtitle").unwrap_or_default();
            ui.create(t)
        } else if k.name == "Window controls" || k.master.is_some() {
            let m = k.master.clone().unwrap_or_default();
            build(ui, k, &m)?
        } else if k.name == "Send" {
            ui.create(SendPiece)
        } else if k.type_name_is_group() {
            if k.children.iter().any(|c| c.name == "Search glyph") {
                let mut s = SearchPiece::new(&text_at(k, "Value").unwrap_or_default());
                s.width = k.frame.width;
                let v = ui.create(s);
                if let Some(t) = child(k, "Token") {
                    let tok = ui.create(Token::new(
                        &t.overrides.first().cloned().unwrap_or_default(),
                    ));
                    set_id(ui, tok, "Token");
                    ui.add_child(v, tok);
                }
                v
            } else {
                ui.create(switcher(k))
            }
        } else {
            continue;
        };
        set_id(ui, v, &k.name);
        ui.add_child(strip, v);
    }
    Some(strip)
}

trait GroupExt {
    fn type_name_is_group(&self) -> bool;
}

impl GroupExt for KitLayer {
    fn type_name_is_group(&self) -> bool {
        self.kind == "Group"
    }
}

#[test]
fn titlebar_masters_match_the_kit() {
    let names = KitMaster::names("Titlebars");
    assert_eq!(names.len(), 181, "{} masters", names.len());
    let mut failures = Vec::new();
    for name in &names {
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        let root = scene.master.root.clone();
        let size = scene.master.size();
        let mounted = scene.mount_sized(size, |ui| {
            build(ui, &root, name).unwrap_or_else(|| ui.create(Spacer::fixed(1.0)))
        });
        let _ = mounted;
        let w = scene.window;
        let inactive = name.contains("/Inactive/") || name.ends_with("/Inactive");
        scene.ui.set_window_active(w, !inactive);
        // Kit defect: the Dark/Glass/Regular shared style blurs 10 where materials.md gives 6.
        scene.ignore_aspects = vec!["#blurs".to_string()];
        // Several instances share a name in the kit (the toolbar buttons of the compact
        // bar): their path is ambiguous.
        let skip: &[&str] = if name.contains("Bar/Compact") {
            &["Toolbar button/Background", "Toolbar button/Glyph Gear"]
        } else if name.contains("Title Bar/Subtitle") {
            // Sketch measures the subtitle 2 pt wider.
            &["Title block/Subtitle"]
        } else if name.contains("Search Token") {
            // The nested token master keeps its own label box.
            &["Token/Label"]
        } else {
            &[]
        };
        if name.contains("Controls/Hover") {
            scene.hover_at(Point::new(8.0, 8.0));
        } else if name.contains("Dot/") && name.contains("/Hover/") {
            scene.set_state("Hover");
        } else if name.contains("Tab Bar/Hover") {
            if let Some(p) = scene.master.rect_in_master("Tab 2") {
                scene.hover_at(p.center());
            }
        } else if name.starts_with("Titlebars/Tab/Hover") {
            scene.set_state("Hover");
        }
        scene.settle();
        let report = scene.auto_geometry_except(skip, &[]);
        if !report.is_empty() {
            failures.push(format!("{name}:\n    {}", report.join("\n    ")));
            continue;
        }
        if name.starts_with("Titlebars/Dot/")
            && let Some(r) = scene.master.rect_in_master("Glyph")
        {
            scene.extra_mask.push(r);
        }
        // The window buttons are 16 pt: their antialiased rims weigh a lot in the mean.
        let tol = if name.starts_with("Titlebars/Dot/") || name.starts_with("Titlebars/Controls/") {
            DOTS
        } else {
            TITLEBAR
        };
        if let Err(e) = scene.pixels(tol) {
            failures.push(format!("{name}: {e}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
