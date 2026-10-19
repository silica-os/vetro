//! Helpers shared by the kit tests that build menus from the kit's masters.

#![allow(dead_code)]

use vetro_lib::amalfi::button::{PushButton, PushVariant};
use vetro_lib::amalfi::buttons::MenuButton;
use vetro_lib::amalfi::fields::TextField;
use vetro_lib::amalfi::forms::{MessageKind, form_footer, form_row, form_section};
use vetro_lib::amalfi::menus::{MenuItem, RowKind, menu};
use vetro_lib::amalfi::sidebars::{Badge, Header, Panel, Row, footer, group, row};
use vetro_lib::amalfi::switch::Switch;
use vetro_lib::amalfi::toggles::{Toggle, ToggleState};
use vetro_lib::core::{LayoutCx, Prop, Proposal, Ui, Value, ViewId, Widget};
use vetro_lib::geom::{Rect, Size};
use vetro_test::KitLayer;

pub fn text_at(layer: &KitLayer, path: &str) -> Option<String> {
    let mut cur = layer;
    for part in path.split('/') {
        cur = cur.children.iter().find(|c| c.name == part)?;
    }
    cur.text.clone()
}

pub fn has(layer: &KitLayer, name: &str) -> bool {
    layer.children.iter().any(|c| c.name == name)
}

/// A row view from a kit row (an item master or an instance of one).
pub fn row_view(ui: &mut Ui, kit: &KitLayer, master: &str) -> ViewId {
    let kind = if master.contains("/Separator/") {
        RowKind::Separator
    } else if master.contains("/Header/") {
        RowKind::Header
    } else {
        RowKind::Item
    };
    let mut item = match kind {
        RowKind::Separator => MenuItem::separator(),
        RowKind::Header => MenuItem::header(&text_at(kit, "Label").unwrap_or_default()),
        RowKind::Item => MenuItem::item(&text_at(kit, "Label").unwrap_or_default()),
    };
    if kind == RowKind::Item {
        if let Some(s) = text_at(kit, "Shortcut") {
            item = item.shortcut(&s);
        }
        if let Some(i) = text_at(kit, "Icon") {
            item = item.icon(&i);
        }
        item = item.checked(has(kit, "Check"));
        if has(kit, "Chevron") {
            item = item.submenu();
        }
        if master.contains("/Destructive/") {
            item = item.destructive();
        }
        item.highlighted = master.contains("/Highlighted/");
    }
    let v = ui.create(item);
    if master.contains("/Disabled/") {
        let _ = ui.set(v, Prop::Disabled, Value::Bool(true));
    }
    v
}

/// A menu from a kit menu (a master or an instance of one).
pub fn menu_view(ui: &mut Ui, kit: &KitLayer) -> ViewId {
    let mut rows: Vec<&KitLayer> = kit.children.iter().filter(|c| c.master.is_some()).collect();
    rows.sort_by(|a, b| a.frame.y.total_cmp(&b.frame.y));
    let (m, _) = menu(ui, &[]);
    for r in rows {
        let master = r.master.clone().unwrap_or_default();
        let v = row_view(ui, r, &master);
        let _ = ui.set(v, Prop::Identifier, Value::from(r.name.as_str()));
        ui.add_child(m, v);
    }
    m
}

/// Children at fixed rectangles (the cascade: a menu and its submenu).
pub struct Fixed {
    pub size: Size,
    pub rects: Vec<Rect>,
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
            let _ = cx.measure(c, Proposal::new(Some(r.width), None));
            cx.place(c, r);
        }
    }
}

// Forms built from the kit (shared by the forms and dialogs tests).

fn text_or_empty(layer: &KitLayer, path: &str) -> String {
    let mut cur = layer;
    for part in path.split('/') {
        match cur.children.iter().find(|c| c.name == part) {
            Some(c) => cur = c,
            None => return String::new(),
        }
    }
    cur.text.clone().unwrap_or_default()
}

