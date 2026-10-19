//! The view tree: an arena of nodes with generational ids.
//!
//! A [`ViewId`] stays valid until its view is destroyed; a stale id is detected (generation)
//! and every operation on it is a no-op or an error, never a crash. That is what makes the
//! ids safe to hand across the C ABI as opaque handles.

use crate::color::Color;
use crate::core::display::DisplayList;
use crate::core::layout::{Alignment, Proposal};
use crate::core::widget::Widget;
use crate::core::window::WindowId;
use crate::geom::{Insets, Point, Rect, Size};

/// A view's id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ViewId {
    index: u32,
    generation: u32,
}

impl ViewId {
    /// The id as one number (C handles, accessibility node ids): never 0.
    pub fn to_raw(self) -> u64 {
        (u64::from(self.generation) << 32) | u64::from(self.index + 1)
    }

    /// The id from [`ViewId::to_raw`].
    pub fn from_raw(raw: u64) -> Option<ViewId> {
        let index = (raw & 0xffff_ffff) as u32;
        if index == 0 {
            return None;
        }
        Some(ViewId {
            index: index - 1,
            generation: (raw >> 32) as u32,
        })
    }
}

bitflags::bitflags! {
    /// State bits of a node.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct NodeFlags: u32 {
        /// Its size or its children's placement must be computed again.
        const NEEDS_LAYOUT = 1 << 0;
        /// Its display list must be recorded again.
        const NEEDS_PAINT = 1 << 1;
        /// Not drawn, not laid out, not hit.
        const HIDDEN = 1 << 2;
        /// No interaction, drawn dimmed.
        const DISABLED = 1 << 3;
        /// The pointer is over it.
        const HOVERED = 1 << 4;
        /// Pressed (pointer down and still inside, or an activator key down).
        const PRESSED = 1 << 5;
        /// It has the keyboard focus.
        const FOCUSED = 1 << 6;
        /// The focus came from the keyboard: the halo shows.
        const FOCUS_VISIBLE = 1 << 7;
        /// Clips its children to its bounds.
        const CLIPS = 1 << 8;
        /// Animating: ticked every frame.
        const ANIMATING = 1 << 9;
        /// Is a window's root.
        const ROOT = 1 << 10;
        /// Being removed: drawn while its leave transition plays, out of layout and hits.
        const LEAVING = 1 << 11;
        /// Drawn on an accent fill (a selected row): its subtree uses the emphasized theme.
        const EMPHASIZED = 1 << 12;
    }
}

impl NodeFlags {
    /// The flags a widget is told about when they change.
    pub const INTERACTION: NodeFlags = NodeFlags::HOVERED
        .union(NodeFlags::PRESSED)
        .union(NodeFlags::FOCUSED)
        .union(NodeFlags::FOCUS_VISIBLE)
        .union(NodeFlags::DISABLED);
}

/// Generic layout and appearance modifiers every view has (SwiftUI-style), applied by the
/// core around the widget's own layout.
#[derive(Debug, Clone, PartialEq)]
pub struct Modifiers {
    /// Insets around the content.
    pub padding: Insets,
    /// Fixed width.
    pub width: Option<f32>,
    /// Fixed height.
    pub height: Option<f32>,
    /// Minimum width.
    pub min_width: Option<f32>,
    /// Maximum width (infinite: take all offered).
    pub max_width: Option<f32>,
    /// Minimum height.
    pub min_height: Option<f32>,
    /// Maximum height.
    pub max_height: Option<f32>,
    /// Alignment of the content in a frame larger than it.
    pub alignment: Alignment,
    /// The alignment was set: the content is laid out at its own size, aligned in the
    /// frame (otherwise it fills the frame inside the padding).
    pub aligns_content: bool,
    /// Priority in stacks.
    pub priority: f32,
    /// Opacity (multiplies the subtree).
    pub opacity: f32,
    /// Background fill behind the content.
    pub background: Option<Color>,
    /// Corner radius of the background and the clip.
    pub corner_radius: f32,
    /// Fixed aspect ratio.
    pub aspect_ratio: Option<f32>,
    /// Offset after layout.
    pub offset: Point,
    /// Scale after layout, about the centre.
    pub scale: f32,
    /// Implicit animation of position changes.
    pub animation: Option<crate::motion::Timing>,
    /// Insertion and removal transition.
    pub transition: crate::core::anim::Transition,
}

