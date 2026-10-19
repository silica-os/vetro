//! The runtime: the view tree, windows, properties, signals, focus, event dispatch,
//! animations, timers, and the frame (layout, damage, painting).
//!
//! The core never calls the platform: it queues [`Request`]s (set a title, a cursor, write
//! the clipboard …) that the platform layer drains after each dispatch. Signal handlers run
//! after an event has been handled, with no view borrowed, so they may change the tree
//! freely (re-entrancy safe for the C ABI too).

use std::collections::{HashMap, VecDeque};

use skia_safe::{Canvas, ClipOp};

use crate::amalfi::theme::Theme;
use crate::core::display::{DisplayList, Recorder};
use crate::core::env::Appearance;
use crate::core::event::{Button, Event, Handled, Key, KeyEvent, Mods, Phase};
use crate::core::layout::{Alignment, Proposal};
use crate::core::tree::{NodeFlags, Tree, ViewId};
use crate::core::value::{Prop, Signal, Value};
use crate::core::widget::{Empty, EventCx, FocusKind, Interaction, PaintCx, PropChange, Widget};
use crate::core::window::{Window, WindowId, WindowKind};
use crate::geom::{Point, Rect, Size, Transform};
use crate::shape::Shape;
use crate::style::{PaintOptions, Style};

/// An error of the object API (also the C ABI's error codes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum Error {
    /// The view or window does not exist (destroyed, or never created).
    NoSuchObject = -1,
    /// The property is not one this view has.
    UnknownProperty = -2,
    /// The value has the wrong type for the property.
    BadValue = -3,
    /// The operation would make a cycle in the tree, or the child already has a parent.
    BadHierarchy = -4,
    /// Called from another thread than the UI thread.
    WrongThread = -5,
    /// An argument is invalid (null pointer, bad UTF-8, out of range).
    InvalidArgument = -6,
    /// The platform failed (no display, no GPU).
    Platform = -7,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Error::NoSuchObject => "no such object",
            Error::UnknownProperty => "unknown property",
            Error::BadValue => "bad value",
            Error::BadHierarchy => "bad hierarchy",
            Error::WrongThread => "wrong thread",
            Error::InvalidArgument => "invalid argument",
            Error::Platform => "platform error",
        };
        f.write_str(s)
    }
}

impl std::error::Error for Error {}

/// A pointer shape (cursor-shape-v1 names).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Cursor {
    /// The arrow.
    Default,
    /// A pointing hand (links).
    Pointer,
    /// The I-beam.
    Text,
    /// Busy.
    Wait,
    /// Crosshair.
    Crosshair,
    /// Horizontal resize.
    ColResize,
    /// Vertical resize.
    RowResize,
    /// Grab.
    Grab,
    /// Grabbing.
    Grabbing,
    /// Not allowed.
    NotAllowed,
}

/// What the core asks of the platform.
#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    /// Map a new window.
    ShowWindow(WindowId),
    /// Unmap and destroy a window.
    CloseWindow(WindowId),
    /// The title changed.
    SetTitle(WindowId, String),
    /// Resize (app-initiated).
    Resize(WindowId, Size),
    /// The minimum and maximum size changed.
    SetSizeLimits(WindowId, Option<Size>, Option<Size>),
    /// A frame is wanted.
    Redraw(WindowId),
    /// The pointer shape.
    SetCursor(WindowId, Cursor),
    /// Write text to the clipboard.
    SetClipboard(String),
    /// Read the clipboard's text for a view (answer with [`Ui::deliver_paste`]).
    ReadClipboard(ViewId),
    /// Text input: enable at a caret rectangle (window coordinates), or disable.
    TextInput(WindowId, Option<Rect>),
    /// Surrounding text and selection for the input method.
    TextInputSurrounding(WindowId, String, usize, usize),
    /// Open a URI with the default handler.
    OpenUri(String),
    /// The audible alert.
    Beep,
    /// Leave the event loop.
    Quit,
    /// Start an interactive move of the window (title bar drag).
    StartMove(WindowId),
    /// The accessibility tree changed.
    AccessibilityChanged(WindowId),
}

/// A signal handler.
pub type Handler = Box<dyn FnMut(&mut Ui, ViewId, &Value)>;

/// An app shortcut's handler.
pub type ShortcutHandler = Box<dyn FnMut(&mut Ui)>;

/// A handler's id, to disconnect it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HandlerId(u64);

/// A timer's id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimerId(u64);

struct Timer {
    id: TimerId,
    deadline: f64,
    callback: Box<dyn FnOnce(&mut Ui)>,
}

/// Seconds between two clicks of a double click.
pub const DOUBLE_CLICK_INTERVAL: f64 = 0.5;
/// Distance within which clicks count as one multi-click.
pub const DOUBLE_CLICK_SLOP: f32 = 4.0;

/// The runtime.
pub struct Ui {
    /// The views.
    pub tree: Tree,
    windows: Vec<Window>,
    next_window: u64,
    /// The system appearance.
    pub appearance: Appearance,
    signals: VecDeque<(ViewId, Signal, Value)>,
    handlers: HashMap<(ViewId, Signal), Vec<(HandlerId, Handler)>>,
    next_handler: u64,
    requests: Vec<Request>,
    /// Monotonic time in seconds (set by the platform, or by tests).
    pub now: f64,
    animating: Vec<ViewId>,
    timers: Vec<Timer>,
    next_timer: u64,
    clipboard: Option<String>,
    /// Whether the platform serves the clipboard (headless: an in-process one).
    pub has_platform_clipboard: bool,
    flushing: bool,
    state_check: Vec<ViewId>,
    /// Global keyboard shortcuts of the app (menu key equivalents): shortcut → handler.
    shortcuts: Vec<(crate::core::event::Shortcut, ShortcutHandler)>,
    /// The tooltip presenter.
    pub(crate) tooltips: crate::amalfi::tooltips::Presenter,
}

impl Default for Ui {
    fn default() -> Self {
        Ui::new_headless()
    }
}

impl Ui {
    /// A runtime with no platform (tests, headless rendering).
    pub fn new_headless() -> Ui {
        Ui {
            tree: Tree::default(),
            windows: Vec::new(),
            next_window: 1,
            appearance: Appearance::default(),
            signals: VecDeque::new(),
            handlers: HashMap::new(),
            next_handler: 1,
            requests: Vec::new(),
            now: 0.0,
            animating: Vec::new(),
            timers: Vec::new(),
            next_timer: 1,
            clipboard: None,
            has_platform_clipboard: false,
            flushing: false,
            state_check: Vec::new(),
            shortcuts: Vec::new(),
            tooltips: crate::amalfi::tooltips::Presenter::default(),
        }
    }

    // ----- Views -------------------------------------------------------------------------

    /// Creates a view (one reference, the caller's).
    pub fn create(&mut self, widget: impl Widget + 'static) -> ViewId {
        self.tree.insert(Box::new(widget))
    }

    /// Creates a view from a boxed widget.
    pub fn create_boxed(&mut self, widget: Box<dyn Widget>) -> ViewId {
        self.tree.insert(widget)
    }

    /// Appends `child` to `parent`.
    pub fn add_child(&mut self, parent: ViewId, child: ViewId) {
        let n = self.tree.children(parent).len();
        let _ = self.insert_child(parent, n, child);
    }

    /// Inserts `child` at `index` among `parent`'s children.
    pub fn insert_child(
        &mut self,
        parent: ViewId,
        index: usize,
        child: ViewId,
    ) -> Result<(), Error> {
        if !self.tree.contains(parent) || !self.tree.contains(child) {
            return Err(Error::NoSuchObject);
        }
        if self.tree.is_ancestor_or_self(child, parent) {
            return Err(Error::BadHierarchy);
        }
        if self.tree.parent(child).is_some() {
            self.detach(child);
        }
        let window = self.tree.get(parent).and_then(|n| n.window);
        if let Some(p) = self.tree.get_mut(parent) {
            let i = index.min(p.children.len());
            p.children.insert(i, child);
        }
        if let Some(c) = self.tree.get_mut(child) {
            c.parent = Some(parent);
            c.refs = c.refs.saturating_add(1);
        }
        self.set_window_recursive(child, window);
        self.request_layout(parent);
        self.request_paint(child);
        Ok(())
    }

    /// Removes `child` from its parent (the parent's reference is dropped; a view nobody
    /// references any more is destroyed).
    pub fn detach(&mut self, child: ViewId) {
        let Some(parent) = self.tree.parent(child) else {
            return;
        };
        self.damage_subtree(child);
        if let Some(p) = self.tree.get_mut(parent) {
            p.children.retain(|c| *c != child);
        }
        if let Some(c) = self.tree.get_mut(child) {
            c.parent = None;
        }
        self.set_window_recursive(child, None);
        self.request_layout(parent);
        self.release(child);
    }

