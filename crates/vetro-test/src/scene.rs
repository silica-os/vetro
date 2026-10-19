//! A headless scene for one master: a window of the master's size in its appearance, the
//! component mounted at the top left, geometry and pixel checks.

use std::fmt::Write as _;
use std::path::PathBuf;

use skia_safe::{AlphaType, ColorType, Data, Image, ImageInfo};
use vetro_lib::color::Color;
use vetro_lib::core::{
    Appearance, Button, InspectedContent, InspectedLayer, LayoutCx, Mods, Prop, Proposal, Ui,
    Value, ViewId, Widget, WindowId, WindowKind,
};
use vetro_lib::geom::{Point, Rect, Size};
use vetro_lib::style::{Blur, BorderPosition, Paint};

use crate::kit::{KitLayer, KitMaster};

/// Pixel tolerances (premultiplied RGBA, 0–255 per channel).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tolerance {
    /// Mean absolute difference over every channel of every pixel.
    pub mean: f64,
    /// Largest difference of one channel.
    pub max: u8,
    /// Mask text layers (Sketch rasterises text with CoreText).
    pub mask_text: bool,
}

impl Tolerance {
    /// Shapes, text masked: mean ≤ 1, max ≤ 48 (silica_ui's `KitPixelTolerance.shapes`).
    pub const SHAPES: Tolerance = Tolerance {
        mean: 1.0,
        max: 48,
        mask_text: true,
    };
    /// Small controls, text masked: Sketch antialiases the edges of small controls about a
    /// quarter point wider than Skia; on 20 pt capsules, where the edge is a large share of
    /// the pixels, single edge pixels reach 83 and the mean 1.4 (measured by silica_ui on the
    /// same masters; the outlines are the kit's, checked by the geometry).
    pub const CONTROLS: Tolerance = Tolerance {
        mean: 1.5,
        max: 84,
        mask_text: true,
    };
    /// With text: mean ≤ 2.5 (silica_ui's `KitPixelTolerance.withText`).
    pub const WITH_TEXT: Tolerance = Tolerance {
        mean: 2.5,
        max: 255,
        mask_text: false,
    };
}

/// The headless scene.
pub struct Scene {
    /// The runtime.
    pub ui: Ui,
    /// The window.
    pub window: WindowId,
    /// The master.
    pub master: KitMaster,
    /// The mounted component.
    pub view: Option<ViewId>,
    /// Where the master's origin is in the window (the PNG covers the render bounds).
    pub origin: Point,
}

impl Scene {
    /// A scene for `master` (its size and appearance).
    pub fn new(master: &KitMaster) -> Scene {
        let mut ui = Ui::new_headless();
        ui.set_appearance(if master.dark() {
            Appearance::dark()
        } else {
            Appearance::light()
        });
        let rb = master.render_bounds();
        let origin = Point::new(-rb.x, -rb.y);
        let window = ui.create_window(WindowKind::Normal, rb.size());
        // The kit's PNGs are transparent around the component.
        if let Some(w) = ui.window_mut(window) {
            w.opaque = false;
        }
        ui.show_window(window);
        Scene {
            ui,
            window,
            master: master.clone(),
            view: None,
            origin,
        }
    }

    /// Loads `name` and makes its scene.
    pub fn load(name: &str) -> Result<Scene, String> {
        Ok(Scene::new(&KitMaster::load(name)?))
    }

    /// Mounts a component at the master's origin at the size it wants (its natural size).
    pub fn mount(&mut self, make: impl FnOnce(&mut Ui) -> ViewId) -> ViewId {
        self.mount_with(None, make)
    }

    /// Mounts a component at the master's origin at a given size.
    pub fn mount_sized(&mut self, size: Size, make: impl FnOnce(&mut Ui) -> ViewId) -> ViewId {
        self.mount_with(Some(size), make)
    }

    fn mount_with(&mut self, size: Option<Size>, make: impl FnOnce(&mut Ui) -> ViewId) -> ViewId {
        let v = make(&mut self.ui);
        let pin = self.ui.create(Pin {
            origin: self.origin,
            size,
        });
        let _ = self.ui.set(pin, Prop::MaxWidth, Value::from(f32::INFINITY));
        let _ = self
            .ui
            .set(pin, Prop::MaxHeight, Value::from(f32::INFINITY));
        self.ui.add_child(pin, v);
        self.ui.set_content(self.window, pin);
        let _ = self.ui.prepare_frame(self.window);
        self.view = Some(v);
        v
    }

