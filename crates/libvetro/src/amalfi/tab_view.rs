//! Window tabs: a container whose children are the tabs' contents, with the kit's tab bar
//! (`Titlebars/Tab Bar`, `Apps/Mustache/Tab Bar`) above them while there are two tabs or
//! more. One content shows at a time.
//!
//! Signals: `ValueChanged` (the selected tab, `Int`), `CloseRequested` (a tab's ×, `Int`),
//! `Activated` (the new-tab button). ⌃Tab and ⌃⇧Tab move between tabs; ⌘1…⌘9 pick one.
//! Opening and closing tabs is the app's: it adds and removes contents and sets
//! `Prop::Items` (the titles) to match.

use crate::amalfi::buttons::IconButton;
use crate::amalfi::foundation::ControlSize;
use crate::amalfi::titlebars::{Tab, TabBar, metrics as bar};
use crate::core::a11y::{A11y, Role};
use crate::core::event::{Event, Handled, Key, Mods};
use crate::core::layout::Proposal;
use crate::core::tree::ViewId;
use crate::core::ui::Ui;
use crate::core::value::{Prop, Signal, Value};
use crate::core::widget::{EventCx, LayoutCx, PropChange, Widget};
use crate::geom::{Rect, Size};

/// The tab bar's inset from the container's sides (kit: `Apps/Mustache/Tab Bar`).
pub const BAR_INSET: f32 = 8.0;

/// Sets a property only when it changes (no layout requested for nothing).
fn set_if_changed(ui: &mut Ui, v: ViewId, prop: Prop, value: Value) {
    if ui.get(v, prop).as_ref() != Some(&value) {
        let _ = ui.set(v, prop, value);
    }
}

/// A tabbed container.
#[derive(Debug, Default)]
pub struct TabView {
    titles: Vec<String>,
    selected: usize,
    /// The bar shows with a single tab too.
    always_show_bar: bool,
    bar: Option<ViewId>,
    tabs: Vec<ViewId>,
    /// The tabs must be made again (titles changed).
    stale: bool,
}

impl TabView {
    /// Tabs with `titles`.
    pub fn new(titles: &[&str]) -> TabView {
        TabView {
            titles: titles.iter().map(|t| t.to_string()).collect(),
            stale: true,
            ..TabView::default()
        }
    }

    /// The selected tab.
    pub fn selected(&self) -> usize {
        self.selected
    }

    fn bar_visible(&self) -> bool {
        self.titles.len() >= 2 || self.always_show_bar
    }

    fn contents(&self, cx: &LayoutCx<'_>) -> Vec<ViewId> {
        let bar = self.bar;
        cx.ui
            .tree
            .children(cx.id())
            .iter()
            .copied()
            .filter(|c| Some(*c) != bar)
            .collect()
    }

    /// Makes the bar and its tabs (again).
    fn sync_bar(&mut self, ui: &mut Ui, me: ViewId) {
        let bar = match self.bar.filter(|b| ui.tree.contains(*b)) {
            Some(b) => b,
            None => {
                let b = ui.create(TabBar);
                let _ = ui.insert_child(me, 0, b);
                ui.release(b);
                let plus = ui.create(IconButton::new("plus").size(ControlSize::Small));
                let _ = ui.set(plus, Prop::Identifier, Value::from("New tab"));
                let _ = ui.set(plus, Prop::AccessibilityLabel, Value::from("New Tab"));
                ui.add_child(b, plus);
                ui.release(plus);
                ui.connect(plus, Signal::Activated, move |ui, _, _| {
                    ui.emit(me, Signal::Activated, Value::None);
                });
                self.bar = Some(b);
                self.stale = true;
                b
            }
        };
        if self.stale {
            for t in std::mem::take(&mut self.tabs) {
                ui.detach_now(t);
            }
            for (i, title) in self.titles.iter().enumerate() {
                let t = ui.create(Tab::new(title).selected(i == self.selected));
                let _ = ui.insert_child(bar, i, t);
                ui.release(t);
                ui.connect(t, Signal::Activated, move |ui, _, _| {
                    let changed = ui
                        .with_widget::<TabView, _>(me, |v| v.select(i))
                        .unwrap_or(false);
                    if changed {
                        ui.emit(me, Signal::ValueChanged, Value::from(i));
                    }
                });
                ui.connect(t, Signal::CloseRequested, move |ui, _, _| {
                    ui.emit(me, Signal::CloseRequested, Value::from(i));
                });
                self.tabs.push(t);
            }
            self.stale = false;
        }
        for (i, t) in self.tabs.iter().enumerate() {
            set_if_changed(ui, *t, Prop::State, Value::Bool(i == self.selected));
        }
        set_if_changed(ui, bar, Prop::Hidden, Value::Bool(!self.bar_visible()));
    }