    /// Removes every child of `parent`.
    pub fn clear_children(&mut self, parent: ViewId) {
        for c in self.tree.children(parent).to_vec() {
            self.detach(c);
        }
    }

    /// Moves a child to another index among its siblings.
    pub fn move_child(&mut self, child: ViewId, index: usize) {
        let Some(parent) = self.tree.parent(child) else {
            return;
        };
        if let Some(p) = self.tree.get_mut(parent) {
            p.children.retain(|c| *c != child);
            let i = index.min(p.children.len());
            p.children.insert(i, child);
        }
        self.request_layout(parent);
        self.request_paint(parent);
    }

    /// Adds a reference (C handles).
    pub fn retain(&mut self, id: ViewId) -> bool {
        match self.tree.get_mut(id) {
            Some(n) => {
                n.refs = n.refs.saturating_add(1);
                true
            }
            None => false,
        }
    }

    /// Drops a reference; at zero the view and its subtree are destroyed.
    pub fn release(&mut self, id: ViewId) {
        let Some(n) = self.tree.get_mut(id) else {
            return;
        };
        n.refs = n.refs.saturating_sub(1);
        if n.refs == 0 {
            self.destroy(id);
        }
    }

    /// Destroys a view and its subtree now, whatever its references (stale ids are safe).
    pub fn destroy(&mut self, id: ViewId) {
        if let Some(parent) = self.tree.parent(id) {
            self.damage_subtree(id);
            if let Some(p) = self.tree.get_mut(parent) {
                p.children.retain(|c| *c != id);
            }
            self.request_layout(parent);
        }
        for v in self.tree.descendants(id) {
            self.forget_view(v);
            self.tree.remove(v);
        }
    }

    fn forget_view(&mut self, v: ViewId) {
        self.handlers.retain(|(view, _), _| *view != v);
        self.animating.retain(|a| *a != v);
        for w in &mut self.windows {
            if w.focus == Some(v) {
                w.focus = None;
            }
            if w.capture == Some(v) {
                w.capture = None;
            }
            w.hover.retain(|h| *h != v);
        }
    }

    fn set_window_recursive(&mut self, id: ViewId, window: Option<WindowId>) {
        let mut attached = Vec::new();
        for v in self.tree.descendants(id) {
            if let Some(n) = self.tree.get_mut(v) {
                if n.window.is_none() && window.is_some() {
                    attached.push(v);
                }
                n.window = window;
                n.flags.insert(NodeFlags::NEEDS_PAINT);
            }
        }
        for v in attached {
            if let Some(mut w) = self.tree.take_widget(v) {
                {
                    let mut cx = EventCx { ui: self, id: v };
                    w.attached(&mut cx);
                }
                self.tree.put_widget(v, w);
            }
        }
    }

    /// The window a view is shown in.
    pub fn window_of(&self, id: ViewId) -> Option<WindowId> {
        self.tree.get(id).and_then(|n| n.window)
    }

    /// Runs `f` on the widget as its concrete type, then repaints and lays out the view.
    pub fn with_widget<W: Widget, R>(
        &mut self,
        id: ViewId,
        f: impl FnOnce(&mut W) -> R,
    ) -> Option<R> {
        let r = self
            .tree
            .widget_mut(id)
            .and_then(|w| w.as_any_mut().downcast_mut::<W>())
            .map(f);
        if r.is_some() {
            self.request_layout(id);
            self.request_paint(id);
        }
        r
    }

    /// The widget as its concrete type.
    pub fn widget<W: Widget>(&self, id: ViewId) -> Option<&W> {
        self.tree
            .widget(id)
            .and_then(|w| w.as_any().downcast_ref::<W>())
    }

    /// Finds a view by its identifier in a subtree.
    pub fn find(&self, root: ViewId, identifier: &str) -> Option<ViewId> {
        self.tree
            .descendants(root)
            .into_iter()
            .find(|v| self.tree.get(*v).and_then(|n| n.identifier.as_deref()) == Some(identifier))
    }

    // ----- Properties --------------------------------------------------------------------

    /// Sets a property (generic or the component's).
    pub fn set(&mut self, id: ViewId, prop: Prop, value: Value) -> Result<(), Error> {
        let Some(node) = self.tree.get_mut(id) else {
            return Err(Error::NoSuchObject);
        };
        let m = &mut node.modifiers;
        let opt_len = |v: &Value| -> Result<Option<f32>, Error> {
            match v {
                Value::None => Ok(None),
                other => other.as_f32().map(Some).ok_or(Error::BadValue),
            }
        };
        let change = match prop {
            Prop::Padding => {
                m.padding = value.as_insets().ok_or(Error::BadValue)?;
                PropChange::Layout
            }
            Prop::Width => {
                m.width = opt_len(&value)?;
                PropChange::Layout
            }
            Prop::Height => {
                m.height = opt_len(&value)?;
                PropChange::Layout
            }
            Prop::MinWidth => {
                m.min_width = opt_len(&value)?;
                PropChange::Layout
            }
            Prop::MaxWidth => {
                m.max_width = opt_len(&value)?;
                PropChange::Layout
            }
            Prop::MinHeight => {
                m.min_height = opt_len(&value)?;
                PropChange::Layout
            }
            Prop::MaxHeight => {
                m.max_height = opt_len(&value)?;
                PropChange::Layout
            }
            Prop::FrameAlignment => {
                m.alignment = value
                    .as_int()
                    .and_then(Alignment::from_int)
                    .ok_or(Error::BadValue)?;
                PropChange::Layout
            }
            Prop::LayoutPriority => {
                m.priority = value.as_f32().ok_or(Error::BadValue)?;
                PropChange::Layout
            }
            Prop::Opacity => {
                m.opacity = value.as_f32().ok_or(Error::BadValue)?.clamp(0.0, 1.0);
                PropChange::Paint
            }
            Prop::Background => {
                m.background = match value {
                    Value::None => None,
                    v => Some(v.as_color().ok_or(Error::BadValue)?),
                };
                PropChange::Paint
            }
            Prop::CornerRadius => {
                m.corner_radius = value.as_f32().ok_or(Error::BadValue)?.max(0.0);
                PropChange::Paint
            }
            Prop::AspectRatio => {
                m.aspect_ratio = opt_len(&value)?;
                PropChange::Layout
            }
            Prop::Offset => {
                m.offset = match value {
                    Value::Point(p) => p,
                    _ => return Err(Error::BadValue),
                };
                PropChange::Paint
            }
            Prop::Scale => {
                m.scale = value.as_f32().ok_or(Error::BadValue)?;
                PropChange::Paint
            }
            Prop::Hidden => {
                let hidden = value.as_bool().ok_or(Error::BadValue)?;
                if hidden != node.flags.contains(NodeFlags::HIDDEN) {
                    self.damage_subtree(id);
                    if let Some(n) = self.tree.get_mut(id) {
                        n.flags.set(NodeFlags::HIDDEN, hidden);
                    }
                    if hidden {
                        self.drop_focus_inside(id);
                    }
                    if let Some(p) = self.tree.parent(id) {
                        self.request_layout(p);
                    }
                }
                PropChange::Layout
            }
            Prop::Disabled => {
                let d = value.as_bool().ok_or(Error::BadValue)?;
                node.flags.set(NodeFlags::DISABLED, d);
                if d {
                    node.flags.remove(NodeFlags::PRESSED | NodeFlags::HOVERED);
                    self.drop_focus_inside(id);
                }
                self.request_paint_subtree(id);
                PropChange::Paint
            }
            Prop::Identifier => {
                node.identifier = value.as_str().map(str::to_string);
                PropChange::Nothing
            }
            Prop::Tooltip => {
                node.tooltip = value.as_str().map(str::to_string);
                PropChange::Nothing
            }
            Prop::AccessibilityLabel => {
                node.a11y.label = value.as_str().map(str::to_string);
                PropChange::Nothing
            }
            Prop::AccessibilityHelp => {
                node.a11y.help = value.as_str().map(str::to_string);
                PropChange::Nothing
            }
            Prop::ClipsChildren => {
                node.flags
                    .set(NodeFlags::CLIPS, value.as_bool().ok_or(Error::BadValue)?);
                PropChange::Paint
            }
            Prop::AppearanceOverride => {
                node.dark_override = match value.as_int() {
                    Some(0) => None,
                    Some(1) => Some(false),
                    Some(2) => Some(true),
                    _ => return Err(Error::BadValue),
                };
                self.request_paint_subtree(id);
                PropChange::Paint
            }
            _ => match self.tree.widget_mut(id) {
                Some(w) => w.set_prop(prop, &value),
                None => PropChange::Unknown,
            },
        };
        match change {
            PropChange::Unknown => Err(Error::UnknownProperty),
            PropChange::Layout => {
                self.request_layout(id);
                self.request_paint(id);
                self.start_animating(id);
                Ok(())
            }
            PropChange::Paint => {
                self.request_paint(id);
                // A property can start an animation that runs on its own (an indeterminate
                // progress bar): a view with none stops ticking at once.
                self.start_animating(id);
                Ok(())
            }
            PropChange::Nothing => Ok(()),
        }
    }

