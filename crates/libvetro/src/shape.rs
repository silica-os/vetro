//! Shapes: smooth (continuous) rounded rectangles, ellipses, capsules and free paths.
//!
//! Smooth corners: a rounded rectangle whose corners blend into the sides with a curvature
//! that grows gradually instead of jumping from 0 to 1 / radius as a circular arc does. Each
//! corner is a circular arc in the middle, joined to each side by a cubic Bézier that starts
//! tangent to the side. With smoothing s on a side, the corner starts (1 + s) × radius from
//! the vertex along that side instead of radius, and the Bézier covers α = 45° × s of the
//! corner's turn; the arc covers what is left (90° × (1 − s) when both sides have s). Per
//! side, for a corner of radius r (angles in degrees):
//!
//! ```text
//!   p     = (1 + s) r                       distance from the vertex where the corner starts
//!   α     = 45 s                            angle covered by the Bézier
//!   c     = r tan(α / 2) cos α,  d = c tan α
//!   b     = (p − r + r sin α − c) / 3,  a = 2 b
//! ```
//!
//! The Bézier from the side has the relative control points (a, 0), (a + b, 0) and ends on
//! the circle at α; the arc runs to 90 − α of the other side, whose Bézier mirrors the first.
//!
//! The smoothing is fitted to each side on its own, as the kit's editor does: on a side
//! shorter than 2 p it shrinks until the corner fits in half of the side (a capsule keeps the
//! smoothing along its long sides and meets its short sides as a circle). Every radius is
//! clamped to half the shorter side, so a large radius gives a capsule. This is the same
//! geometry as silica_ui's `AmalfiSmoothBorder` (ported), so both draw identical outlines.

use skia_safe::{Path, PathBuilder, PathDirection, path_builder::ArcSize};

use crate::geom::{Corners, Point, Rect};

/// Amalfi's smoothing on every rounded surface (tokens: radii.cornerStyle).
pub const AMALFI_SMOOTHING: f32 = 0.6;

/// The outline of a layer.
#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    /// A rectangle with a radius per corner and a smoothing (0: circular arcs).
    Rect {
        /// Corner radii (clamped to half the shorter side when drawn).
        corners: Corners,
        /// Smoothing, 0–1.
        smoothing: f32,
    },
    /// A capsule: radius half the shorter side, smooth.
    Capsule,
    /// The ellipse inscribed in the frame.
    Ellipse,
    /// A path in the layer's own coordinates (its frame's origin at 0, 0).
    Path(Path),
}

impl Shape {
    /// A rectangle with square corners.
    pub const RECT: Shape = Shape::Rect {
        corners: Corners::ZERO,
        smoothing: 0.0,
    };

    /// A rectangle with smooth corners of radius `r` (Amalfi smoothing).
    pub fn rounded(r: f32) -> Shape {
        Shape::Rect {
            corners: Corners::all(r),
            smoothing: AMALFI_SMOOTHING,
        }
    }

    /// A rectangle with smooth corners of their own radii (Amalfi smoothing).
    pub fn rounded_corners(corners: Corners) -> Shape {
        Shape::Rect {
            corners,
            smoothing: AMALFI_SMOOTHING,
        }
    }

    /// The outline in `rect`.
    pub fn path(&self, rect: Rect) -> Path {
        match self {
            Shape::Rect { corners, smoothing } => smooth_rect_path(rect, *corners, *smoothing),
            Shape::Capsule => {
                let r = rect.width.min(rect.height) / 2.0;
                smooth_rect_path(rect, Corners::all(r), AMALFI_SMOOTHING)
            }
            Shape::Ellipse => {
                let mut b = PathBuilder::new();
                b.add_oval(rect.to_sk(), None, None);
                b.detach()
            }
            Shape::Path(p) => p.make_offset((rect.x, rect.y)),
        }
    }

