//! The widget trait (a view's behaviour) and the contexts its methods receive.

use std::any::Any;

use crate::amalfi::theme::Theme;
use crate::core::a11y::A11y;
use crate::core::display::Recorder;
use crate::core::event::{Event, Handled};
use crate::core::layout::{Axis, Proposal};
use crate::core::tree::{NodeFlags, ViewId};
use crate::core::ui::{Cursor, Request, Ui};
use crate::core::value::{Prop, Signal, Value};
use crate::core::window::WindowId;
use crate::geom::{Point, Rect, Size};

/// Typed access to a widget behind `dyn Widget`.
pub trait AsAny: Any {
    /// `self` as `Any`.
    fn as_any(&self) -> &dyn Any;
    /// `self` as `Any`, mutably.
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T: Any> AsAny for T {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// How a view reacts to presses and activator keys (the core tracks hover, press and focus
/// for it, and sends [`Event::Activate`] on a click released inside or an activator key).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interaction {
    /// No built-in interaction.
    None,
    /// A button: Space and Return activate it.
    Button,
    /// A toggle: Space activates it.
    Toggle,
    /// Hover tracking only (rows, links).
    Hover,
}

/// Whether and when a view takes the keyboard focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusKind {
    /// Never.
    None,
    /// Always in the key view loop (text fields, lists, tables).
    Always,
    /// In the key view loop only with Full Keyboard Access (buttons, toggles, sliders); a
    /// click never focuses it.
    Control,
}

/// What setting a property changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropChange {
    /// The widget does not have this property.
    Unknown,
    /// Accepted, the drawing changed.
    Paint,
    /// Accepted, the size may have changed.
    Layout,
    /// Accepted, nothing visible changed.
    Nothing,
}

/// A view's behaviour: layout, drawing, events, properties, accessibility.
///
/// Positions are in the view's own coordinates; the view's content box (inside its padding
/// and aligned in its frame) is `cx.bounds()`.
pub trait Widget: AsAny {
    /// The kind's name (`Button`, `Stack` …), for the inspector and errors.
    fn kind(&self) -> &'static str;

    /// The size the view wants for `proposal` (SwiftUI-style: the parent proposes, the view
    /// chooses). The default stacks the children on each other (an overlay).
    fn measure(&mut self, cx: &mut LayoutCx<'_>, proposal: Proposal) -> Size {
        let mut size = Size::ZERO;
        for c in cx.children() {
            let s = cx.measure(c, proposal);
            size = Size::new(size.width.max(s.width), size.height.max(s.height));
        }
        size
    }

    /// Places the children in a content box of `size`. The default centres each child at
    /// its size for the box.
    fn place(&mut self, cx: &mut LayoutCx<'_>, size: Size) {
        for c in cx.children() {
            let s = cx.measure(c, Proposal::fixed(size));
            let origin = Point::new((size.width - s.width) / 2.0, (size.height - s.height) / 2.0);
            cx.place(c, Rect::from_origin_size(origin, s));
        }
    }