    /// Reads a property.
    pub fn get(&self, id: ViewId, prop: Prop) -> Option<Value> {
        let n = self.tree.get(id)?;
        let m = &n.modifiers;
        let opt = |v: Option<f32>| v.map(Value::from).unwrap_or(Value::None);
        Some(match prop {
            Prop::Padding => Value::Insets(m.padding),
            Prop::Width => opt(m.width),
            Prop::Height => opt(m.height),
            Prop::MinWidth => opt(m.min_width),
            Prop::MaxWidth => opt(m.max_width),
            Prop::MinHeight => opt(m.min_height),
            Prop::MaxHeight => opt(m.max_height),
            Prop::FrameAlignment => Value::Int(m.alignment.to_int()),
            Prop::LayoutPriority => Value::from(m.priority),
            Prop::Opacity => Value::from(m.opacity),
            Prop::Hidden => Value::Bool(n.flags.contains(NodeFlags::HIDDEN)),
            Prop::Disabled => Value::Bool(n.flags.contains(NodeFlags::DISABLED)),
            Prop::Identifier => n.identifier.clone().map(Value::Str).unwrap_or_default(),
            Prop::Tooltip => n.tooltip.clone().map(Value::Str).unwrap_or_default(),
            Prop::AccessibilityLabel => n.a11y.label.clone().map(Value::Str).unwrap_or_default(),
            Prop::AccessibilityHelp => n.a11y.help.clone().map(Value::Str).unwrap_or_default(),
            Prop::ClipsChildren => Value::Bool(n.flags.contains(NodeFlags::CLIPS)),
            Prop::Background => m.background.map(Value::Color).unwrap_or_default(),
            Prop::CornerRadius => Value::from(m.corner_radius),
            Prop::AspectRatio => opt(m.aspect_ratio),
            Prop::Offset => Value::Point(m.offset),
            Prop::Scale => Value::from(m.scale),
            Prop::AppearanceOverride => Value::Int(match n.dark_override {
                None => 0,
                Some(false) => 1,
                Some(true) => 2,
            }),
            other => return n.widget.as_deref().and_then(|w| w.prop(other)),
        })
    }

    // ----- Invalidation -------------------------------------------------------------------

    /// Records the view's drawing again at the next frame.
    pub fn request_paint(&mut self, id: ViewId) {
        if !self.state_check.contains(&id) {
            self.state_check.push(id);
        }
        let window = match self.tree.get_mut(id) {
            Some(n) => {
                n.flags.insert(NodeFlags::NEEDS_PAINT);
                n.window
            }
            None => return,
        };
        if let Some(w) = window {
            self.request_frame(w);
        }
    }

    fn request_paint_subtree(&mut self, id: ViewId) {
        for v in self.tree.descendants(id) {
            if let Some(n) = self.tree.get_mut(v) {
                n.flags.insert(NodeFlags::NEEDS_PAINT);
            }
            if !self.state_check.contains(&v) {
                self.state_check.push(v);
            }
        }
        if let Some(w) = self.window_of(id) {
            self.request_frame(w);
        }
    }

    /// Lays the view (and its ancestors) out again at the next frame.
    pub fn request_layout(&mut self, id: ViewId) {
        let mut cur = Some(id);
        let mut window = None;
        let mut guard = 0;
        while let Some(v) = cur {
            let Some(n) = self.tree.get_mut(v) else { break };
            n.flags.insert(NodeFlags::NEEDS_LAYOUT);
            n.measure_cache.clear();
            window = window.or(n.window);
            cur = n.parent;
            guard += 1;
            if guard > 10_000 {
                break;
            }
        }
        if let Some(w) = window {
            if let Some(win) = self.window_mut(w) {
                win.needs_layout = true;
            }
            self.request_frame(w);
        }
    }

    /// Asks the platform for a frame of `window`.
    pub fn request_frame(&mut self, window: WindowId) {
        let Some(w) = self.window_mut(window) else {
            return;
        };
        if !w.needs_frame {
            w.needs_frame = true;
            self.requests.push(Request::Redraw(window));
        }
    }

    // ----- Signals -----------------------------------------------------------------------

    /// Calls `handler` whenever `id` emits `signal`.
    pub fn connect(
        &mut self,
        id: ViewId,
        signal: Signal,
        handler: impl FnMut(&mut Ui, ViewId, &Value) + 'static,
    ) -> HandlerId {
        let hid = HandlerId(self.next_handler);
        self.next_handler += 1;
        self.handlers
            .entry((id, signal))
            .or_default()
            .push((hid, Box::new(handler)));
        hid
    }

    /// Removes a handler.
    pub fn disconnect(&mut self, handler: HandlerId) {
        for list in self.handlers.values_mut() {
            list.retain(|(h, _)| *h != handler);
        }
    }

    /// Queues a signal; handlers run in [`Ui::flush`].
    pub fn emit(&mut self, id: ViewId, signal: Signal, value: Value) {
        self.signals.push_back((id, signal, value));
    }

    /// Runs the queued signals' handlers (and those they cause), then fires due timers.
    pub fn flush(&mut self) {
        if self.flushing {
            return;
        }
        self.flushing = true;
        self.notify_states();
        let mut budget = 10_000;
        while let Some((id, signal, value)) = self.signals.pop_front() {
            budget -= 1;
            if budget == 0 {
                tracing::warn!("signal storm: dropping the rest of the queue");
                self.signals.clear();
                break;
            }
            let Some(mut list) = self.handlers.remove(&(id, signal)) else {
                continue;
            };
            for (_, h) in &mut list {
                h(self, id, &value);
            }
            // Handlers connected meanwhile were added to a fresh entry: keep both.
            let entry = self.handlers.entry((id, signal)).or_default();
            let added = std::mem::take(entry);
            list.extend(added);
            if self.tree.contains(id) {
                *entry = list;
            } else {
                self.handlers.remove(&(id, signal));
            }
        }
        self.flushing = false;
    }

    /// Tells widgets whose interaction flags changed.
    fn notify_states(&mut self) {
        let mut rounds = 0;
        while !self.state_check.is_empty() && rounds < 8 {
            rounds += 1;
            for id in std::mem::take(&mut self.state_check) {
                let Some(n) = self.tree.get(id) else { continue };
                let mut now = n.flags & NodeFlags::INTERACTION;
                if self.is_disabled(id) {
                    now |= NodeFlags::DISABLED;
                }
                let old = n.seen_flags;
                if old == now {
                    continue;
                }
                if let Some(n) = self.tree.get_mut(id) {
                    n.seen_flags = now;
                }
                if let Some(mut w) = self.tree.take_widget(id) {
                    {
                        let mut cx = EventCx { ui: self, id };
                        w.state_changed(&mut cx, old, now);
                    }
                    self.tree.put_widget(id, w);
                }
            }
        }
    }

    // ----- Requests -----------------------------------------------------------------------

    /// Queues a platform request.
    pub fn request(&mut self, r: Request) {
        self.requests.push(r);
    }

    /// Takes the queued platform requests.
    pub fn take_requests(&mut self) -> Vec<Request> {
        std::mem::take(&mut self.requests)
    }

    /// Writes the clipboard.
    pub fn set_clipboard(&mut self, text: String) {
        self.clipboard = Some(text.clone());
        self.requests.push(Request::SetClipboard(text));
    }

    /// Asks for the clipboard's text for `view` (delivered as [`Event::Paste`]).
    pub fn request_paste(&mut self, view: ViewId) {
        if self.has_platform_clipboard {
            self.requests.push(Request::ReadClipboard(view));
        } else if let Some(text) = self.clipboard.clone() {
            self.deliver_paste(view, text);
        }
    }

    /// Delivers clipboard text read by the platform.
    pub fn deliver_paste(&mut self, view: ViewId, text: String) {
        self.send(view, &Event::Paste(text));
        self.flush();
    }

    // ----- Windows -----------------------------------------------------------------------

    /// Creates a window of `kind` and `size` with an empty root view; returns it (not yet
    /// shown).
    pub fn create_window(&mut self, kind: WindowKind, size: Size) -> WindowId {
        let id = WindowId(self.next_window);
        self.next_window += 1;
        let root = self.create(Empty);
        if let Some(n) = self.tree.get_mut(root) {
            n.flags.insert(NodeFlags::ROOT);
            n.window = Some(id);
        }
        self.windows.push(Window::new(id, root, kind, size));
        id
    }

    /// Replaces the root view's content: `view` fills the window.
    pub fn set_content(&mut self, window: WindowId, view: ViewId) {
        let Some(root) = self.window(window).map(|w| w.root) else {
            return;
        };
        self.clear_children(root);
        self.add_child(root, view);
    }

