//! Proposal-based layout (SwiftUI-style: the parent proposes a size, the child chooses its
//! own, the parent places it), the generic frame and padding modifiers, and the layout
//! containers: stacks, overlays, grids and spacers.
//!
//! A proposal dimension is `None` (the view's ideal size), `0` (its minimum) or `∞` (its
//! maximum), or a concrete length. Measurements are cached per node and invalidated by the
//! `NEEDS_LAYOUT` flag, which propagates to the ancestors.

use crate::core::tree::{NodeFlags, ViewId};
use crate::core::ui::Ui;
use crate::core::value::{Prop, Value};
use crate::core::widget::{LayoutCx, PropChange, Widget};
use crate::geom::{Point, Rect, Size};

/// A proposed size.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Proposal {
    /// Proposed width (`None`: ideal).
    pub width: Option<f32>,
    /// Proposed height (`None`: ideal).
    pub height: Option<f32>,
}

impl Proposal {
    /// Ideal size in both dimensions.
    pub const UNSPECIFIED: Proposal = Proposal {
        width: None,
        height: None,
    };
    /// Minimum size.
    pub const ZERO: Proposal = Proposal {
        width: Some(0.0),
        height: Some(0.0),
    };
    /// Maximum size.
    pub const INFINITY: Proposal = Proposal {
        width: Some(f32::INFINITY),
        height: Some(f32::INFINITY),
    };

    /// A proposal.
    pub const fn new(width: Option<f32>, height: Option<f32>) -> Self {
        Proposal { width, height }
    }

    /// Exactly `size`.
    pub fn fixed(size: Size) -> Self {
        Proposal {
            width: Some(size.width),
            height: Some(size.height),
        }
    }

    /// The dimension along `axis`.
    pub fn along(self, axis: Axis) -> Option<f32> {
        match axis {
            Axis::Horizontal => self.width,
            Axis::Vertical => self.height,
        }
    }

    /// The dimension across `axis`.
    pub fn across(self, axis: Axis) -> Option<f32> {
        match axis {
            Axis::Horizontal => self.height,
            Axis::Vertical => self.width,
        }
    }

    /// A proposal from lengths along and across `axis`.
    pub fn on(axis: Axis, along: Option<f32>, across: Option<f32>) -> Self {
        match axis {
            Axis::Horizontal => Proposal::new(along, across),
            Axis::Vertical => Proposal::new(across, along),
        }
    }

    /// A concrete size, with `fallback` for unspecified dimensions and finite lengths.
    pub fn replacing_unspecified(self, fallback: Size) -> Size {
        let pick = |v: Option<f32>, f: f32| match v {
            Some(x) if x.is_finite() => x,
            _ => f,
        };
        Size::new(
            pick(self.width, fallback.width),
            pick(self.height, fallback.height),
        )
    }
}

/// A layout axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    /// Left to right.
    Horizontal,
    /// Top to bottom.
    Vertical,
}

impl Axis {
    /// The length of `size` along the axis.
    pub fn of(self, size: Size) -> f32 {
        match self {
            Axis::Horizontal => size.width,
            Axis::Vertical => size.height,
        }
    }

    /// The length of `size` across the axis.
    pub fn cross(self, size: Size) -> f32 {
        match self {
            Axis::Horizontal => size.height,
            Axis::Vertical => size.width,
        }
    }

    /// A size from lengths along and across the axis.
    pub fn size(self, along: f32, across: f32) -> Size {
        match self {
            Axis::Horizontal => Size::new(along, across),
            Axis::Vertical => Size::new(across, along),
        }
    }

    /// A point from positions along and across the axis.
    pub fn point(self, along: f32, across: f32) -> Point {
        match self {
            Axis::Horizontal => Point::new(along, across),
            Axis::Vertical => Point::new(across, along),
        }
    }
}

/// Horizontal alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HAlign {
    /// Left (leading).
    Leading,
    /// Centre.
    Center,
    /// Right (trailing).
    Trailing,
}