impl Default for Modifiers {
    fn default() -> Self {
        Modifiers {
            padding: Insets::ZERO,
            width: None,
            height: None,
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            alignment: Alignment::CENTER,
            aligns_content: false,
            priority: 0.0,
            opacity: 1.0,
            background: None,
            corner_radius: 0.0,
            aspect_ratio: None,
            offset: Point::ZERO,
            scale: 1.0,
            animation: None,
            transition: crate::core::anim::Transition::None,
        }
    }
}

/// Accessibility overrides of a view.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct A11yProps {
    /// Label.
    pub label: Option<String>,
    /// Help.
    pub help: Option<String>,
}

/// A node of the tree.
pub struct Node {
    /// The view's behaviour; taken out while one of its methods runs.
    pub widget: Option<Box<dyn Widget>>,
    /// Parent.
    pub parent: Option<ViewId>,
    /// Children in painting order.
    pub children: Vec<ViewId>,
    /// The window it is shown in (set on the root, inherited).
    pub window: Option<WindowId>,
    /// Frame in the parent's coordinates.
    pub frame: Rect,
    /// State bits.
    pub flags: NodeFlags,
    /// Generic modifiers.
    pub modifiers: Modifiers,
    /// Accessibility overrides.
    pub a11y: A11yProps,
    /// Identifier for tests and the inspector.
    pub identifier: Option<String>,
    /// Tooltip.
    pub tooltip: Option<String>,
    /// Appearance override for the subtree: `Some(true)` dark, `Some(false)` light.
    pub dark_override: Option<bool>,
    /// External references (C handles); the tree's own link to a parent counts one.
    pub refs: u32,
    /// Cached measurements: (proposal, size).
    pub measure_cache: Vec<(Proposal, Size)>,
    /// The content box in the view's coordinates when the content is aligned in a larger
    /// frame (see `Modifiers::aligns_content`).
    pub content: Option<Rect>,
    /// The last recorded drawing (under the children).
    pub display: DisplayList,
    /// The last recorded drawing over the children.
    pub display_over: DisplayList,
    /// The bounds painted last time, in window coordinates (for damage).
    pub painted_bounds: Rect,
    /// The interaction flags the widget was last told about ([`Widget::state_changed`]).
    pub seen_flags: NodeFlags,
    /// Implicit animation state.
    pub anim: Option<Box<crate::core::anim::NodeAnim>>,
    /// What the frame knows about the subtree, and its raster cache.
    pub raster: crate::core::raster::RasterState,
}

impl Node {
    fn new(widget: Box<dyn Widget>) -> Node {
        Node {
            widget: Some(widget),
            parent: None,
            children: Vec::new(),
            window: None,
            frame: Rect::ZERO,
            flags: NodeFlags::NEEDS_LAYOUT | NodeFlags::NEEDS_PAINT,
            modifiers: Modifiers::default(),
            a11y: A11yProps::default(),
            identifier: None,
            tooltip: None,
            dark_override: None,
            refs: 1,
            measure_cache: Vec::new(),
            content: None,
            display: DisplayList::default(),
            display_over: DisplayList::default(),
            painted_bounds: Rect::ZERO,
            seen_flags: NodeFlags::empty(),
            anim: None,
            raster: crate::core::raster::RasterState::default(),
        }
    }

    /// The widget's kind name.
    pub fn kind(&self) -> &'static str {
        self.widget.as_ref().map(|w| w.kind()).unwrap_or("(busy)")
    }
}

struct Slot {
    generation: u32,
    node: Option<Node>,
}

/// The arena.
#[derive(Default)]
pub struct Tree {
    slots: Vec<Slot>,
    free: Vec<u32>,
    live: usize,
}

impl Tree {
    /// Adds a node; returns its id (with one reference, the caller's).
    pub fn insert(&mut self, widget: Box<dyn Widget>) -> ViewId {
        self.live += 1;
        if let Some(index) = self.free.pop()
            && let Some(slot) = self.slots.get_mut(index as usize)
        {
            slot.node = Some(Node::new(widget));
            return ViewId {
                index,
                generation: slot.generation,
            };
        }
        let index = u32::try_from(self.slots.len()).unwrap_or(u32::MAX);
        self.slots.push(Slot {
            generation: 1,
            node: Some(Node::new(widget)),
        });
        ViewId {
            index,
            generation: 1,
        }
    }

    /// Removes a node (not its children); returns it.
    pub fn remove(&mut self, id: ViewId) -> Option<Node> {
        let slot = self.slots.get_mut(id.index as usize)?;
        if slot.generation != id.generation {
            return None;
        }
        let node = slot.node.take()?;
        slot.generation = slot.generation.wrapping_add(1).max(1);
        self.free.push(id.index);
        self.live = self.live.saturating_sub(1);
        Some(node)
    }