    /// Shows a window.
    pub fn show_window(&mut self, window: WindowId) {
        if let Some(w) = self.window_mut(window) {
            w.visible = true;
            w.needs_layout = true;
        }
        self.requests.push(Request::ShowWindow(window));
        self.request_frame(window);
    }

    /// Closes and destroys a window and its views.
    pub fn close_window(&mut self, window: WindowId) {
        let Some(pos) = self.windows.iter().position(|w| w.id == window) else {
            return;
        };
        // Transients go first.
        let children: Vec<WindowId> = self
            .windows
            .iter()
            .filter(|w| w.kind.parent() == Some(window))
            .map(|w| w.id)
            .collect();
        for c in children {
            self.close_window(c);
        }
        let Some(pos) = self
            .windows
            .iter()
            .position(|w| w.id == window)
            .or(Some(pos))
        else {
            return;
        };
        if pos >= self.windows.len() {
            return;
        }
        let w = self.windows.remove(pos);
        self.destroy(w.root);
        self.requests.push(Request::CloseWindow(window));
    }

    /// The window.
    pub fn window(&self, id: WindowId) -> Option<&Window> {
        self.windows.iter().find(|w| w.id == id)
    }

    /// The window, mutably.
    pub fn window_mut(&mut self, id: WindowId) -> Option<&mut Window> {
        self.windows.iter_mut().find(|w| w.id == id)
    }

    /// Every window.
    pub fn windows(&self) -> impl Iterator<Item = &Window> {
        self.windows.iter()
    }

    /// Sets the window's title.
    pub fn set_title(&mut self, window: WindowId, title: &str) {
        if let Some(w) = self.window_mut(window) {
            w.title = title.to_string();
        }
        self.requests
            .push(Request::SetTitle(window, title.to_string()));
    }

    /// The platform resized the window (or a test does).
    pub fn resize_window(&mut self, window: WindowId, size: Size, scale: f32) {
        let Some(w) = self.window_mut(window) else {
            return;
        };
        let scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
        if w.size == size && w.scale == scale {
            return;
        }
        let rescale = w.scale != scale;
        w.size = size;
        w.scale = scale;
        w.needs_layout = true;
        w.damage = Rect::from_size(size);
        let root = w.root;
        if rescale {
            self.request_paint_subtree(root);
        }
        self.request_layout(root);
    }

    /// Sets the minimum and maximum size.
    pub fn set_size_limits(&mut self, window: WindowId, min: Option<Size>, max: Option<Size>) {
        if let Some(w) = self.window_mut(window) {
            w.min_size = min;
            w.max_size = max;
        }
        self.requests.push(Request::SetSizeLimits(window, min, max));
    }

    /// The window became (in)active.
    pub fn set_window_active(&mut self, window: WindowId, active: bool) {
        let root = match self.window_mut(window) {
            Some(w) => {
                w.active = active;
                w.root
            }
            None => return,
        };
        self.request_paint_subtree(root);
    }

    // ----- Appearance --------------------------------------------------------------------

    /// Changes the system appearance; everything repaints.
    pub fn set_appearance(&mut self, appearance: Appearance) {
        if self.appearance == appearance {
            return;
        }
        self.appearance = appearance;
        let roots: Vec<ViewId> = self.windows.iter().map(|w| w.root).collect();
        for r in roots {
            self.request_paint_subtree(r);
            self.request_layout(r);
        }
    }

    /// The theme a view is drawn with (its own, its ancestors' or its window's override).
    pub fn theme_of(&self, id: ViewId) -> Theme {
        let mut appearance = self.appearance;
        let mut cur = Some(id);
        let mut found = None;
        let mut guard = 0;
        while let Some(v) = cur {
            let Some(n) = self.tree.get(v) else { break };
            if let Some(d) = n.dark_override {
                found = Some(d);
                break;
            }
            cur = n.parent;
            guard += 1;
            if guard > 10_000 {
                break;
            }
        }
        if found.is_none() {
            found = self
                .window_of(id)
                .and_then(|w| self.window(w))
                .and_then(|w| w.dark_override);
        }
        if let Some(d) = found {
            appearance.dark = d;
        }
        Theme::new(appearance)
    }

    /// Whether the view or an ancestor is disabled.
    pub fn is_disabled(&self, id: ViewId) -> bool {
        let mut cur = Some(id);
        let mut guard = 0;
        while let Some(v) = cur {
            let Some(n) = self.tree.get(v) else {
                return false;
            };
            if n.flags.contains(NodeFlags::DISABLED) {
                return true;
            }
            cur = n.parent;
            guard += 1;
            if guard > 10_000 {
                break;
            }
        }
        false
    }

    fn is_visible(&self, id: ViewId) -> bool {
        if self.tree.flags(id).contains(NodeFlags::HIDDEN) {
            return false;
        }
        !self
            .tree
            .ancestors(id)
            .iter()
            .any(|a| self.tree.flags(*a).contains(NodeFlags::HIDDEN))
    }

    // ----- Focus -------------------------------------------------------------------------

    /// Gives the keyboard focus to `id` (`keyboard`: from keyboard navigation, the halo
    /// shows).
    pub fn set_focus(&mut self, id: ViewId, keyboard: bool) {
        let Some(window) = self.window_of(id) else {
            return;
        };
        if self.is_disabled(id) || !self.is_visible(id) {
            return;
        }
        let old = self.window(window).and_then(|w| w.focus);
        if old == Some(id) {
            if let Some(n) = self.tree.get_mut(id)
                && keyboard
                && !n.flags.contains(NodeFlags::FOCUS_VISIBLE)
            {
                n.flags.insert(NodeFlags::FOCUS_VISIBLE);
                self.request_paint(id);
            }
            return;
        }
        if let Some(o) = old {
            if let Some(n) = self.tree.get_mut(o) {
                n.flags
                    .remove(NodeFlags::FOCUSED | NodeFlags::FOCUS_VISIBLE);
            }
            self.send(o, &Event::FocusOut);
            self.emit(o, Signal::FocusOut, Value::None);
            self.request_paint(o);
        }
        if let Some(w) = self.window_mut(window) {
            w.focus = Some(id);
            w.keyboard_navigation = keyboard;
        }
        if let Some(n) = self.tree.get_mut(id) {
            n.flags.insert(NodeFlags::FOCUSED);
            n.flags.set(NodeFlags::FOCUS_VISIBLE, keyboard);
        }
        self.send(id, &Event::FocusIn);
        self.emit(id, Signal::FocusIn, Value::None);
        self.request_paint(id);
        self.requests.push(Request::AccessibilityChanged(window));
    }

    /// Removes the focus from the window.
    pub fn clear_focus(&mut self, window: WindowId) {
        let old = self.window_mut(window).and_then(|w| w.focus.take());
        if let Some(o) = old {
            if let Some(n) = self.tree.get_mut(o) {
                n.flags
                    .remove(NodeFlags::FOCUSED | NodeFlags::FOCUS_VISIBLE);
            }
            self.send(o, &Event::FocusOut);
            self.emit(o, Signal::FocusOut, Value::None);
            self.request_paint(o);
        }
    }

    fn drop_focus_inside(&mut self, id: ViewId) {
        let Some(window) = self.window_of(id) else {
            return;
        };
        if let Some(f) = self.window(window).and_then(|w| w.focus)
            && self.tree.is_ancestor_or_self(id, f)
        {
            self.clear_focus(window);
        }
    }

    /// The focused view of a window.
    pub fn focus(&self, window: WindowId) -> Option<ViewId> {
        self.window(window).and_then(|w| w.focus)
    }

    /// The views of the key view loop, in order.
    pub fn key_view_loop(&self, window: WindowId) -> Vec<ViewId> {
        let Some(root) = self.window(window).map(|w| w.root) else {
            return Vec::new();
        };
        let fka = self.appearance.full_keyboard_access;
        let mut out = Vec::new();
        let mut stack = vec![root];
        while let Some(v) = stack.pop() {
            let Some(n) = self.tree.get(v) else { continue };
            if n.flags.intersects(NodeFlags::HIDDEN | NodeFlags::DISABLED) {
                continue;
            }
            let kind = n
                .widget
                .as_deref()
                .map(|w| w.focus())
                .unwrap_or(FocusKind::None);
            if kind == FocusKind::Always || (kind == FocusKind::Control && fka) {
                out.push(v);
            }
            for c in n.children.iter().rev() {
                stack.push(*c);
            }
        }
        out
    }

    /// Moves the focus to the next (or previous) view of the key view loop.
    pub fn focus_next(&mut self, window: WindowId, backwards: bool) -> bool {
        let lp = self.key_view_loop(window);
        if lp.is_empty() {
            return false;
        }
        let cur = self
            .focus(window)
            .and_then(|f| lp.iter().position(|v| *v == f));
        let n = lp.len();
        let next = match (cur, backwards) {
            (None, false) => 0,
            (None, true) => n - 1,
            (Some(i), false) => (i + 1) % n,
            (Some(i), true) => (i + n - 1) % n,
        };
        match lp.get(next) {
            Some(v) => {
                self.set_focus(*v, true);
                true
            }
            None => false,
        }
    }