/// Vertical alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VAlign {
    /// Top.
    Top,
    /// Centre.
    Center,
    /// Bottom.
    Bottom,
    /// The first text baselines line up.
    FirstBaseline,
}

/// A two-dimensional alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Alignment {
    /// Horizontal.
    pub h: HAlign,
    /// Vertical.
    pub v: VAlign,
}

impl Alignment {
    /// Centre.
    pub const CENTER: Alignment = Alignment {
        h: HAlign::Center,
        v: VAlign::Center,
    };
    /// Top left.
    pub const TOP_LEADING: Alignment = Alignment {
        h: HAlign::Leading,
        v: VAlign::Top,
    };
    /// Top centre.
    pub const TOP: Alignment = Alignment {
        h: HAlign::Center,
        v: VAlign::Top,
    };
    /// Left centre.
    pub const LEADING: Alignment = Alignment {
        h: HAlign::Leading,
        v: VAlign::Center,
    };
    /// Right centre.
    pub const TRAILING: Alignment = Alignment {
        h: HAlign::Trailing,
        v: VAlign::Center,
    };
    /// Bottom centre.
    pub const BOTTOM: Alignment = Alignment {
        h: HAlign::Center,
        v: VAlign::Bottom,
    };

    /// The ABI number (row-major 3 × 3 grid: 0 top-leading … 8 bottom-trailing; 9 leading
    /// first-baseline, 10 centre first-baseline).
    pub fn to_int(self) -> i64 {
        let h = match self.h {
            HAlign::Leading => 0,
            HAlign::Center => 1,
            HAlign::Trailing => 2,
        };
        match self.v {
            VAlign::Top => h,
            VAlign::Center => 3 + h,
            VAlign::Bottom => 6 + h,
            VAlign::FirstBaseline => 9 + h.min(1),
        }
    }

    /// The alignment of an ABI number.
    pub fn from_int(n: i64) -> Option<Alignment> {
        let h = |i: i64| match i {
            0 => HAlign::Leading,
            1 => HAlign::Center,
            _ => HAlign::Trailing,
        };
        Some(match n {
            0..=2 => Alignment {
                h: h(n),
                v: VAlign::Top,
            },
            3..=5 => Alignment {
                h: h(n - 3),
                v: VAlign::Center,
            },
            6..=8 => Alignment {
                h: h(n - 6),
                v: VAlign::Bottom,
            },
            9 | 10 => Alignment {
                h: h(n - 9),
                v: VAlign::FirstBaseline,
            },
            _ => return None,
        })
    }

    /// The origin of a box of `inner` aligned in `outer`.
    pub fn place(self, outer: Rect, inner: Size) -> Point {
        let x = match self.h {
            HAlign::Leading => outer.x,
            HAlign::Center => outer.x + (outer.width - inner.width) / 2.0,
            HAlign::Trailing => outer.right() - inner.width,
        };
        let y = match self.v {
            VAlign::Top | VAlign::FirstBaseline => outer.y,
            VAlign::Center => outer.y + (outer.height - inner.height) / 2.0,
            VAlign::Bottom => outer.bottom() - inner.height,
        };
        Point::new(x, y)
    }
}

impl Ui {
    /// The size `id` takes for `proposal`, with its modifiers (frame, padding) applied.
    pub fn measure(&mut self, id: ViewId, proposal: Proposal) -> Size {
        self.measure_in(id, proposal, None)
    }

