//! Implicit animations of views: a view with an animation moves to its new place smoothly
//! when its layout changes (the layout is final at once; the drawing slides from the old
//! position, FLIP-style), and a view with a transition fades (or fades and scales) in when
//! added and out when removed (it stays drawn, out of layout and hit testing, until the
//! fade ends). Reduce Motion turns springs into the quick cross-fade and removes the scale.

use crate::amalfi::tokens::motion;
use crate::core::tree::{NodeFlags, ViewId};
use crate::core::ui::Ui;
use crate::geom::{Point, Rect};
use crate::motion::{Animated, Timing};

/// How a view enters and leaves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Transition {
    /// Appears and disappears at once.
    #[default]
    None,
    /// Fades.
    Opacity,
    /// Fades and scales from 95 %.
    Scale,
}

impl Transition {
    /// The transition of an ABI number.
    pub fn from_int(n: i64) -> Option<Transition> {
        Some(match n {
            0 => Transition::None,
            1 => Transition::Opacity,
            2 => Transition::Scale,
            _ => return None,
        })
    }
}

/// The timing of an ABI number (`VETRO_PROP_ANIMATION`): 0 none, 1–5 the motion springs
/// (snappy, panel, window, morph, bouncy), 6–9 the fades (instant, quick, standard, slow).
pub fn timing_from_int(n: i64) -> Option<Option<Timing>> {
    use crate::motion::Timing as T;
    Some(match n {
        0 => None,
        1 => Some(T::Spring(motion::SPRING_SNAPPY)),
        2 => Some(T::Spring(motion::SPRING_PANEL)),
        3 => Some(T::Spring(motion::SPRING_WINDOW)),
        4 => Some(T::Spring(motion::SPRING_MORPH)),
        5 => Some(T::Spring(motion::SPRING_BOUNCY)),
        6 => Some(T::curve(motion::FADE_INSTANT_CURVE, motion::FADE_INSTANT)),
        7 => Some(T::curve(motion::FADE_QUICK_CURVE, motion::FADE_QUICK)),
        8 => Some(T::curve(motion::FADE_STANDARD_CURVE, motion::FADE_STANDARD)),
        9 => Some(T::curve(motion::FADE_SLOW_CURVE, motion::FADE_SLOW)),
        _ => return None,
    })
}

/// The animated drawing state of a view.
#[derive(Debug, Clone, Copy)]
pub struct NodeAnim {
    /// Offset from the laid-out position.
    pub offset: Animated<Point>,
    /// Opacity multiplier.
    pub opacity: Animated<f32>,
    /// Scale multiplier.
    pub scale: Animated<f32>,
}

impl Default for NodeAnim {
    fn default() -> Self {
        NodeAnim {
            offset: Animated::new(Point::ZERO),
            opacity: Animated::new(1.0),
            scale: Animated::new(1.0),
        }
    }
}

impl NodeAnim {
    fn tick(&mut self, dt: f32) -> bool {
        let a = self.offset.tick(dt);
        let b = self.opacity.tick(dt);
        let c = self.scale.tick(dt);
        a || b || c
    }
}

impl Ui {
    fn motion_timing(&self, t: Timing) -> Timing {
        if self.appearance.reduce_motion {
            match t {
                Timing::Spring(_) => Timing::curve(motion::FADE_QUICK_CURVE, motion::FADE_QUICK),
                other => other,
            }
        } else {
            t
        }
    }

    fn shown(&self, id: ViewId) -> bool {
        self.window_of(id)
            .and_then(|w| self.window(w))
            .is_some_and(|w| w.visible)
    }

    fn anim_mut(&mut self, id: ViewId) -> Option<&mut NodeAnim> {
        let n = self.tree.get_mut(id)?;
        Some(n.anim.get_or_insert_with(Box::default))
    }

    fn track(&mut self, id: ViewId) {
        if !self.node_anims.contains(&id) {
            self.node_anims.push(id);
        }
        if let Some(w) = self.window_of(id) {
            self.request_frame(w);
        }
    }

    /// The view moved from `old` to `new` (parent coordinates).
    pub(crate) fn animate_move(&mut self, id: ViewId, old: Rect, new: Rect) {
        let Some(t) = self.tree.get(id).and_then(|n| n.modifiers.animation) else {
            return;
        };
        if old.is_empty() || !self.shown(id) || self.appearance.reduce_motion {
            return;
        }
        let t = self.motion_timing(t);
        let Some(a) = self.anim_mut(id) else { return };
        let d = old.origin() - new.origin();
        let current = a.offset.get();
        a.offset.jump(current + d);
        a.offset.animate_to(Point::ZERO, t);
        self.track(id);
    }