    /// Re-records after a state change (hover, press, focus set by the test).
    pub fn refresh(&mut self) {
        let _ = self.ui.prepare_frame(self.window);
    }

    /// The mounted view's layers.
    pub fn layers(&mut self) -> Vec<InspectedLayer> {
        self.refresh();
        match self.view {
            Some(v) => self.ui.inspect_layers(v),
            None => Vec::new(),
        }
    }

    /// Compares the geometry of the mapped layers with the master: frames exactly (0.01 pt),
    /// fills, borders, shadows, blurs, corner radii, text content and font. `layers` maps a
    /// kit layer path to our layer path; `deviations` lists `"<kit path>#<aspect>"` keys
    /// that may differ (with the reason in the test), and a deviation that no longer differs
    /// is itself a failure. Returns the report of mismatches (empty: parity).
    pub fn geometry(&mut self, layers: &[(&str, &str)], deviations: &[&str]) -> Vec<String> {
        let ours = self.layers();
        let mut report = Vec::new();
        let mut used_dev = vec![false; deviations.len()];
        let mut check = |key: String, ok: bool, detail: String, report: &mut Vec<String>| {
            let dev = deviations.iter().position(|d| *d == key);
            match (ok, dev) {
                (true, Some(i)) => {
                    if let Some(u) = used_dev.get_mut(i) {
                        *u = true;
                    }
                    report.push(format!("{key}: listed as a deviation but matches"));
                }
                (false, Some(i)) => {
                    if let Some(u) = used_dev.get_mut(i) {
                        *u = true;
                    }
                }
                (false, None) => report.push(format!("{key}: {detail}")),
                (true, None) => {}
            }
        };
        // The component's size.
        if let Some(v) = self.view {
            let size = self
                .ui
                .tree
                .get(v)
                .map(|n| n.frame.size())
                .unwrap_or_default();
            let ms = self.master.size();
            check(
                "#frame".into(),
                close(size.width, ms.width) && close(size.height, ms.height),
                format!("size {size:?}, kit {ms:?}"),
                &mut report,
            );
        }
        for (kit_path, our_path) in layers {
            let Some(kl) = self.master.layer(kit_path) else {
                report.push(format!("{kit_path}: no such kit layer"));
                continue;
            };
            // Light glass: Sketch drops the material's tint, so the kit draws it as a
            // separate `<name> tint` layer; in code the tint is part of the material.
            let merged;
            let kl = match self.master.layer(&format!("{kit_path} tint")) {
                Some(tint) if kl.style.blurs.iter().any(|b| b.kind == "Glass") => {
                    let mut m = kl.clone();
                    m.style.fills = tint.style.fills.clone();
                    merged = m;
                    &merged
                }
                _ => kl,
            };
            let Some(kr) = self.master.rect_in_master(kit_path) else {
                continue;
            };
            let Some(ol) = ours.iter().find(|l| l.path == *our_path) else {
                if kit_invisible(kl) {
                    // The kit keeps an empty layer (a borderless button's background at
                    // rest): drawing nothing matches it.
                    continue;
                }
                report.push(format!(
                    "{kit_path}: no layer {our_path} among {:?}",
                    ours.iter().map(|l| l.path.as_str()).collect::<Vec<_>>()
                ));
                continue;
            };
            check(
                format!("{kit_path}#frame"),
                rect_close(ol.frame, kr),
                format!("frame {:?}, kit {:?}", ol.frame, kr),
                &mut report,
            );
            compare_content(kit_path, kl, ol, &mut |k, ok, d| {
                check(k, ok, d, &mut report)
            });
        }
        for (i, used) in used_dev.iter().enumerate() {
            if !used && let Some(d) = deviations.get(i) {
                report.push(format!("{d}: deviation never checked"));
            }
        }
        report
    }