    pub(crate) fn measure_in(
        &mut self,
        id: ViewId,
        proposal: Proposal,
        axis: Option<Axis>,
    ) -> Size {
        let Some(node) = self.tree.get(id) else {
            return Size::ZERO;
        };
        if node.flags.contains(NodeFlags::HIDDEN) {
            return Size::ZERO;
        }
        if !node.flags.contains(NodeFlags::NEEDS_LAYOUT)
            && let Some((_, s)) = node.measure_cache.iter().find(|(p, _)| *p == proposal)
        {
            return *s;
        }
        let m = node.modifiers.clone();
        // The proposal the content sees: fixed sizes replace it, min/max clamp it, padding
        // shrinks it.
        let clamp_dim = |v: Option<f32>, fixed: Option<f32>, min: Option<f32>, max: Option<f32>| {
            if let Some(f) = fixed {
                return Some(f);
            }
            v.map(|x| {
                let mut x = x;
                if let Some(mx) = max {
                    x = x.min(mx);
                }
                if let Some(mn) = min {
                    x = x.max(mn);
                }
                x
            })
        };
        let mut inner = Proposal::new(
            clamp_dim(proposal.width, m.width, m.min_width, m.max_width),
            clamp_dim(proposal.height, m.height, m.min_height, m.max_height),
        );
        if let Some(ratio) = m.aspect_ratio.filter(|r| *r > 0.0 && r.is_finite()) {
            inner = fit_ratio(inner, ratio);
        }
        let pad = m.padding;
        let content_p = Proposal::new(
            inner.width.map(|w| (w - pad.horizontal()).max(0.0)),
            inner.height.map(|h| (h - pad.vertical()).max(0.0)),
        );
        let content = match self.tree.take_widget(id) {
            Some(mut w) => {
                let s = {
                    let mut cx = LayoutCx { ui: self, id, axis };
                    w.measure(&mut cx, content_p)
                };
                self.tree.put_widget(id, w);
                s
            }
            None => Size::ZERO,
        };
        let natural = Size::new(
            content.width + pad.horizontal(),
            content.height + pad.vertical(),
        );
        let resolve = |nat: f32,
                       prop: Option<f32>,
                       fixed: Option<f32>,
                       min: Option<f32>,
                       max: Option<f32>| {
            if let Some(f) = fixed {
                return f;
            }
            let mut v = nat;
            if let Some(mn) = min {
                v = v.max(mn);
            }
            if let Some(mx) = max {
                if mx.is_infinite() {
                    if let Some(p) = prop.filter(|p| p.is_finite()) {
                        v = v.max(p);
                    }
                } else {
                    v = v.min(mx);
                }
            }
            v
        };
        let mut size = Size::new(
            resolve(
                natural.width,
                proposal.width,
                m.width,
                m.min_width,
                m.max_width,
            ),
            resolve(
                natural.height,
                proposal.height,
                m.height,
                m.min_height,
                m.max_height,
            ),
        );
        if let Some(ratio) = m.aspect_ratio.filter(|r| *r > 0.0 && r.is_finite())
            && m.width.is_none()
            && m.height.is_none()
        {
            let fitted = fit_ratio(Proposal::fixed(size), ratio);
            size = fitted.replacing_unspecified(size);
        }
        if let Some(n) = self.tree.get_mut(id) {
            if n.flags.contains(NodeFlags::NEEDS_LAYOUT) {
                n.measure_cache.clear();
            }
            if n.measure_cache.len() >= 8 {
                n.measure_cache.remove(0);
            }
            n.measure_cache.push((proposal, size));
        }
        size
    }

    /// Places `id` at `rect` (parent coordinates) and lays out its children.
    pub fn place(&mut self, id: ViewId, rect: Rect) {
        let Some(node) = self.tree.get(id) else {
            return;
        };
        if node.flags.contains(NodeFlags::HIDDEN) {
            return;
        }
        let changed = node.frame != rect;
        let needs = node.flags.contains(NodeFlags::NEEDS_LAYOUT);
        if changed {
            let old = node.frame;
            if let Some(n) = self.tree.get_mut(id) {
                n.frame = rect;
            }
            if old.size() != rect.size() {
                self.request_paint(id);
            }
            self.animate_move(id, old, rect);
        }
        if !changed && !needs {
            return;
        }
        self.align_content(id);
        let content = self.content_box(id);
        if let Some(mut w) = self.tree.take_widget(id) {
            {
                let mut cx = LayoutCx {
                    ui: self,
                    id,
                    axis: None,
                };
                w.place(&mut cx, content.size());
            }
            self.tree.put_widget(id, w);
        }
        if let Some(n) = self.tree.get_mut(id) {
            n.flags.remove(NodeFlags::NEEDS_LAYOUT);
            // Keep only the measurement made for the final size.
            n.measure_cache.retain(|(_, s)| *s == rect.size());
        }
        if changed {
            self.emit(
                id,
                crate::core::value::Signal::Resized,
                Value::Size(rect.size()),
            );
        }
    }

