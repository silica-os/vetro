//! Motion: cubic Bézier curves, springs and interruptible animated values.
//!
//! Curves and springs are the measured ones (silica-desktop `docs/spec`) wherever a
//! measurement exists; the tokens of `motion.md` apply only to animations never measured.
//! Springs follow `motion.md`: response (s) and damping ratio ζ, i.e. mass 1, stiffness
//! k = (2π / response)², damping c = 2 ζ √k. An animated value carries its velocity over when
//! retargeted, so interrupted animations stay continuous.

/// A CSS-style cubic Bézier timing curve from (0, 0) to (1, 1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cubic {
    /// First control point.
    pub x1: f32,
    /// First control point.
    pub y1: f32,
    /// Second control point.
    pub x2: f32,
    /// Second control point.
    pub y2: f32,
}

impl Cubic {
    /// The linear curve.
    pub const LINEAR: Cubic = Cubic::new(0.0, 0.0, 1.0, 1.0);

    /// A curve with control points (x1, y1) and (x2, y2).
    pub const fn new(x1: f32, y1: f32, x2: f32, y2: f32) -> Self {
        Cubic { x1, y1, x2, y2 }
    }

    fn sample(a1: f32, a2: f32, t: f32) -> f32 {
        // B(t) = 3(1-t)²t a1 + 3(1-t)t² a2 + t³
        let u = 1.0 - t;
        3.0 * u * u * t * a1 + 3.0 * u * t * t * a2 + t * t * t
    }

    fn slope(a1: f32, a2: f32, t: f32) -> f32 {
        let u = 1.0 - t;
        3.0 * u * u * a1 + 6.0 * u * t * (a2 - a1) + 3.0 * t * t * (1.0 - a2)
    }

    /// The curve's progress at time fraction `x` (0–1, clamped).
    pub fn transform(&self, x: f32) -> f32 {
        if !x.is_finite() {
            return 1.0;
        }
        let x = x.clamp(0.0, 1.0);
        if x <= 0.0 {
            return 0.0;
        }
        if x >= 1.0 {
            return 1.0;
        }
        // Newton's method, then bisection as a fallback.
        let mut t = x;
        for _ in 0..8 {
            let err = Self::sample(self.x1, self.x2, t) - x;
            if err.abs() < 1e-6 {
                return Self::sample(self.y1, self.y2, t);
            }
            let d = Self::slope(self.x1, self.x2, t);
            if d.abs() < 1e-6 {
                break;
            }
            t = (t - err / d).clamp(0.0, 1.0);
        }
        let (mut lo, mut hi) = (0.0f32, 1.0f32);
        t = x;
        for _ in 0..40 {
            let v = Self::sample(self.x1, self.x2, t);
            if (v - x).abs() < 1e-6 {
                break;
            }
            if v < x {
                lo = t;
            } else {
                hi = t;
            }
            t = (lo + hi) / 2.0;
        }
        Self::sample(self.y1, self.y2, t)
    }
}

/// A spring as `motion.md` gives it: response in seconds and damping ratio.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpringToken {
    /// Response (period of the undamped oscillation), seconds.
    pub response: f32,
    /// Damping ratio ζ.
    pub damping_ratio: f32,
}

impl SpringToken {
    /// Stiffness k = (2π / response)².
    pub fn stiffness(&self) -> f32 {
        let r = self.response.max(0.001);
        (std::f32::consts::TAU / r).powi(2)
    }

    /// Damping c = 2 ζ √k.
    pub fn damping(&self) -> f32 {
        2.0 * self.damping_ratio.max(0.0) * self.stiffness().sqrt()
    }
}

/// A timing: a curve over a duration, a spring, or none (jump).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Timing {
    /// Jump to the target.
    None,
    /// A Bézier curve over `duration` seconds, after `delay` seconds.
    Curve {
        /// The curve.
        curve: Cubic,
        /// Seconds.
        duration: f32,
        /// Seconds before the change starts.
        delay: f32,
    },
    /// A spring (mass 1).
    Spring(SpringToken),
}

impl Timing {
    /// A curve over `duration` seconds.
    pub const fn curve(curve: Cubic, duration: f32) -> Self {
        Timing::Curve {
            curve,
            duration,
            delay: 0.0,
        }
    }
}

