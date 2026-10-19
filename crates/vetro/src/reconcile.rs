//! The reconciler: diffs a render's elements against the instances of the previous render
//! and issues the minimal operations on vetro-kit views (create, set changed properties,
//! swap handlers, insert, move, remove).

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::{Rc, Weak};

use vetro_kit::{Connection, Prop, Signal, View as KitView};

use crate::element::{
    Effects, Element, Handler, Key, MenuEntry, PValue, Presentation, Rows, What, same_what,
};
use crate::{Component, Cx, Root, Scheduler, Slots};

/// A presentation's content as a component.
struct FnComponent(Rc<dyn Fn() -> Element>);

impl Component for FnComponent {
    fn body(&self, _cx: &mut Cx) -> Element {
        (self.0)()
    }
}

/// An open sheet or popover.
struct Shown {
    window: vetro_kit::Window,
    _root: Root,
    content: Rc<RefCell<Rc<dyn Fn() -> Element>>>,
    _close: Option<Connection>,
}

impl Drop for Shown {
    fn drop(&mut self) {
        let _ = self.window.close();
    }
}

/// The entries of a context menu, swapped each render.
type MenuCell = Rc<RefCell<Rc<Vec<MenuEntry>>>>;

/// The state of an element's effects.
type RowFn = Rc<RefCell<Rc<dyn Fn(usize) -> Element>>>;

/// The live rows of a virtualised list: each row is a tree of its own mounted in a host view
/// the list owns.
struct RowsState {
    count: usize,
    height: f32,
    row: RowFn,
    live: Rc<RefCell<Vec<Rc<RootInner>>>>,
}

#[derive(Default)]
struct EffectState {
    menu: Option<(MenuCell, Connection)>,
    rows: Option<RowsState>,
    sheet: Option<Shown>,
    popover: Option<Shown>,
}

/// A rendered element.
pub(crate) struct Instance {
    what: What,
    view: Option<KitView>,
    props: Vec<(Prop, PValue)>,
    handlers: Vec<(Signal, Rc<RefCell<Handler>>, Connection)>,
    children: Vec<Instance>,
    key: Option<Key>,
    slots: Option<Rc<Slots>>,
    child: Option<Box<Instance>>,
    effects: EffectState,
}

impl Instance {
    /// The kit view this instance shows (a component shows its body's).
    pub(crate) fn view(&self) -> Option<&KitView> {
        match &self.child {
            Some(c) => c.view(),
            None => self.view.as_ref(),
        }
    }

    fn remove(self) {
        if let Some(v) = self.view() {
            let _ = v.remove_from_parent();
        }
        self.dispose();
    }

    fn dispose(self) {
        if let Some((_, c)) = self.effects.menu {
            c.disconnect();
        }
        for (_, _, c) in self.handlers {
            c.disconnect();
        }
        for c in self.children {
            c.dispose();
        }
        if let Some(c) = self.child {
            c.dispose();
        }
    }
}

/// A component's body with the component element's own modifiers applied to its root.
fn body_of(elem: &Element, slots: &Rc<Slots>, sched: &Weak<Scheduler>) -> Element {
    let What::Component(c) = &elem.what else {
        return elem.clone();
    };
    let mut cx = Cx::new(slots.clone(), sched.clone());
    let mut body = c.body(&mut cx);
    for (p, v) in &elem.props {
        body = body.prop(*p, v.clone());
    }
    body.handlers.extend(elem.handlers.iter().cloned());
    if body.key.is_none() {
        body.key.clone_from(&elem.key);
    }
    body
}

fn connect(
    view: &KitView,
    signal: Signal,
    handler: Handler,
) -> Option<(Signal, Rc<RefCell<Handler>>, Connection)> {
    let cell = Rc::new(RefCell::new(handler));
    let c2 = cell.clone();
    let conn = view
        .on(signal, move |_, v| {
            // Cloned out so that the handler may change state (and re-render) freely.
            let h = c2.borrow().clone();
            h(v);
        })
        .ok()?;
    Some((signal, cell, conn))
}