    /// The content box of `id` in its own coordinates: inside the padding, or, when the
    /// view has a frame alignment, the content's own size aligned in it.
    pub fn content_box(&self, id: ViewId) -> Rect {
        let Some(n) = self.tree.get(id) else {
            return Rect::ZERO;
        };
        match n.content {
            Some(r) if n.modifiers.aligns_content => r,
            _ => Rect::from_size(n.frame.size()).inset(n.modifiers.padding),
        }
    }

    /// Lays an aligned view's content out at its own size in its frame.
    fn align_content(&mut self, id: ViewId) {
        let Some(n) = self.tree.get(id) else { return };
        if !n.modifiers.aligns_content {
            return;
        }
        let inner = Rect::from_size(n.frame.size()).inset(n.modifiers.padding);
        let alignment = n.modifiers.alignment;
        let natural = match self.tree.take_widget(id) {
            Some(mut w) => {
                let s = {
                    let mut cx = LayoutCx {
                        ui: self,
                        id,
                        axis: None,
                    };
                    w.measure(&mut cx, Proposal::fixed(inner.size()))
                };
                self.tree.put_widget(id, w);
                s
            }
            None => inner.size(),
        };
        let size = Size::new(
            natural.width.min(inner.width).max(0.0),
            natural.height.min(inner.height).max(0.0),
        );
        let o = alignment.place(inner, size);
        let r = Rect::new(o.x.round(), o.y.round(), size.width, size.height);
        if let Some(n) = self.tree.get_mut(id) {
            if n.content != Some(r) {
                n.flags.insert(NodeFlags::NEEDS_PAINT);
            }
            n.content = Some(r);
        }
    }

    pub(crate) fn content_origin(&self, id: ViewId) -> Point {
        self.content_box(id).origin()
    }

    /// The first baseline of `id` from its top.
    pub fn baseline(&mut self, id: ViewId) -> Option<f32> {
        let pad = self.tree.get(id)?.modifiers.padding.top;
        self.tree.widget(id)?.baseline().map(|b| b + pad)
    }
}

fn fit_ratio(p: Proposal, ratio: f32) -> Proposal {
    match (p.width, p.height) {
        (Some(w), Some(h)) if w.is_finite() && h.is_finite() => {
            if w / ratio <= h {
                Proposal::new(Some(w), Some(w / ratio))
            } else {
                Proposal::new(Some(h * ratio), Some(h))
            }
        }
        (Some(w), _) if w.is_finite() => Proposal::new(Some(w), Some(w / ratio)),
        (_, Some(h)) if h.is_finite() => Proposal::new(Some(h * ratio), Some(h)),
        other => Proposal::new(other.0, other.1),
    }
}

/// A stack: children in a row or a column, with spacing and cross-axis alignment.
///
/// Space is shared as SwiftUI does: children are offered the remaining length in order of
/// layout priority, then least flexible first (flexibility: the difference between a
/// child's maximum and minimum length), each taking what it needs.
#[derive(Debug, Clone)]
pub struct Stack {
    /// The axis.
    pub axis: Axis,
    /// Space between children.
    pub spacing: f32,
    /// Cross-axis alignment (horizontal for a column, vertical for a row).
    pub alignment: Alignment,
    sizes: Vec<Size>,
}