/// A value that can be animated: interpolation and the vector space springs need.
pub trait Animatable: Copy + PartialEq {
    /// `a + (b − a) t`.
    fn lerp(a: Self, b: Self, t: f32) -> Self;
    /// Components for spring integration (at most 4).
    fn to_vec(self) -> [f32; 4];
    /// The inverse of [`Animatable::to_vec`].
    fn from_vec(v: [f32; 4]) -> Self;
}

impl Animatable for f32 {
    fn lerp(a: f32, b: f32, t: f32) -> f32 {
        a + (b - a) * t
    }
    fn to_vec(self) -> [f32; 4] {
        [self, 0.0, 0.0, 0.0]
    }
    fn from_vec(v: [f32; 4]) -> f32 {
        v[0]
    }
}

impl Animatable for crate::geom::Point {
    fn lerp(a: Self, b: Self, t: f32) -> Self {
        crate::geom::Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
    }
    fn to_vec(self) -> [f32; 4] {
        [self.x, self.y, 0.0, 0.0]
    }
    fn from_vec(v: [f32; 4]) -> Self {
        crate::geom::Point::new(v[0], v[1])
    }
}

impl Animatable for crate::geom::Rect {
    fn lerp(a: Self, b: Self, t: f32) -> Self {
        crate::geom::Rect::new(
            a.x + (b.x - a.x) * t,
            a.y + (b.y - a.y) * t,
            a.width + (b.width - a.width) * t,
            a.height + (b.height - a.height) * t,
        )
    }
    fn to_vec(self) -> [f32; 4] {
        [self.x, self.y, self.width, self.height]
    }
    fn from_vec(v: [f32; 4]) -> Self {
        crate::geom::Rect::new(v[0], v[1], v[2], v[3])
    }
}

impl Animatable for crate::color::Color {
    fn lerp(a: Self, b: Self, t: f32) -> Self {
        a.lerp(b, t)
    }
    fn to_vec(self) -> [f32; 4] {
        [
            f32::from(self.r),
            f32::from(self.g),
            f32::from(self.b),
            f32::from(self.a),
        ]
    }
    fn from_vec(v: [f32; 4]) -> Self {
        let c = |x: f32| x.round().clamp(0.0, 255.0) as u8;
        crate::color::Color::rgba(c(v[0]), c(v[1]), c(v[2]), c(v[3]))
    }
}

/// An animated value: set a target with a timing, advance it with the frame clock.
#[derive(Debug, Clone, Copy)]
pub struct Animated<T: Animatable> {
    value: T,
    from: T,
    target: T,
    velocity: [f32; 4],
    timing: Timing,
    elapsed: f32,
    active: bool,
}

impl<T: Animatable> Animated<T> {
    /// A value at rest.
    pub fn new(value: T) -> Self {
        Animated {
            value,
            from: value,
            target: value,
            velocity: [0.0; 4],
            timing: Timing::None,
            elapsed: 0.0,
            active: false,
        }
    }

    /// The current value.
    pub fn get(&self) -> T {
        self.value
    }

    /// The target.
    pub fn target(&self) -> T {
        self.target
    }

    /// Whether the value is still moving.
    pub fn is_animating(&self) -> bool {
        self.active
    }

    /// Moves to `target` with `timing`, from the current value and velocity. Reduce Motion
    /// is applied by the caller (a cross-fade timing or [`Timing::None`]).
    pub fn animate_to(&mut self, target: T, timing: Timing) {
        if target == self.target && self.active {
            return;
        }
        self.target = target;
        self.from = self.value;
        self.elapsed = 0.0;
        self.timing = timing;
        match timing {
            Timing::None => self.jump(target),
            Timing::Curve { duration, .. } if duration <= 0.0 => self.jump(target),
            _ => {
                if self.value == target && self.velocity.iter().all(|v| v.abs() < 1e-4) {
                    self.active = false;
                } else {
                    self.active = true;
                }
            }
        }
    }

    /// Sets the value with no animation.
    pub fn jump(&mut self, value: T) {
        self.value = value;
        self.from = value;
        self.target = value;
        self.velocity = [0.0; 4];
        self.active = false;
    }

