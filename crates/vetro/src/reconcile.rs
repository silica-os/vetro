//! The reconciler: diffs a render's elements against the instances of the previous render
//! and issues the minimal operations on vetro-kit views (create, set changed properties,
//! swap handlers, insert, move, remove).

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::{Rc, Weak};

use vetro_kit::{Connection, Prop, Signal, View as KitView};

use crate::element::{Element, Handler, Key, PValue, What, same_what};
use crate::{Cx, Scheduler, Slots};

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
    Some(Instance {
        what: elem.what,
        view: Some(view),
        props: elem.props,
        handlers,
        children,
        key: elem.key,
        slots: None,
        child: None,
    })
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
    Some(Instance {
        what: elem.what,
        view: Some(view),
        props: elem.props,
        handlers,
        children,
        key: elem.key,
        slots: None,
        child: None,
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