impl Stack {
    /// A row.
    pub fn row(spacing: f32) -> Self {
        Stack {
            axis: Axis::Horizontal,
            spacing,
            alignment: Alignment::CENTER,
            sizes: Vec::new(),
        }
    }

    /// A column.
    pub fn column(spacing: f32) -> Self {
        Stack {
            axis: Axis::Vertical,
            spacing,
            alignment: Alignment::CENTER,
            sizes: Vec::new(),
        }
    }

    /// With an alignment.
    pub fn aligned(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }

    fn distribute(&mut self, cx: &mut LayoutCx<'_>, p: Proposal) -> Vec<Size> {
        let axis = self.axis;
        let children = cx.children();
        let n = children.len();
        if n == 0 {
            return Vec::new();
        }
        let gaps = self.spacing * (n.saturating_sub(1)) as f32;
        let across = p.across(axis);
        let Some(total) = p.along(axis) else {
            // Ideal: every child at its ideal length.
            return children
                .iter()
                .map(|c| cx.measure_in(*c, Proposal::on(axis, None, across), axis))
                .collect();
        };
        if total.is_infinite() {
            return children
                .iter()
                .map(|c| cx.measure_in(*c, Proposal::on(axis, Some(f32::INFINITY), across), axis))
                .collect();
        }
        let mut remaining = (total - gaps).max(0.0);
        // Flexibility of each child.
        let mut order: Vec<(usize, f32, f32)> = children
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let min = axis.of(cx.measure_in(*c, Proposal::on(axis, Some(0.0), across), axis));
                let max = axis.of(cx.measure_in(
                    *c,
                    Proposal::on(axis, Some(f32::INFINITY), across),
                    axis,
                ));
                (i, cx.priority(*c), max - min)
            })
            .collect();
        order.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal))
        });
        let mut sizes = vec![Size::ZERO; n];
        // Children of higher priority groups see only what lower groups leave at their
        // minimum.
        let mins: Vec<f32> = children
            .iter()
            .map(|c| axis.of(cx.measure_in(*c, Proposal::on(axis, Some(0.0), across), axis)))
            .collect();
        let mut left = n;
        let mut idx = 0;
        while idx < order.len() {
            let prio = order.get(idx).map(|o| o.1).unwrap_or(0.0);
            let group: Vec<usize> = order
                .iter()
                .skip(idx)
                .take_while(|o| o.1 == prio)
                .map(|o| o.0)
                .collect();
            let reserved: f32 = order
                .iter()
                .skip(idx + group.len())
                .filter_map(|o| mins.get(o.0))
                .sum();
            let mut budget = (remaining - reserved).max(0.0);
            let mut in_group = group.len();
            for i in &group {
                let share = budget / in_group.max(1) as f32;
                let Some(c) = children.get(*i) else { continue };
                let s = cx.measure_in(*c, Proposal::on(axis, Some(share), across), axis);
                let used = axis.of(s);
                budget = (budget - used).max(0.0);
                remaining = (remaining - used).max(0.0);
                if let Some(slot) = sizes.get_mut(*i) {
                    *slot = s;
                }
                in_group -= 1;
                left -= 1;
            }
            idx += group.len();
        }
        let _ = left;
        sizes
    }
}

