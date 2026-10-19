//! Geometry in points (1 pt = 1 logical pixel), `f32` like Skia.

use std::ops::{Add, Mul, Sub};

/// A point or an offset.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point {
    /// Horizontal coordinate.
    pub x: f32,
    /// Vertical coordinate.
    pub y: f32,
}

impl Point {
    /// The origin.
    pub const ZERO: Point = Point { x: 0.0, y: 0.0 };

    /// A point.
    pub const fn new(x: f32, y: f32) -> Self {
        Point { x, y }
    }

    /// The distance from the origin.
    pub fn length(self) -> f32 {
        self.x.hypot(self.y)
    }
}

impl Add for Point {
    type Output = Point;
    fn add(self, o: Point) -> Point {
        Point::new(self.x + o.x, self.y + o.y)
    }
}

impl Sub for Point {
    type Output = Point;
    fn sub(self, o: Point) -> Point {
        Point::new(self.x - o.x, self.y - o.y)
    }
}

impl Mul<f32> for Point {
    type Output = Point;
    fn mul(self, k: f32) -> Point {
        Point::new(self.x * k, self.y * k)
    }
}

/// A size. Never negative once built through [`Size::new`].
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Size {
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
}

impl Size {
    /// The empty size.
    pub const ZERO: Size = Size {
        width: 0.0,
        height: 0.0,
    };

    /// A size; negative or non-finite dimensions become 0 (infinite stays infinite).
    pub fn new(width: f32, height: f32) -> Self {
        Size {
            width: sanitize_len(width),
            height: sanitize_len(height),
        }
    }

    /// Whether either dimension is 0.
    pub fn is_empty(self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
    }

    /// This size clamped between `min` and `max`.
    pub fn clamp(self, min: Size, max: Size) -> Size {
        Size::new(
            self.width.max(min.width).min(max.width.max(min.width)),
            self.height.max(min.height).min(max.height.max(min.height)),
        )
    }
}

/// A length that is finite and not negative, or infinite.
pub fn sanitize_len(v: f32) -> f32 {
    if v.is_nan() || v < 0.0 { 0.0 } else { v }
}

/// An axis-aligned rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Width (not negative).
    pub width: f32,
    /// Height (not negative).
    pub height: f32,
}

impl Rect {
    /// The empty rectangle at the origin.
    pub const ZERO: Rect = Rect {
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
    };

    /// A rectangle; negative sizes become 0.
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Rect {
            x: if x.is_finite() { x } else { 0.0 },
            y: if y.is_finite() { y } else { 0.0 },
            width: finite_len(width),
            height: finite_len(height),
        }
    }

    /// A rectangle at `origin` of `size`.
    pub fn from_origin_size(origin: Point, size: Size) -> Self {
        Rect::new(origin.x, origin.y, size.width, size.height)
    }

    /// A rectangle at the origin of `size`.
    pub fn from_size(size: Size) -> Self {
        Rect::new(0.0, 0.0, size.width, size.height)
    }

    /// The rectangle spanning two corners (in any order).
    pub fn from_points(a: Point, b: Point) -> Self {
        Rect::new(
            a.x.min(b.x),
            a.y.min(b.y),
            (a.x - b.x).abs(),
            (a.y - b.y).abs(),
        )
    }

    /// Top-left corner.
    pub fn origin(self) -> Point {
        Point::new(self.x, self.y)
    }

    /// Size.
    pub fn size(self) -> Size {
        Size::new(self.width, self.height)
    }

    /// Right edge.
    pub fn right(self) -> f32 {
        self.x + self.width
    }

    /// Bottom edge.
    pub fn bottom(self) -> f32 {
        self.y + self.height
    }

    /// Centre.
    pub fn center(self) -> Point {
        Point::new(self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    /// Whether the rectangle has no area.
    pub fn is_empty(self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
    }

    /// Whether `p` is inside (left and top edges included, right and bottom excluded).
    pub fn contains(self, p: Point) -> bool {
        p.x >= self.x && p.y >= self.y && p.x < self.right() && p.y < self.bottom()
    }

    /// This rectangle moved by `d`.
    pub fn translate(self, d: Point) -> Rect {
        Rect::new(self.x + d.x, self.y + d.y, self.width, self.height)
    }

    /// This rectangle grown by `d` on every side (negative shrinks it).
    pub fn inflate(self, dx: f32, dy: f32) -> Rect {
        Rect::new(
            self.x - dx,
            self.y - dy,
            self.width + 2.0 * dx,
            self.height + 2.0 * dy,
        )
    }

    /// This rectangle shrunk by `insets`.
    pub fn inset(self, i: Insets) -> Rect {
        Rect::new(
            self.x + i.left,
            self.y + i.top,
            self.width - i.left - i.right,
            self.height - i.top - i.bottom,
        )
    }

    /// The overlap of two rectangles, empty if none.
    pub fn intersect(self, o: Rect) -> Rect {
        let x0 = self.x.max(o.x);
        let y0 = self.y.max(o.y);
        let x1 = self.right().min(o.right());
        let y1 = self.bottom().min(o.bottom());
        if x1 <= x0 || y1 <= y0 {
            Rect::new(x0, y0, 0.0, 0.0)
        } else {
            Rect::new(x0, y0, x1 - x0, y1 - y0)
        }
    }

    /// The smallest rectangle containing both; an empty one is ignored.
    pub fn union(self, o: Rect) -> Rect {
        if self.is_empty() {
            return o;
        }
        if o.is_empty() {
            return self;
        }
        let x0 = self.x.min(o.x);
        let y0 = self.y.min(o.y);
        Rect::new(
            x0,
            y0,
            self.right().max(o.right()) - x0,
            self.bottom().max(o.bottom()) - y0,
        )
    }

    /// Whether the rectangles overlap.
    pub fn intersects(self, o: Rect) -> bool {
        !self.intersect(o).is_empty()
    }

    /// This rectangle with its edges moved out to whole multiples of `1 / scale`
    /// (device pixels): what a damage rectangle must cover.
    pub fn round_out(self, scale: f32) -> Rect {
        let s = if scale > 0.0 && scale.is_finite() {
            scale
        } else {
            1.0
        };
        let x0 = (self.x * s).floor() / s;
        let y0 = (self.y * s).floor() / s;
        let x1 = (self.right() * s).ceil() / s;
        let y1 = (self.bottom() * s).ceil() / s;
        Rect::new(x0, y0, x1 - x0, y1 - y0)
    }

    /// The Skia rectangle.
    pub fn to_sk(self) -> skia_safe::Rect {
        skia_safe::Rect::from_xywh(self.x, self.y, self.width, self.height)
    }
}

fn finite_len(v: f32) -> f32 {
    if v.is_finite() && v > 0.0 { v } else { 0.0 }
}

/// Distances from the four edges.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Insets {
    /// Top.
    pub top: f32,
    /// Left.
    pub left: f32,
    /// Bottom.
    pub bottom: f32,
    /// Right.
    pub right: f32,
}