    /// The radii a kit-parity check sees: the corners as drawn in a frame of `size`.
    pub fn corner_radii(&self, rect: Rect) -> [f32; 4] {
        let limit = rect.width.min(rect.height) / 2.0;
        match self {
            Shape::Rect { corners, .. } => corners.to_array().map(|r| r.clamp(0.0, limit)),
            Shape::Capsule => [limit; 4],
            Shape::Ellipse | Shape::Path(_) => [0.0; 4],
        }
    }

    /// The same shape with every rounded corner grown by `d` (for a spread shadow or a ring
    /// drawn around the shape).
    pub fn grown(&self, d: f32) -> Shape {
        match self {
            Shape::Rect { corners, smoothing } => Shape::Rect {
                corners: corners.grow(d),
                smoothing: *smoothing,
            },
            other => other.clone(),
        }
    }
}

/// The outline of `rect` with a smooth corner of its own radius at each corner (0: square),
/// all with `smoothing`. An empty rect gives an empty path.
pub fn smooth_rect_path(rect: Rect, corners: Corners, smoothing: f32) -> Path {
    let mut path = PathBuilder::new();
    if rect.is_empty() {
        return path.detach();
    }
    let half_w = rect.width / 2.0;
    let half_h = rect.height / 2.0;
    let limit = half_w.min(half_h);
    let s = if smoothing.is_finite() {
        smoothing.clamp(0.0, 1.0)
    } else {
        0.0
    };
    // Going clockwise, each corner is entered along one side and left along the other.
    let tr = SmoothCorner::new(corners.top_right, s, limit, half_w, half_h);
    let br = SmoothCorner::new(corners.bottom_right, s, limit, half_h, half_w);
    let bl = SmoothCorner::new(corners.bottom_left, s, limit, half_w, half_h);
    let tl = SmoothCorner::new(corners.top_left, s, limit, half_h, half_w);
    if tl.is_square() && tr.is_square() && br.is_square() && bl.is_square() {
        path.add_rect(rect.to_sk(), None, None);
        return path.detach();
    }
    let (left, top, right, bottom) = (rect.x, rect.y, rect.right(), rect.bottom());
    // Each corner is drawn in its own frame (see SmoothCorner), rotated onto the rect.
    let at_tr = move |p: Point| Point::new(right + p.x, top + p.y);
    let at_br = move |p: Point| Point::new(right - p.y, bottom + p.x);
    let at_bl = move |p: Point| Point::new(left - p.x, bottom - p.y);
    let at_tl = move |p: Point| Point::new(left + p.y, top - p.x);

    let start = at_tl(tl.end());
    path.move_to((start.x, start.y));
    tr.add_to(&mut path, &at_tr);
    br.add_to(&mut path, &at_br);
    bl.add_to(&mut path, &at_bl);
    tl.add_to(&mut path, &at_tl);
    path.close();
    path.detach()
}

/// The geometry of one smooth corner in the frame of a top-right corner: the vertex at the
/// origin, entered along the top edge (y = 0, from negative x) and left along the right edge
/// (x = 0, towards positive y).
#[derive(Debug, Clone, Copy)]
struct SmoothCorner {
    r: f32,
    s_in: f32,
    s_out: f32,
}

impl SmoothCorner {
    fn new(radius: f32, smoothing: f32, limit: f32, in_budget: f32, out_budget: f32) -> Self {
        let r = if radius.is_finite() {
            radius.clamp(0.0, limit.max(0.0))
        } else {
            0.0
        };
        if r <= 0.0 {
            return SmoothCorner {
                r: 0.0,
                s_in: 0.0,
                s_out: 0.0,
            };
        }
        // On each side the smoothing shrinks until the corner fits in half of that side.
        let fit = |budget: f32| (smoothing.min(budget / r - 1.0)).max(0.0);
        SmoothCorner {
            r,
            s_in: fit(in_budget),
            s_out: fit(out_budget),
        }
    }

    fn is_square(&self) -> bool {
        self.r <= 0.0
    }

    /// The point of the corner's circle at `degrees` from the top (0) to the right (90).
    fn on_circle(&self, degrees: f32) -> Point {
        let a = degrees.to_radians();
        Point::new(-self.r + self.r * a.sin(), self.r - self.r * a.cos())
    }