    // ----- Shortcuts ----------------------------------------------------------------------

    /// Registers an app-wide keyboard shortcut (a menu item's key equivalent).
    pub fn add_shortcut(&mut self, shortcut: &str, f: impl FnMut(&mut Ui) + 'static) -> bool {
        match crate::core::event::Shortcut::parse(shortcut) {
            Some(sc) => {
                self.shortcuts.push((sc, Box::new(f)));
                true
            }
            None => false,
        }
    }

    // ----- Events ------------------------------------------------------------------------

    /// Delivers an event to one view (no bubbling); returns whether it handled it.
    pub fn send(&mut self, id: ViewId, event: &Event) -> bool {
        let Some(mut w) = self.tree.take_widget(id) else {
            return false;
        };
        let handled = {
            let mut cx = EventCx { ui: self, id };
            w.event(&mut cx, event)
        };
        self.tree.put_widget(id, w);
        handled == Handled::Yes
    }

    /// Delivers an event to `id` and then up its ancestors until one handles it; pointer
    /// positions are converted into each view's coordinates.
    pub fn send_bubbling(&mut self, id: ViewId, event: &Event) -> Option<ViewId> {
        let mut chain = vec![id];
        chain.extend(self.tree.ancestors(id));
        let origin = self.tree.window_origin(id);
        for v in chain {
            if self.is_disabled(v) {
                continue;
            }
            let local = self.localize(event, v, origin);
            if self.send(v, &local) {
                return Some(v);
            }
        }
        None
    }

    fn localize(&self, event: &Event, view: ViewId, from_origin: Point) -> Event {
        let delta = from_origin - self.tree.window_origin(view);
        let shift = |p: Point| p + delta;
        match event {
            Event::PointerDown {
                pos,
                button,
                clicks,
                mods,
            } => Event::PointerDown {
                pos: shift(*pos),
                button: *button,
                clicks: *clicks,
                mods: *mods,
            },
            Event::PointerUp { pos, button, mods } => Event::PointerUp {
                pos: shift(*pos),
                button: *button,
                mods: *mods,
            },
            Event::PointerMove {
                pos,
                mods,
                dragging,
            } => Event::PointerMove {
                pos: shift(*pos),
                mods: *mods,
                dragging: *dragging,
            },
            Event::Scroll {
                pos,
                dx,
                dy,
                precise,
                phase,
            } => Event::Scroll {
                pos: shift(*pos),
                dx: *dx,
                dy: *dy,
                precise: *precise,
                phase: *phase,
            },
            Event::Drop { pos, items } => Event::Drop {
                pos: shift(*pos),
                items: items.clone(),
            },
            other => other.clone(),
        }
    }

    /// The deepest view under `pos` (window coordinates) that accepts hits, and the
    /// position in its coordinates.
    pub fn hit_test(&self, window: WindowId, pos: Point) -> Option<(ViewId, Point)> {
        let root = self.window(window)?.root;
        self.hit_view(root, pos)
    }

    fn hit_view(&self, id: ViewId, p: Point) -> Option<(ViewId, Point)> {
        let n = self.tree.get(id)?;
        if n.flags.contains(NodeFlags::HIDDEN) || n.modifiers.opacity <= 0.0 {
            return None;
        }
        let mut local = p - n.frame.origin() - n.modifiers.offset;
        let s = n.modifiers.scale;
        if (s - 1.0).abs() > 1e-6 && s > 0.0 {
            let c = Point::new(n.frame.width / 2.0, n.frame.height / 2.0);
            local = c + (local - c) * (1.0 / s);
        }
        let inside = Rect::from_size(n.frame.size()).contains(local);
        if n.flags.contains(NodeFlags::CLIPS) && !inside {
            return None;
        }
        for c in n.children.iter().rev() {
            if let Some(hit) = self.hit_view(*c, local) {
                return Some(hit);
            }
        }
        if !inside {
            return None;
        }
        let hits = match n.widget.as_deref() {
            Some(w) => w.hit(local, n.frame.size()) || n.modifiers.background.is_some(),
            None => false,
        };
        hits.then_some((id, local))
    }

    fn interaction_of(&self, id: ViewId) -> Interaction {
        self.tree
            .widget(id)
            .map(|w| w.interaction())
            .unwrap_or(Interaction::None)
    }

    /// The nearest view from `id` up that has a built-in interaction.
    fn interactive_ancestor(&self, id: ViewId) -> Option<ViewId> {
        let mut chain = vec![id];
        chain.extend(self.tree.ancestors(id));
        chain.into_iter().find(|v| {
            matches!(
                self.interaction_of(*v),
                Interaction::Button | Interaction::Toggle
            )
        })
    }

    /// The pointer moved to `pos` (window coordinates).
    pub fn pointer_move(&mut self, window: WindowId, pos: Point, mods: Mods) {
        let Some(w) = self.window_mut(window) else {
            return;
        };
        w.pointer = Some(pos);
        let capture = w.capture;
        if let Some(c) = capture
            && self.tree.contains(c)
        {
            let origin = self.tree.window_origin(c);
            let ev = Event::PointerMove {
                pos: pos - origin,
                mods,
                dragging: true,
            };
            self.send(c, &ev);
            // Pressed only while the pointer is still inside.
            let inside = self.tree.window_rect(c).contains(pos);
            if let Some(n) = self.tree.get_mut(c)
                && matches!(
                    n.widget.as_deref().map(|w| w.interaction()),
                    Some(Interaction::Button | Interaction::Toggle)
                )
                && n.flags.contains(NodeFlags::PRESSED) != inside
            {
                n.flags.set(NodeFlags::PRESSED, inside);
                self.request_paint(c);
            }
        }
        self.update_hover(window, Some(pos), mods);
        if capture.is_none()
            && let Some((target, local)) = self.hit_test(window, pos)
        {
            let ev = Event::PointerMove {
                pos: local,
                mods,
                dragging: false,
            };
            self.send_bubbling(target, &self.localize_back(&ev, target));
        }
        self.flush();
    }

    fn localize_back(&self, ev: &Event, _target: ViewId) -> Event {
        ev.clone()
    }

    fn update_hover(&mut self, window: WindowId, pos: Option<Point>, _mods: Mods) {
        let new_chain: Vec<ViewId> = match pos.and_then(|p| self.hit_test(window, p)) {
            Some((v, _)) => {
                let mut c = self.tree.ancestors(v);
                c.reverse();
                c.push(v);
                c
            }
            None => Vec::new(),
        };
        let old_chain = self
            .window(window)
            .map(|w| w.hover.clone())
            .unwrap_or_default();
        if old_chain == new_chain {
            return;
        }
        for v in old_chain.iter().rev() {
            if !new_chain.contains(v) {
                if let Some(n) = self.tree.get_mut(*v) {
                    n.flags.remove(NodeFlags::HOVERED);
                }
                self.send(*v, &Event::PointerLeave);
                self.emit(*v, Signal::PointerExited, Value::None);
                self.request_paint(*v);
            }
        }
        for v in &new_chain {
            if !old_chain.contains(v) && !self.is_disabled(*v) {
                if let Some(n) = self.tree.get_mut(*v) {
                    n.flags.insert(NodeFlags::HOVERED);
                }
                self.send(*v, &Event::PointerEnter);
                self.emit(*v, Signal::PointerEntered, Value::None);
                self.request_paint(*v);
            }
        }
        if let Some(w) = self.window_mut(window) {
            w.hover = new_chain;
        }
        crate::amalfi::tooltips::hover_changed(self, window);
    }

    /// The pointer left the window.
    pub fn pointer_leave(&mut self, window: WindowId) {
        if let Some(w) = self.window_mut(window) {
            w.pointer = None;
        }
        self.update_hover(window, None, Mods::empty());
        self.flush();
    }