    /// Whether the id names a live node.
    pub fn contains(&self, id: ViewId) -> bool {
        self.get(id).is_some()
    }

    /// The node.
    pub fn get(&self, id: ViewId) -> Option<&Node> {
        let slot = self.slots.get(id.index as usize)?;
        if slot.generation != id.generation {
            return None;
        }
        slot.node.as_ref()
    }

    /// The node, mutably.
    pub fn get_mut(&mut self, id: ViewId) -> Option<&mut Node> {
        let slot = self.slots.get_mut(id.index as usize)?;
        if slot.generation != id.generation {
            return None;
        }
        slot.node.as_mut()
    }

    /// Number of live nodes.
    pub fn len(&self) -> usize {
        self.live
    }

    /// Whether the tree is empty.
    pub fn is_empty(&self) -> bool {
        self.live == 0
    }

    /// The children of a node (empty for a stale id).
    pub fn children(&self, id: ViewId) -> &[ViewId] {
        self.get(id).map(|n| n.children.as_slice()).unwrap_or(&[])
    }

    /// The parent.
    pub fn parent(&self, id: ViewId) -> Option<ViewId> {
        self.get(id).and_then(|n| n.parent)
    }

    /// The ancestors from the parent up.
    pub fn ancestors(&self, id: ViewId) -> Vec<ViewId> {
        let mut out = Vec::new();
        let mut cur = self.parent(id);
        while let Some(p) = cur {
            if out.contains(&p) {
                break;
            }
            out.push(p);
            cur = self.parent(p);
        }
        out
    }

    /// Whether `a` is `b` or one of its ancestors.
    pub fn is_ancestor_or_self(&self, a: ViewId, b: ViewId) -> bool {
        a == b || self.ancestors(b).contains(&a)
    }

    /// The subtree in depth-first pre-order (self first).
    pub fn descendants(&self, id: ViewId) -> Vec<ViewId> {
        let mut out = Vec::new();
        let mut stack = vec![id];
        while let Some(v) = stack.pop() {
            if !self.contains(v) {
                continue;
            }
            out.push(v);
            for c in self.children(v).iter().rev() {
                stack.push(*c);
            }
        }
        out
    }

    /// Flags of a node (empty for a stale id).
    pub fn flags(&self, id: ViewId) -> NodeFlags {
        self.get(id).map(|n| n.flags).unwrap_or_default()
    }

    /// The node's origin in window coordinates (the sum of frames up to the root, with
    /// offsets).
    pub fn window_origin(&self, id: ViewId) -> Point {
        let mut p = Point::ZERO;
        let mut cur = Some(id);
        let mut guard = 0;
        while let Some(v) = cur {
            let Some(n) = self.get(v) else { break };
            p = p + n.frame.origin() + n.modifiers.offset;
            cur = n.parent;
            guard += 1;
            if guard > 10_000 {
                break;
            }
        }
        p
    }

    /// The node's bounds in window coordinates.
    pub fn window_rect(&self, id: ViewId) -> Rect {
        let size = self.get(id).map(|n| n.frame.size()).unwrap_or_default();
        Rect::from_origin_size(self.window_origin(id), size)
    }

    /// Takes the widget out to run one of its methods.
    pub fn take_widget(&mut self, id: ViewId) -> Option<Box<dyn Widget>> {
        self.get_mut(id).and_then(|n| n.widget.take())
    }

    /// Puts a widget back.
    pub fn put_widget(&mut self, id: ViewId, w: Box<dyn Widget>) {
        if let Some(n) = self.get_mut(id) {
            n.widget = Some(w);
        }
    }

    /// The widget, for typed access.
    pub fn widget(&self, id: ViewId) -> Option<&dyn Widget> {
        self.get(id).and_then(|n| n.widget.as_deref())
    }

    /// The widget, mutably.
    pub fn widget_mut(&mut self, id: ViewId) -> Option<&mut (dyn Widget + 'static)> {
        self.get_mut(id).and_then(|n| n.widget.as_deref_mut())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::widget::Empty;

    #[test]
    fn generations_detect_stale_ids() {
        let mut t = Tree::default();
        let a = t.insert(Box::new(Empty));
        assert!(t.contains(a));
        assert!(t.remove(a).is_some());
        assert!(!t.contains(a));
        let b = t.insert(Box::new(Empty));
        assert_ne!(a, b);
        assert!(t.get(a).is_none());
        assert_eq!(ViewId::from_raw(b.to_raw()), Some(b));
        assert_eq!(ViewId::from_raw(0), None);
        assert_eq!(t.len(), 1);
    }
}
