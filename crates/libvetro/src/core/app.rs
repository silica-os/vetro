//! The app: its identity, the standard menus every app has (silica-ui#99 semantics: the App
//! menu first, the Window and Help menus last, built once here), and the lifecycle events
//! the system sends (reopen, open files, quit) through Silica's app registry.

use crate::core::menu::{Menu, MenuItem};
use crate::core::ui::{Request, Ui};

/// Who the app is (the About panel, the menu bar, the registry).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AppInfo {
    /// Reverse-DNS id (`dev.frascella.Calculator`).
    pub id: String,
    /// Display name.
    pub name: String,
    /// Version.
    pub version: String,
    /// Copyright line.
    pub copyright: String,
    /// The app has a Quit item (a file manager may have none).
    pub quit_enabled: bool,
}

/// A lifecycle event.
#[derive(Debug, Clone, PartialEq)]
pub enum AppEvent {
    /// The app was launched again (Dock click, launcher): show a window.
    Reopen,
    /// Open these paths or URIs.
    Open(Vec<String>),
    /// Quit (log out, Force Quit's polite path, the registry).
    Quit,
    /// The screens changed ([`Ui::outputs`]).
    OutputsChanged,
    /// An event of Ossido's shell channel ([`Ui::connect_shell`]).
    Shell(crate::core::shell::ShellEvent),
}

/// A handler of lifecycle events.
pub type AppHandler = Box<dyn FnMut(&mut Ui, &AppEvent)>;

/// A standard item the menu bar knows: with an action the app runs it, without one the bar
/// runs the system roles (Quit, Hide, Minimize …) and shows the others disabled.
pub fn system_item(role: &str) -> MenuItem {
    let (title, shortcut) = match role {
        "about" => ("About", None),
        "settings" => ("Settings…", Some("cmd+,")),
        "services" => ("Services", None),
        "hide" => ("Hide", Some("cmd+h")),
        "hide_others" => ("Hide Others", Some("cmd+alt+h")),
        "show_all" => ("Show All", None),
        "quit" => ("Quit", Some("cmd+q")),
        "close" => ("Close", Some("cmd+w")),
        "close_all" => ("Close All", Some("cmd+alt+w")),
        "minimize" => ("Minimize", Some("cmd+m")),
        "fullscreen" => ("Enter Full Screen", Some("cmd+ctrl+f")),
        "bring_all_to_front" => ("Bring All to Front", None),
        "undo" => ("Undo", Some("cmd+z")),
        "redo" => ("Redo", Some("cmd+shift+z")),
        "cut" => ("Cut", Some("cmd+x")),
        "copy" => ("Copy", Some("cmd+c")),
        "paste" => ("Paste", Some("cmd+v")),
        "delete" => ("Delete", None),
        "select_all" => ("Select All", Some("cmd+a")),
        "help" => ("Help", Some("cmd+?")),
        other => (other, None),
    };
    let mut item = MenuItem::new(title).role(role);
    if let Some(s) = shortcut {
        item = item.shortcut(s);
    }
    item
}

/// The App menu: About, the app's `items` (with Settings when `settings`), Services, Hide,
/// Hide Others, Show All and Quit.
pub fn app_menu(info: &AppInfo, settings: Option<MenuItem>, items: Vec<MenuItem>) -> Menu {
    let mut v = vec![{
        let mut about = system_item("about");
        about.title = format!("About {}", info.name);
        about
    }];
    if settings.is_some() || !items.is_empty() {
        v.push(MenuItem::separator());
        v.extend(settings);
        v.extend(items);
    }
    v.push(MenuItem::separator());
    v.push(system_item("services"));
    v.push(MenuItem::separator());
    let mut hide = system_item("hide");
    hide.title = format!("Hide {}", info.name);
    v.push(hide);
    v.push(system_item("hide_others"));
    v.push(system_item("show_all"));
    if info.quit_enabled {
        v.push(MenuItem::separator());
        let mut quit = system_item("quit");
        quit.title = format!("Quit {}", info.name);
        quit.action = Some(std::rc::Rc::new(|ui: &mut Ui| ui.request(Request::Quit)));
        v.push(quit);
    }
    Menu {
        title: info.name.clone(),
        role: Some("app".into()),
        items: v,
    }
}