impl Insets {
    /// No insets.
    pub const ZERO: Insets = Insets {
        top: 0.0,
        left: 0.0,
        bottom: 0.0,
        right: 0.0,
    };

    /// The same inset on every edge.
    pub const fn all(v: f32) -> Self {
        Insets {
            top: v,
            left: v,
            bottom: v,
            right: v,
        }
    }

    /// Horizontal and vertical insets.
    pub const fn symmetric(horizontal: f32, vertical: f32) -> Self {
        Insets {
            top: vertical,
            left: horizontal,
            bottom: vertical,
            right: horizontal,
        }
    }

    /// Each edge.
    pub const fn new(top: f32, left: f32, bottom: f32, right: f32) -> Self {
        Insets {
            top,
            left,
            bottom,
            right,
        }
    }

    /// Left + right.
    pub fn horizontal(self) -> f32 {
        self.left + self.right
    }

    /// Top + bottom.
    pub fn vertical(self) -> f32 {
        self.top + self.bottom
    }
}

/// The radius of each corner, clockwise from the top left (the kit's order).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Corners {
    /// Top-left.
    pub top_left: f32,
    /// Top-right.
    pub top_right: f32,
    /// Bottom-right.
    pub bottom_right: f32,
    /// Bottom-left.
    pub bottom_left: f32,
}

impl Corners {
    /// Square corners.
    pub const ZERO: Corners = Corners {
        top_left: 0.0,
        top_right: 0.0,
        bottom_right: 0.0,
        bottom_left: 0.0,
    };

    /// The same radius at every corner.
    pub const fn all(r: f32) -> Self {
        Corners {
            top_left: r,
            top_right: r,
            bottom_right: r,
            bottom_left: r,
        }
    }

    /// Each corner, clockwise from the top left.
    pub const fn new(top_left: f32, top_right: f32, bottom_right: f32, bottom_left: f32) -> Self {
        Corners {
            top_left,
            top_right,
            bottom_right,
            bottom_left,
        }
    }

    /// The radii in the kit's order.
    pub fn to_array(self) -> [f32; 4] {
        [
            self.top_left,
            self.top_right,
            self.bottom_right,
            self.bottom_left,
        ]
    }

    /// Whether every corner is square.
    pub fn is_zero(self) -> bool {
        self.to_array().iter().all(|r| *r <= 0.0)
    }

    /// Every rounded corner grown by `d` (never below 0); square corners stay square.
    pub fn grow(self, d: f32) -> Corners {
        let g = |r: f32| if r > 0.0 { (r + d).max(0.0) } else { 0.0 };
        Corners::new(
            g(self.top_left),
            g(self.top_right),
            g(self.bottom_right),
            g(self.bottom_left),
        )
    }
}

