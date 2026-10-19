//! Menus: the app's menu bar and pop-up menus (context menus, pop-up and pull-down
//! buttons), always through the system menu service (silica-desktop#241, `silica-menu-v1`):
//! the shell draws them, the app describes them. Items carry ids; choosing one runs its
//! action. Shortcuts of the menu bar's items become app key equivalents.

use crate::core::event::Shortcut;
use crate::core::tree::ViewId;
use crate::core::ui::{Request, Ui};
use crate::core::window::WindowId;
use crate::geom::Rect;

/// An item's action.
pub type MenuAction = std::rc::Rc<dyn Fn(&mut Ui)>;

/// One row of a menu.
#[derive(Clone, Default)]
pub struct MenuItem {
    /// Id (assigned by [`Ui::set_menu_bar`] or [`Ui::popup_menu`] when 0).
    pub id: u32,
    /// Label.
    pub title: String,
    /// Symbol name (Silica Symbols) or absolute icon path.
    pub icon: Option<String>,
    /// Key equivalent (`cmd+s` …).
    pub shortcut: Option<String>,
    /// Enabled.
    pub enabled: bool,
    /// Checkmark: `Some(true)` checked, `Some(false)` mixed, `None` none.
    pub checked: Option<bool>,
    /// Destructive (drawn in red).
    pub destructive: bool,
    /// A system role (`about`, `quit`, `hide`, `preferences` …) the shell may treat specially.
    pub role: Option<String>,
    /// A separator row (the other fields are ignored).
    pub separator: bool,
    /// A section heading row.
    pub heading: bool,
    /// Submenu rows.
    pub submenu: Option<Vec<MenuItem>>,
    /// What choosing it does.
    pub action: Option<MenuAction>,
}

impl std::fmt::Debug for MenuItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MenuItem")
            .field("id", &self.id)
            .field("title", &self.title)
            .field("shortcut", &self.shortcut)
            .field("separator", &self.separator)
            .field("submenu", &self.submenu)
            .finish()
    }
}

impl MenuItem {
    /// An enabled item.
    pub fn new(title: &str) -> Self {
        MenuItem {
            title: title.to_string(),
            enabled: true,
            ..MenuItem::default()
        }
    }

    /// A separator.
    pub fn separator() -> Self {
        MenuItem {
            separator: true,
            ..MenuItem::default()
        }
    }

    /// A section heading.
    pub fn heading(title: &str) -> Self {
        MenuItem {
            title: title.to_string(),
            heading: true,
            ..MenuItem::default()
        }
    }

    /// A submenu.
    pub fn submenu(title: &str, items: Vec<MenuItem>) -> Self {
        MenuItem {
            submenu: Some(items),
            ..MenuItem::new(title)
        }
    }

    /// With a key equivalent.
    pub fn shortcut(mut self, s: &str) -> Self {
        self.shortcut = Some(s.to_string());
        self
    }

    /// With an action.
    pub fn action(mut self, f: impl Fn(&mut Ui) + 'static) -> Self {
        self.action = Some(std::rc::Rc::new(f));
        self
    }

    /// With a role.
    pub fn role(mut self, r: &str) -> Self {
        self.role = Some(r.to_string());
        self
    }

    /// Enabled or not.
    pub fn enabled(mut self, e: bool) -> Self {
        self.enabled = e;
        self
    }

    /// Checked state.
    pub fn checked(mut self, c: Option<bool>) -> Self {
        self.checked = c;
        self
    }

    /// With an icon.
    pub fn icon(mut self, name: &str) -> Self {
        self.icon = Some(name.to_string());
        self
    }
}

/// A top-level menu of the bar.
#[derive(Debug, Clone, Default)]
pub struct Menu {
    /// Title in the bar.
    pub title: String,
    /// `app` (the bold menu named after the app), `window`, `help`, or none.
    pub role: Option<String>,
    /// Rows.
    pub items: Vec<MenuItem>,
}

/// The kind of a pop-up menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupKind {
    /// A context menu at the pointer, or a pull-down below a control.
    Context,
    /// A pop-up button's menu, the selected item over the control.
    PopUp,
}

/// A pop-up menu waiting to be shown by the platform.
#[derive(Debug, Clone)]
pub struct PopupMenuRequest {
    /// Token to answer with [`Ui::popup_menu_result`].
    pub token: u64,
    /// The window it belongs to.
    pub window: WindowId,
    /// Anchor in window coordinates (a 0 × 0 rectangle at the pointer for context menus).
    pub anchor: Rect,
    /// Kind.
    pub kind: PopupKind,
    /// The id of the item a pop-up button shows (0: none).
    pub selected: u32,
    /// Rows (ids assigned).
    pub items: Vec<MenuItem>,
}