    /// The view was added: its transition plays in.
    pub(crate) fn animate_insert(&mut self, id: ViewId) {
        let Some(n) = self.tree.get(id) else { return };
        let tr = n.modifiers.transition;
        if tr == Transition::None || !self.shown(id) {
            return;
        }
        let t = self.motion_timing(n.modifiers.animation.unwrap_or(Timing::curve(
            motion::FADE_STANDARD_CURVE,
            motion::FADE_STANDARD,
        )));
        let scale = tr == Transition::Scale && !self.appearance.reduce_motion;
        let Some(a) = self.anim_mut(id) else { return };
        a.opacity.jump(0.0);
        a.opacity.animate_to(1.0, t);
        if scale {
            a.scale.jump(0.95);
            a.scale.animate_to(1.0, t);
        }
        self.track(id);
    }

    /// The view is being removed: true when its transition plays out first (the caller then
    /// leaves it in the tree; [`Ui::advance`] removes it at the end).
    pub(crate) fn begin_leave(&mut self, id: ViewId) -> bool {
        let Some(n) = self.tree.get(id) else {
            return false;
        };
        let tr = n.modifiers.transition;
        if tr == Transition::None || n.flags.contains(NodeFlags::LEAVING) || !self.shown(id) {
            return false;
        }
        let t = self.motion_timing(n.modifiers.animation.unwrap_or(Timing::curve(
            motion::FADE_INSTANT_CURVE,
            motion::FADE_INSTANT,
        )));
        let scale = tr == Transition::Scale && !self.appearance.reduce_motion;
        if let Some(n) = self.tree.get_mut(id) {
            n.flags.insert(NodeFlags::LEAVING);
        }
        if let Some(a) = self.anim_mut(id) {
            a.opacity.animate_to(0.0, t);
            if scale {
                a.scale.animate_to(0.95, t);
            }
        }
        self.track(id);
        if let Some(p) = self.tree.parent(id) {
            self.request_layout(p);
        }
        true
    }

    /// Advances the views' implicit animations; removes views whose leave transition ended.
    pub(crate) fn tick_node_anims(&mut self, dt: f32) {
        let list = std::mem::take(&mut self.node_anims);
        let mut still = Vec::new();
        let mut done_leaving = Vec::new();
        for id in list {
            let Some(n) = self.tree.get_mut(id) else {
                continue;
            };
            let leaving = n.flags.contains(NodeFlags::LEAVING);
            let more = n.anim.as_mut().is_some_and(|a| a.tick(dt));
            self.request_paint(id);
            if more {
                still.push(id);
            } else if leaving {
                done_leaving.push(id);
            }
        }
        for id in std::mem::take(&mut self.node_anims) {
            if !still.contains(&id) {
                still.push(id);
            }
        }
        self.node_anims = still;
        for id in done_leaving {
            self.detach_now(id);
        }
    }

    /// Whether views animate implicitly.
    pub fn has_node_anims(&self) -> bool {
        !self.node_anims.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::layout::Stack;
    use crate::core::value::{Prop, Value};
    use crate::core::widget::Empty;
    use crate::core::window::WindowKind;
    use crate::geom::Size;

    fn boxed(ui: &mut Ui) -> ViewId {
        let v = ui.create(Empty);
        ui.set(v, Prop::Width, Value::from(20.0)).unwrap();
        ui.set(v, Prop::Height, Value::from(20.0)).unwrap();
        v
    }

    #[test]
    fn moves_slide_and_removals_fade() {
        let mut ui = Ui::new_headless();
        let w = ui.create_window(WindowKind::Normal, Size::new(200.0, 100.0));
        let row = ui.create(Stack::row(10.0));
        let a = boxed(&mut ui);
        let b = boxed(&mut ui);
        ui.set(b, Prop::Animation, Value::Int(3)).unwrap();
        ui.set(a, Prop::Transition, Value::Int(1)).unwrap();
        ui.add_child(row, a);
        ui.add_child(row, b);
        ui.set_content(w, row);
        ui.show_window(w);
        ui.prepare_frame(w);
        let bx = ui.tree.window_rect(b).x;
        // Removing `a` moves `b` left; `a` fades out first.
        ui.detach(a);
        assert_eq!(ui.tree.parent(a), Some(row), "still drawn while it fades");
        ui.prepare_frame(w);
        assert!(ui.tree.window_rect(b).x < bx);
        let off = ui
            .tree
            .get(b)
            .and_then(|n| n.anim.as_ref())
            .map(|x| x.offset.get().x)
            .unwrap();
        assert!(off > 0.0, "b slides from where it was");
        for _ in 0..120 {
            ui.advance(1.0 / 60.0);
        }
        assert_eq!(ui.tree.parent(a), None, "a removed after its fade");
        let off = ui
            .tree
            .get(b)
            .and_then(|n| n.anim.as_ref())
            .map(|x| x.offset.get().x)
            .unwrap();
        assert_eq!(off, 0.0);
    }
}
