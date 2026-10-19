//! Helpers shared by the kit tests that build menus from the kit's masters.

#![allow(dead_code)]

use vetro_lib::amalfi::alerts::{AlertButton, AlertSpec, ButtonKind, alert};
use vetro_lib::amalfi::button::{PushButton, PushVariant};
use vetro_lib::amalfi::buttons::MenuButton;
use vetro_lib::amalfi::buttons::{IconButton, ToolbarButton};
use vetro_lib::amalfi::fields::TextField;
use vetro_lib::amalfi::fields::Token;
use vetro_lib::amalfi::forms::{MessageKind, form_footer, form_row, form_section};
use vetro_lib::amalfi::foundation::ControlSize;
use vetro_lib::amalfi::menus::{MenuItem, RowKind, menu};
use vetro_lib::amalfi::segments::{ActionSegments, Segment};
use vetro_lib::amalfi::sidebars::{Badge, Header, Panel, Row, footer, group, row};
use vetro_lib::amalfi::switch::Switch;
use vetro_lib::amalfi::text::TextRole;
use vetro_lib::amalfi::titlebars::{
    Backdrop, Dot, DotKind, SearchPiece, SendPiece, Spacer, Strip, SwitchItem, SwitcherPiece, Tab,
    TabBar, TitleBar, TitleBlock, TitleStyle, window_controls_view,
};
use vetro_lib::amalfi::toggles::{Toggle, ToggleState};
use vetro_lib::core::{Alignment, Stack};
use vetro_lib::core::{LayoutCx, Prop, Proposal, Ui, Value, ViewId, Widget};
use vetro_lib::geom::{Rect, Size};
use vetro_lib::text::Align;
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
            let p = ui.create(Panel::default());
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

// Titlebar views built from the kit.

pub fn first_text(kit: &KitLayer) -> Option<String> {
    kit.children.iter().find_map(|c| c.text.clone())
}

pub fn set_id(ui: &mut Ui, v: ViewId, name: &str) {
    let _ = ui.set(v, Prop::Identifier, Value::from(name));
}

/// A strip from the children of `kit` (sorted by x), with the given padding.
pub fn strip_of(
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
            titlebar_view(ui, k, &m)?
        };
        // The title and the spacer are plain layers of the bar in the kit.
        if k.master.is_some() {
            set_id(ui, v, &k.name);
        }
        ui.add_child(s, v);
    }
    Some(s)
}

pub fn action_segments(kit: &KitLayer, master: &str) -> ActionSegments {
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

pub fn switcher(group: &KitLayer) -> SwitcherPiece {
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
pub fn titlebar_view(ui: &mut Ui, kit: &KitLayer, master: &str) -> Option<ViewId> {
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
                let v = titlebar_view(ui, t, &m)?;
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
            let c = titlebar_view(ui, controls, controls.master.as_deref()?)?;
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
pub fn piece(ui: &mut Ui, kit: &KitLayer, parts: &[&str]) -> Option<ViewId> {
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
            titlebar_view(ui, k, &m)?
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

// Alert views built from the kit.

pub struct Swatch(pub Size);

impl Widget for Swatch {
    fn kind(&self) -> &'static str {
        "Swatch"
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        self.0
    }
}

pub fn kind_of(k: &KitLayer) -> ButtonKind {
    let m = k.master.as_deref().unwrap_or_default();
    if m.contains("/Primary/") {
        ButtonKind::Primary
    } else if m.contains("/Destructive/") {
        ButtonKind::Destructive
    } else {
        ButtonKind::Secondary
    }
}

pub fn alert_view(ui: &mut Ui, kit: &KitLayer, name: &str) -> ViewId {
    let icon_layer = child(kit, "App icon").or_else(|| child(kit, "Volume icon"));
    let icon = ui.create(Swatch(Size::new(56.0, 56.0)));
    if let Some(i) = icon_layer {
        set_id(ui, icon, &i.name);
    }
    let grouped = child(kit, "Text").is_some();
    let text = child(kit, "Text").unwrap_or(kit);
    // Accessories: whatever sits between the text and the buttons.
    let mut accessories = Vec::new();
    if let Some(f) = child(kit, "Fields") {
        let col = ui.create(Stack::column(8.0).aligned(Alignment::LEADING));
        let _ = ui.set(col, Prop::Width, Value::from(240.0));
        let mut fields: Vec<&KitLayer> = f.children.iter().collect();
        fields.sort_by(|a, b| a.frame.y.total_cmp(&b.frame.y));
        for k in fields {
            let tf =
                ui.create(TextField::text().with_text(&text_at(k, "Value").unwrap_or_default()));
            let _ = ui.set(tf, Prop::Width, Value::from(240.0));
            set_id(ui, tf, &k.name);
            ui.add_child(col, tf);
        }
        let _ = ui.set(
            col,
            Prop::Padding,
            Value::Insets(vetro_lib::geom::Insets {
                bottom: 4.0,
                ..vetro_lib::geom::Insets::ZERO
            }),
        );
        set_id(ui, col, "Fields");
        accessories.push(col);
    }
    if let Some(f) = child(kit, "Accessory field") {
        let tf =
            ui.create(TextField::text().with_placeholder(&text_at(f, "Value").unwrap_or_default()));
        let _ = ui.set(tf, Prop::Width, Value::from(240.0));
        set_id(ui, tf, "Accessory field");
        accessories.push(tf);
    }
    if let Some(s) = child(kit, "Suppression") {
        let t = ui.create(
            Toggle::checkbox()
                .label(&text_at(s, "Label").unwrap_or_default())
                .state(ToggleState::Off),
        );
        set_id(ui, t, "Suppression");
        accessories.push(t);
    }
    let buttons = child(kit, "Buttons");
    let mut list: Vec<&KitLayer> = buttons.map_or_else(Vec::new, |b| b.children.iter().collect());
    let stacked = buttons.is_some_and(|b| b.frame.height > 40.0);
    if stacked {
        list.sort_by(|a, b| a.frame.y.total_cmp(&b.frame.y));
    } else {
        list.sort_by(|a, b| a.frame.x.total_cmp(&b.frame.x));
    }
    let confirm = name.contains("/Confirm/");
    let wide = stacked && buttons.is_some_and(|b| b.frame.width >= 240.0);
    let spec = AlertSpec {
        icon: Some(icon),
        title: text_at(text, "Title").unwrap_or_default(),
        message: text_at(text, "Message"),
        grouped_text: grouped,
        accessories,
        buttons: list
            .iter()
            .map(|k| AlertButton::new(&k.name, kind_of(k)))
            .collect(),
        stacked,
        wide_buttons: wide,
        glass: name.contains("/Glass/"),
        badge: child(kit, "Badge disc").is_some(),
        message_role: confirm.then_some(TextRole::Tertiary),
        // Kit defect: Sketch fits the Confirm title in 240 pt where its text is 240.85 wide.
        text_width: confirm.then_some(241.0),
    };
    alert(ui, spec)
}