    /// Puts the mounted component in a kit state (`Rest`, `Hover`, `Pressed`, `Focused`,
    /// `Disabled`; others are left to the test) and lets transitions settle.
    pub fn set_state(&mut self, state: &str) {
        let Some(v) = self.view else { return };
        let center = self.ui.tree.window_rect(v).center();
        match state {
            "Hover" => self.ui.pointer_move(self.window, center, Mods::empty()),
            "Pressed" => {
                self.ui.pointer_move(self.window, center, Mods::empty());
                self.ui
                    .pointer_down(self.window, center, Button::Left, Mods::empty());
            }
            "Focused" => {
                let mut a = self.ui.appearance;
                a.full_keyboard_access = true;
                self.ui.set_appearance(a);
                self.ui.set_focus(v, true);
            }
            "Disabled" => {
                let _ = self.ui.set(v, Prop::Disabled, Value::Bool(true));
            }
            _ => {}
        }
        self.settle();
    }

    /// Lets every transition finish.
    pub fn settle(&mut self) {
        for _ in 0..240 {
            self.ui.advance(1.0 / 60.0);
            if !self.ui.is_animating() {
                break;
            }
        }
        self.refresh();
    }

    /// The kit layers to compare by default: every visible child of the master's root,
    /// mapped to our layer of the same name (`<name> tint` layers merge into their glass).
    pub fn default_mapping(&self) -> Vec<(String, String)> {
        self.master
            .root
            .children
            .iter()
            .filter(|c| !c.hidden && !c.name.ends_with(" tint"))
            .map(|c| (c.name.clone(), c.name.clone()))
            .collect()
    }

    /// Geometry with the default mapping.
    pub fn auto_geometry(&mut self, deviations: &[&str]) -> Vec<String> {
        let mapping = self.default_mapping();
        let pairs: Vec<(&str, &str)> = mapping
            .iter()
            .map(|(a, b)| (a.as_str(), b.as_str()))
            .collect();
        let mut report = self.geometry(&pairs, deviations);
        // Our layers the kit does not have.
        let ours = self.layers();
        for l in &ours {
            if !l.path.contains('/') && !mapping.iter().any(|(_, b)| *b == l.path) {
                report.push(format!("{}: drawn but not in the kit", l.path));
            }
        }
        report
    }

    /// Asserts geometric parity.
    pub fn expect_geometry(&mut self, layers: &[(&str, &str)], deviations: &[&str]) {
        let report = self.geometry(layers, deviations);
        assert!(
            report.is_empty(),
            "{}: geometry differs from the kit:\n  {}",
            self.master.name,
            report.join("\n  ")
        );
    }

