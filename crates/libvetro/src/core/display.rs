//! Display lists: what a view paints, as named kit-like layers.
//!
//! A view records its drawing as a list of items (shapes with a kit style, text, symbol
//! glyphs, images, clips and transforms). The list is replayed on a Skia canvas to render,
//! and read back as named layers by the kit-parity tests and the inspector: a component's
//! "Surface", "Label" and "Focus Halo" are the same names as the kit master's layers.

use std::rc::Rc;

use skia_safe::{Canvas, ClipOp, Font, Paint as SkPaint, SamplingOptions};

use crate::color::Color;
use crate::geom::{Point, Rect, Transform};
use crate::shape::Shape;
use crate::style::{PaintOptions, Style, paint_style};
use crate::text::TextLayout;
use crate::text::fonts::{Fonts, SYMBOLS_FAMILY};

/// A custom drawing callback (terminal grids, canvases): draws in the view's coordinates.
pub type CustomDraw = Rc<dyn Fn(&Canvas, Rect)>;

/// One item of a display list. Rectangles are in the view's own coordinates.
#[derive(Clone)]
pub enum DisplayItem {
    /// A shape painted with a kit style.
    Layer {
        /// The kit layer's name (`Surface`, `Box` …).
        name: Rc<str>,
        /// Frame.
        rect: Rect,
        /// Outline.
        shape: Shape,
        /// Style.
        style: Style,
    },
    /// A glass surface the compositor draws when the window's backdrop is outside it
    /// (menus, popovers, the Dock: `silica-glass-v1` regions); painted by the client like a
    /// layer otherwise (headless, nested glass, no compositor support).
    Glass {
        /// The kit layer's name.
        name: Rc<str>,
        /// Frame.
        rect: Rect,
        /// Outline.
        shape: Shape,
        /// The material as a style (blur, tint, rims, shadow).
        style: Style,
        /// The material's name (`Glass/Regular`, `Glass/Thick`, `Glass/Clear`).
        material: &'static str,
        /// The compositor profile (silica-glass-v1 `profile`).
        profile: GlassProfile,
    },
    /// Laid-out text.
    Text {
        /// The kit layer's name (`Label` …).
        name: Rc<str>,
        /// The text box (the layout's top left is at the box's origin).
        rect: Rect,
        /// The layout.
        layout: Rc<TextLayout>,
        /// The type token's name, for parity checks (`Body` …).
        token: Option<&'static str>,
        /// The text colour (also baked in the layout).
        color: Color,
    },
    /// A Silica Symbols glyph in an icon box (font size = box size).
    Glyph {
        /// The kit layer's name (`Icon` …).
        name: Rc<str>,
        /// The icon box.
        rect: Rect,
        /// The symbol's name.
        symbol: Rc<str>,
        /// Codepoint.
        ch: char,
        /// Font size (= the icon box).
        size: f32,
        /// Weight (300, 400, 600).
        weight: u16,
        /// Colour.
        color: Color,
    },
    /// An image scaled into a rectangle.
    Image {
        /// Layer name.
        name: Rc<str>,
        /// Destination.
        rect: Rect,
        /// The image.
        image: skia_safe::Image,
        /// Opacity.
        opacity: f32,
    },
    /// Starts clipping to a shape.
    PushClip {
        /// The clip's frame.
        rect: Rect,
        /// The clip's outline.
        shape: Shape,
    },
    /// Starts a transform.
    PushTransform(Transform),
    /// Starts a group with an opacity.
    PushOpacity(f32),
    /// Ends the last push.
    Pop,
    /// Custom drawing.
    Custom {
        /// Layer name.
        name: Rc<str>,
        /// Bounds.
        rect: Rect,
        /// The drawing.
        draw: CustomDraw,
    },
}

impl std::fmt::Debug for DisplayItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DisplayItem::Layer { name, rect, .. } => write!(f, "Layer({name}, {rect:?})"),
            DisplayItem::Glass {
                name,
                rect,
                material,
                ..
            } => {
                write!(f, "Glass({name}, {rect:?}, {material})")
            }
            DisplayItem::Text {
                name, rect, layout, ..
            } => {
                write!(f, "Text({name}, {rect:?}, {:?})", layout.text())
            }
            DisplayItem::Glyph {
                name, rect, symbol, ..
            } => write!(f, "Glyph({name}, {rect:?}, {symbol})"),
            DisplayItem::Image { name, rect, .. } => write!(f, "Image({name}, {rect:?})"),
            DisplayItem::PushClip { rect, .. } => write!(f, "PushClip({rect:?})"),
            DisplayItem::PushTransform(t) => write!(f, "PushTransform({t:?})"),
            DisplayItem::PushOpacity(o) => write!(f, "PushOpacity({o})"),
            DisplayItem::Pop => write!(f, "Pop"),
            DisplayItem::Custom { name, rect, .. } => write!(f, "Custom({name}, {rect:?})"),
        }
    }
}

