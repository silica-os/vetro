//! The kit masters as a typed tree (the silica-design JSON export).

use std::path::PathBuf;

use serde_json::Value;
use vetro_lib::color::Color;
use vetro_lib::geom::{Point, Rect};
use vetro_lib::style::{BorderPosition, Paint, Shadow};

/// The fixtures' directory.
pub fn kit_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("kit")
}

/// A border of a kit layer.
#[derive(Debug, Clone, PartialEq)]
pub struct KitBorder {
    /// Colour.
    pub color: Color,
    /// Thickness.
    pub thickness: f32,
    /// Position.
    pub position: BorderPosition,
    /// Dash pattern.
    pub dash: Vec<f32>,
}

/// A blur of a kit layer.
#[derive(Debug, Clone, PartialEq)]
pub struct KitBlur {
    /// `Background` or `Glass`.
    pub kind: String,
    /// Radius.
    pub radius: f32,
    /// Saturation.
    pub saturation: f32,
    /// Brightness.
    pub brightness: f32,
}

/// The style of a kit layer.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct KitStyle {
    /// Opacity.
    pub opacity: f32,
    /// Fills.
    pub fills: Vec<Paint>,
    /// Fill swatches (token names), parallel to `fills`.
    pub swatches: Vec<Option<String>>,
    /// Borders.
    pub borders: Vec<KitBorder>,
    /// Drop shadows.
    pub shadows: Vec<Shadow>,
    /// Inner shadows.
    pub inner_shadows: Vec<Shadow>,
    /// Blurs.
    pub blurs: Vec<KitBlur>,
    /// Corner radii (empty: square).
    pub radii: Vec<f32>,
}

/// The font of a kit text layer.
#[derive(Debug, Clone, PartialEq)]
pub struct KitFont {
    /// Family.
    pub family: String,
    /// Size.
    pub size: f32,
    /// CSS weight (from Sketch's 0–12 scale: 5 → 400, 6 → 500, 8 → 600, 9 → 700).
    pub weight: u16,
    /// Line height.
    pub line_height: Option<f32>,
    /// Alignment.
    pub align: String,
    /// Colour.
    pub color: Color,
}

/// One layer of a master.
#[derive(Debug, Clone, PartialEq)]
pub struct KitLayer {
    /// Name.
    pub name: String,
    /// Type (`ShapePath`, `Text`, `SymbolInstance`, `Group`, `Image`, `SymbolMaster`).
    pub kind: String,
    /// Frame in the parent's coordinates.
    pub frame: Rect,
    /// Shared style name (`Light/Glass/Thick` …).
    pub shared_style: Option<String>,
    /// Style.
    pub style: KitStyle,
    /// Shape type (`Rectangle`, `Oval`, `Custom`).
    pub shape_type: Option<String>,
    /// Text content.
    pub text: Option<String>,
    /// Font of a text layer.
    pub font: Option<KitFont>,
    /// The nested master of a symbol instance.
    pub master: Option<String>,
    /// The library of a symbol instance (`Silica 1.0 Amalfi Icon Kit` for glyphs).
    pub library: Option<String>,
    /// Hidden.
    pub hidden: bool,
    /// Clips its children.
    pub clips: bool,
    /// Children.
    pub children: Vec<KitLayer>,
    /// `stringValue` overrides of a symbol instance, in order (the text of its labels).
    pub overrides: Vec<String>,
    /// The text comes from an override of the instance: the layer's width is that of the
    /// master's default text, not of this one.
    pub text_overridden: bool,
}

/// A master: the root layer and its PNG.
#[derive(Debug, Clone)]
pub struct KitMaster {
    /// The full name (`Buttons/Push/Primary/Regular/Rest/Light`).
    pub name: String,
    /// The root (its frame at the origin).
    pub root: KitLayer,
    /// The 2× PNG.
    pub png: PathBuf,
}

fn color(v: &Value) -> Option<Color> {
    v.as_str().and_then(Color::parse)
}

fn num(v: &Value) -> f32 {
    v.as_f64().unwrap_or(0.0) as f32
}

fn shadow(v: &Value) -> Option<Shadow> {
    Some(Shadow {
        color: color(&v["color"])?,
        x: num(&v["x"]),
        y: num(&v["y"]),
        blur: num(&v["blur"]),
        spread: num(&v["spread"]),
    })
}

