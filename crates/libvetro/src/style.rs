//! Layer styles as the Amalfi kit defines them, and their painting on a Skia canvas.
//!
//! A style is what a kit layer carries: fills, borders, drop shadows, inner shadows and
//! background or glass blurs. Painting follows the kit (and silica_ui's foundation):
//!
//! - drop shadows have CSS `box-shadow` semantics, a Gaussian of σ = blur / 2, drawn outside
//!   the shape only (a shadow never shows through a translucent fill);
//! - then the backdrop blur (glass), clipped to the shape;
//! - then the fills in order, bottom first;
//! - then inner shadows, clipped to the shape;
//! - then borders: inside borders are stroked within the outline, outside ones beyond it.

use skia_safe::{
    BlurStyle, Canvas, ClipOp, ColorMatrix, MaskFilter, Paint as SkPaint, PaintStyle, Path,
    PathEffect, PathOp, TileMode,
    canvas::SaveLayerRec,
    color_filters,
    gradient::{Colors, Gradient, Interpolation, shaders},
    image_filters,
};

use crate::color::Color;
use crate::geom::{Point, Rect};
use crate::shape::Shape;

/// A gradient stop: position 0–1 and colour.
pub type Stop = (f32, Color);

/// What a fill or a border paints with.
#[derive(Debug, Clone, PartialEq)]
pub enum Paint {
    /// One colour.
    Solid(Color),
    /// A linear gradient between two points given in unit coordinates of the layer's frame.
    Linear {
        /// Start, in 0–1 of the frame.
        from: Point,
        /// End, in 0–1 of the frame.
        to: Point,
        /// Stops.
        stops: Vec<Stop>,
    },
    /// A radial gradient from `from` (centre) to `to` (a point on the outer circle).
    Radial {
        /// Centre, in 0–1 of the frame.
        from: Point,
        /// A point of the outer circle, in 0–1 of the frame.
        to: Point,
        /// Stops.
        stops: Vec<Stop>,
    },
    /// An angular (sweep) gradient around the frame's centre, starting at 12 o'clock.
    Angular {
        /// Stops.
        stops: Vec<Stop>,
    },
}

impl Paint {
    /// The colour of a solid paint.
    pub fn solid(&self) -> Option<Color> {
        match self {
            Paint::Solid(c) => Some(*c),
            _ => None,
        }
    }

    /// Whether the paint draws nothing.
    pub fn is_invisible(&self) -> bool {
        match self {
            Paint::Solid(c) => c.a == 0,
            Paint::Linear { stops, .. }
            | Paint::Radial { stops, .. }
            | Paint::Angular { stops } => stops.iter().all(|(_, c)| c.a == 0),
        }
    }

    fn apply(&self, paint: &mut SkPaint, rect: Rect, opacity: f32) {
        let unit =
            |p: Point| skia_safe::Point::new(rect.x + p.x * rect.width, rect.y + p.y * rect.height);
        let stops_of = |stops: &[Stop]| -> (Vec<skia_safe::Color4f>, Vec<f32>) {
            (
                stops
                    .iter()
                    .map(|(_, c)| c.with_opacity(opacity).to_sk4f())
                    .collect(),
                stops.iter().map(|(p, _)| p.clamp(0.0, 1.0)).collect(),
            )
        };
        let (colors, positions) = match self {
            Paint::Solid(c) => {
                paint.set_color(c.with_opacity(opacity).to_sk());
                return;
            }
            Paint::Linear { stops, .. }
            | Paint::Radial { stops, .. }
            | Paint::Angular { stops } => stops_of(stops),
        };
        if colors.len() < 2 || colors.len() != positions.len() {
            let c = colors
                .first()
                .copied()
                .unwrap_or(skia_safe::Color4f::new(0.0, 0.0, 0.0, 0.0));
            paint.set_color4f(c, None);
            return;
        }
        let gradient = Gradient::new(
            Colors::new(&colors, Some(&positions), TileMode::Clamp, None),
            Interpolation::default(),
        );
        let shader = match self {
            Paint::Solid(_) => None,
            Paint::Linear { from, to, .. } => {
                shaders::linear_gradient((unit(*from), unit(*to)), &gradient, None)
            }
            Paint::Radial { from, to, .. } => {
                let center = unit(*from);
                let radius = (unit(*to) - center).length().max(0.001);
                shaders::radial_gradient((center, radius), &gradient, None)
            }
            Paint::Angular { .. } => {
                let center = rect.center();
                // Skia sweeps from 3 o'clock; the kit from 12.
                let m = skia_safe::Matrix::rotate_deg_pivot(-90.0, (center.x, center.y));
                shaders::sweep_gradient((center.x, center.y), (0.0, 360.0), &gradient, Some(&m))
            }
        };
        match shader {
            Some(s) => {
                paint.set_shader(s);
            }
            None => {
                if let Some(c) = colors.first() {
                    paint.set_color4f(*c, None);
                }
            }
        }
    }
}

