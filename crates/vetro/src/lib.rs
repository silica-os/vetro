//! Vetro's declarative layer (SwiftUI-like): components return [`Element`] values from
//! `body`; state lives in [`State`] cells; a reconciler diffs each render against the previous
//! one and issues the minimal operations on vetro-kit objects. The heavy state (layout,
//! drawing, text, interaction) stays in libvetro.
//!
//! ```no_run
//! use vetro::prelude::*;
//!
//! struct Counter;
//!
//! impl Component for Counter {
//!     fn body(&self, cx: &mut Cx) -> Element {
//!         let count = cx.state(|| 0);
//!         column((
//!             label(format!("Count: {}", count.get())).text_style("Title 2"),
//!             button("Increment").on_activate(move || count.update(|c| *c += 1)),
//!         ))
//!         .spacing(12.0)
//!         .padding(20.0)
//!     }
//! }
//!
//! fn main() {
//!     let _ = vetro::run("dev.silica.Counter", "Counter", (320.0, 200.0), || Counter);
//! }
//! ```
//!
//! Identity is structural: a child keeps its view and state while it stays at the same
//! position with the same kind, or the same [`Key`] ([`for_each`]). State is positional
//! inside a component's `body` (the order of `cx.state` calls must not change between
//! renders, as with hooks). Any state change schedules one re-render of the root on the next
//! turn of the loop; the reconciler makes it cheap.

mod element;
mod reconcile;

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

pub use element::{
    Component, Element, Handler, IntoChildren, Key, PValue, button, column, component, custom,
    for_each, grid, group, host, label, overlay, row, spacer, symbol, text_view, view,
};
pub use vetro_kit as kit;

/// Everything an app needs.
pub mod prelude {
    pub use crate::{
        Binding, Component, Cx, Element, Model, State, button, column, component, custom, for_each,
        grid, group, host, label, overlay, row, spacer, symbol, view,
    };
    pub use vetro_kit::{Color, Signal, Value, rgba};
}

/// The scheduler of a root: re-renders once per loop turn after a change.
#[derive(Default)]
pub(crate) struct Scheduler {
    pending: Cell<bool>,
    render: RefCell<Option<Box<dyn Fn()>>>,
}

impl Scheduler {
    pub(crate) fn schedule(self: &Rc<Self>) {
        if self.pending.replace(true) {
            return;
        }
        let me = Rc::downgrade(self);
        let r = vetro_kit::after(0.0, move || {
            if let Some(s) = me.upgrade() {
                s.pending.set(false);
                let f = s.render.borrow_mut().take();
                if let Some(f) = f {
                    f();
                    let mut slot = s.render.borrow_mut();
                    if slot.is_none() {
                        *slot = Some(f);
                    }
                }
            }
        });
        if r.is_err() {
            self.pending.set(false);
        }
    }
}

/// The state cells of one component instance, in call order.
#[derive(Default)]
pub(crate) struct Slots {
    cells: RefCell<Vec<Rc<dyn Any>>>,
}

/// The context of a component's `body`.
pub struct Cx {
    slots: Rc<Slots>,
    next: usize,
    scheduler: Weak<Scheduler>,
}

impl Cx {
    pub(crate) fn new(slots: Rc<Slots>, scheduler: Weak<Scheduler>) -> Cx {
        Cx {
            slots,
            next: 0,
            scheduler,
        }
    }

    /// A state cell of this component, made with `init` on the first render.
    pub fn state<T: 'static>(&mut self, init: impl FnOnce() -> T) -> State<T> {
        let i = self.next;
        self.next += 1;
        let existing = self
            .slots
            .cells
            .borrow()
            .get(i)
            .cloned()
            .and_then(|c| c.downcast::<RefCell<T>>().ok());
        let cell = match existing {
            Some(c) => c,
            None => {
                let c: Rc<RefCell<T>> = Rc::new(RefCell::new(init()));
                let mut cells = self.slots.cells.borrow_mut();
                let any: Rc<dyn Any> = c.clone();
                if i < cells.len() {
                    if let Some(slot) = cells.get_mut(i) {
                        *slot = any;
                    }
                } else {
                    cells.push(any);
                }
                c
            }
        };
        State {
            cell,
            scheduler: self.scheduler.clone(),
        }
    }

    /// Whether the appearance is dark.
    pub fn dark(&self) -> bool {
        vetro_kit::is_dark()
    }

    /// Re-renders this component's root on the next turn of the loop.
    pub fn invalidate(&self) {
        if let Some(s) = self.scheduler.upgrade() {
            s.schedule();
        }
    }

    /// Subscribes this render to a model: a change of the model re-renders.
    pub fn watch<T: 'static>(&self, model: &Model<T>) {
        model.subscribe(self.scheduler.clone());
    }
}