    /// Selects tab `i`; returns whether it changed.
    fn select(&mut self, i: usize) -> bool {
        if i >= self.titles.len().max(1) || i == self.selected {
            return false;
        }
        self.selected = i;
        true
    }

    fn select_from_key(&mut self, cx: &mut EventCx<'_>, i: usize) -> Handled {
        if self.select(i) {
            cx.emit(Signal::ValueChanged, Value::from(i));
            cx.request_layout();
        }
        Handled::Yes
    }
}

impl Widget for TabView {
    fn kind(&self) -> &'static str {
        "TabView"
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        let top = if self.bar_visible() {
            bar::TAB_BAR_HEIGHT
        } else {
            0.0
        };
        let inner = Proposal::new(p.width, p.height.map(|h| (h - top).max(0.0)));
        let content = self
            .contents(cx)
            .get(self.selected)
            .copied()
            .map_or(Size::ZERO, |c| cx.measure(c, inner));
        p.replacing_unspecified(Size::new(content.width, content.height + top))
    }

    fn place(&mut self, cx: &mut LayoutCx<'_>, size: Size) {
        let me = cx.id();
        self.sync_bar(cx.ui, me);
        let top = if self.bar_visible() {
            bar::TAB_BAR_HEIGHT
        } else {
            0.0
        };
        if let Some(b) = self.bar
            && self.bar_visible()
        {
            cx.place(
                b,
                Rect::new(
                    BAR_INSET,
                    0.0,
                    (size.width - 2.0 * BAR_INSET).max(0.0),
                    bar::TAB_BAR_HEIGHT,
                ),
            );
        }
        let contents = self.contents(cx);
        for (i, c) in contents.iter().enumerate() {
            let shown = i == self.selected;
            set_if_changed(cx.ui, *c, Prop::Hidden, Value::Bool(!shown));
            if shown {
                cx.place(
                    *c,
                    Rect::new(0.0, top, size.width, (size.height - top).max(0.0)),
                );
            }
        }
    }

    fn event(&mut self, cx: &mut EventCx<'_>, event: &Event) -> Handled {
        let Event::Key(k) = event else {
            return Handled::No;
        };
        if !k.pressed {
            return Handled::No;
        }
        let n = self.titles.len();
        if n < 2 {
            return Handled::No;
        }
        match k.key {
            Key::Tab if k.mods.contains(Mods::CTRL) => {
                let i = if k.mods.contains(Mods::SHIFT) {
                    (self.selected + n - 1) % n
                } else {
                    (self.selected + 1) % n
                };
                self.select_from_key(cx, i)
            }
            Key::Char(c) if k.mods.contains(Mods::LOGO) && ('1'..='9').contains(&c) => {
                let d = c as usize - '1' as usize;
                // ⌘9 is the last tab.
                let i = if c == '9' { n - 1 } else { d.min(n - 1) };
                self.select_from_key(cx, i)
            }
            _ => Handled::No,
        }
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match prop {
            Prop::Items => match value.as_str_list() {
                Some(l) => {
                    self.titles = l.to_vec();
                    self.selected = self.selected.min(self.titles.len().saturating_sub(1));
                    self.stale = true;
                    PropChange::Layout
                }
                None => PropChange::Unknown,
            },
            Prop::SelectedIndex => match value.as_int().and_then(|i| usize::try_from(i).ok()) {
                Some(i) => {
                    self.selected = i.min(self.titles.len().saturating_sub(1));
                    PropChange::Layout
                }
                None => PropChange::Unknown,
            },
            Prop::HeaderVisible => match value.as_bool() {
                Some(b) => {
                    self.always_show_bar = b;
                    PropChange::Layout
                }
                None => PropChange::Unknown,
            },
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::Items => Some(Value::StrList(self.titles.clone())),
            Prop::SelectedIndex => Some(Value::from(self.selected)),
            Prop::HeaderVisible => Some(Value::Bool(self.always_show_bar)),
            _ => None,
        }
    }

    fn accessibility(&self, a: &mut A11y) {
        a.role = Role::Group;
        a.label = self.titles.get(self.selected).cloned();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::amalfi::label::Label;
    use crate::core::event::{Button, KeyEvent};
    use crate::core::window::WindowKind;
    use crate::geom::Point;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn tabs_switch_contents_and_report() {
        let mut ui = Ui::new_headless();
        let v = ui.create(TabView::new(&["One", "Two", "Three"]));
        let mut pages = Vec::new();
        for t in ["page 1", "page 2", "page 3"] {
            let p = ui.create(Label::new(t));
            ui.add_child(v, p);
            pages.push(p);
        }
        ui.set(v, Prop::MaxWidth, Value::from(f32::INFINITY))
            .unwrap();
        ui.set(v, Prop::MaxHeight, Value::from(f32::INFINITY))
            .unwrap();
        let w = ui.create_window(WindowKind::Normal, Size::new(600.0, 300.0));
        ui.set_content(w, v);
        ui.show_window(w);
        ui.prepare_frame(w);
        let log = Rc::new(RefCell::new(Vec::new()));
        for s in [
            Signal::ValueChanged,
            Signal::CloseRequested,
            Signal::Activated,
        ] {
            let l = log.clone();
            ui.connect(v, s, move |_, _, val| l.borrow_mut().push((s, val.clone())));
        }
        let hidden = |ui: &Ui, p: ViewId| ui.get(p, Prop::Hidden) == Some(Value::Bool(true));
        assert!(!hidden(&ui, pages[0]) && hidden(&ui, pages[1]));
        // The content sits under the bar.
        assert_eq!(ui.tree.window_rect(pages[0]).y, bar::TAB_BAR_HEIGHT);
        // Click the second tab (the bar: 8 in, tabs share the width).
        let r = ui.tree.window_rect(v);
        let p = Point::new(r.x + 8.0 + 4.0 + 200.0, r.y + 18.0);
        ui.pointer_move(w, p, Mods::empty());
        ui.pointer_down(w, p, Button::Left, Mods::empty());
        ui.pointer_up(w, p, Button::Left, Mods::empty());
        ui.prepare_frame(w);
        assert!(hidden(&ui, pages[0]) && !hidden(&ui, pages[1]));
        // ⌃Tab goes on, ⌘9 to the last.
        ui.set_focus(pages[1], false);
        ui.key(w, KeyEvent::press(Key::Tab, Mods::CTRL));
        ui.prepare_frame(w);
        assert!(!hidden(&ui, pages[2]));
        ui.key(w, KeyEvent::press(Key::Char('1'), Mods::LOGO));
        ui.prepare_frame(w);
        assert!(!hidden(&ui, pages[0]));
        let got: Vec<(Signal, Value)> = log.borrow().clone();
        assert_eq!(
            got,
            vec![
                (Signal::ValueChanged, Value::from(1usize)),
                (Signal::ValueChanged, Value::from(2usize)),
                (Signal::ValueChanged, Value::from(0usize)),
            ]
        );
        // One tab: no bar, the content takes the whole height.
        ui.set(v, Prop::Items, Value::StrList(vec!["One".into()]))
            .unwrap();
        ui.prepare_frame(w);
        assert_eq!(ui.tree.window_rect(pages[0]).y, 0.0);
    }
}