    /// Advances by `dt` seconds; returns whether the value is still moving.
    pub fn tick(&mut self, dt: f32) -> bool {
        if !self.active {
            return false;
        }
        let dt = if dt.is_finite() { dt.clamp(0.0, 0.25) } else { 0.0 };
        self.elapsed += dt;
        match self.timing {
            Timing::None => self.jump(self.target),
            Timing::Curve {
                curve,
                duration,
                delay,
            } => {
                let t = ((self.elapsed - delay) / duration.max(1e-4)).clamp(0.0, 1.0);
                let prev = self.value.to_vec();
                self.value = T::lerp(self.from, self.target, curve.transform(t));
                let now = self.value.to_vec();
                if dt > 0.0 {
                    for i in 0..4 {
                        if let (Some(v), Some(n), Some(p)) =
                            (self.velocity.get_mut(i), now.get(i), prev.get(i))
                        {
                            *v = (n - p) / dt;
                        }
                    }
                }
                if t >= 1.0 {
                    self.jump(self.target);
                }
            }
            Timing::Spring(spring) => {
                let k = spring.stiffness();
                let c = spring.damping();
                let target = self.target.to_vec();
                let mut x = self.value.to_vec();
                // Semi-implicit Euler in sub-steps of at most 1 ms: stable for every token.
                let steps = (dt / 0.001).ceil().max(1.0) as usize;
                let h = dt / steps as f32;
                for _ in 0..steps {
                    for i in 0..4 {
                        if let (Some(xi), Some(vi), Some(ti)) =
                            (x.get_mut(i), self.velocity.get_mut(i), target.get(i))
                        {
                            let a = -k * (*xi - ti) - c * *vi;
                            *vi += a * h;
                            *xi += *vi * h;
                        }
                    }
                }
                self.value = T::from_vec(x);
                let settled = x
                    .iter()
                    .zip(target.iter())
                    .zip(self.velocity.iter())
                    .all(|((xi, ti), vi)| (xi - ti).abs() < 0.01 && vi.abs() < 0.05);
                if settled {
                    self.jump(self.target);
                }
            }
        }
        self.active
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cubic_endpoints_and_monotonic() {
        let c = Cubic::new(0.4, 0.0, 0.2, 1.0);
        assert_eq!(c.transform(0.0), 0.0);
        assert_eq!(c.transform(1.0), 1.0);
        let mut last = 0.0;
        for i in 1..=100 {
            let v = c.transform(i as f32 / 100.0);
            assert!(v >= last - 1e-5);
            last = v;
        }
        assert!((Cubic::LINEAR.transform(0.3) - 0.3).abs() < 1e-4);
        assert_eq!(c.transform(f32::NAN), 1.0);
    }

    #[test]
    fn spring_settles_with_overshoot_by_damping() {
        let bouncy = SpringToken {
            response: 0.6,
            damping_ratio: 0.5,
        };
        assert!((bouncy.stiffness() - 109.66).abs() < 0.1);
        assert!((bouncy.damping() - 10.47).abs() < 0.05);
        let mut a = Animated::new(0.0f32);
        a.animate_to(100.0, Timing::Spring(bouncy));
        let mut max: f32 = 0.0;
        let mut frames = 0;
        while a.tick(1.0 / 120.0) && frames < 2000 {
            max = max.max(a.get());
            frames += 1;
        }
        assert_eq!(a.get(), 100.0);
        // ζ 0.5 overshoots about 16 % (motion.md).
        assert!(max > 112.0 && max < 120.0, "{max}");
    }

    #[test]
    fn retarget_keeps_continuity() {
        let mut a = Animated::new(0.0f32);
        let snappy = SpringToken {
            response: 0.25,
            damping_ratio: 0.9,
        };
        a.animate_to(10.0, Timing::Spring(snappy));
        for _ in 0..10 {
            a.tick(1.0 / 120.0);
        }
        let before = a.get();
        a.animate_to(-10.0, Timing::Spring(snappy));
        a.tick(1.0 / 120.0);
        assert!((a.get() - before).abs() < 1.0);
    }

    #[test]
    fn curve_runs_its_duration() {
        let mut a = Animated::new(0.0f32);
        a.animate_to(1.0, Timing::curve(Cubic::LINEAR, 0.1));
        a.tick(0.05);
        assert!((a.get() - 0.5).abs() < 1e-3);
        assert!(!a.tick(0.06));
        assert_eq!(a.get(), 1.0);
    }
}