fn paint(v: &Value) -> Option<Paint> {
    if let Some(c) = color(&v["color"]) {
        return Some(Paint::Solid(c));
    }
    let g = &v["gradient"];
    let stops: Vec<(f32, Color)> = g["stops"]
        .as_array()?
        .iter()
        .filter_map(|s| Some((num(s.get(0)?), color(s.get(1)?)?)))
        .collect();
    let pt = |p: &Value| Point::new(num(&p["x"]), num(&p["y"]));
    match g["type"].as_str()? {
        "Linear" => Some(Paint::Linear {
            from: pt(&g["from"]),
            to: pt(&g["to"]),
            stops,
        }),
        "Radial" => Some(Paint::Radial {
            from: pt(&g["from"]),
            to: pt(&g["to"]),
            stops,
        }),
        _ => Some(Paint::Angular { stops }),
    }
}

fn style(v: &Value) -> KitStyle {
    let list = |k: &str| v[k].as_array().cloned().unwrap_or_default();
    let fills = list("fills");
    KitStyle {
        opacity: v["opacity"].as_f64().unwrap_or(1.0) as f32,
        fills: fills.iter().filter_map(paint).collect(),
        swatches: fills
            .iter()
            .filter(|f| paint(f).is_some())
            .map(|f| f["swatch"].as_str().map(str::to_string))
            .collect(),
        borders: list("borders")
            .iter()
            .filter_map(|b| {
                Some(KitBorder {
                    color: color(&b["color"])?,
                    thickness: num(&b["thickness"]),
                    position: match b["position"].as_str() {
                        Some("Outside") => BorderPosition::Outside,
                        Some("Center") => BorderPosition::Center,
                        _ => BorderPosition::Inside,
                    },
                    dash: v["borderOptions"]["dashPattern"]
                        .as_array()
                        .map(|a| a.iter().map(num).collect())
                        .unwrap_or_default(),
                })
            })
            .collect(),
        shadows: list("shadows").iter().filter_map(shadow).collect(),
        inner_shadows: list("innerShadows").iter().filter_map(shadow).collect(),
        blurs: list("blurs")
            .iter()
            .map(|b| KitBlur {
                kind: b["type"].as_str().unwrap_or_default().to_string(),
                radius: num(&b["radius"]),
                saturation: b["saturation"].as_f64().unwrap_or(1.0) as f32,
                brightness: b["brightness"].as_f64().unwrap_or(1.0) as f32,
            })
            .collect(),
        radii: v["corners"]["radii"]
            .as_array()
            .map(|a| a.iter().map(num).collect())
            .unwrap_or_default(),
    }
}

fn effects(rect: Rect, s: &KitStyle) -> Rect {
    let mut out = rect;
    for b in &s.borders {
        let grow = match b.position {
            BorderPosition::Outside => b.thickness,
            BorderPosition::Center => b.thickness / 2.0,
            BorderPosition::Inside => 0.0,
        };
        out = out.union(rect.inflate(grow, grow));
    }
    for sh in &s.shadows {
        let g = sh.blur + sh.spread;
        out = out.union(rect.translate(Point::new(sh.x, sh.y)).inflate(g, g));
    }
    out
}

fn bounds_of(l: &KitLayer, origin: Point, root: bool, depth: usize) -> Option<Rect> {
    if l.hidden || depth > 16 {
        return None;
    }
    let at = if root {
        origin
    } else {
        origin + l.frame.origin()
    };
    let rect = Rect::from_origin_size(at, l.frame.size());
    if let Some(m) = l.master.as_deref()
        && l.library.is_none()
        && let Ok(nested) = KitMaster::load(m)
    {
        let inner = bounds_of(&nested.root, Point::ZERO, true, depth + 1)?;
        let ns = nested.size();
        let sx = if ns.width == 0.0 {
            1.0
        } else {
            l.frame.width / ns.width
        };
        let sy = if ns.height == 0.0 {
            1.0
        } else {
            l.frame.height / ns.height
        };
        let r = Rect::new(
            at.x + inner.x * sx,
            at.y + inner.y * sy,
            inner.width * sx,
            inner.height * sy,
        );
        return Some(effects(r, &l.style));
    }
    let mut children: Option<Rect> = None;
    for c in &l.children {
        if let Some(b) = bounds_of(c, at, false, depth + 1) {
            children = Some(children.map_or(b, |x| x.union(b)));
        }
    }
    if l.clips {
        children = children.map(|c| c.intersect(rect));
    }
    let own = effects(rect, &l.style);
    Some(children.map_or(own, |c| own.union(c)))
}