impl Widget for Stack {
    fn kind(&self) -> &'static str {
        "Stack"
    }

    fn hit(&self, _p: Point, _size: Size) -> bool {
        false
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        let sizes = self.distribute(cx, p);
        let n = sizes.len();
        let along: f32 = sizes.iter().map(|s| self.axis.of(*s)).sum::<f32>()
            + self.spacing * n.saturating_sub(1) as f32;
        let across = sizes
            .iter()
            .map(|s| self.axis.cross(*s))
            .fold(0.0f32, f32::max);
        self.axis.size(along, across)
    }

    fn place(&mut self, cx: &mut LayoutCx<'_>, size: Size) {
        let axis = self.axis;
        let p = Proposal::fixed(size);
        let sizes = self.distribute(cx, p);
        self.sizes.clone_from(&sizes);
        let children = cx.children();
        let cross_len = axis.cross(size);
        // Baselines for first-baseline alignment of a row.
        let baselines: Vec<Option<f32>> =
            if axis == Axis::Horizontal && self.alignment.v == VAlign::FirstBaseline {
                children.iter().map(|c| cx.baseline(*c)).collect()
            } else {
                Vec::new()
            };
        let max_base = baselines.iter().flatten().fold(0.0f32, |a, b| a.max(*b));
        let mut pos = 0.0;
        for (i, c) in children.iter().enumerate() {
            let s = sizes.get(i).copied().unwrap_or_default();
            let cross = match axis {
                Axis::Horizontal => match self.alignment.v {
                    VAlign::Top => 0.0,
                    VAlign::Center => (cross_len - s.height) / 2.0,
                    VAlign::Bottom => cross_len - s.height,
                    VAlign::FirstBaseline => match baselines.get(i).copied().flatten() {
                        Some(b) => max_base - b,
                        None => (cross_len - s.height) / 2.0,
                    },
                },
                Axis::Vertical => match self.alignment.h {
                    HAlign::Leading => 0.0,
                    HAlign::Center => (cross_len - s.width) / 2.0,
                    HAlign::Trailing => cross_len - s.width,
                },
            };
            cx.place(*c, Rect::from_origin_size(axis.point(pos, cross), s));
            pos += axis.of(s) + self.spacing;
        }
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match prop {
            Prop::Spacing => match value.as_f32() {
                Some(s) => {
                    self.spacing = s.max(0.0);
                    PropChange::Layout
                }
                None => PropChange::Unknown,
            },
            Prop::Alignment => match value.as_int().and_then(Alignment::from_int) {
                Some(a) => {
                    self.alignment = a;
                    PropChange::Layout
                }
                None => PropChange::Unknown,
            },
            Prop::Vertical => match value.as_bool() {
                Some(v) => {
                    self.axis = if v { Axis::Vertical } else { Axis::Horizontal };
                    PropChange::Layout
                }
                None => PropChange::Unknown,
            },
            _ => PropChange::Unknown,
        }
    }

    fn prop(&self, prop: Prop) -> Option<Value> {
        match prop {
            Prop::Spacing => Some(Value::from(self.spacing)),
            Prop::Alignment => Some(Value::Int(self.alignment.to_int())),
            Prop::Vertical => Some(Value::Bool(self.axis == Axis::Vertical)),
            _ => None,
        }
    }
}

/// A flexible space along its stack's axis (a minimum length, then whatever is left).
#[derive(Debug, Clone, Copy, Default)]
pub struct Spacer {
    /// Minimum length.
    pub min: f32,
}

impl Widget for Spacer {
    fn kind(&self) -> &'static str {
        "Spacer"
    }

    fn hit(&self, _p: Point, _size: Size) -> bool {
        false
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        // A spacer takes everything offered (∞ included: it is the most flexible view).
        let take = |v: Option<f32>, min: f32| match v {
            Some(x) => x.max(min),
            None => min,
        };
        match cx.parent_axis() {
            Some(Axis::Horizontal) => Size::new(take(p.width, self.min), 0.0),
            Some(Axis::Vertical) => Size::new(0.0, take(p.height, self.min)),
            None => Size::new(take(p.width, self.min), take(p.height, self.min)),
        }
    }
}

/// An overlay: children on top of each other, aligned.
#[derive(Debug, Clone, Copy)]
pub struct Overlay {
    /// Alignment of each child.
    pub alignment: Alignment,
}

impl Widget for Overlay {
    fn kind(&self) -> &'static str {
        "Overlay"
    }

    fn hit(&self, _p: Point, _size: Size) -> bool {
        false
    }

    fn place(&mut self, cx: &mut LayoutCx<'_>, size: Size) {
        for c in cx.children() {
            let s = cx.measure(c, Proposal::fixed(size));
            let o = self.alignment.place(Rect::from_size(size), s);
            cx.place(c, Rect::from_origin_size(o, s));
        }
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match (prop, value.as_int().and_then(Alignment::from_int)) {
            (Prop::Alignment, Some(a)) => {
                self.alignment = a;
                PropChange::Layout
            }
            _ => PropChange::Unknown,
        }
    }
}