    fn start(&self) -> Point {
        Point::new(-(1.0 + self.s_in) * self.r, 0.0)
    }

    fn end(&self) -> Point {
        Point::new(0.0, (1.0 + self.s_out) * self.r)
    }

    fn add_to(&self, path: &mut PathBuilder, at: &dyn Fn(Point) -> Point) {
        if self.is_square() {
            let v = at(Point::ZERO);
            path.line_to((v.x, v.y));
            return;
        }
        let r = self.r;
        let start = self.start();
        let s0 = at(start);
        path.line_to((s0.x, s0.y));
        let cubic = |path: &mut PathBuilder, c1: Point, c2: Point, e: Point| {
            let (p1, p2, p3) = (at(c1), at(c2), at(e));
            path.cubic_to((p1.x, p1.y), (p2.x, p2.y), (p3.x, p3.y));
        };
        let alpha_in = 45.0 * self.s_in;
        let arc_start = self.on_circle(alpha_in);
        if alpha_in > 0.0 {
            let c = r * (alpha_in / 2.0).to_radians().tan() * alpha_in.to_radians().cos();
            let b = (arc_start.x - start.x - c) / 3.0;
            cubic(
                path,
                start + Point::new(2.0 * b, 0.0),
                start + Point::new(3.0 * b, 0.0),
                arc_start,
            );
        }
        let alpha_out = 45.0 * self.s_out;
        let arc_end = self.on_circle(90.0 - alpha_out);
        if 90.0 - alpha_out - alpha_in > 1e-6 {
            let e = at(arc_end);
            path.arc_to_radius((r, r), 0.0, ArcSize::Small, PathDirection::CW, (e.x, e.y));
        }
        if alpha_out > 0.0 {
            let end = self.end();
            let c = r * (alpha_out / 2.0).to_radians().tan() * alpha_out.to_radians().cos();
            let b = (end.y - arc_end.y - c) / 3.0;
            cubic(
                path,
                Point::new(0.0, arc_end.y + c),
                Point::new(0.0, arc_end.y + b + c),
                end,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounds(p: &Path) -> skia_safe::Rect {
        *p.bounds()
    }

    #[test]
    fn square_and_empty() {
        let p = smooth_rect_path(Rect::new(0.0, 0.0, 10.0, 5.0), Corners::ZERO, 0.6);
        assert_eq!(bounds(&p), skia_safe::Rect::from_xywh(0.0, 0.0, 10.0, 5.0));
        assert!(smooth_rect_path(Rect::ZERO, Corners::all(4.0), 0.6).is_empty());
    }

    #[test]
    fn smooth_corner_stays_inside_and_touches_edges() {
        let r = Rect::new(2.0, 3.0, 116.0, 24.0);
        let p = smooth_rect_path(r, Corners::all(8.0), AMALFI_SMOOTHING);
        let b = bounds(&p);
        assert!((b.left - 2.0).abs() < 1e-3 && (b.top - 3.0).abs() < 1e-3);
        assert!((b.right - 118.0).abs() < 1e-3 && (b.bottom - 27.0).abs() < 1e-3);
        // The vertex is outside; a point near the middle of an edge is inside.
        assert!(!p.contains((2.2, 3.2)));
        assert!(p.contains((60.0, 3.5)));
        // The corner starts (1 + s) r from the vertex along the top edge: 12.8 like the kit.
        // The top edge is straight from 12.8 to 103.2, as the kit's own path.
        assert!(p.contains((2.0 + 13.0, 3.02)));
        assert!(p.contains((2.0 + 103.0, 3.02)));
        assert!(!p.contains((2.0 + 3.0, 3.3)));
    }

    #[test]
    fn capsule_clamps() {
        let p = Shape::Capsule.path(Rect::new(0.0, 0.0, 40.0, 20.0));
        assert!(p.contains((20.0, 10.0)));
        assert!(!p.contains((0.5, 0.5)));
        assert_eq!(
            Shape::rounded(50.0).corner_radii(Rect::new(0.0, 0.0, 40.0, 20.0)),
            [10.0; 4]
        );
    }
}