/// Fills the children of symbol instances of other masters (not the Icon Kit's glyphs) with
/// the nested master's layers, unscaled, so their layers can be compared by path
/// (`Checkbox box/Box`).
fn expand_nested(l: &mut KitLayer, depth: usize) {
    if depth > 8 {
        return;
    }
    for c in &mut l.children {
        if let Some(m) = c.master.as_deref()
            && c.library.is_none()
            && c.children.is_empty()
            && let Ok(nested) = KitMaster::load(m)
        {
            c.children = nested.root.children;
            // The export keeps an override by the object id of the nested layer, which the
            // layers do not carry: the overrides come bottom layer first, the layers top
            // layer first, so the i-th text override goes to the i-th text layer that is not
            // a glyph, counting from the last.
            let mut texts = Vec::new();
            collect_texts(&mut c.children, &mut texts);
            texts.reverse();
            for (value, target) in c.overrides.clone().into_iter().zip(texts) {
                target.text = Some(value);
                target.text_overridden = true;
            }
        }
        expand_nested(c, depth + 1);
    }
}

fn collect_texts<'a>(layers: &'a mut [KitLayer], out: &mut Vec<&'a mut KitLayer>) {
    for l in layers {
        if l.kind == "Text" && !l.name.starts_with("Glyph") {
            out.push(l);
        } else {
            collect_texts(&mut l.children, out);
        }
    }
}

/// The layer at `path` under `from` and its origin relative to `from`'s: names are joined
/// with `/`, and a name may itself contain one (`Palette/Mare`), so every split is tried.
fn resolve<'a>(from: &'a KitLayer, path: &str) -> Option<(&'a KitLayer, Point)> {
    if path.is_empty() {
        return Some((from, Point::ZERO));
    }
    let parts: Vec<&str> = path.split('/').collect();
    for k in 1..=parts.len() {
        let name = parts[..k].join("/");
        let rest = parts[k..].join("/");
        for c in from.children.iter().filter(|c| c.name == name) {
            if let Some((l, o)) = resolve(c, &rest) {
                return Some((l, c.frame.origin() + o));
            }
        }
    }
    None
}

/// Sketch's weight scale to CSS.
pub fn css_weight(sketch: u64) -> u16 {
    match sketch {
        0..=2 => 100,
        3 => 200,
        4 => 300,
        5 => 400,
        6 => 500,
        7 | 8 => 600,
        9 => 700,
        10 => 800,
        _ => 900,
    }
}