    /// Records the drawing under the children.
    fn paint(&mut self, _cx: &mut PaintCx<'_>) {}

    /// Records the drawing over the children (focus halos, overlays).
    fn paint_over(&mut self, _cx: &mut PaintCx<'_>) {}

    /// Handles an event; an unhandled one bubbles to the parent.
    fn event(&mut self, _cx: &mut EventCx<'_>, _event: &Event) -> Handled {
        Handled::No
    }

    /// The interaction state changed (hover, press, focus, disabled): start the
    /// transitions. Called after the event that caused it.
    fn state_changed(&mut self, _cx: &mut EventCx<'_>, _old: NodeFlags, _new: NodeFlags) {}

    /// Built-in interaction.
    fn interaction(&self) -> Interaction {
        Interaction::None
    }

    /// Keyboard focus.
    fn focus(&self) -> FocusKind {
        FocusKind::None
    }

    /// Whether `p` (view coordinates, within `size`) hits the view. The default: inside
    /// the bounds, for views that interact or draw a background.
    fn hit(&self, p: Point, size: Size) -> bool {
        Rect::from_size(size).contains(p)
    }

    /// Advances the view's animations by `dt` seconds; returns whether it still animates.
    fn tick(&mut self, _cx: &mut EventCx<'_>, _dt: f32) -> bool {
        false
    }

    /// Sets a component property.
    fn set_prop(&mut self, _prop: Prop, _value: &Value) -> PropChange {
        PropChange::Unknown
    }

    /// Reads a component property.
    fn prop(&self, _prop: Prop) -> Option<Value> {
        None
    }

    /// Describes the view for accessibility.
    fn accessibility(&self, _a11y: &mut A11y) {}

    /// The first baseline from the content box's top, for baseline alignment.
    fn baseline(&self) -> Option<f32> {
        None
    }

    /// The view was put in a window (it can start animations that run on their own: a
    /// spinner).
    fn attached(&mut self, _cx: &mut EventCx<'_>) {}

    /// Whether the core dims the whole view when it is disabled (Amalfi: 35 %).
    fn dims_when_disabled(&self) -> bool {
        true
    }
}

/// A view that draws nothing (a plain container: children are overlaid).
#[derive(Debug, Default, Clone, Copy)]
pub struct Empty;

impl Widget for Empty {
    fn kind(&self) -> &'static str {
        "View"
    }

    fn hit(&self, _p: Point, _size: Size) -> bool {
        false
    }
}

/// The context of [`Widget::measure`] and [`Widget::place`].
pub struct LayoutCx<'a> {
    pub(crate) ui: &'a mut Ui,
    pub(crate) id: ViewId,
    pub(crate) axis: Option<Axis>,
}

impl LayoutCx<'_> {
    /// The view's id.
    pub fn id(&self) -> ViewId {
        self.id
    }

    /// The axis of the stack the view is in (for spacers).
    pub fn parent_axis(&self) -> Option<Axis> {
        self.axis
    }

    /// The visible children.
    pub fn children(&self) -> Vec<ViewId> {
        self.ui
            .tree
            .children(self.id)
            .iter()
            .copied()
            .filter(|c| {
                !self
                    .ui
                    .tree
                    .flags(*c)
                    .intersects(NodeFlags::HIDDEN | NodeFlags::LEAVING)
            })
            .collect()
    }

    /// Measures a child.
    pub fn measure(&mut self, child: ViewId, proposal: Proposal) -> Size {
        self.ui.measure_in(child, proposal, None)
    }

    /// Measures a child of a stack along `axis` (spacers take that axis).
    pub fn measure_in(&mut self, child: ViewId, proposal: Proposal, axis: Axis) -> Size {
        self.ui.measure_in(child, proposal, Some(axis))
    }

    /// Places a child in the content box's coordinates.
    pub fn place(&mut self, child: ViewId, rect: Rect) {
        let origin = self.ui.content_origin(self.id);
        self.ui.place(child, rect.translate(origin));
    }

    /// A child's layout priority.
    pub fn priority(&self, child: ViewId) -> f32 {
        self.ui
            .tree
            .get(child)
            .map(|n| n.modifiers.priority)
            .unwrap_or(0.0)
    }

    /// A child's first baseline from its top.
    pub fn baseline(&mut self, child: ViewId) -> Option<f32> {
        self.ui.baseline(child)
    }

    /// Whether a child is a spacer.
    pub fn is_spacer(&self, child: ViewId) -> bool {
        self.ui.tree.get(child).map(|n| n.kind()) == Some("Spacer")
    }

    /// The theme of the view.
    pub fn theme(&self) -> Theme {
        self.ui.theme_of(self.id)
    }
}

/// The context of [`Widget::paint`].
pub struct PaintCx<'a> {
    pub(crate) ui: &'a Ui,
    pub(crate) id: ViewId,
    /// The recorder: what the view draws.
    pub rec: &'a mut Recorder,
    pub(crate) bounds: Rect,
    pub(crate) theme: Theme,
    pub(crate) flags: NodeFlags,
}

impl PaintCx<'_> {
    /// The view's id.
    pub fn id(&self) -> ViewId {
        self.id
    }

    /// The content box.
    pub fn bounds(&self) -> Rect {
        self.bounds
    }

    /// The content box's size.
    pub fn size(&self) -> Size {
        self.bounds.size()
    }

    /// The theme.
    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// The pointer is over the view.
    pub fn is_hovered(&self) -> bool {
        self.flags.contains(NodeFlags::HOVERED)
    }

    /// The view is pressed.
    pub fn is_pressed(&self) -> bool {
        self.flags.contains(NodeFlags::PRESSED)
    }

    /// The view has the keyboard focus.
    pub fn is_focused(&self) -> bool {
        self.flags.contains(NodeFlags::FOCUSED)
    }

    /// The focus halo shows (keyboard focus).
    pub fn focus_visible(&self) -> bool {
        self.flags.contains(NodeFlags::FOCUS_VISIBLE)
    }

    /// The view is disabled (itself or an ancestor).
    pub fn is_disabled(&self) -> bool {
        self.ui.is_disabled(self.id)
    }

    /// The window is the key window (inactive windows draw some controls flatter).
    pub fn window_active(&self) -> bool {
        self.ui
            .window_of(self.id)
            .and_then(|w| self.ui.window(w))
            .is_none_or(|w| w.active)
    }

    /// The display scale.
    pub fn scale(&self) -> f32 {
        self.ui
            .window_of(self.id)
            .and_then(|w| self.ui.window(w))
            .map(|w| w.scale)
            .unwrap_or(1.0)
    }

    /// The current time (seconds, monotonic).
    pub fn now(&self) -> f64 {
        self.ui.now
    }

    /// Records a glass surface of `material` (`Glass/Regular`, `Glass/Thick`,
    /// `Glass/Clear`): drawn by the compositor in windows whose backdrop is outside them,
    /// by the client otherwise.
    pub fn glass(
        &mut self,
        name: &str,
        rect: Rect,
        shape: crate::shape::Shape,
        material: &'static str,
        profile: crate::core::display::GlassProfile,
    ) {
        let style = self.theme.material_style(material);
        self.rec.glass(name, rect, shape, style, material, profile);
    }
}