/// A state cell: reading is free, writing schedules a re-render.
pub struct State<T> {
    cell: Rc<RefCell<T>>,
    scheduler: Weak<Scheduler>,
}

impl<T> Clone for State<T> {
    fn clone(&self) -> Self {
        State {
            cell: self.cell.clone(),
            scheduler: self.scheduler.clone(),
        }
    }
}

impl<T: Clone> State<T> {
    /// The value.
    pub fn get(&self) -> T {
        self.cell.borrow().clone()
    }
}

impl<T: 'static> State<T> {
    /// Reads the value.
    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        f(&self.cell.borrow())
    }

    /// Replaces the value.
    pub fn set(&self, v: T) {
        if let Ok(mut c) = self.cell.try_borrow_mut() {
            *c = v;
        }
        self.changed();
    }

    /// Changes the value in place.
    pub fn update(&self, f: impl FnOnce(&mut T)) {
        if let Ok(mut c) = self.cell.try_borrow_mut() {
            f(&mut c);
        }
        self.changed();
    }

    fn changed(&self) {
        if let Some(s) = self.scheduler.upgrade() {
            s.schedule();
        }
    }

    /// A binding to this state (for controls that edit a value).
    pub fn binding(&self) -> Binding<T>
    where
        T: Clone,
    {
        let a = self.clone();
        let b = self.clone();
        Binding {
            get: Rc::new(move || a.get()),
            set: Rc::new(move |v| b.set(v)),
        }
    }
}

/// A two-way connection to a value owned elsewhere.
pub struct Binding<T> {
    get: Rc<dyn Fn() -> T>,
    set: Rc<dyn Fn(T)>,
}

impl<T> Clone for Binding<T> {
    fn clone(&self) -> Self {
        Binding {
            get: self.get.clone(),
            set: self.set.clone(),
        }
    }
}

impl<T> Binding<T> {
    /// A binding from a getter and a setter.
    pub fn new(get: impl Fn() -> T + 'static, set: impl Fn(T) + 'static) -> Self {
        Binding {
            get: Rc::new(get),
            set: Rc::new(set),
        }
    }

    /// The value.
    pub fn get(&self) -> T {
        (self.get)()
    }

    /// Sets the value.
    pub fn set(&self, v: T) {
        (self.set)(v)
    }
}

/// An observable model shared by components (and by imperative code): every root that
/// watched it ([`Cx::watch`]) re-renders when it changes.
pub struct Model<T> {
    inner: Rc<ModelInner<T>>,
}

struct ModelInner<T> {
    value: RefCell<T>,
    watchers: RefCell<Vec<Weak<Scheduler>>>,
}

impl<T> Clone for Model<T> {
    fn clone(&self) -> Self {
        Model {
            inner: self.inner.clone(),
        }
    }
}

impl<T: 'static> Model<T> {
    /// A model holding `v`.
    pub fn new(v: T) -> Self {
        Model {
            inner: Rc::new(ModelInner {
                value: RefCell::new(v),
                watchers: RefCell::new(Vec::new()),
            }),
        }
    }

    /// Reads the value.
    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        f(&self.inner.value.borrow())
    }

    /// Changes the value and notifies the watchers.
    pub fn update(&self, f: impl FnOnce(&mut T)) {
        if let Ok(mut v) = self.inner.value.try_borrow_mut() {
            f(&mut v);
        }
        let watchers: Vec<Rc<Scheduler>> = {
            let mut w = self.inner.watchers.borrow_mut();
            w.retain(|s| s.strong_count() > 0);
            w.iter().filter_map(Weak::upgrade).collect()
        };
        for s in watchers {
            s.schedule();
        }
    }

    fn subscribe(&self, s: Weak<Scheduler>) {
        let mut w = self.inner.watchers.borrow_mut();
        if !w.iter().any(|x| x.ptr_eq(&s)) {
            w.push(s);
        }
    }
}