impl PartialEq for PopupMenuRequest {
    fn eq(&self, other: &Self) -> bool {
        self.token == other.token
    }
}

/// The menu state of the runtime.
#[derive(Default)]
pub struct MenuState {
    pub(crate) bar: Vec<Menu>,
    pub(crate) next_id: u32,
    pub(crate) bar_actions: Vec<(u32, MenuAction)>,
    pub(crate) popups: Vec<PendingPopup>,
    pub(crate) next_token: u64,
}

/// A pop-up menu shown and not yet answered: token, item actions, completion.
pub(crate) type PendingPopup = (u64, Vec<(u32, MenuAction)>, Option<PopupDone>);

/// Called when a pop-up menu ends: the chosen item's id, or `None` when dismissed.
pub type PopupDone = Box<dyn FnOnce(&mut Ui, Option<u32>)>;

fn assign(items: &mut [MenuItem], next: &mut u32, actions: &mut Vec<(u32, MenuAction)>) {
    for it in items {
        if it.separator || it.heading {
            continue;
        }
        if it.id == 0 {
            *next = next.wrapping_add(1).max(1);
            it.id = *next;
        }
        if let Some(a) = &it.action {
            actions.push((it.id, a.clone()));
        }
        if let Some(sub) = &mut it.submenu {
            assign(sub, next, actions);
        }
    }
}

fn shortcuts(items: &[MenuItem], out: &mut Vec<(String, u32)>) {
    for it in items {
        if let (Some(s), true) = (&it.shortcut, it.enabled) {
            out.push((s.clone(), it.id));
        }
        if let Some(sub) = &it.submenu {
            shortcuts(sub, out);
        }
    }
}

impl Ui {
    /// Sets the app's menu bar (sent to the menu service; key equivalents registered).
    pub fn set_menu_bar(&mut self, mut menus: Vec<Menu>) {
        let mut actions = Vec::new();
        let mut next = self.menus.next_id;
        for m in &mut menus {
            assign(&mut m.items, &mut next, &mut actions);
        }
        self.menus.next_id = next;
        let mut keys = Vec::new();
        for m in &menus {
            shortcuts(&m.items, &mut keys);
        }
        self.clear_menu_shortcuts();
        for (s, id) in keys {
            self.add_menu_shortcut(&s, id);
        }
        self.menus.bar_actions = actions;
        self.menus.bar = menus;
        self.request(Request::SetMenuBar);
    }

    /// The menu bar.
    pub fn menu_bar(&self) -> &[Menu] {
        &self.menus.bar
    }

    /// The menu service (or a key equivalent) chose a menu-bar item.
    pub fn menu_activated(&mut self, id: u32) {
        let action = self
            .menus
            .bar_actions
            .iter()
            .find(|(i, _)| *i == id)
            .map(|(_, a)| a.clone());
        if let Some(a) = action {
            a(self);
        }
        self.flush();
    }

    /// Shows a pop-up menu for `view` anchored at `anchor` (view coordinates); `done` gets
    /// the chosen id (after the item's own action ran) or `None`.
    pub fn popup_menu(
        &mut self,
        view: ViewId,
        anchor: Rect,
        kind: PopupKind,
        selected: u32,
        mut items: Vec<MenuItem>,
        done: Option<PopupDone>,
    ) -> Option<u64> {
        let window = self.window_of(view)?;
        let mut actions = Vec::new();
        let mut next = 0;
        assign(&mut items, &mut next, &mut actions);
        self.menus.next_token += 1;
        let token = self.menus.next_token;
        let origin = self.tree.window_origin(view);
        self.menus.popups.push((token, actions, done));
        self.request(Request::PopupMenu(Box::new(PopupMenuRequest {
            token,
            window,
            anchor: anchor.translate(origin),
            kind,
            selected,
            items,
        })));
        Some(token)
    }

    /// The platform's answer to a pop-up menu.
    pub fn popup_menu_result(&mut self, token: u64, chosen: Option<u32>) {
        let Some(pos) = self.menus.popups.iter().position(|(t, _, _)| *t == token) else {
            return;
        };
        let (_, actions, done) = self.menus.popups.remove(pos);
        if let Some(id) = chosen
            && let Some((_, a)) = actions.iter().find(|(i, _)| *i == id)
        {
            a(self);
        }
        if let Some(d) = done {
            d(self, chosen);
        }
        self.flush();
    }
}