    /// A button went down at `pos`.
    pub fn pointer_down(&mut self, window: WindowId, pos: Point, button: Button, mods: Mods) {
        let now = self.now;
        let clicks = match self.window_mut(window) {
            Some(w) => {
                let (t, p, n) = w.last_click;
                let count = if now - t <= DOUBLE_CLICK_INTERVAL
                    && (pos - p).length() <= DOUBLE_CLICK_SLOP
                {
                    n + 1
                } else {
                    1
                };
                w.last_click = (now, pos, count);
                w.pointer = Some(pos);
                count
            }
            None => return,
        };
        let Some((target, _)) = self.hit_test(window, pos) else {
            // A click on nothing removes the focus.
            self.clear_focus(window);
            self.flush();
            return;
        };
        if self.is_disabled(target) {
            self.flush();
            return;
        }
        let interactive = self.interactive_ancestor(target);
        // Text fields and lists take the focus on click; controls do not.
        let mut chain = vec![target];
        chain.extend(self.tree.ancestors(target));
        let focus_target = chain
            .into_iter()
            .find(|v| self.tree.widget(*v).map(|w| w.focus()) == Some(FocusKind::Always));
        match focus_target {
            Some(f) => self.set_focus(f, false),
            None => {
                if interactive.is_none() {
                    self.clear_focus(window);
                }
            }
        }
        let ev = Event::PointerDown {
            pos: pos - self.tree.window_origin(target),
            button,
            clicks,
            mods,
        };
        let handled_by = self.send_bubbling(target, &ev);
        let capture = match (button, interactive) {
            (Button::Left, Some(i)) => {
                if let Some(n) = self.tree.get_mut(i) {
                    n.flags.insert(NodeFlags::PRESSED);
                }
                self.request_paint(i);
                Some(i)
            }
            _ => handled_by,
        };
        if let Some(w) = self.window_mut(window)
            && w.capture.is_none()
        {
            w.capture = capture;
        }
        if button == Button::Right
            && let Some(v) = handled_by.or(Some(target))
        {
            self.emit(v, Signal::ContextMenu, Value::Point(pos));
        }
        self.flush();
    }

    /// A button went up at `pos`.
    pub fn pointer_up(&mut self, window: WindowId, pos: Point, button: Button, mods: Mods) {
        let capture = self.window_mut(window).and_then(|w| w.capture.take());
        if let Some(c) = capture.filter(|c| self.tree.contains(*c)) {
            let ev = Event::PointerUp {
                pos: pos - self.tree.window_origin(c),
                button,
                mods,
            };
            self.send(c, &ev);
            let was_pressed = self.tree.flags(c).contains(NodeFlags::PRESSED);
            if let Some(n) = self.tree.get_mut(c) {
                n.flags.remove(NodeFlags::PRESSED);
            }
            self.request_paint(c);
            let inside = self.tree.window_rect(c).contains(pos);
            if was_pressed && inside && button == Button::Left && !self.is_disabled(c) {
                self.send(c, &Event::Activate);
            }
        } else if let Some((target, _)) = self.hit_test(window, pos) {
            let ev = Event::PointerUp {
                pos: pos - self.tree.window_origin(target),
                button,
                mods,
            };
            self.send_bubbling(target, &ev);
        }
        self.update_hover(window, Some(pos), mods);
        self.flush();
    }

    /// Scroll at `pos`.
    pub fn scroll(
        &mut self,
        window: WindowId,
        pos: Point,
        dx: f32,
        dy: f32,
        precise: bool,
        phase: Phase,
    ) {
        if let Some((target, _)) = self.hit_test(window, pos) {
            let ev = Event::Scroll {
                pos: pos - self.tree.window_origin(target),
                dx,
                dy,
                precise,
                phase,
            };
            self.send_bubbling(target, &ev);
        }
        self.flush();
    }

    /// A key went down or up. Order: the focused view and its ancestors (responder chain);
    /// then the window's keys (Tab, Return for the default button, Escape for cancel); then
    /// the app's shortcuts; else [`Signal::KeyUnhandled`] on the root.
    pub fn key(&mut self, window: WindowId, key: KeyEvent) {
        let focus = self.focus(window);
        let root = self.window(window).map(|w| w.root);
        let mut handled = false;
        // Activator keys on a focused control.
        if let Some(f) = focus {
            let interaction = self.interaction_of(f);
            let activator = match interaction {
                Interaction::Button => matches!(key.key, Key::Space | Key::Enter),
                Interaction::Toggle => key.key == Key::Space,
                _ => false,
            };
            if activator && key.mods.is_empty() && !self.is_disabled(f) {
                if key.pressed {
                    if !key.repeat {
                        if let Some(n) = self.tree.get_mut(f) {
                            n.flags.insert(NodeFlags::PRESSED);
                        }
                        self.request_paint(f);
                    }
                } else if self.tree.flags(f).contains(NodeFlags::PRESSED) {
                    if let Some(n) = self.tree.get_mut(f) {
                        n.flags.remove(NodeFlags::PRESSED);
                    }
                    self.request_paint(f);
                    self.send(f, &Event::Activate);
                }
                handled = true;
            }
        }
        if !handled && let Some(f) = focus {
            handled = self.send_bubbling(f, &Event::Key(key.clone())).is_some();
        }
        if !handled && key.pressed {
            handled = match key.key {
                Key::Tab if !key.mods.intersects(Mods::LOGO | Mods::CTRL | Mods::ALT) => {
                    self.focus_next(window, key.mods.contains(Mods::SHIFT))
                }
                Key::Enter if key.mods.is_empty() => self.activate_marked(window, Prop::IsDefault),
                Key::Escape if key.mods.is_empty() => self.activate_marked(window, Prop::IsCancel),
                _ => false,
            };
        }
        if !handled && key.pressed {
            let mut list = std::mem::take(&mut self.shortcuts);
            for (sc, f) in &mut list {
                if sc.key == key.key.name() && sc.mods == (key.mods - Mods::CAPS) {
                    f(self);
                    handled = true;
                    break;
                }
            }
            list.append(&mut self.shortcuts);
            self.shortcuts = list;
        }
        if !handled
            && key.pressed
            && let Some(r) = root
        {
            self.emit(r, Signal::KeyUnhandled, Value::Str(key.key.name()));
        }
        self.flush();
    }

    fn activate_marked(&mut self, window: WindowId, marker: Prop) -> bool {
        let Some(root) = self.window(window).map(|w| w.root) else {
            return false;
        };
        let target = self.tree.descendants(root).into_iter().find(|v| {
            self.is_visible(*v)
                && !self.is_disabled(*v)
                && self.tree.widget(*v).and_then(|w| w.prop(marker)) == Some(Value::Bool(true))
        });
        match target {
            Some(t) => {
                self.send(t, &Event::Activate);
                true
            }
            None => false,
        }
    }

    /// Committed text (keyboard or input method) for the focused view.
    pub fn text_input(&mut self, window: WindowId, text: &str) {
        if let Some(f) = self.focus(window) {
            self.send_bubbling(f, &Event::Text(text.to_string()));
        }
        self.flush();
    }

    /// The input method's pre-edit text for the focused view.
    pub fn preedit(&mut self, window: WindowId, text: &str, cursor: Option<(usize, usize)>) {
        if let Some(f) = self.focus(window) {
            self.send(
                f,
                &Event::Preedit {
                    text: text.to_string(),
                    cursor,
                },
            );
        }
        self.flush();
    }

    /// The input method deletes around the caret.
    pub fn delete_surrounding(&mut self, window: WindowId, before: usize, after: usize) {
        if let Some(f) = self.focus(window) {
            self.send(f, &Event::DeleteSurrounding { before, after });
        }
        self.flush();
    }

    /// Items dropped at `pos`.
    pub fn drop_items(&mut self, window: WindowId, pos: Point, items: Vec<String>) {
        if let Some((target, _)) = self.hit_test(window, pos) {
            let ev = Event::Drop {
                pos: pos - self.tree.window_origin(target),
                items: items.clone(),
            };
            if self.send_bubbling(target, &ev).is_none() {
                self.emit(target, Signal::Dropped, Value::StrList(items));
            }
        }
        self.flush();
    }

    // ----- Animations and timers ------------------------------------------------------------

    pub(crate) fn start_animating(&mut self, id: ViewId) {
        if !self.animating.contains(&id) {
            self.animating.push(id);
        }
        if let Some(n) = self.tree.get_mut(id) {
            n.flags.insert(NodeFlags::ANIMATING);
        }
        if let Some(w) = self.window_of(id) {
            self.request_frame(w);
        }
    }

    /// Whether anything animates.
    pub fn is_animating(&self) -> bool {
        !self.animating.is_empty()
    }

    /// Advances time by `dt` seconds: ticks animations, fires due timers, runs handlers.
    pub fn advance(&mut self, dt: f64) {
        let dt = if dt.is_finite() { dt.max(0.0) } else { 0.0 };
        self.now += dt;
        let list = std::mem::take(&mut self.animating);
        let mut still = Vec::new();
        for id in list {
            let Some(mut w) = self.tree.take_widget(id) else {
                continue;
            };
            let more = {
                let mut cx = EventCx { ui: self, id };
                w.tick(&mut cx, dt as f32)
            };
            self.tree.put_widget(id, w);
            self.request_paint(id);
            if more {
                still.push(id);
            } else if let Some(n) = self.tree.get_mut(id) {
                n.flags.remove(NodeFlags::ANIMATING);
            }
        }
        for id in std::mem::take(&mut self.animating) {
            if !still.contains(&id) {
                still.push(id);
            }
        }
        self.animating = still;
        self.fire_timers();
        self.flush();
    }