/// A 2D affine transform `[a c e; b d f]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    /// Matrix entries: x' = a x + c y + e, y' = b x + d y + f.
    pub m: [f32; 6],
}

impl Default for Transform {
    fn default() -> Self {
        Transform::IDENTITY
    }
}

impl Transform {
    /// The identity.
    pub const IDENTITY: Transform = Transform {
        m: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
    };

    /// A translation.
    pub const fn translate(x: f32, y: f32) -> Self {
        Transform {
            m: [1.0, 0.0, 0.0, 1.0, x, y],
        }
    }

    /// A scale about the origin.
    pub const fn scale(sx: f32, sy: f32) -> Self {
        Transform {
            m: [sx, 0.0, 0.0, sy, 0.0, 0.0],
        }
    }

    /// A rotation about the origin, in radians.
    pub fn rotate(radians: f32) -> Self {
        let (s, c) = radians.sin_cos();
        Transform {
            m: [c, s, -s, c, 0.0, 0.0],
        }
    }

    /// `self` then `next`.
    pub fn then(self, next: Transform) -> Transform {
        let [a1, b1, c1, d1, e1, f1] = self.m;
        let [a2, b2, c2, d2, e2, f2] = next.m;
        Transform {
            m: [
                a2 * a1 + c2 * b1,
                b2 * a1 + d2 * b1,
                a2 * c1 + c2 * d1,
                b2 * c1 + d2 * d1,
                a2 * e1 + c2 * f1 + e2,
                b2 * e1 + d2 * f1 + f2,
            ],
        }
    }

    /// The image of `p`.
    pub fn apply(self, p: Point) -> Point {
        let [a, b, c, d, e, f] = self.m;
        Point::new(a * p.x + c * p.y + e, b * p.x + d * p.y + f)
    }

    /// The inverse, if the transform is invertible.
    pub fn invert(self) -> Option<Transform> {
        let [a, b, c, d, e, f] = self.m;
        let det = a * d - b * c;
        if det.abs() < 1e-12 || !det.is_finite() {
            return None;
        }
        let id = 1.0 / det;
        Some(Transform {
            m: [
                d * id,
                -b * id,
                -c * id,
                a * id,
                (c * f - d * e) * id,
                (b * e - a * f) * id,
            ],
        })
    }

    /// Whether this is the identity.
    pub fn is_identity(self) -> bool {
        self == Transform::IDENTITY
    }

    /// The bounding box of the image of `r`.
    pub fn map_rect(self, r: Rect) -> Rect {
        let pts = [
            self.apply(Point::new(r.x, r.y)),
            self.apply(Point::new(r.right(), r.y)),
            self.apply(Point::new(r.x, r.bottom())),
            self.apply(Point::new(r.right(), r.bottom())),
        ];
        let mut x0 = f32::INFINITY;
        let mut y0 = f32::INFINITY;
        let mut x1 = f32::NEG_INFINITY;
        let mut y1 = f32::NEG_INFINITY;
        for p in pts {
            x0 = x0.min(p.x);
            y0 = y0.min(p.y);
            x1 = x1.max(p.x);
            y1 = y1.max(p.y);
        }
        Rect::new(x0, y0, x1 - x0, y1 - y0)
    }

    /// The Skia matrix.
    pub fn to_sk(self) -> skia_safe::Matrix {
        let [a, b, c, d, e, f] = self.m;
        skia_safe::Matrix::new_all(a, c, e, b, d, f, 0.0, 0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_ops() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        let b = Rect::new(5.0, 5.0, 10.0, 10.0);
        assert_eq!(a.intersect(b), Rect::new(5.0, 5.0, 5.0, 5.0));
        assert_eq!(a.union(b), Rect::new(0.0, 0.0, 15.0, 15.0));
        assert!(a.contains(Point::new(0.0, 0.0)));
        assert!(!a.contains(Point::new(10.0, 5.0)));
        assert_eq!(Rect::new(0.0, 0.0, -3.0, f32::NAN).size(), Size::ZERO);
        assert_eq!(
            Rect::new(0.3, 0.6, 1.0, 1.0).round_out(2.0),
            Rect::new(0.0, 0.5, 1.5, 1.5)
        );
    }

    #[test]
    fn transform_inverse() {
        let t = Transform::translate(3.0, 4.0)
            .then(Transform::scale(2.0, 2.0))
            .then(Transform::rotate(0.5));
        let p = Point::new(7.0, -2.0);
        let q = t.invert().unwrap().apply(t.apply(p));
        assert!((q.x - p.x).abs() < 1e-4 && (q.y - p.y).abs() < 1e-4);
        assert!(Transform::scale(0.0, 1.0).invert().is_none());
    }
}