pub(crate) fn create(elem: Element, sched: &Weak<Scheduler>) -> Option<Instance> {
    if let What::Component(_) = &elem.what {
        let slots = Rc::new(Slots::default());
        let body = body_of(&elem, &slots, sched);
        let child = create(body, sched)?;
        return Some(Instance {
            what: elem.what,
            view: None,
            props: Vec::new(),
            handlers: Vec::new(),
            children: Vec::new(),
            key: elem.key,
            slots: Some(slots),
            child: Some(Box::new(child)),
            effects: EffectState::default(),
        });
    }
    let view = match &elem.what {
        What::Kind(k) => KitView::new(*k).ok()?,
        What::Host(v) => v.clone(),
        What::Custom(_, make) => make()?,
        What::Component(_) => return None,
    };
    for (p, v) in &elem.props {
        v.apply(&view, *p);
    }
    let handlers = elem
        .handlers
        .iter()
        .filter_map(|(s, h)| connect(&view, *s, h.clone()))
        .collect();
    let children = if matches!(elem.what, What::Host(_)) {
        Vec::new()
    } else {
        reconcile_children(&view, Vec::new(), elem.children, sched)
    };
    let mut effects = EffectState::default();
    apply_effects(&view, &mut effects, &elem.effects, sched);
    Some(Instance {
        what: elem.what,
        view: Some(view),
        props: elem.props,
        handlers,
        children,
        key: elem.key,
        slots: None,
        child: None,
        effects,
    })
}

/// Opens, updates or closes the element's sheet, popover and context menu.
fn apply_effects(view: &KitView, state: &mut EffectState, want: &Effects, sched: &Weak<Scheduler>) {
    apply_rows(view, &mut state.rows, want.rows.as_ref(), sched);
    // Context menu: one connection; the entries are swapped each render.
    match (&want.menu, &mut state.menu) {
        (Some(entries), Some((cell, _))) => *cell.borrow_mut() = entries.clone(),
        (Some(entries), None) => {
            let cell = Rc::new(RefCell::new(entries.clone()));
            let c2 = cell.clone();
            let conn = view.on(Signal::CONTEXT_MENU, move |v, value| {
                let entries = c2.borrow().clone();
                let Ok(mut menu) = vetro_kit::Menu::new("", None) else {
                    return;
                };
                for (i, e) in entries.iter().enumerate() {
                    let next = if e.separator {
                        menu.separator()
                    } else {
                        let flags = if e.action.is_some() {
                            0
                        } else {
                            vetro_kit::menu_flags::DISABLED
                        };
                        menu.item(&e.title, e.shortcut.as_deref(), (i + 1) as u32, flags)
                    };
                    match next {
                        Ok(m) => menu = m,
                        Err(_) => return,
                    }
                }
                let frame = v.frame().unwrap_or_default();
                let (x, y) = match value {
                    vetro_kit::Value::Point(x, y) => (*x - frame.x, *y - frame.y),
                    _ => (frame.width / 2.0, frame.height / 2.0),
                };
                let anchor = vetro_kit::Rect {
                    x,
                    y,
                    width: 0.0,
                    height: 0.0,
                };
                let _ = v.popup_menu(menu, anchor, false, 0, move |id| {
                    if id == 0 {
                        return;
                    }
                    let i = (id - 1) as usize;
                    if let Some(a) = entries.get(i).and_then(|e| e.action.clone()) {
                        a();
                    }
                });
            });
            if let Ok(c) = conn {
                state.menu = Some((cell, c));
            }
        }
        (None, slot) => {
            if let Some((_, c)) = slot.take() {
                c.disconnect();
            }
        }
    }
    present(view, &mut state.sheet, want.sheet.as_ref(), true);
    present(view, &mut state.popover, want.popover.as_ref(), false);
}