/// The kind of surface a compositor glass region belongs to (silica-glass-v1 `profile`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum GlassProfile {
    /// From the material.
    Default = 0,
    /// Menus and context menus.
    Menu = 1,
    /// Large panels: Notification Centre, search, Launcher.
    Panel = 2,
    /// Control Center modules and HUDs.
    Module = 3,
    /// Small controls on glass.
    Control = 4,
    /// The Dock and the app switcher.
    Dock = 5,
    /// Tooltips and labels.
    Tooltip = 6,
    /// Sheets and alerts.
    Sheet = 7,
    /// Notification banners.
    Banner = 8,
    /// Popovers.
    Popover = 9,
}

/// A recorded list of items.
#[derive(Debug, Clone, Default)]
pub struct DisplayList {
    /// The items in painting order.
    pub items: Vec<DisplayItem>,
}

impl DisplayList {
    /// Whether nothing was recorded.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The bounds of everything painted, including shadows.
    pub fn bounds(&self) -> Rect {
        let mut r = Rect::ZERO;
        for item in &self.items {
            let b = match item {
                DisplayItem::Layer { rect, style, .. } | DisplayItem::Glass { rect, style, .. } => {
                    let o = style.overflow();
                    rect.inflate(o, o)
                }
                DisplayItem::Text { rect, layout, .. } => {
                    let s = layout.size();
                    rect.union(Rect::new(rect.x, rect.y, s.width, s.height))
                }
                DisplayItem::Glyph { rect, .. }
                | DisplayItem::Image { rect, .. }
                | DisplayItem::Custom { rect, .. } => *rect,
                _ => Rect::ZERO,
            };
            r = r.union(b);
        }
        r
    }

    /// Replays the list on `canvas` (in the view's coordinates).
    pub fn paint(&self, canvas: &Canvas, opts: PaintOptions) {
        let mut depth = 0usize;
        for item in &self.items {
            match item {
                DisplayItem::Layer {
                    rect, shape, style, ..
                } => paint_style(canvas, *rect, shape, style, opts),
                DisplayItem::Glass {
                    rect, shape, style, ..
                } => {
                    if !opts.compositor_glass {
                        paint_style(canvas, *rect, shape, style, opts);
                    }
                }
                DisplayItem::Text { rect, layout, .. } => layout.paint(canvas, rect.origin()),
                DisplayItem::Glyph {
                    rect,
                    ch,
                    size,
                    weight,
                    color,
                    ..
                } => paint_glyph(canvas, *rect, *ch, *size, *weight, *color),
                DisplayItem::Image {
                    rect,
                    image,
                    opacity,
                    ..
                } => {
                    let mut p = SkPaint::default();
                    p.set_anti_alias(true);
                    p.set_alpha_f(opacity.clamp(0.0, 1.0));
                    canvas.draw_image_rect_with_sampling_options(
                        image,
                        None,
                        rect.to_sk(),
                        SamplingOptions::new(
                            skia_safe::FilterMode::Linear,
                            skia_safe::MipmapMode::Linear,
                        ),
                        &p,
                    );
                }
                DisplayItem::PushClip { rect, shape } => {
                    canvas.save();
                    match shape {
                        Shape::Rect { corners, .. } if corners.is_zero() => {
                            canvas.clip_rect(rect.to_sk(), ClipOp::Intersect, true);
                        }
                        _ => {
                            canvas.clip_path(&shape.path(*rect), ClipOp::Intersect, true);
                        }
                    }
                    depth += 1;
                }
                DisplayItem::PushTransform(t) => {
                    canvas.save();
                    canvas.concat(&t.to_sk());
                    depth += 1;
                }
                DisplayItem::PushOpacity(o) => {
                    let alpha = (o.clamp(0.0, 1.0) * 255.0).round() as u32;
                    canvas.save_layer_alpha(None, alpha);
                    depth += 1;
                }
                DisplayItem::Pop => {
                    if depth > 0 {
                        canvas.restore();
                        depth -= 1;
                    }
                }
                DisplayItem::Custom { rect, draw, .. } => {
                    canvas.save();
                    draw(canvas, *rect);
                    canvas.restore();
                }
            }
        }
        while depth > 0 {
            canvas.restore();
            depth -= 1;
        }
    }
}