impl<T: Clone + 'static> Model<T> {
    /// The value.
    pub fn get(&self) -> T {
        self.inner.value.borrow().clone()
    }
}

/// A declarative tree mounted in an imperative view.
pub struct Root {
    inner: Rc<reconcile::RootInner>,
    scheduler: Rc<Scheduler>,
}

impl Root {
    /// Renders now (normally a re-render follows state changes on its own).
    pub fn render(&self) {
        self.inner.render(&self.scheduler);
    }

    /// The imperative view the tree is mounted in.
    pub fn host(&self) -> &vetro_kit::View {
        &self.inner.host
    }
}

/// Mounts a component made by `make` into `host` (the reverse escape hatch: declarative UI
/// inside an imperative view). The tree renders immediately.
pub fn mount<C: Component>(host: vetro_kit::View, make: impl Fn() -> C + 'static) -> Root {
    let scheduler = Rc::new(Scheduler::default());
    let inner = Rc::new(reconcile::RootInner::new(
        host,
        Rc::new(move || component(make())),
    ));
    let weak_inner = Rc::downgrade(&inner);
    let weak_sched = Rc::downgrade(&scheduler);
    *scheduler.render.borrow_mut() = Some(Box::new(move || {
        if let (Some(i), Some(s)) = (weak_inner.upgrade(), weak_sched.upgrade()) {
            i.render(&s);
        }
    }));
    let root = Root { inner, scheduler };
    root.render();
    root
}

thread_local! {
    static ROOTS: RefCell<Vec<Root>> = const { RefCell::new(Vec::new()) };
}

/// Opens a window titled `title` of `size` showing the component `make` builds, and runs the
/// app until it quits.
pub fn run<C: Component>(
    app_id: &str,
    title: &str,
    size: (f32, f32),
    make: impl Fn() -> C + 'static,
) -> vetro_kit::Result<()> {
    let title = title.to_string();
    vetro_kit::run(app_id, move || {
        if let Ok(root) = window(&title, size, make) {
            ROOTS.with(|r| r.borrow_mut().push(root));
        }
    })
}

/// Opens a window showing a component (inside [`run`] or a headless runtime); the returned
/// root keeps the tree alive.
pub fn window<C: Component>(
    title: &str,
    size: (f32, f32),
    make: impl Fn() -> C + 'static,
) -> vetro_kit::Result<Root> {
    let w = vetro_kit::Window::new(size.0, size.1)?;
    w.set_title(title)?;
    let host = vetro_kit::View::new(vetro_kit::Kind::VIEW)?;
    host.set_float(vetro_kit::Prop::MAX_WIDTH, f64::INFINITY)?;
    host.set_float(vetro_kit::Prop::MAX_HEIGHT, f64::INFINITY)?;
    w.set_content(&host)?;
    let root = mount(host, make);
    w.show()?;
    Ok(root)
}

/// Renders a component headlessly at `size` and returns a PNG at `scale` (the preview
/// harness, for tests and the inspector). Needs a headless runtime on this thread.
pub fn preview<C: Component>(
    size: (f32, f32),
    scale: f32,
    dark: bool,
    make: impl Fn() -> C + 'static,
) -> vetro_kit::Result<Vec<u8>> {
    vetro_kit::set_dark(dark)?;
    let w = vetro_kit::Window::new(size.0, size.1)?;
    let host = vetro_kit::View::new(vetro_kit::Kind::VIEW)?;
    host.set_float(vetro_kit::Prop::MAX_WIDTH, f64::INFINITY)?;
    host.set_float(vetro_kit::Prop::MAX_HEIGHT, f64::INFINITY)?;
    w.set_content(&host)?;
    let root = mount(host, make);
    w.show()?;
    let png = w.snapshot_png(scale);
    drop(root);
    w.close()?;
    png
}

#[cfg(test)]
mod tests;