impl From<Color> for Paint {
    fn from(c: Color) -> Self {
        Paint::Solid(c)
    }
}

/// Where a border is stroked relative to the outline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BorderPosition {
    /// Within the outline (the kit's default).
    #[default]
    Inside,
    /// Centred on the outline.
    Center,
    /// Beyond the outline.
    Outside,
}

/// A border.
#[derive(Debug, Clone, PartialEq)]
pub struct Border {
    /// What it paints with.
    pub paint: Paint,
    /// Thickness in points.
    pub thickness: f32,
    /// Position.
    pub position: BorderPosition,
    /// Dash pattern (on, off, …); empty for a solid line.
    pub dash: Vec<f32>,
    /// Round line ends (for dashes).
    pub round_ends: bool,
}

impl Border {
    /// A solid inside border.
    pub fn inside(color: Color, thickness: f32) -> Self {
        Border {
            paint: Paint::Solid(color),
            thickness,
            position: BorderPosition::Inside,
            dash: Vec::new(),
            round_ends: false,
        }
    }

    /// A solid border at `position`.
    pub fn at(color: Color, thickness: f32, position: BorderPosition) -> Self {
        Border {
            position,
            ..Border::inside(color, thickness)
        }
    }
}

/// A drop or inner shadow, with the kit's (CSS) blur.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shadow {
    /// Colour.
    pub color: Color,
    /// Offset.
    pub x: f32,
    /// Offset.
    pub y: f32,
    /// CSS blur (σ = blur / 2).
    pub blur: f32,
    /// Spread.
    pub spread: f32,
}

impl Shadow {
    /// A shadow without spread.
    pub const fn new(color: Color, x: f32, y: f32, blur: f32) -> Self {
        Shadow {
            color,
            x,
            y,
            blur,
            spread: 0.0,
        }
    }
}

/// The parameters of a glass material (kit blur type `Glass`, tokens' materials).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Glass {
    /// Backdrop blur σ in points.
    pub radius: f32,
    /// Saturation of the backdrop.
    pub saturation: f32,
    /// Brightness of the backdrop.
    pub brightness: f32,
    /// Refraction strength at the edge.
    pub distortion: f32,
    /// Width of the refracting band, as a fraction of half the shorter side.
    pub depth: f32,
    /// Chromatic split.
    pub chromatic_aberration: f32,
    /// Specular highlight on the rim.
    pub specular: bool,
    /// Magnification of a lens (1: none).
    pub magnification: f32,
}

impl Default for Glass {
    fn default() -> Self {
        Glass {
            radius: 0.0,
            saturation: 1.0,
            brightness: 1.0,
            distortion: 0.0,
            depth: 0.0,
            chromatic_aberration: 0.0,
            specular: false,
            magnification: 1.0,
        }
    }
}

/// A blur of what is behind the layer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Blur {
    /// A plain background blur (CSS `blur()`, σ = radius) with saturation and brightness.
    Background {
        /// σ in points.
        radius: f32,
        /// Saturation.
        saturation: f32,
        /// Brightness.
        brightness: f32,
    },
    /// Glass: blur, refraction at the edge, saturation, brightness, specular rim.
    Glass(Glass),
}

/// Everything a layer paints around and inside its shape.
#[derive(Debug, Clone, PartialEq)]
pub struct Style {
    /// Opacity of the whole layer, 0–1.
    pub opacity: f32,
    /// Fills, bottom first.
    pub fills: Vec<Paint>,
    /// Borders, bottom first.
    pub borders: Vec<Border>,
    /// Drop shadows (outside the shape).
    pub shadows: Vec<Shadow>,
    /// Inner shadows (inside the shape).
    pub inner_shadows: Vec<Shadow>,
    /// Backdrop blurs.
    pub blurs: Vec<Blur>,
}

impl Default for Style {
    fn default() -> Self {
        Style {
            opacity: 1.0,
            fills: Vec::new(),
            borders: Vec::new(),
            shadows: Vec::new(),
            inner_shadows: Vec::new(),
            blurs: Vec::new(),
        }
    }
}