/// The Edit menu with the standard items (the focused text view handles them).
pub fn edit_menu(title: &str, items: Vec<MenuItem>) -> Menu {
    let mut v = vec![
        system_item("undo"),
        system_item("redo"),
        MenuItem::separator(),
        system_item("cut"),
        system_item("copy"),
        system_item("paste"),
        system_item("delete"),
        system_item("select_all"),
    ];
    if !items.is_empty() {
        v.push(MenuItem::separator());
        v.extend(items);
    }
    Menu {
        title: title.into(),
        role: Some("edit".into()),
        items: v,
    }
}

/// `menus` with the standard menus: the App menu first (when the app gives none), the
/// Window menu before Help, and the Help menu last. The app's own versions are kept.
pub fn with_standard(info: &AppInfo, menus: Vec<Menu>) -> Vec<Menu> {
    let has = |r: &str| menus.iter().any(|m| m.role.as_deref() == Some(r));
    let (has_app, has_window, has_help) = (has("app"), has("window"), has("help"));
    let mut out = Vec::new();
    if !has_app {
        out.push(app_menu(info, None, Vec::new()));
    }
    out.extend(menus);
    if !has_window {
        let at = out
            .iter()
            .position(|m| m.role.as_deref() == Some("help"))
            .unwrap_or(out.len());
        out.insert(
            at,
            Menu {
                title: "Window".into(),
                role: Some("window".into()),
                items: Vec::new(),
            },
        );
    }
    if !has_help {
        out.push(Menu {
            title: "Help".into(),
            role: Some("help".into()),
            items: Vec::new(),
        });
    }
    out
}

impl Ui {
    /// Sets who the app is.
    pub fn set_app_info(&mut self, info: AppInfo) {
        self.app_info = info;
    }

    /// Who the app is.
    pub fn app_info(&self) -> &AppInfo {
        &self.app_info
    }

    /// Handles lifecycle events (the default reopens the first window and quits on Quit).
    pub fn on_app_event(&mut self, f: impl FnMut(&mut Ui, &AppEvent) + 'static) {
        self.app_handler = Some(Box::new(f));
    }

    /// The system sent a lifecycle event.
    pub fn app_event(&mut self, e: AppEvent) {
        match self.app_handler.take() {
            Some(mut h) => {
                h(self, &e);
                if self.app_handler.is_none() {
                    self.app_handler = Some(h);
                }
            }
            None => match &e {
                AppEvent::Reopen => {
                    let first = self.windows().next().map(|w| w.id);
                    if let Some(w) = first {
                        self.show_window(w);
                    }
                }
                AppEvent::Quit => self.request(Request::Quit),
                AppEvent::Open(_) | AppEvent::OutputsChanged | AppEvent::Shell(_) => {}
            },
        }
        self.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_menus() {
        let info = AppInfo {
            name: "Clock".into(),
            quit_enabled: true,
            ..AppInfo::default()
        };
        let menus = with_standard(
            &info,
            vec![Menu {
                title: "View".into(),
                role: None,
                items: vec![],
            }],
        );
        let roles: Vec<Option<&str>> = menus.iter().map(|m| m.role.as_deref()).collect();
        assert_eq!(roles, [Some("app"), None, Some("window"), Some("help")]);
        let app = &menus[0];
        assert_eq!(app.items[0].title, "About Clock");
        assert_eq!(app.items.last().unwrap().title, "Quit Clock");
        assert_eq!(app.items.last().unwrap().shortcut.as_deref(), Some("cmd+q"));
        let no_quit = app_menu(&AppInfo::default(), None, vec![]);
        assert!(
            no_quit
                .items
                .iter()
                .all(|i| i.role.as_deref() != Some("quit"))
        );
    }

    #[test]
    fn default_events() {
        let mut ui = Ui::new_headless();
        ui.app_event(AppEvent::Quit);
        assert!(ui.take_requests().contains(&Request::Quit));
        let got = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let g = got.clone();
        ui.on_app_event(move |_, e| g.borrow_mut().push(e.clone()));
        ui.app_event(AppEvent::Open(vec!["/tmp/a".into()]));
        assert_eq!(
            got.borrow().as_slice(),
            [AppEvent::Open(vec!["/tmp/a".into()])]
        );
    }
}