/// The JSON model of `silica-menu-v1` (the protocol carries JSON in a memfd: a format owned
/// by silica-desktop, built here with its own types).
pub fn bar_model(menus: &[Menu]) -> silica_protocols::menu::model::Model {
    use silica_protocols::menu::model as m;
    fn item(it: &MenuItem) -> m::Item {
        if it.separator {
            return m::Item::separator();
        }
        let shortcut = it.shortcut.as_deref().and_then(Shortcut::parse).map(|s| {
            use crate::core::event::Mods;
            let key = match s.key.as_str() {
                "enter" => "Return".to_string(),
                "escape" => "Escape".to_string(),
                "left" => "Left".to_string(),
                "right" => "Right".to_string(),
                "up" => "Up".to_string(),
                "down" => "Down".to_string(),
                "backspace" => "BackSpace".to_string(),
                "delete" => "Delete".to_string(),
                "tab" => "Tab".to_string(),
                "space" => "space".to_string(),
                k => k.to_uppercase(),
            };
            m::Shortcut {
                key,
                meta: s.mods.contains(Mods::LOGO),
                ctrl: s.mods.contains(Mods::CTRL),
                alt: s.mods.contains(Mods::ALT),
                shift: s.mods.contains(Mods::SHIFT),
            }
        });
        m::Item {
            separator: false,
            id: Some(it.id),
            label: it.title.clone(),
            enabled: it.enabled && !it.heading,
            checked: it.checked == Some(true),
            role: it.role.clone().unwrap_or_default(),
            shortcut,
            items: it.submenu.as_ref().map(|s| s.iter().map(item).collect()),
        }
    }
    m::Model {
        menus: menus
            .iter()
            .map(|menu| m::Menu {
                label: menu.title.clone(),
                role: menu.role.clone().unwrap_or_default(),
                items: menu.items.iter().map(item).collect(),
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::event::{Key, KeyEvent, Mods};
    use crate::core::window::WindowKind;
    use crate::geom::Size;
    use std::cell::Cell;
    use std::rc::Rc;

    #[test]
    fn bar_ids_actions_and_key_equivalents() {
        let mut ui = Ui::new_headless();
        let win = ui.create_window(WindowKind::Normal, Size::new(100.0, 100.0));
        let hits = Rc::new(Cell::new(0));
        let h = hits.clone();
        ui.set_menu_bar(vec![Menu {
            title: "File".into(),
            role: None,
            items: vec![
                MenuItem::new("New")
                    .shortcut("cmd+n")
                    .action(move |_| h.set(h.get() + 1)),
                MenuItem::separator(),
                MenuItem::submenu("Recent", vec![MenuItem::new("a.txt")]),
            ],
        }]);
        assert!(ui.take_requests().contains(&Request::SetMenuBar));
        let id = ui.menu_bar()[0].items[0].id;
        assert_ne!(id, 0);
        ui.menu_activated(id);
        assert_eq!(hits.get(), 1);
        ui.key(win, KeyEvent::press(Key::Char('n'), Mods::LOGO));
        assert_eq!(hits.get(), 2);
        let model = bar_model(ui.menu_bar());
        assert_eq!(model.menus[0].items.len(), 3);
        assert_eq!(model.menus[0].items[0].shortcut.as_ref().unwrap().key, "N");
        assert!(model.menus[0].items[1].separator);
    }

    #[test]
    fn popup_result_runs_the_action_and_done() {
        let mut ui = Ui::new_headless();
        let win = ui.create_window(WindowKind::Normal, Size::new(100.0, 100.0));
        let root = ui.window(win).unwrap().root;
        let got = Rc::new(Cell::new(0u32));
        let g = got.clone();
        let g2 = got.clone();
        let token = ui
            .popup_menu(
                root,
                Rect::new(10.0, 10.0, 0.0, 0.0),
                PopupKind::Context,
                0,
                vec![MenuItem::new("Copy").action(move |_| g.set(g.get() + 10))],
                Some(Box::new(move |_, id| g2.set(g2.get() + id.unwrap_or(0)))),
            )
            .unwrap();
        let req = ui
            .take_requests()
            .into_iter()
            .find_map(|r| match r {
                Request::PopupMenu(p) => Some(p),
                _ => None,
            })
            .unwrap();
        let id = req.items[0].id;
        ui.popup_menu_result(token, Some(id));
        assert_eq!(got.get(), 10 + id);
        // A second answer is ignored.
        ui.popup_menu_result(token, None);
        assert_eq!(got.get(), 10 + id);
    }
}