impl Style {
    /// A style with one solid fill.
    pub fn fill(color: Color) -> Self {
        Style {
            fills: vec![Paint::Solid(color)],
            ..Style::default()
        }
    }

    /// Adds a fill.
    pub fn with_fill(mut self, paint: impl Into<Paint>) -> Self {
        self.fills.push(paint.into());
        self
    }

    /// Adds a border.
    pub fn with_border(mut self, border: Border) -> Self {
        self.borders.push(border);
        self
    }

    /// Adds a drop shadow.
    pub fn with_shadow(mut self, shadow: Shadow) -> Self {
        self.shadows.push(shadow);
        self
    }

    /// Adds an inner shadow.
    pub fn with_inner_shadow(mut self, shadow: Shadow) -> Self {
        self.inner_shadows.push(shadow);
        self
    }

    /// Adds a blur.
    pub fn with_blur(mut self, blur: Blur) -> Self {
        self.blurs.push(blur);
        self
    }

    /// Sets the opacity.
    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity;
        self
    }

    /// Whether painting this style draws nothing.
    pub fn is_invisible(&self) -> bool {
        self.opacity <= 0.0
            || (self.fills.iter().all(Paint::is_invisible)
                && self.borders.iter().all(|b| b.paint.is_invisible())
                && self.shadows.iter().all(|s| s.color.a == 0)
                && self.inner_shadows.iter().all(|s| s.color.a == 0)
                && self.blurs.is_empty())
    }

    /// How far beyond the frame the style can paint (shadows, outside borders).
    pub fn overflow(&self) -> f32 {
        let mut reach: f32 = 0.0;
        for s in &self.shadows {
            reach = reach.max(s.blur * 1.5 + s.spread.abs() + s.x.abs().max(s.y.abs()) + 1.0);
        }
        for b in &self.borders {
            reach = reach.max(match b.position {
                BorderPosition::Inside => 0.0,
                BorderPosition::Center => b.thickness / 2.0,
                BorderPosition::Outside => b.thickness,
            });
        }
        reach
    }
}

/// Options of a painting pass.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PaintOptions {
    /// Draw backdrop blurs (off: Reduce Transparency, or a backend without them, which then
    /// fills with `blur_fallback`).
    pub backdrop: bool,
    /// The opaque fill that replaces glass when backdrops are off (tint over Surface/Window).
    pub blur_fallback: Option<Color>,
}

impl Default for PaintOptions {
    fn default() -> Self {
        PaintOptions {
            backdrop: true,
            blur_fallback: None,
        }
    }
}

/// Paints `style` for `shape` in `rect`.
pub fn paint_style(canvas: &Canvas, rect: Rect, shape: &Shape, style: &Style, opts: PaintOptions) {
    if rect.is_empty() || style.opacity <= 0.0 {
        return;
    }
    let outline = shape.path(rect);
    let opacity = if style.opacity.is_finite() {
        style.opacity.clamp(0.0, 1.0)
    } else {
        1.0
    };
    paint_shadows(canvas, rect, shape, &outline, &style.shadows, opacity);
    for blur in &style.blurs {
        paint_backdrop(canvas, rect, &outline, blur, opts);
    }
    for fill in &style.fills {
        let mut p = SkPaint::default();
        p.set_anti_alias(true);
        fill.apply(&mut p, rect, opacity);
        canvas.draw_path(&outline, &p);
    }
    paint_inner_shadows(canvas, rect, shape, &outline, &style.inner_shadows, opacity);
    for border in &style.borders {
        paint_border(canvas, rect, shape, &outline, border, opacity);
    }
}

/// The blur σ of a kit (CSS) shadow blur.
pub fn shadow_sigma(blur: f32) -> f32 {
    blur / 2.0
}

fn blur_filter(blur: f32) -> Option<MaskFilter> {
    if blur > 0.0 && blur.is_finite() {
        MaskFilter::blur(BlurStyle::Normal, shadow_sigma(blur), false)
    } else {
        None
    }
}

/// Drop shadows, outside the shape only.
pub fn paint_shadows(
    canvas: &Canvas,
    rect: Rect,
    shape: &Shape,
    outline: &Path,
    shadows: &[Shadow],
    opacity: f32,
) {
    if shadows.is_empty() {
        return;
    }
    canvas.save();
    canvas.clip_path(outline, ClipOp::Difference, true);
    for s in shadows {
        if s.color.a == 0 {
            continue;
        }
        let mut p = SkPaint::default();
        p.set_anti_alias(true);
        p.set_color(s.color.with_opacity(opacity).to_sk());
        if let Some(f) = blur_filter(s.blur) {
            p.set_mask_filter(f);
        }
        let r = rect
            .translate(Point::new(s.x, s.y))
            .inflate(s.spread, s.spread);
        canvas.draw_path(&shape.grown(s.spread).path(r), &p);
    }
    canvas.restore();
}