/// Sets a list's rows (when their number or height changed) or renders the live rows again.
fn apply_rows(
    view: &KitView,
    state: &mut Option<RowsState>,
    want: Option<&Rows>,
    sched: &Weak<Scheduler>,
) {
    let Some(want) = want else {
        *state = None;
        return;
    };
    if let Some(st) = state.as_mut()
        && st.count == want.count
        && st.height == want.height
    {
        *st.row.borrow_mut() = want.row.clone();
        let Some(s) = sched.upgrade() else {
            return;
        };
        // Rows the list dropped have no parent any more.
        let live: Vec<Rc<RootInner>> = {
            let mut l = st.live.borrow_mut();
            l.retain(|r| r.host.parent().is_some());
            l.clone()
        };
        for r in live {
            r.render(&s);
        }
        return;
    }
    let row: RowFn = Rc::new(RefCell::new(want.row.clone()));
    let live: Rc<RefCell<Vec<Rc<RootInner>>>> = Rc::default();
    let (row2, live2, sched2) = (row.clone(), Rc::downgrade(&live), sched.clone());
    let set = view.set_rows(want.count, want.height, move |i| {
        let host = KitView::new(vetro_kit::Kind::VIEW).ok()?;
        let r = row2.clone();
        let root = Rc::new(RootInner::new(
            host.clone(),
            Rc::new(move || {
                let f = r.borrow().clone();
                f(i).fill()
            }),
        ));
        if let Some(s) = sched2.upgrade() {
            root.render(&s);
        }
        if let Some(l) = live2.upgrade() {
            let mut l = l.borrow_mut();
            l.retain(|r| r.host.parent().is_some());
            l.push(root);
        }
        Some(host)
    });
    if set.is_ok() {
        *state = Some(RowsState {
            count: want.count,
            height: want.height,
            row,
            live,
        });
    }
}

fn present(view: &KitView, slot: &mut Option<Shown>, want: Option<&Presentation>, sheet: bool) {
    let presented = want.is_some_and(|p| p.presented);
    match (presented, slot.as_ref()) {
        (true, Some(shown)) => {
            if let Some(p) = want {
                *shown.content.borrow_mut() = p.content.clone();
            }
        }
        (true, None) => {
            let Some(p) = want else { return };
            let Some(parent) = view.window() else { return };
            let window = if sheet {
                vetro_kit::Window::sheet(parent, p.size.0, p.size.1)
            } else {
                let f = view.frame().unwrap_or_default();
                vetro_kit::Window::popup(parent, f, p.size.0, p.size.1)
            };
            let Ok(window) = window else { return };
            let Ok(host) = KitView::new(vetro_kit::Kind::VIEW) else {
                return;
            };
            let _ = host.set_float(Prop::MAX_WIDTH, f64::INFINITY);
            let _ = host.set_float(Prop::MAX_HEIGHT, f64::INFINITY);
            let _ = window.set_content(&host);
            let content = Rc::new(RefCell::new(p.content.clone()));
            let c2 = content.clone();
            let root = crate::mount(host, move || FnComponent(c2.borrow().clone()));
            let dismissed = p.dismissed.clone();
            let close = window.root().and_then(|r| {
                let d = dismissed.clone();
                r.on(Signal::CLOSED, move |_, _| {
                    if let Some(d) = &d {
                        d();
                    }
                })
                .ok()
            });
            let _ = window.show();
            *slot = Some(Shown {
                window,
                _root: root,
                content,
                _close: close,
            });
        }
        (false, Some(_)) => {
            *slot = None;
        }
        (false, None) => {}
    }
}