fn layer(v: &Value, root: bool) -> KitLayer {
    let f = &v["frame"];
    let frame = if root {
        Rect::new(0.0, 0.0, num(&f["w"]), num(&f["h"]))
    } else {
        Rect::new(num(&f["x"]), num(&f["y"]), num(&f["w"]), num(&f["h"]))
    };
    let font = v.get("font").and_then(|ft| {
        Some(KitFont {
            family: ft["family"].as_str()?.to_string(),
            size: num(&ft["size"]),
            weight: css_weight(ft["weight"].as_u64().unwrap_or(5)),
            line_height: ft["lineHeight"].as_f64().map(|x| x as f32),
            align: ft["align"].as_str().unwrap_or("left").to_string(),
            color: color(&ft["color"]).unwrap_or(Color::BLACK),
        })
    });
    KitLayer {
        name: v["name"].as_str().unwrap_or_default().to_string(),
        kind: v["type"].as_str().unwrap_or_default().to_string(),
        frame,
        shared_style: v["sharedStyle"].as_str().map(str::to_string),
        style: style(&v["style"]),
        shape_type: v["shapeType"].as_str().map(str::to_string),
        text: v["text"].as_str().map(str::to_string),
        font,
        master: v["master"].as_str().map(str::to_string),
        library: v["library"].as_str().map(str::to_string),
        hidden: v["hidden"].as_bool() == Some(true) || v["isVisible"].as_bool() == Some(false),
        clips: v["clipsContent"].as_bool() == Some(true),
        children: v["children"]
            .as_array()
            .map(|a| a.iter().map(|c| layer(c, false)).collect())
            .unwrap_or_default(),
        overrides: v["overrides"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter(|o| o["property"].as_str() == Some("stringValue"))
                    .filter_map(|o| o["value"].as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
        text_overridden: false,
    }
}

impl KitMaster {
    /// Loads a master by name.
    pub fn load(name: &str) -> Result<KitMaster, String> {
        let path = kit_dir().join(format!("{name}.json"));
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("{}: {e} (run cargo xtask sync-kit)", path.display()))?;
        let v: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
        let mut root = layer(&v, true);
        expand_nested(&mut root, 0);
        Ok(KitMaster {
            name: name.to_string(),
            root,
            png: kit_dir().join(format!("{name}@2x.png")),
        })
    }

    /// Whether the master is the dark appearance.
    pub fn dark(&self) -> bool {
        self.name.ends_with("/Dark")
    }

    /// The master's size.
    pub fn size(&self) -> vetro_lib::geom::Size {
        self.root.frame.size()
    }

    /// The layer at `path` (child names joined with `/`).
    pub fn layer(&self, path: &str) -> Option<&KitLayer> {
        resolve(&self.root, path).map(|(l, _)| l)
    }

    /// The frame of the layer at `path` in the master's coordinates.
    pub fn rect_in_master(&self, path: &str) -> Option<Rect> {
        let (l, origin) = resolve(&self.root, path)?;
        Some(Rect::from_origin_size(origin, l.frame.size()))
    }

    /// Every layer with its path and its frame in the master's coordinates.
    pub fn flatten(&self) -> Vec<(String, Rect, &KitLayer)> {
        let mut out = Vec::new();
        fn walk<'a>(
            l: &'a KitLayer,
            prefix: &str,
            origin: Point,
            out: &mut Vec<(String, Rect, &'a KitLayer)>,
        ) {
            for c in &l.children {
                let path = if prefix.is_empty() {
                    c.name.clone()
                } else {
                    format!("{prefix}/{}", c.name)
                };
                let o = origin + c.frame.origin();
                out.push((path.clone(), Rect::from_origin_size(o, c.frame.size()), c));
                walk(c, &path, o, out);
            }
        }
        walk(&self.root, "", Point::ZERO, &mut out);
        out
    }

    /// The area the master paints, effects (outside borders, shadows) and nested masters
    /// included, in master coordinates: the kit PNG covers it at 2×.
    pub fn render_bounds(&self) -> Rect {
        bounds_of(&self.root, Point::ZERO, true, 0).unwrap_or(self.root.frame)
    }

    /// The regions of refracting glass (Regular and Thick: a `Glass` blur that is not the
    /// Clear lens) with the band width (`depth` × half the shorter side), in master
    /// coordinates.
    pub fn glass_bands(&self) -> Vec<(Rect, f32, f32)> {
        let mut out = Vec::new();
        for (_, r, l) in self.flatten() {
            for b in &l.style.blurs {
                if b.kind == "Glass" && b.radius >= 4.0 {
                    let radius = l.style.radii.first().copied().unwrap_or(0.0);
                    let band = 0.35 * r.width.min(r.height) / 2.0 + 1.5;
                    out.push((r, radius, band));
                }
            }
        }
        out
    }

    /// The masters whose names start with `prefix` (sorted).
    pub fn names(prefix: &str) -> Vec<String> {
        let mut out = Vec::new();
        let base = kit_dir();
        let mut stack = vec![base.join(prefix)];
        while let Some(dir) = stack.pop() {
            let Ok(rd) = std::fs::read_dir(&dir) else {
                continue;
            };
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().and_then(|x| x.to_str()) == Some("json")
                    && let Ok(rel) = p.strip_prefix(&base)
                {
                    let s = rel.to_string_lossy();
                    out.push(s.trim_end_matches(".json").to_string());
                }
            }
        }
        out.sort();
        out
    }
}