/// Inner shadows: the outside of the shape, offset and blurred, seen through the shape.
pub fn paint_inner_shadows(
    canvas: &Canvas,
    rect: Rect,
    shape: &Shape,
    outline: &Path,
    shadows: &[Shadow],
    opacity: f32,
) {
    if shadows.is_empty() {
        return;
    }
    canvas.save();
    canvas.clip_path(outline, ClipOp::Intersect, true);
    for s in shadows {
        if s.color.a == 0 {
            continue;
        }
        let reach = s.blur * 2.0 + s.spread.abs() + s.x.abs() + s.y.abs() + 2.0;
        let hole_rect = rect
            .translate(Point::new(s.x, s.y))
            .inflate(-s.spread, -s.spread);
        let hole = shape.grown(-s.spread).path(hole_rect);
        let mut outer = skia_safe::PathBuilder::new();
        outer.add_rect(rect.inflate(reach, reach).to_sk(), None, None);
        let outer = outer.detach();
        let Some(ring) = outer.op(&hole, PathOp::Difference) else {
            continue;
        };
        let mut p = SkPaint::default();
        p.set_anti_alias(true);
        p.set_color(s.color.with_opacity(opacity).to_sk());
        if let Some(f) = blur_filter(s.blur) {
            p.set_mask_filter(f);
        }
        canvas.draw_path(&ring, &p);
    }
    canvas.restore();
}

fn paint_border(
    canvas: &Canvas,
    rect: Rect,
    shape: &Shape,
    outline: &Path,
    border: &Border,
    opacity: f32,
) {
    let t = border.thickness;
    if t <= 0.0 || !t.is_finite() || border.paint.is_invisible() {
        return;
    }
    let mut p = SkPaint::default();
    p.set_anti_alias(true);
    p.set_style(PaintStyle::Stroke);
    border.paint.apply(&mut p, rect, opacity);
    if border.round_ends {
        p.set_stroke_cap(skia_safe::paint::Cap::Round);
    }
    if border.dash.len() >= 2
        && let Some(e) = PathEffect::dash(&border.dash, 0.0)
    {
        p.set_path_effect(e);
    }
    match border.position {
        BorderPosition::Inside if border.dash.is_empty() => {
            // A stroke of twice the width, clipped to the inside: exact on every outline.
            p.set_stroke_width(t * 2.0);
            canvas.save();
            canvas.clip_path(outline, ClipOp::Intersect, true);
            canvas.draw_path(outline, &p);
            canvas.restore();
        }
        BorderPosition::Outside if border.dash.is_empty() => {
            p.set_stroke_width(t * 2.0);
            canvas.save();
            canvas.clip_path(outline, ClipOp::Difference, true);
            canvas.draw_path(outline, &p);
            canvas.restore();
        }
        position => {
            // Centred, or dashed: stroke a path offset by half the width so that dashes keep
            // their round or square ends.
            let offset = match position {
                BorderPosition::Inside => -t / 2.0,
                BorderPosition::Center => 0.0,
                BorderPosition::Outside => t / 2.0,
            };
            p.set_stroke_width(t);
            let path = shape.grown(offset).path(rect.inflate(offset, offset));
            canvas.draw_path(&path, &p);
        }
    }
}

/// The colour matrix of a backdrop's saturation and brightness.
pub fn saturation_brightness_matrix(saturation: f32, brightness: f32) -> ColorMatrix {
    // Rec. 709 luma weights, as Ossido's glass pass.
    let s = saturation;
    let (lr, lg, lb) = (0.2126 * (1.0 - s), 0.7152 * (1.0 - s), 0.0722 * (1.0 - s));
    let k = brightness;
    ColorMatrix::new(
        (lr + s) * k,
        lg * k,
        lb * k,
        0.0,
        0.0,
        lr * k,
        (lg + s) * k,
        lb * k,
        0.0,
        0.0,
        lr * k,
        lg * k,
        (lb + s) * k,
        0.0,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
        0.0,
    )
}