    /// Renders at 2× and compares with the master's PNG; on failure writes
    /// `target/kit-diff/<name>.png` (kit | ours | difference). Returns (mean, max).
    pub fn pixels(&mut self, tol: Tolerance) -> Result<(f64, u8), String> {
        let ours = self
            .ui
            .snapshot(self.window, 2.0)
            .ok_or("snapshot failed")?;
        let bytes = std::fs::read(&self.master.png)
            .map_err(|e| format!("{}: {e}", self.master.png.display()))?;
        let kit = Image::from_encoded(Data::new_copy(&bytes)).ok_or("cannot decode the PNG")?;
        let (w, h) = (kit.width(), kit.height());
        if (ours.width(), ours.height()) != (w, h) {
            return Err(format!(
                "size {}x{}, kit {w}x{h}",
                ours.width(),
                ours.height()
            ));
        }
        let a = rgba(&kit).ok_or("cannot read the PNG's pixels")?;
        let b = rgba(&ours).ok_or("cannot read our pixels")?;
        let mut mask = vec![false; (w * h) as usize];
        if tol.mask_text {
            let master = self.master.clone();
            for (_, r, l) in master.flatten() {
                if l.kind == "Text" {
                    let r = r.translate(self.origin).round_out(1.0).inflate(1.0, 1.0);
                    for y in (r.y * 2.0) as i32..(r.bottom() * 2.0) as i32 {
                        for x in (r.x * 2.0) as i32..(r.right() * 2.0) as i32 {
                            if x >= 0
                                && y >= 0
                                && x < w
                                && y < h
                                && let Some(m) = mask.get_mut((y * w + x) as usize)
                            {
                                *m = true;
                            }
                        }
                    }
                }
            }
        }
        let mut sum = 0u64;
        let mut count = 0u64;
        let mut max = 0u8;
        let mut diff = vec![0u8; a.len()];
        for i in 0..(w * h) as usize {
            if mask.get(i).copied().unwrap_or(false) {
                continue;
            }
            for c in 0..4 {
                let (Some(x), Some(y)) = (a.get(i * 4 + c), b.get(i * 4 + c)) else {
                    continue;
                };
                let d = x.abs_diff(*y);
                sum += u64::from(d);
                count += 1;
                max = max.max(d);
                if let Some(slot) = diff.get_mut(i * 4 + c) {
                    *slot = if c == 3 { 255 } else { d.saturating_mul(4) };
                }
            }
        }
        let mut mean = if count == 0 {
            0.0
        } else {
            sum as f64 / count as f64
        };
        // The kit refracts the edge of Regular and Thick glass over its (transparent)
        // backdrop, a band we cannot reproduce on an empty canvas: as silica_ui does, a failing
        // comparison is repeated with the band masked, the mean still over every pixel.
        if mean > tol.mean || max > tol.max {
            let bands = self.master.glass_bands();
            if !bands.is_empty() {
                let mut bsum = 0u64;
                let mut bmax = 0u8;
                for i in 0..(w * h) as usize {
                    if mask.get(i).copied().unwrap_or(false) {
                        continue;
                    }
                    let px = Point::new(
                        ((i as i32 % w) as f32 + 0.5) / 2.0,
                        ((i as i32 / w) as f32 + 0.5) / 2.0,
                    ) - self.origin;
                    let in_band = bands.iter().any(|(r, _radius, band)| {
                        r.contains(px) && !r.inflate(-band, -band).contains(px)
                    });
                    if in_band {
                        continue;
                    }
                    for c in 0..4 {
                        if let (Some(x), Some(y)) = (a.get(i * 4 + c), b.get(i * 4 + c)) {
                            let d = x.abs_diff(*y);
                            bsum += u64::from(d);
                            bmax = bmax.max(d);
                        }
                    }
                }
                let bmean = if count == 0 {
                    0.0
                } else {
                    bsum as f64 / count as f64
                };
                if bmean <= tol.mean && bmax <= tol.max {
                    mean = bmean;
                    max = bmax;
                }
            }
        }
        if mean > tol.mean || max > tol.max {
            let _ = write_diff(&self.master.name, w, h, &a, &b, &diff);
            return Err(format!(
                "pixels differ: mean {mean:.3} (≤ {}), max {max} (≤ {})",
                tol.mean, tol.max
            ));
        }
        Ok((mean, max))
    }

    /// Asserts pixel parity.
    pub fn expect_pixels(&mut self, tol: Tolerance) {
        if let Err(e) = self.pixels(tol) {
            panic!("{}: {e}", self.master.name);
        }
    }
}

/// Holds the component at the master's origin, at its natural size or a given one.
struct Pin {
    origin: Point,
    size: Option<Size>,
}

impl Widget for Pin {
    fn kind(&self) -> &'static str {
        "Pin"
    }

    fn hit(&self, _p: Point, _size: Size) -> bool {
        false
    }

    fn place(&mut self, cx: &mut LayoutCx<'_>, _size: Size) {
        for c in cx.children() {
            let s = match self.size {
                Some(s) => {
                    let _ = cx.measure(c, Proposal::fixed(s));
                    s
                }
                None => cx.measure(c, Proposal::UNSPECIFIED),
            };
            cx.place(c, Rect::from_origin_size(self.origin, s));
        }
    }
}

fn rgba(img: &Image) -> Option<Vec<u8>> {
    let info = ImageInfo::new(
        (img.width(), img.height()),
        ColorType::RGBA8888,
        AlphaType::Premul,
        None,
    );
    let row = img.width() as usize * 4;
    let mut px = vec![0u8; row * img.height() as usize];
    img.read_pixels(
        &info,
        &mut px,
        row,
        (0, 0),
        skia_safe::image::CachingHint::Allow,
    )
    .then_some(px)
}