/// Paints one symbol glyph: font size = icon box, the glyph centred in the box
/// horizontally, its em box (ascender = em, descender 0) filling it vertically.
pub fn paint_glyph(canvas: &Canvas, rect: Rect, ch: char, size: f32, weight: u16, color: Color) {
    let Some(tf) = Fonts::with(|f| f.bundled(SYMBOLS_FAMILY, weight)) else {
        return;
    };
    let mut font = Font::from_typeface(tf, size);
    font.set_subpixel(true);
    font.set_edging(skia_safe::font::Edging::AntiAlias);
    let mut buf = [0u8; 4];
    let s = ch.encode_utf8(&mut buf);
    let (advance, _) = font.measure_str(&*s, None);
    let mut p = SkPaint::default();
    p.set_anti_alias(true);
    p.set_color(color.to_sk());
    let x = rect.x + (rect.width - advance) / 2.0;
    // Ascender = em: the baseline is the box's bottom when the box is the font size.
    let y = rect.y + (rect.height + size) / 2.0;
    canvas.draw_str(&*s, (x, y), &font, &p);
}

/// Records a display list.
#[derive(Debug, Default)]
pub struct Recorder {
    list: DisplayList,
    depth: usize,
}

impl Recorder {
    /// An empty recorder.
    pub fn new() -> Self {
        Recorder::default()
    }

    /// A shape with a style.
    pub fn layer(&mut self, name: &str, rect: Rect, shape: Shape, style: Style) {
        if style.is_invisible() {
            return;
        }
        self.list.items.push(DisplayItem::Layer {
            name: Rc::from(name),
            rect,
            shape,
            style,
        });
    }

    /// A glass surface (see [`DisplayItem::Glass`]).
    pub fn glass(
        &mut self,
        name: &str,
        rect: Rect,
        shape: Shape,
        style: Style,
        material: &'static str,
        profile: GlassProfile,
    ) {
        self.list.items.push(DisplayItem::Glass {
            name: Rc::from(name),
            rect,
            shape,
            style,
            material,
            profile,
        });
    }

    /// Text in a box.
    pub fn text(
        &mut self,
        name: &str,
        rect: Rect,
        layout: Rc<TextLayout>,
        token: Option<&'static str>,
        color: Color,
    ) {
        self.list.items.push(DisplayItem::Text {
            name: Rc::from(name),
            rect,
            layout,
            token,
            color,
        });
    }

    /// A symbol glyph by name in an icon box of `rect` (font size = the box's height).
    pub fn glyph(&mut self, name: &str, rect: Rect, symbol: &str, weight: u16, color: Color) {
        let Some(ch) = crate::text::fonts::symbols::codepoint(symbol) else {
            tracing::debug!("unknown symbol {symbol}");
            return;
        };
        self.list.items.push(DisplayItem::Glyph {
            name: Rc::from(name),
            rect,
            symbol: Rc::from(symbol),
            ch,
            size: rect.height,
            weight,
            color,
        });
    }

    /// An image.
    pub fn image(&mut self, name: &str, rect: Rect, image: skia_safe::Image, opacity: f32) {
        self.list.items.push(DisplayItem::Image {
            name: Rc::from(name),
            rect,
            image,
            opacity,
        });
    }

    /// Custom drawing.
    pub fn custom(&mut self, name: &str, rect: Rect, draw: CustomDraw) {
        self.list.items.push(DisplayItem::Custom {
            name: Rc::from(name),
            rect,
            draw,
        });
    }

    /// Clips what follows until [`Recorder::pop`].
    pub fn push_clip(&mut self, rect: Rect, shape: Shape) {
        self.list.items.push(DisplayItem::PushClip { rect, shape });
        self.depth += 1;
    }

    /// Transforms what follows until [`Recorder::pop`].
    pub fn push_transform(&mut self, t: Transform) {
        self.list.items.push(DisplayItem::PushTransform(t));
        self.depth += 1;
    }

    /// Fades what follows until [`Recorder::pop`].
    pub fn push_opacity(&mut self, opacity: f32) {
        self.list.items.push(DisplayItem::PushOpacity(opacity));
        self.depth += 1;
    }

    /// Ends the last push (ignored without one).
    pub fn pop(&mut self) {
        if self.depth > 0 {
            self.list.items.push(DisplayItem::Pop);
            self.depth -= 1;
        }
    }

    /// The list, with unbalanced pushes closed.
    pub fn finish(mut self) -> DisplayList {
        while self.depth > 0 {
            self.pop();
        }
        self.list
    }