fn paint_backdrop(canvas: &Canvas, rect: Rect, outline: &Path, blur: &Blur, opts: PaintOptions) {
    if !opts.backdrop {
        if let Some(c) = opts.blur_fallback {
            let mut p = SkPaint::default();
            p.set_anti_alias(true);
            p.set_color(c.to_sk());
            canvas.draw_path(outline, &p);
        }
        return;
    }
    let (sigma, saturation, brightness, glass) = match *blur {
        Blur::Background {
            radius,
            saturation,
            brightness,
        } => (radius, saturation, brightness, None),
        Blur::Glass(g) => (g.radius, g.saturation, g.brightness, Some(g)),
    };
    let cf = color_filters::matrix(&saturation_brightness_matrix(saturation, brightness), None);
    let blur_f = if sigma > 0.0 {
        image_filters::blur((sigma, sigma), TileMode::Clamp, None, None)
    } else {
        None
    };
    let filter = match (blur_f, glass) {
        (Some(b), Some(g)) => crate::render::glass::refraction_filter(rect, &g, Some(b))
            .and_then(|f| image_filters::color_filter(cf.clone(), Some(f), None)),
        (None, Some(g)) => crate::render::glass::refraction_filter(rect, &g, None)
            .and_then(|f| image_filters::color_filter(cf.clone(), Some(f), None)),
        (Some(b), None) => image_filters::color_filter(cf.clone(), Some(b), None),
        (None, None) => image_filters::color_filter(cf.clone(), None, None),
    };
    let Some(filter) = filter else {
        return;
    };
    canvas.save();
    canvas.clip_path(outline, ClipOp::Intersect, true);
    let bounds = rect.to_sk();
    let rec = SaveLayerRec::default().bounds(&bounds).backdrop(&filter);
    canvas.save_layer(&rec);
    canvas.restore();
    if let Some(g) = glass
        && g.specular
    {
        crate::render::glass::paint_specular(canvas, rect, outline);
    }
    canvas.restore();
}

#[cfg(test)]
mod tests {
    use super::*;
    use skia_safe::{AlphaType, ColorType, ImageInfo, surfaces};

    fn pixel(surface: &mut skia_safe::Surface, x: i32, y: i32) -> [u8; 4] {
        let info = ImageInfo::new((1, 1), ColorType::RGBA8888, AlphaType::Unpremul, None);
        let mut px = [0u8; 4];
        assert!(surface.read_pixels(&info, &mut px, 4, (x, y)));
        px
    }

    #[test]
    fn fill_border_shadow() {
        let mut s = surfaces::raster_n32_premul((60, 60)).unwrap();
        let style = Style::fill(Color::rgb(255, 0, 0))
            .with_border(Border::inside(Color::rgb(0, 0, 255), 2.0))
            .with_shadow(Shadow::new(Color::rgba(0, 0, 0, 128), 0.0, 4.0, 4.0));
        paint_style(
            s.canvas(),
            Rect::new(10.0, 10.0, 40.0, 30.0),
            &Shape::rounded(6.0),
            &style,
            PaintOptions::default(),
        );
        assert_eq!(pixel(&mut s, 30, 25), [255, 0, 0, 255]);
        assert_eq!(pixel(&mut s, 30, 10), [0, 0, 255, 255]);
        // Shadow below the shape, nothing above it.
        assert!(pixel(&mut s, 30, 42)[3] > 0);
        assert_eq!(pixel(&mut s, 30, 5)[3], 0);
    }

    #[test]
    fn inner_shadow_stays_inside() {
        let mut s = surfaces::raster_n32_premul((40, 40)).unwrap();
        let style = Style::default().with_inner_shadow(Shadow::new(Color::WHITE, 0.0, 1.0, 1.0));
        paint_style(
            s.canvas(),
            Rect::new(10.0, 10.0, 20.0, 20.0),
            &Shape::rounded(4.0),
            &style,
            PaintOptions::default(),
        );
        assert!(pixel(&mut s, 20, 10)[3] > 0);
        assert_eq!(pixel(&mut s, 20, 8)[3], 0);
        assert_eq!(pixel(&mut s, 20, 20)[3], 0);
    }

    #[test]
    fn linear_gradient() {
        let mut s = surfaces::raster_n32_premul((10, 10)).unwrap();
        let style = Style::default().with_fill(Paint::Linear {
            from: Point::new(0.5, 0.0),
            to: Point::new(0.5, 1.0),
            stops: vec![(0.0, Color::BLACK), (1.0, Color::WHITE)],
        });
        paint_style(
            s.canvas(),
            Rect::new(0.0, 0.0, 10.0, 10.0),
            &Shape::RECT,
            &style,
            PaintOptions::default(),
        );
        assert!(pixel(&mut s, 5, 0)[0] < 30);
        assert!(pixel(&mut s, 5, 9)[0] > 220);
    }
}