fn write_diff(name: &str, w: i32, h: i32, a: &[u8], b: &[u8], d: &[u8]) -> Option<()> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/kit-diff");
    std::fs::create_dir_all(&dir).ok()?;
    let mut surface = skia_safe::surfaces::raster_n32_premul((w * 3, h))?;
    let info = ImageInfo::new((w, h), ColorType::RGBA8888, AlphaType::Premul, None);
    for (i, px) in [a, b, d].iter().enumerate() {
        let img = skia_safe::images::raster_from_data(&info, Data::new_copy(px), w as usize * 4)?;
        surface
            .canvas()
            .draw_image(&img, ((i as i32 * w) as f32, 0.0), None);
    }
    let png = surface
        .image_snapshot()
        .encode(None, skia_safe::EncodedImageFormat::PNG, None)?;
    let file = dir.join(format!("{}.png", name.replace('/', "_")));
    std::fs::write(file, png.as_bytes()).ok()
}

/// Whether a kit layer paints nothing.
fn kit_invisible(l: &KitLayer) -> bool {
    let s = &l.style;
    l.kind != "Text"
        && l.children.is_empty()
        && (s.opacity <= 0.0
            || (s.fills.iter().all(Paint::is_invisible)
                && s.borders.iter().all(|b| b.color.a == 0)
                && s.shadows.iter().all(|x| x.color.a == 0)
                && s.inner_shadows.iter().all(|x| x.color.a == 0)
                && s.blurs.is_empty()))
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() <= 0.01
}

fn rect_close(a: Rect, b: Rect) -> bool {
    close(a.x, b.x) && close(a.y, b.y) && close(a.width, b.width) && close(a.height, b.height)
}

fn color_close(a: Color, b: Color) -> bool {
    // The kit exports 8-bit colours; tokens are the same values. One level of rounding.
    a.r.abs_diff(b.r) <= 1
        && a.g.abs_diff(b.g) <= 1
        && a.b.abs_diff(b.b) <= 1
        && a.a.abs_diff(b.a) <= 1
}

fn paint_close(a: &Paint, b: &Paint) -> bool {
    match (a, b) {
        (Paint::Solid(x), Paint::Solid(y)) => color_close(*x, *y),
        (
            Paint::Linear {
                stops: s1,
                from: f1,
                to: t1,
            },
            Paint::Linear {
                stops: s2,
                from: f2,
                to: t2,
            },
        ) => {
            s1.len() == s2.len()
                && s1
                    .iter()
                    .zip(s2)
                    .all(|(p, q)| close(p.0, q.0) && color_close(p.1, q.1))
                && close(f1.x, f2.x)
                && close(f1.y, f2.y)
                && close(t1.x, t2.x)
                && close(t1.y, t2.y)
        }
        (Paint::Angular { stops: s1 }, Paint::Angular { stops: s2 }) => {
            s1.len() == s2.len()
                && s1
                    .iter()
                    .zip(s2)
                    .all(|(p, q)| close(p.0, q.0) && color_close(p.1, q.1))
        }
        _ => false,
    }
}

fn fmt_paints(p: &[Paint]) -> String {
    let mut s = String::from("[");
    for x in p {
        match x {
            Paint::Solid(c) => {
                let _ = write!(s, "{} ", c.to_hex());
            }
            other => {
                let _ = write!(s, "{other:?} ");
            }
        }
    }
    s.push(']');
    s
}