/// A form row from a kit row instance (expanded).
pub fn build_row(ui: &mut Ui, row: &KitLayer) -> ViewId {
    let master = row.master.clone().unwrap_or_default();
    let label = text_or_empty(row, "Label cell/Label");
    let control = row
        .children
        .iter()
        .find(|c| c.name == "Value")
        .and_then(|v| v.children.iter().find(|c| c.name == "Control"));
    let cm = control.and_then(|c| c.master.clone()).unwrap_or_default();
    let on = cm.contains("/On/");
    let (view, expand) = if cm.contains("Fields/Text Field") {
        let value = control
            .map(|c| text_or_empty(c, "Value"))
            .unwrap_or_default();
        let mut f = TextField::text().with_text(&value);
        if cm.contains("/Error/") {
            f = f.with_error("Invalid");
        }
        (ui.create(f), true)
    } else if cm.contains("Toggles/Checkbox") {
        let text = control
            .map(|c| text_or_empty(c, "Label"))
            .unwrap_or_default();
        let state = if on {
            ToggleState::On
        } else {
            ToggleState::Off
        };
        (
            ui.create(Toggle::checkbox().label(&text).state(state)),
            false,
        )
    } else if cm.contains("Toggles/Switch") {
        let text = control
            .map(|c| text_or_empty(c, "Label"))
            .unwrap_or_default();
        (ui.create(Switch::new().on(on).label(&text)), true)
    } else {
        let text = control
            .map(|c| text_or_empty(c, "Label"))
            .unwrap_or_default();
        (
            ui.create(MenuButton::pop_up(&[text.as_str(), "Other"], 0)),
            false,
        )
    };
    let message = row
        .children
        .iter()
        .find(|c| c.name == "Value")
        .map(|v| text_or_empty(v, "Message"))
        .filter(|m| !m.is_empty());
    let kind = if master.contains("/Error/") {
        MessageKind::Error
    } else {
        MessageKind::Help
    };
    let v = form_row(
        ui,
        &label,
        view,
        message.as_deref().map(|m| (m, kind)),
        expand,
    );
    let _ = ui.set(v, Prop::Identifier, Value::from(row.name.as_str()));
    v
}

pub fn build_section(ui: &mut Ui, section: &KitLayer) -> ViewId {
    let title = section
        .children
        .iter()
        .find(|c| c.name == "Title")
        .and_then(|t| t.text.clone());
    let mut rows: Vec<&KitLayer> = section
        .children
        .iter()
        .find(|c| c.name == "Group")
        .map(|g| g.children.iter().filter(|c| c.master.is_some()).collect())
        .unwrap_or_default();
    rows.sort_by(|a, b| a.frame.y.total_cmp(&b.frame.y));
    let views: Vec<ViewId> = rows.iter().map(|r| build_row(ui, r)).collect();
    let v = form_section(ui, title.as_deref(), &views);
    let _ = ui.set(v, Prop::Identifier, Value::from(section.name.as_str()));
    v
}

pub fn build_footer(ui: &mut Ui, footer: &KitLayer) -> ViewId {
    let mut buttons: Vec<&KitLayer> = footer
        .children
        .iter()
        .filter(|c| c.master.is_some())
        .collect();
    buttons.sort_by(|a, b| a.frame.x.total_cmp(&b.frame.x));
    let views: Vec<ViewId> = buttons
        .iter()
        .map(|b| {
            let label = text_or_empty(b, "Label");
            let primary = b.master.as_deref().is_some_and(|m| m.contains("/Primary/"));
            let v = ui.create(PushButton::new(&label).variant(if primary {
                PushVariant::Primary
            } else {
                PushVariant::Secondary
            }));
            let _ = ui.set(v, Prop::Identifier, Value::from(b.name.as_str()));
            v
        })
        .collect();
    let f = form_footer(ui, &views);
    let _ = ui.set(f, Prop::Identifier, Value::from(footer.name.as_str()));
    f
}

// Sidebar views built from the kit (shared by the sidebars and dialogs tests).

pub fn child<'a>(kit: &'a KitLayer, name: &str) -> Option<&'a KitLayer> {
    kit.children.iter().find(|c| c.name == name)
}

/// A view from a kit layer (a master root or an instance of a sidebar master).
pub fn sidebar_view(ui: &mut Ui, kit: &KitLayer, master: &str) -> Option<ViewId> {
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
                let v = sidebar_view(ui, k, &m)?;
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
                let v = sidebar_view(ui, k, &m)?;
                let _ = ui.set(v, Prop::Identifier, Value::from(k.name.as_str()));
                ui.add_child(p, v);
            }
            p
        }
        _ => return None,
    };
    Some(v)
}