    /// Runs `f` after `delay` seconds.
    pub fn after(&mut self, delay: f64, f: impl FnOnce(&mut Ui) + 'static) -> TimerId {
        let id = TimerId(self.next_timer);
        self.next_timer += 1;
        self.timers.push(Timer {
            id,
            deadline: self.now + delay.max(0.0),
            callback: Box::new(f),
        });
        id
    }

    /// Cancels a timer.
    pub fn cancel_timer(&mut self, id: TimerId) {
        self.timers.retain(|t| t.id != id);
    }

    /// The earliest timer deadline.
    pub fn next_deadline(&self) -> Option<f64> {
        self.timers.iter().map(|t| t.deadline).reduce(f64::min)
    }

    /// Fires the timers that are due.
    pub fn fire_timers(&mut self) {
        let now = self.now;
        let (due, rest): (Vec<Timer>, Vec<Timer>) = std::mem::take(&mut self.timers)
            .into_iter()
            .partition(|t| t.deadline <= now);
        self.timers = rest;
        for t in due {
            (t.callback)(self);
        }
        self.flush();
    }

    // ----- Frame -------------------------------------------------------------------------

    /// Lays out a window if needed.
    pub fn layout_window(&mut self, window: WindowId) {
        let Some(w) = self.window(window) else {
            return;
        };
        let (root, size) = (w.root, w.size);
        let needs = w.needs_layout || self.tree.flags(root).contains(NodeFlags::NEEDS_LAYOUT);
        if !needs {
            return;
        }
        // The root fills the window; its children fill the root.
        if let Some(n) = self.tree.get_mut(root) {
            n.modifiers.width = Some(size.width);
            n.modifiers.height = Some(size.height);
        }
        let _ = self.measure(root, Proposal::fixed(size));
        self.place(root, Rect::from_size(size));
        // The content is offered the window's size and centred at the size it takes (a
        // content view that fills has an infinite maximum width and height).
        for c in self.tree.children(root).to_vec() {
            let s = self.measure(c, Proposal::fixed(size));
            let o = Alignment::CENTER.place(Rect::from_size(size), s);
            self.place(c, Rect::from_origin_size(o, s));
        }
        if let Some(w) = self.window_mut(window) {
            w.needs_layout = false;
        }
        self.flush();
    }

    /// The preferred size of a window's content (for windows that size to fit).
    pub fn fitting_size(&mut self, window: WindowId) -> Size {
        let Some(root) = self.window(window).map(|w| w.root) else {
            return Size::ZERO;
        };
        let mut size = Size::ZERO;
        for c in self.tree.children(root).to_vec() {
            let s = self.measure(c, Proposal::UNSPECIFIED);
            size = Size::new(size.width.max(s.width), size.height.max(s.height));
        }
        size
    }

    /// Records the drawing of every view that needs it, and returns the window's damage
    /// (window coordinates): the old and new painted bounds of every view that changed.
    pub fn prepare_frame(&mut self, window: WindowId) -> Rect {
        self.flush();
        self.layout_window(window);
        let Some(root) = self.window(window).map(|w| w.root) else {
            return Rect::ZERO;
        };
        let mut damage = self.window(window).map(|w| w.damage).unwrap_or_default();
        let theme_root = self.theme_of(root);
        self.record_subtree(root, Point::ZERO, theme_root, &mut damage);
        let size = self.window(window).map(|w| w.size).unwrap_or_default();
        let damage = damage.intersect(Rect::from_size(size));
        if let Some(w) = self.window_mut(window) {
            w.damage = Rect::ZERO;
            w.needs_frame = false;
        }
        damage
    }

    fn record_subtree(
        &mut self,
        id: ViewId,
        parent_origin: Point,
        parent_theme: Theme,
        damage: &mut Rect,
    ) {
        let Some(n) = self.tree.get(id) else { return };
        if n.flags.contains(NodeFlags::HIDDEN) {
            return;
        }
        let origin = parent_origin + n.frame.origin() + n.modifiers.offset;
        let theme = match n.dark_override {
            Some(d) if d != parent_theme.dark() => {
                let mut a = parent_theme.appearance;
                a.dark = d;
                Theme::new(a)
            }
            _ => parent_theme,
        };
        let needs_paint = n.flags.contains(NodeFlags::NEEDS_PAINT);
        if needs_paint {
            self.record_view(id, theme);
        }
        let Some(n) = self.tree.get(id) else { return };
        let local = n.display.bounds().union(n.display_over.bounds()).union(
            if n.modifiers.background.is_some() {
                Rect::from_size(n.frame.size())
            } else {
                Rect::ZERO
            },
        );
        let s = n.modifiers.scale;
        let local = if (s - 1.0).abs() > 1e-6 {
            let c = Point::new(n.frame.width / 2.0, n.frame.height / 2.0);
            Transform::translate(-c.x, -c.y)
                .then(Transform::scale(s, s))
                .then(Transform::translate(c.x, c.y))
                .map_rect(local)
        } else {
            local
        };
        let now_bounds = local.translate(origin);
        let old = n.painted_bounds;
        if needs_paint || old != now_bounds {
            *damage = damage.union(old).union(now_bounds);
        }
        if let Some(n) = self.tree.get_mut(id) {
            n.painted_bounds = now_bounds;
        }
        for c in self.tree.children(id).to_vec() {
            self.record_subtree(c, origin, theme, damage);
        }
    }

    fn record_view(&mut self, id: ViewId, theme: Theme) {
        let Some(mut w) = self.tree.take_widget(id) else {
            return;
        };
        let bounds = self.content_box(id);
        let flags = self.tree.flags(id);
        let mut under = Recorder::new();
        let mut over = Recorder::new();
        {
            let mut cx = PaintCx {
                ui: self,
                id,
                rec: &mut under,
                bounds,
                theme,
                flags,
            };
            w.paint(&mut cx);
            cx.rec = &mut over;
            w.paint_over(&mut cx);
        }
        self.tree.put_widget(id, w);
        if let Some(n) = self.tree.get_mut(id) {
            n.display = under.finish();
            n.display_over = over.finish();
            n.flags.remove(NodeFlags::NEEDS_PAINT);
        }
    }

    /// Paints a prepared window on `canvas` (window coordinates, already scaled to device
    /// pixels by the caller), clipped to `clip` if given.
    pub fn paint_window(&self, window: WindowId, canvas: &Canvas, clip: Option<Rect>) {
        let Some(root) = self.window(window).map(|w| w.root) else {
            return;
        };
        let theme = self.theme_of(root);
        let opts = PaintOptions {
            backdrop: !theme.appearance.reduce_transparency,
            blur_fallback: None,
        };
        canvas.save();
        if let Some(c) = clip {
            canvas.clip_rect(c.to_sk(), ClipOp::Intersect, false);
        }
        self.paint_view(root, canvas, opts, false);
        canvas.restore();
    }

    fn paint_view(&self, id: ViewId, canvas: &Canvas, opts: PaintOptions, dimmed: bool) {
        let Some(n) = self.tree.get(id) else { return };
        if n.flags.contains(NodeFlags::HIDDEN) || n.modifiers.opacity <= 0.0 {
            return;
        }
        let m = &n.modifiers;
        canvas.save();
        canvas.translate((n.frame.x + m.offset.x, n.frame.y + m.offset.y));
        if (m.scale - 1.0).abs() > 1e-6 {
            let c = (n.frame.width / 2.0, n.frame.height / 2.0);
            canvas.translate(c);
            canvas.scale((m.scale, m.scale));
            canvas.translate((-c.0, -c.1));
        }
        let dim_here = !dimmed
            && n.flags.contains(NodeFlags::DISABLED)
            && n.widget.as_deref().is_some_and(|w| w.dims_when_disabled());
        let mut alpha = m.opacity;
        if dim_here {
            alpha *= crate::amalfi::tokens::states::DISABLED_OPACITY;
        }
        let layered = alpha < 1.0;
        if layered {
            canvas.save_layer_alpha(None, (alpha.clamp(0.0, 1.0) * 255.0).round() as u32);
        }
        let size = n.frame.size();
        if let Some(bg) = m.background {
            let shape = if m.corner_radius > 0.0 {
                Shape::rounded(m.corner_radius)
            } else {
                Shape::RECT
            };
            crate::style::paint_style(
                canvas,
                Rect::from_size(size),
                &shape,
                &Style::fill(bg),
                opts,
            );
        }
        n.display.paint(canvas, opts);
        let clips = n.flags.contains(NodeFlags::CLIPS);
        if clips {
            canvas.save();
            if m.corner_radius > 0.0 {
                canvas.clip_path(
                    &Shape::rounded(m.corner_radius).path(Rect::from_size(size)),
                    ClipOp::Intersect,
                    true,
                );
            } else {
                canvas.clip_rect(Rect::from_size(size).to_sk(), ClipOp::Intersect, false);
            }
        }
        for c in &n.children {
            self.paint_view(*c, canvas, opts, dimmed || dim_here);
        }
        if clips {
            canvas.restore();
        }
        n.display_over.paint(canvas, opts);
        if layered {
            canvas.restore();
        }
        canvas.restore();
    }