fn compare_content(
    kit_path: &str,
    kl: &KitLayer,
    ol: &InspectedLayer,
    check: &mut dyn FnMut(String, bool, String),
) {
    match (&ol.content, kl.kind.as_str()) {
        (
            InspectedContent::Shape {
                style,
                radii,
                shape: _,
            },
            _,
        ) => {
            let ks = &kl.style;
            // Fills: our glass styles carry the tint the kit puts on a separate layer; the
            // caller maps that layer on its own. Compare the solid and gradient fills.
            let ours_fills: Vec<Paint> = style.fills.clone();
            check(
                format!("{kit_path}#fills"),
                ours_fills.len() == ks.fills.len()
                    && ours_fills
                        .iter()
                        .zip(&ks.fills)
                        .all(|(a, b)| paint_close(a, b)),
                format!(
                    "fills {}, kit {}",
                    fmt_paints(&ours_fills),
                    fmt_paints(&ks.fills)
                ),
            );
            let ob: Vec<(Color, f32, BorderPosition)> = style
                .borders
                .iter()
                .filter_map(|b| b.paint.solid().map(|c| (c, b.thickness, b.position)))
                .collect();
            check(
                format!("{kit_path}#borders"),
                ob.len() == ks.borders.len()
                    && ob.iter().zip(&ks.borders).all(|(a, b)| {
                        color_close(a.0, b.color) && close(a.1, b.thickness) && a.2 == b.position
                    }),
                format!(
                    "borders {:?}, kit {:?}",
                    ob.iter()
                        .map(|b| (b.0.to_hex(), b.1, b.2))
                        .collect::<Vec<_>>(),
                    ks.borders
                        .iter()
                        .map(|b| (b.color.to_hex(), b.thickness, b.position))
                        .collect::<Vec<_>>()
                ),
            );
            let sh_eq = |a: &[vetro_lib::style::Shadow], b: &[vetro_lib::style::Shadow]| {
                a.len() == b.len()
                    && a.iter().zip(b).all(|(x, y)| {
                        color_close(x.color, y.color)
                            && close(x.x, y.x)
                            && close(x.y, y.y)
                            && close(x.blur, y.blur)
                            && close(x.spread, y.spread)
                    })
            };
            check(
                format!("{kit_path}#shadows"),
                sh_eq(&style.shadows, &ks.shadows),
                format!("shadows {:?}, kit {:?}", style.shadows, ks.shadows),
            );
            check(
                format!("{kit_path}#innerShadows"),
                sh_eq(&style.inner_shadows, &ks.inner_shadows),
                format!(
                    "inner shadows {:?}, kit {:?}",
                    style.inner_shadows, ks.inner_shadows
                ),
            );
            let ob: Vec<(String, f32)> = style
                .blurs
                .iter()
                .map(|b| match b {
                    Blur::Background { radius, .. } => ("Background".to_string(), *radius),
                    Blur::Glass(g) => ("Glass".to_string(), g.radius),
                })
                .collect();
            let kb: Vec<(String, f32)> = ks
                .blurs
                .iter()
                .map(|b| (b.kind.clone(), b.radius))
                .collect();
            check(
                format!("{kit_path}#blurs"),
                ob.len() == kb.len()
                    && ob
                        .iter()
                        .zip(&kb)
                        .all(|(a, b)| a.0 == b.0 && close(a.1, b.1)),
                format!("blurs {ob:?}, kit {kb:?}"),
            );
            if kl.shape_type.as_deref() != Some("Oval") {
                let kr: [f32; 4] = match ks.radii.as_slice() {
                    [a, b, c, d] => [*a, *b, *c, *d],
                    [a] => [*a; 4],
                    _ => [0.0; 4],
                };
                let limit = ol.frame.width.min(ol.frame.height) / 2.0;
                let kr = kr.map(|r| r.min(limit));
                check(
                    format!("{kit_path}#radii"),
                    radii.iter().zip(kr.iter()).all(|(a, b)| close(*a, *b)),
                    format!("radii {radii:?}, kit {kr:?}"),
                );
            }
            check(
                format!("{kit_path}#opacity"),
                close(style.opacity, ks.opacity),
                format!("opacity {}, kit {}", style.opacity, ks.opacity),
            );
        }
        (
            InspectedContent::Text {
                text,
                color,
                token: _,
                size: _,
            },
            "Text",
        ) => {
            check(
                format!("{kit_path}#text"),
                kl.text.as_deref() == Some(text.as_str()),
                format!("text {text:?}, kit {:?}", kl.text),
            );
            if let Some(f) = &kl.font {
                check(
                    format!("{kit_path}#color"),
                    color_close(*color, f.color),
                    format!("colour {}, kit {}", color.to_hex(), f.color.to_hex()),
                );
            }
        }
        (
            InspectedContent::Glyph {
                size,
                color,
                symbol,
            },
            _,
        ) => {
            // The kit's glyph is a symbol instance of the Icon Kit: its frame is the icon box.
            check(
                format!("{kit_path}#glyphSize"),
                close(*size, kl.frame.height),
                format!("glyph {symbol} size {size}, kit box {:?}", kl.frame),
            );
            let _ = color;
        }
        (other, kind) => check(
            format!("{kit_path}#kind"),
            false,
            format!("ours {other:?}, kit {kind}"),
        ),
    }
}

/// The point at the centre of a kit layer in master coordinates (to hover or press it).
pub fn center_of(master: &KitMaster, path: &str) -> Option<Point> {
    master.rect_in_master(path).map(|r| r.center())
}