/// The context of [`Widget::event`] and [`Widget::tick`].
pub struct EventCx<'a> {
    pub(crate) ui: &'a mut Ui,
    pub(crate) id: ViewId,
}

impl EventCx<'_> {
    /// The view's id.
    pub fn id(&self) -> ViewId {
        self.id
    }

    /// The runtime (for views that manage children, as lists and the declarative layer).
    pub fn ui(&mut self) -> &mut Ui {
        self.ui
    }

    /// The content box.
    pub fn bounds(&self) -> Rect {
        self.ui.content_box(self.id)
    }

    /// The content box's size.
    pub fn size(&self) -> Size {
        self.bounds().size()
    }

    /// The theme.
    pub fn theme(&self) -> Theme {
        self.ui.theme_of(self.id)
    }

    /// Records the drawing again.
    pub fn request_paint(&mut self) {
        self.ui.request_paint(self.id);
    }

    /// Computes the layout again.
    pub fn request_layout(&mut self) {
        self.ui.request_layout(self.id);
    }

    /// Tells observers (after the event is handled).
    pub fn emit(&mut self, signal: Signal, value: Value) {
        self.ui.emit(self.id, signal, value);
    }

    /// Keeps receiving pointer events until the button is released.
    pub fn capture_pointer(&mut self) {
        if let Some(w) = self.ui.window_of(self.id)
            && let Some(win) = self.ui.window_mut(w)
        {
            win.capture = Some(self.id);
        }
    }

    /// Gives the keyboard focus to this view.
    pub fn focus_self(&mut self) {
        self.ui.set_focus(self.id, false);
    }

    /// Gives up the keyboard focus.
    pub fn resign_focus(&mut self) {
        if let Some(w) = self.ui.window_of(self.id) {
            self.ui.clear_focus(w);
        }
    }

    /// The view has the keyboard focus.
    pub fn is_focused(&self) -> bool {
        self.ui.tree.flags(self.id).contains(NodeFlags::FOCUSED)
    }

    /// The pointer is over the view.
    pub fn is_hovered(&self) -> bool {
        self.ui.tree.flags(self.id).contains(NodeFlags::HOVERED)
    }

    /// The view is pressed.
    pub fn is_pressed(&self) -> bool {
        self.ui.tree.flags(self.id).contains(NodeFlags::PRESSED)
    }

    /// Keeps ticking the view every frame until [`Widget::tick`] returns false.
    pub fn animate(&mut self) {
        self.ui.start_animating(self.id);
    }

    /// Sets the pointer shape over the view.
    pub fn set_cursor(&mut self, cursor: Cursor) {
        if let Some(w) = self.ui.window_of(self.id) {
            self.ui.request(Request::SetCursor(w, cursor));
        }
    }

    /// Sends a request to the platform.
    pub fn request(&mut self, r: Request) {
        self.ui.request(r);
    }

    /// The window.
    pub fn window(&self) -> Option<WindowId> {
        self.ui.window_of(self.id)
    }

    /// The current time (seconds, monotonic).
    pub fn now(&self) -> f64 {
        self.ui.now
    }

    /// The view's rectangle in window coordinates.
    pub fn window_rect(&self) -> Rect {
        self.ui.tree.window_rect(self.id)
    }

    /// Writes text to the clipboard.
    pub fn set_clipboard(&mut self, text: String) {
        self.ui.set_clipboard(text);
    }

    /// Asks for the clipboard's text: it arrives as [`Event::Paste`].
    pub fn request_paste(&mut self) {
        self.ui.request_paste(self.id);
    }
}