/// A grid of equal columns; rows as tall as their tallest cell.
#[derive(Debug, Clone, Copy)]
pub struct Grid {
    /// Number of columns.
    pub columns: usize,
    /// Space between columns.
    pub column_spacing: f32,
    /// Space between rows.
    pub row_spacing: f32,
    /// Alignment of each cell's content.
    pub alignment: Alignment,
}

impl Grid {
    fn column_width(&self, width: Option<f32>, cx: &mut LayoutCx<'_>) -> f32 {
        let cols = self.columns.max(1);
        match width.filter(|w| w.is_finite()) {
            Some(w) => ((w - self.column_spacing * (cols - 1) as f32) / cols as f32).max(0.0),
            None => cx
                .children()
                .iter()
                .map(|c| cx.measure(*c, Proposal::UNSPECIFIED).width)
                .fold(0.0, f32::max),
        }
    }

    fn rows(&self, cx: &mut LayoutCx<'_>, col_w: f32) -> Vec<f32> {
        let cols = self.columns.max(1);
        let children = cx.children();
        children
            .chunks(cols)
            .map(|row| {
                row.iter()
                    .map(|c| cx.measure(*c, Proposal::new(Some(col_w), None)).height)
                    .fold(0.0, f32::max)
            })
            .collect()
    }
}

