//! Menus through the C ABI: built with `vetro_menu_*`, handed to the menu bar or a pop-up.

use std::ffi::{c_char, c_void};
use std::rc::Rc;

use vetro_sys::*;

use super::{UserData, enter, guard, opt_str, req_str, set_last, view_id, with_ui};
use crate::core::menu::{Menu, MenuItem, PopupKind};
use crate::geom::Rect;

/// A menu being built (behind `VetroMenu *`).
struct Building {
    menu: Menu,
}

fn building<'a>(m: *mut VetroMenu) -> Option<&'a mut Building> {
    if m.is_null() {
        None
    } else {
        // SAFETY: the pointer came from vetro_menu_new and was not freed or consumed.
        Some(unsafe { &mut *(m as *mut Building) })
    }
}

fn take(m: *mut VetroMenu) -> Option<Box<Building>> {
    if m.is_null() {
        None
    } else {
        // SAFETY: as above; ownership moves back to Rust exactly once.
        Some(unsafe { Box::from_raw(m as *mut Building) })
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn vetro_menu_new(title: *const c_char, role: *const c_char) -> *mut VetroMenu {
    guard(std::ptr::null_mut(), || {
        let (Ok(title), Ok(role)) = (opt_str(title), opt_str(role)) else {
            set_last(VETRO_ERROR_INVALID_ARGUMENT);
            return std::ptr::null_mut();
        };
        let b = Box::new(Building {
            menu: Menu {
                title: title.unwrap_or_default().to_string(),
                role: role.map(str::to_string),
                items: Vec::new(),
            },
        });
        set_last(VETRO_OK);
        Box::into_raw(b) as *mut VetroMenu
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vetro_menu_free(menu: *mut VetroMenu) {
    guard((), || drop(take(menu)));
}

#[unsafe(no_mangle)]
pub extern "C" fn vetro_menu_add_item(
    menu: *mut VetroMenu,
    title: *const c_char,
    shortcut: *const c_char,
    id: u32,
    flags: u32,
) -> VetroStatus {
    guard(VETRO_ERROR_PLATFORM, || {
        let (Some(b), Ok(title), Ok(shortcut)) =
            (building(menu), req_str(title), opt_str(shortcut))
        else {
            return set_last(VETRO_ERROR_INVALID_ARGUMENT);
        };
        if id == 0 && flags & VETRO_MENU_HEADING == 0 {
            return set_last(VETRO_ERROR_INVALID_ARGUMENT);
        }
        let mut item = if flags & VETRO_MENU_HEADING != 0 {
            MenuItem::heading(title)
        } else {
            MenuItem::new(title)
        };
        item.id = id;
        item.shortcut = shortcut.map(str::to_string);
        item.enabled = flags & VETRO_MENU_DISABLED == 0;
        item.checked = if flags & VETRO_MENU_CHECKED != 0 {
            Some(true)
        } else if flags & VETRO_MENU_MIXED != 0 {
            Some(false)
        } else {
            None
        };
        item.destructive = flags & VETRO_MENU_DESTRUCTIVE != 0;
        b.menu.items.push(item);
        set_last(VETRO_OK)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vetro_menu_add_separator(menu: *mut VetroMenu) -> VetroStatus {
    guard(VETRO_ERROR_PLATFORM, || match building(menu) {
        Some(b) => {
            b.menu.items.push(MenuItem::separator());
            set_last(VETRO_OK)
        }
        None => set_last(VETRO_ERROR_INVALID_ARGUMENT),
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vetro_menu_add_submenu(
    menu: *mut VetroMenu,
    submenu: *mut VetroMenu,
) -> VetroStatus {
    guard(VETRO_ERROR_PLATFORM, || {
        if menu == submenu {
            return set_last(VETRO_ERROR_INVALID_ARGUMENT);
        }
        let Some(b) = building(menu) else {
            return set_last(VETRO_ERROR_INVALID_ARGUMENT);
        };
        let Some(sub) = take(submenu) else {
            return set_last(VETRO_ERROR_INVALID_ARGUMENT);
        };
        b.menu
            .items
            .push(MenuItem::submenu(&sub.menu.title, sub.menu.items));
        set_last(VETRO_OK)
    })
}

/// Gives every item with an id an action calling `f(id)`.
fn wire(items: &mut [MenuItem], f: unsafe extern "C" fn(u32, *mut c_void), data: &Rc<UserData>) {
    for it in items {
        if let Some(sub) = &mut it.submenu {
            wire(sub, f, data);
        } else if it.id != 0 && !it.separator {
            let id = it.id;
            let d = data.clone();
            it.action = Some(Rc::new(move |ui| {
                let user = d.user;
                // SAFETY: the caller's callback with its own data.
                enter(ui, || unsafe { f(id, user) });
            }));
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn vetro_app_set_menu_bar(
    menus: *const *mut VetroMenu,
    count: usize,
    callback: VetroMenuFn,
    user: *mut c_void,
    destroy: VetroDestroy,
) -> VetroStatus {
    guard(VETRO_ERROR_PLATFORM, || {
        if menus.is_null() && count > 0 {
            return set_last(VETRO_ERROR_INVALID_ARGUMENT);
        }
        let data = Rc::new(UserData::new(user, destroy));
        let mut list = Vec::new();
        for i in 0..count {
            // SAFETY: the caller passes `count` menu pointers; each is consumed.
            let m = unsafe { *menus.add(i) };
            if let Some(mut b) = take(m) {
                if let Some(f) = callback {
                    wire(&mut b.menu.items, f, &data);
                }
                list.push(b.menu);
            }
        }
        match with_ui(|ui| ui.set_menu_bar(list)) {
            Ok(()) => set_last(VETRO_OK),
            Err(e) => set_last(e),
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vetro_view_popup_menu(
    view: *mut VetroView,
    menu: *mut VetroMenu,
    anchor: VetroRect,
    kind: u32,
    selected: u32,
    callback: VetroMenuFn,
    user: *mut c_void,
    destroy: VetroDestroy,
) -> VetroStatus {
    guard(VETRO_ERROR_PLATFORM, || {
        let Some(b) = take(menu) else {
            return set_last(VETRO_ERROR_INVALID_ARGUMENT);
        };
        let v = match view_id(view) {
            Ok(v) => v,
            Err(e) => return set_last(e),
        };
        let kind = if kind == VETRO_POPUP_POP_UP {
            PopupKind::PopUp
        } else {
            PopupKind::Context
        };
        let data = UserData::new(user, destroy);
        let done: Option<crate::core::menu::PopupDone> = callback.map(|f| {
            Box::new(move |ui: &mut crate::core::Ui, chosen: Option<u32>| {
                let d = data;
                // SAFETY: the caller's callback with its own data.
                enter(ui, || unsafe { f(chosen.unwrap_or(0), d.user) });
            }) as crate::core::menu::PopupDone
        });
        let rect = Rect::new(anchor.x, anchor.y, anchor.width, anchor.height);
        match with_ui(|ui| ui.popup_menu(v, rect, kind, selected, b.menu.items, done)) {
            Ok(Some(_)) => set_last(VETRO_OK),
            Ok(None) => set_last(VETRO_ERROR_NO_SUCH_OBJECT),
            Err(e) => set_last(e),
        }
    })
}