/// Updates `old` to show `elem` (same kind, checked by the caller).
fn update(mut old: Instance, elem: Element, sched: &Weak<Scheduler>) -> Option<Instance> {
    if let What::Component(_) = &elem.what {
        let slots = old.slots.clone().unwrap_or_default();
        let body = body_of(&elem, &slots, sched);
        let child = match old.child.take() {
            Some(c) if same_what(&c.what, &body.what) && c.key == body.key => {
                update(*c, body, sched)?
            }
            Some(c) => {
                c.remove();
                create(body, sched)?
            }
            None => create(body, sched)?,
        };
        old.what = elem.what;
        old.slots = Some(slots);
        old.child = Some(Box::new(child));
        old.key = elem.key;
        return Some(old);
    }
    let view = old.view.clone()?;
    // Properties: set what changed, clear what is gone.
    for (p, v) in &elem.props {
        let prev = old.props.iter().find(|(q, _)| q == p).map(|(_, x)| x);
        if prev != Some(v) {
            v.apply(&view, *p);
        }
    }
    for (p, _) in &old.props {
        if !elem.props.iter().any(|(q, _)| q == p) {
            PValue::None.apply(&view, *p);
        }
    }
    // Handlers: swap the closures behind the existing connections.
    let mut handlers = Vec::new();
    let mut old_handlers = std::mem::take(&mut old.handlers);
    for (s, h) in &elem.handlers {
        match old_handlers.iter().position(|(os, _, _)| os == s) {
            Some(i) => {
                let entry = old_handlers.remove(i);
                *entry.1.borrow_mut() = h.clone();
                handlers.push(entry);
            }
            None => {
                if let Some(c) = connect(&view, *s, h.clone()) {
                    handlers.push(c);
                }
            }
        }
    }
    for (_, _, c) in old_handlers {
        c.disconnect();
    }
    let children = if matches!(elem.what, What::Host(_)) {
        Vec::new()
    } else {
        reconcile_children(
            &view,
            std::mem::take(&mut old.children),
            elem.children,
            sched,
        )
    };
    let mut effects = std::mem::take(&mut old.effects);
    apply_effects(&view, &mut effects, &elem.effects, sched);
    Some(Instance {
        what: elem.what,
        view: Some(view),
        props: elem.props,
        handlers,
        children,
        key: elem.key,
        slots: None,
        child: None,
        effects,
    })
}

/// Reconciles the children of `parent`: keyed children match by key, others by position;
/// then the kit view's children are put in order.
pub(crate) fn reconcile_children(
    parent: &KitView,
    old: Vec<Instance>,
    new: Vec<Element>,
    sched: &Weak<Scheduler>,
) -> Vec<Instance> {
    let mut keyed: HashMap<Key, Instance> = HashMap::new();
    let mut unkeyed: VecDeque<Instance> = VecDeque::new();
    for inst in old {
        match inst.key.clone() {
            Some(k) => {
                if let Some(dup) = keyed.insert(k, inst) {
                    dup.remove();
                }
            }
            None => unkeyed.push_back(inst),
        }
    }
    let mut out: Vec<Instance> = Vec::with_capacity(new.len());
    for elem in new {
        let candidate = match &elem.key {
            Some(k) => keyed.remove(k),
            None => unkeyed.pop_front(),
        };
        let inst = match candidate {
            Some(o) if same_what(&o.what, &elem.what) => update(o, elem, sched),
            Some(o) => {
                o.remove();
                create(elem, sched)
            }
            None => create(elem, sched),
        };
        if let Some(i) = inst {
            out.push(i);
        }
    }
    for (_, o) in keyed {
        o.remove();
    }
    for o in unkeyed {
        o.remove();
    }
    // Order: insert (which also moves) each view where it belongs.
    let mut current = parent.children();
    for (i, inst) in out.iter().enumerate() {
        let Some(v) = inst.view() else { continue };
        if current.get(i) != Some(v) {
            let _ = parent.insert(i, v);
            current = parent.children();
        }
    }
    // Views in the parent that no instance owns (removed elsewhere) are left alone.
    out
}

/// A mounted tree.
pub(crate) struct RootInner {
    pub(crate) host: KitView,
    make: Rc<dyn Fn() -> Element>,
    instance: RefCell<Vec<Instance>>,
}

impl RootInner {
    pub(crate) fn new(host: KitView, make: Rc<dyn Fn() -> Element>) -> Self {
        RootInner {
            host,
            make,
            instance: RefCell::new(Vec::new()),
        }
    }

    pub(crate) fn render(&self, scheduler: &Rc<Scheduler>) {
        let Ok(mut slot) = self.instance.try_borrow_mut() else {
            // A render inside a render: schedule another pass instead.
            scheduler.schedule();
            return;
        };
        let elem = (self.make)();
        let old = std::mem::take(&mut *slot);
        *slot = reconcile_children(&self.host, old, vec![elem], &Rc::downgrade(scheduler));
    }
}

impl Drop for RootInner {
    fn drop(&mut self) {
        for i in self.instance.take() {
            i.remove();
        }
    }
}