    /// Items recorded so far.
    pub fn len(&self) -> usize {
        self.list.items.len()
    }

    /// Whether nothing was recorded.
    pub fn is_empty(&self) -> bool {
        self.list.items.is_empty()
    }
}

/// A painted layer as an inspector or a kit-parity test sees it.
#[derive(Debug, Clone)]
pub struct InspectedLayer {
    /// Path of names from the inspected root (`Button/Surface`).
    pub path: String,
    /// The item's own name.
    pub name: String,
    /// Frame relative to the inspected root.
    pub frame: Rect,
    /// What it is.
    pub content: InspectedContent,
}

/// The content of an inspected layer.
#[derive(Debug, Clone)]
pub enum InspectedContent {
    /// A styled shape.
    Shape {
        /// Outline.
        shape: Shape,
        /// Style.
        style: Style,
        /// The radius of each corner as drawn.
        radii: [f32; 4],
    },
    /// Text.
    Text {
        /// The string.
        text: String,
        /// Type token.
        token: Option<&'static str>,
        /// Colour.
        color: Color,
        /// Measured size of the text.
        size: crate::geom::Size,
    },
    /// A symbol.
    Glyph {
        /// Symbol name.
        symbol: String,
        /// Font size.
        size: f32,
        /// Colour.
        color: Color,
    },
    /// An image or custom drawing.
    Other,
}

impl DisplayList {
    /// The named layers, with frames offset by `origin` and paths under `prefix`.
    pub fn inspect(&self, origin: Point, prefix: &str, out: &mut Vec<InspectedLayer>) {
        let join = |name: &str| {
            if prefix.is_empty() {
                name.to_string()
            } else {
                format!("{prefix}/{name}")
            }
        };
        for item in &self.items {
            match item {
                DisplayItem::Layer {
                    name,
                    rect,
                    shape,
                    style,
                }
                | DisplayItem::Glass {
                    name,
                    rect,
                    shape,
                    style,
                    ..
                } => out.push(InspectedLayer {
                    path: join(name),
                    name: name.to_string(),
                    frame: rect.translate(origin),
                    content: InspectedContent::Shape {
                        shape: shape.clone(),
                        style: style.clone(),
                        radii: shape.corner_radii(*rect),
                    },
                }),
                DisplayItem::Text {
                    name,
                    rect,
                    layout,
                    token,
                    color,
                } => out.push(InspectedLayer {
                    path: join(name),
                    name: name.to_string(),
                    frame: rect.translate(origin),
                    content: InspectedContent::Text {
                        text: layout.text().to_string(),
                        token: *token,
                        color: *color,
                        size: layout.size(),
                    },
                }),
                DisplayItem::Glyph {
                    name,
                    rect,
                    symbol,
                    size,
                    color,
                    ..
                } => out.push(InspectedLayer {
                    path: join(name),
                    name: name.to_string(),
                    frame: rect.translate(origin),
                    content: InspectedContent::Glyph {
                        symbol: symbol.to_string(),
                        size: *size,
                        color: *color,
                    },
                }),
                DisplayItem::Image { name, rect, .. } | DisplayItem::Custom { name, rect, .. } => {
                    out.push(InspectedLayer {
                        path: join(name),
                        name: name.to_string(),
                        frame: rect.translate(origin),
                        content: InspectedContent::Other,
                    })
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_inspect_and_paint() {
        let mut r = Recorder::new();
        r.layer(
            "Surface",
            Rect::new(0.0, 0.0, 40.0, 20.0),
            Shape::rounded(6.0),
            Style::fill(Color::rgb(10, 20, 30)),
        );
        r.layer(
            "Nothing",
            Rect::new(0.0, 0.0, 4.0, 4.0),
            Shape::RECT,
            Style::default(),
        );
        r.push_opacity(0.5);
        r.glyph(
            "Icon",
            Rect::new(2.0, 2.0, 16.0, 16.0),
            "chevron-down",
            400,
            Color::BLACK,
        );
        let list = r.finish();
        assert_eq!(list.items.len(), 4);
        let mut layers = Vec::new();
        list.inspect(Point::new(5.0, 5.0), "Button", &mut layers);
        assert_eq!(layers.len(), 2);
        assert_eq!(layers[0].path, "Button/Surface");
        assert_eq!(layers[0].frame, Rect::new(5.0, 5.0, 40.0, 20.0));
        let mut s = skia_safe::surfaces::raster_n32_premul((50, 30)).unwrap();
        list.paint(s.canvas(), PaintOptions::default());
    }
}