    /// Renders a window into a raster image at `scale` device pixels per point (tests,
    /// screenshots, the inspector).
    pub fn snapshot(&mut self, window: WindowId, scale: f32) -> Option<skia_safe::Image> {
        let _ = self.prepare_frame(window);
        let size = self.window(window)?.size;
        let scale = if scale > 0.0 && scale.is_finite() {
            scale
        } else {
            1.0
        };
        let w = (size.width * scale).ceil().max(1.0) as i32;
        let h = (size.height * scale).ceil().max(1.0) as i32;
        let mut surface = skia_safe::surfaces::raster_n32_premul((w, h))?;
        let canvas = surface.canvas();
        canvas.clear(skia_safe::Color::TRANSPARENT);
        canvas.scale((scale, scale));
        self.paint_window(window, canvas, None);
        Some(surface.image_snapshot())
    }

    fn damage_subtree(&mut self, id: ViewId) {
        let Some(w) = self.window_of(id) else {
            return;
        };
        let mut r = Rect::ZERO;
        for v in self.tree.descendants(id) {
            if let Some(n) = self.tree.get_mut(v) {
                r = r.union(n.painted_bounds);
                n.painted_bounds = Rect::ZERO;
            }
        }
        if let Some(win) = self.window_mut(w) {
            win.damage = win.damage.union(r);
        }
        self.request_frame(w);
    }

    /// The display lists of a view (under, over), for the inspector and parity tests.
    pub fn display_lists(&self, id: ViewId) -> Option<(&DisplayList, &DisplayList)> {
        self.tree.get(id).map(|n| (&n.display, &n.display_over))
    }

    /// The named layers painted by the subtree at `root`, with frames relative to `root`,
    /// in painting order. Each view's layers are prefixed with its identifier when it has
    /// one.
    pub fn inspect_layers(&self, root: ViewId) -> Vec<crate::core::display::InspectedLayer> {
        let mut out = Vec::new();
        let base = self.tree.window_origin(root);
        self.inspect_into(root, base, "", &mut out, true);
        out
    }

    fn inspect_into(
        &self,
        id: ViewId,
        base: Point,
        prefix: &str,
        out: &mut Vec<crate::core::display::InspectedLayer>,
        is_root: bool,
    ) {
        let Some(n) = self.tree.get(id) else { return };
        if n.flags.contains(NodeFlags::HIDDEN) {
            return;
        }
        let origin = self.tree.window_origin(id) - base;
        let prefix = match (&n.identifier, is_root) {
            (Some(ident), false) => {
                if prefix.is_empty() {
                    ident.clone()
                } else {
                    format!("{prefix}/{ident}")
                }
            }
            _ => prefix.to_string(),
        };
        n.display.inspect(origin, &prefix, out);
        for c in &n.children {
            self.inspect_into(*c, base, &prefix, out, false);
        }
        n.display_over.inspect(origin, &prefix, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::layout::Stack;
    use crate::core::widget::{Interaction, LayoutCx};
    use std::cell::Cell;
    use std::rc::Rc;

    struct Clicky {
        size: Size,
    }

    impl Widget for Clicky {
        fn kind(&self) -> &'static str {
            "Clicky"
        }
        fn measure(&mut self, _cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
            self.size
        }
        fn interaction(&self) -> Interaction {
            Interaction::Button
        }
        fn focus(&self) -> FocusKind {
            FocusKind::Control
        }
        fn event(&mut self, cx: &mut EventCx<'_>, e: &Event) -> Handled {
            if *e == Event::Activate {
                cx.emit(Signal::Activated, Value::None);
                return Handled::Yes;
            }
            Handled::No
        }
        fn paint(&mut self, cx: &mut PaintCx<'_>) {
            let c = if cx.is_pressed() {
                crate::color::Color::rgb(0, 0, 255)
            } else {
                crate::color::Color::rgb(255, 0, 0)
            };
            cx.rec
                .layer("Box", cx.bounds(), Shape::RECT, Style::fill(c));
        }
    }

    fn setup() -> (Ui, WindowId, ViewId, ViewId, Rc<Cell<u32>>) {
        let mut ui = Ui::new_headless();
        let win = ui.create_window(WindowKind::Normal, Size::new(200.0, 100.0));
        let row = ui.create(Stack::row(10.0));
        let a = ui.create(Clicky {
            size: Size::new(40.0, 20.0),
        });
        let b = ui.create(Clicky {
            size: Size::new(40.0, 20.0),
        });
        ui.add_child(row, a);
        ui.add_child(row, b);
        ui.set_content(win, row);
        ui.show_window(win);
        let count = Rc::new(Cell::new(0));
        let c2 = count.clone();
        ui.connect(a, Signal::Activated, move |_, _, _| c2.set(c2.get() + 1));
        ui.prepare_frame(win);
        (ui, win, a, b, count)
    }

    #[test]
    fn click_activates_only_when_released_inside() {
        let (mut ui, win, a, _b, count) = setup();
        let r = ui.tree.window_rect(a);
        assert_eq!(r, Rect::new(55.0, 40.0, 40.0, 20.0));
        let inside = r.center();
        ui.pointer_down(win, inside, Button::Left, Mods::empty());
        assert!(ui.tree.flags(a).contains(NodeFlags::PRESSED));
        ui.pointer_up(win, inside, Button::Left, Mods::empty());
        assert_eq!(count.get(), 1);
        ui.pointer_down(win, inside, Button::Left, Mods::empty());
        ui.pointer_move(win, Point::new(5.0, 5.0), Mods::empty());
        assert!(!ui.tree.flags(a).contains(NodeFlags::PRESSED));
        ui.pointer_up(win, Point::new(5.0, 5.0), Button::Left, Mods::empty());
        assert_eq!(count.get(), 1);
    }

    #[test]
    fn keyboard_loop_and_activators() {
        let (mut ui, win, a, b, count) = setup();
        // Without Full Keyboard Access, buttons are not in the loop.
        assert!(ui.key_view_loop(win).is_empty());
        ui.appearance.full_keyboard_access = true;
        assert_eq!(ui.key_view_loop(win), vec![a, b]);
        ui.key(win, KeyEvent::press(Key::Tab, Mods::empty()));
        assert_eq!(ui.focus(win), Some(a));
        assert!(ui.tree.flags(a).contains(NodeFlags::FOCUS_VISIBLE));
        ui.key(win, KeyEvent::press(Key::Space, Mods::empty()));
        let mut up = KeyEvent::press(Key::Space, Mods::empty());
        up.pressed = false;
        ui.key(win, up);
        assert_eq!(count.get(), 1);
        ui.key(win, KeyEvent::press(Key::Tab, Mods::SHIFT));
        assert_eq!(ui.focus(win), Some(b));
    }

    #[test]
    fn damage_and_snapshot() {
        let (mut ui, win, a, _b, _count) = setup();
        let img = ui.snapshot(win, 2.0).unwrap();
        assert_eq!((img.width(), img.height()), (400, 200));
        // Nothing changed: no damage.
        assert_eq!(ui.prepare_frame(win), Rect::ZERO);
        ui.request_paint(a);
        let d = ui.prepare_frame(win);
        assert_eq!(d, Rect::new(55.0, 40.0, 40.0, 20.0));
        ui.set(a, Prop::Hidden, Value::Bool(true)).unwrap();
        let d = ui.prepare_frame(win);
        assert!(d.contains(Point::new(60.0, 45.0)));
    }

    #[test]
    fn handlers_may_mutate_the_tree() {
        let (mut ui, win, a, b, _count) = setup();
        ui.connect(a, Signal::Activated, move |ui, _, _| {
            ui.destroy(b);
        });
        let p = ui.tree.window_rect(a).center();
        ui.pointer_down(win, p, Button::Left, Mods::empty());
        ui.pointer_up(win, p, Button::Left, Mods::empty());
        assert!(!ui.tree.contains(b));
        ui.prepare_frame(win);
        // Stale ids are harmless.
        assert_eq!(
            ui.set(b, Prop::Hidden, Value::Bool(true)),
            Err(Error::NoSuchObject)
        );
        ui.close_window(win);
        assert!(!ui.tree.contains(a));
        assert!(ui.tree.is_empty());
    }

    #[test]
    fn timers_and_shortcuts() {
        let (mut ui, win, _a, _b, count) = setup();
        let c = count.clone();
        ui.after(0.5, move |_| c.set(c.get() + 10));
        ui.advance(0.4);
        assert_eq!(count.get(), 0);
        ui.advance(0.2);
        assert_eq!(count.get(), 10);
        let c = count.clone();
        assert!(ui.add_shortcut("cmd+k", move |_| c.set(c.get() + 100)));
        ui.key(win, KeyEvent::press(Key::Char('k'), Mods::LOGO));
        assert_eq!(count.get(), 110);
    }
}