impl Widget for Grid {
    fn kind(&self) -> &'static str {
        "Grid"
    }

    fn hit(&self, _p: Point, _size: Size) -> bool {
        false
    }

    fn measure(&mut self, cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
        let cols = self.columns.max(1);
        let col_w = self.column_width(p.width, cx);
        let rows = self.rows(cx, col_w);
        let h = rows.iter().sum::<f32>() + self.row_spacing * rows.len().saturating_sub(1) as f32;
        let w = col_w * cols as f32 + self.column_spacing * (cols - 1) as f32;
        Size::new(w, h)
    }

    fn place(&mut self, cx: &mut LayoutCx<'_>, size: Size) {
        let cols = self.columns.max(1);
        let col_w = self.column_width(Some(size.width), cx);
        let rows = self.rows(cx, col_w);
        let children = cx.children();
        let mut y = 0.0;
        for (r, row) in children.chunks(cols).enumerate() {
            let h = rows.get(r).copied().unwrap_or(0.0);
            for (c, child) in row.iter().enumerate() {
                let x = c as f32 * (col_w + self.column_spacing);
                let cell = Rect::new(x, y, col_w, h);
                let s = cx.measure(*child, Proposal::fixed(cell.size()));
                let o = self.alignment.place(cell, s);
                cx.place(*child, Rect::from_origin_size(o, s));
            }
            y += h + self.row_spacing;
        }
    }

    fn set_prop(&mut self, prop: Prop, value: &Value) -> PropChange {
        match prop {
            Prop::Columns => match value.as_int() {
                Some(n) if n > 0 => {
                    self.columns = usize::try_from(n).unwrap_or(1);
                    PropChange::Layout
                }
                _ => PropChange::Unknown,
            },
            Prop::Spacing => match value.as_f32() {
                Some(s) => {
                    self.column_spacing = s.max(0.0);
                    self.row_spacing = s.max(0.0);
                    PropChange::Layout
                }
                None => PropChange::Unknown,
            },
            _ => PropChange::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::widget::Empty;

    /// A box with a fixed ideal size that accepts any proposal within min..max.
    struct Flex {
        min: f32,
        ideal: f32,
        max: f32,
    }

    impl Widget for Flex {
        fn kind(&self) -> &'static str {
            "Flex"
        }
        fn measure(&mut self, _cx: &mut LayoutCx<'_>, p: Proposal) -> Size {
            let w = match p.width {
                None => self.ideal,
                Some(w) => w.clamp(self.min, self.max),
            };
            Size::new(w, 10.0)
        }
    }

    #[test]
    fn row_shares_space_by_flexibility() {
        let mut ui = Ui::new_headless();
        let row = ui.create(Stack::row(10.0));
        let fixed = ui.create(Flex {
            min: 50.0,
            ideal: 50.0,
            max: 50.0,
        });
        let flexible = ui.create(Flex {
            min: 0.0,
            ideal: 20.0,
            max: f32::INFINITY,
        });
        ui.add_child(row, fixed);
        ui.add_child(row, flexible);
        assert_eq!(
            ui.measure(row, Proposal::UNSPECIFIED),
            Size::new(80.0, 10.0)
        );
        let s = ui.measure(row, Proposal::new(Some(200.0), Some(40.0)));
        assert_eq!(s, Size::new(200.0, 10.0));
        ui.place(row, Rect::new(0.0, 0.0, 200.0, 40.0));
        assert_eq!(
            ui.tree.get(fixed).unwrap().frame,
            Rect::new(0.0, 15.0, 50.0, 10.0)
        );
        assert_eq!(
            ui.tree.get(flexible).unwrap().frame,
            Rect::new(60.0, 15.0, 140.0, 10.0)
        );
    }

    #[test]
    fn spacer_pushes_to_edges() {
        let mut ui = Ui::new_headless();
        let row = ui.create(Stack::row(0.0));
        let a = ui.create(Flex {
            min: 30.0,
            ideal: 30.0,
            max: 30.0,
        });
        let sp = ui.create(Spacer::default());
        let b = ui.create(Flex {
            min: 30.0,
            ideal: 30.0,
            max: 30.0,
        });
        for c in [a, sp, b] {
            ui.add_child(row, c);
        }
        ui.place(row, Rect::new(0.0, 0.0, 100.0, 10.0));
        assert_eq!(ui.tree.get(b).unwrap().frame.x, 70.0);
        assert_eq!(ui.tree.get(sp).unwrap().frame.width, 40.0);
    }

    #[test]
    fn frame_and_padding_modifiers() {
        let mut ui = Ui::new_headless();
        let v = ui.create(Flex {
            min: 0.0,
            ideal: 20.0,
            max: f32::INFINITY,
        });
        ui.set(v, Prop::Padding, Value::from(4.0)).unwrap();
        assert_eq!(ui.measure(v, Proposal::UNSPECIFIED), Size::new(28.0, 18.0));
        ui.set(v, Prop::MaxWidth, Value::from(f32::INFINITY))
            .unwrap();
        assert_eq!(ui.measure(v, Proposal::new(Some(300.0), None)).width, 300.0);
        ui.set(v, Prop::Width, Value::from(50.0)).unwrap();
        assert_eq!(ui.measure(v, Proposal::new(Some(300.0), None)).width, 50.0);
        let e = ui.create(Empty);
        ui.set(e, Prop::Hidden, Value::Bool(true)).unwrap();
        assert_eq!(ui.measure(e, Proposal::INFINITY), Size::ZERO);
    }

    #[test]
    fn grid_rows() {
        let mut ui = Ui::new_headless();
        let g = ui.create(Grid {
            columns: 2,
            column_spacing: 10.0,
            row_spacing: 5.0,
            alignment: Alignment::TOP_LEADING,
        });
        for _ in 0..3 {
            let c = ui.create(Flex {
                min: 0.0,
                ideal: 20.0,
                max: f32::INFINITY,
            });
            ui.add_child(g, c);
        }
        let s = ui.measure(g, Proposal::new(Some(110.0), None));
        assert_eq!(s, Size::new(110.0, 25.0));
        assert_eq!(
            Alignment::from_int(Alignment::TRAILING.to_int()),
            Some(Alignment::TRAILING)
        );
    }
}
