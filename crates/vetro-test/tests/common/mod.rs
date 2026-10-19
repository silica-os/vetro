//! Helpers shared by the kit tests that build menus from the kit's masters.

#![allow(dead_code)]

use vetro_lib::amalfi::menus::{MenuItem, RowKind, menu};
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
