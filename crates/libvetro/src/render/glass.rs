//! Nested glass: blur, refraction at the edge and the specular rim of a glass layer inside a
//! surface (controls on glass, the lock clock, lenses), drawn by Skia as a backdrop filter.
//!
//! Window-level and shell glass is drawn by Ossido from `silica-glass-v1` regions; this is
//! the in-surface counterpart, ported from Ossido's material pass
//! (`crates/ossido-render/src/glass.rs` and `shaders/material.frag` in silica-desktop):
//!
//! - the outline's signed distance is Ossido's `continuous_distance` (the corner a
//!   superellipse of exponent 2 + 2.55 × smoothing over r × (1 + smoothing));
//! - refraction: inside a band of `depth × min(w, h) / 2` from the edge, the backdrop is
//!   sampled further inside along the normal, `distortion × band × (1 − t)²` (t: 0 at the
//!   edge, 1 at the band's inner end), with a chromatic split of `chromatic × band` at the rim;
//! - a lens magnifies around the centre by `magnification`.
//!
//! The rest of the material (saturation and brightness, tint, rims, shadow) is the layer's
//! style. Glass passes are batched by the caller: one backdrop per glass layer, never per
//! glyph (silica-desktop#253).

use std::cell::RefCell;

use skia_safe::{
    BlendMode, Canvas, ClipOp, ImageFilter, Paint, PaintStyle, Path, RuntimeEffect,
    gradient::{Colors, Gradient, Interpolation, shaders},
    runtime_effect::RuntimeShaderBuilder,
};

use crate::color::Color;
use crate::geom::Rect;
use crate::style::Glass;

const REFRACTION_SKSL: &str = r#"
uniform shader backdrop;
uniform float4 rect;      // x, y, w, h in the layer's coordinates
uniform float radius;     // corner radius before smoothing
uniform float smoothing;
uniform float band;       // width of the refracting band
uniform float distortion;
uniform float chromatic;
uniform float magnification;

float continuous_distance(float2 p) {
    float2 c = rect.xy + rect.zw * 0.5;
    float2 l = abs(p - c);
    float a = min(radius * (1.0 + smoothing), min(rect.z, rect.w) * 0.5);
    float dx = l.x - (rect.z * 0.5 - a);
    float dy = l.y - (rect.w * 0.5 - a);
    if (a > 0.0 && dx > 0.0 && dy > 0.0) {
        float n = 2.0 + 2.55 * clamp(smoothing, 0.0, 1.0);
        float qx = dx / a;
        float qy = dy / a;
        float f = pow(pow(qx, n) + pow(qy, n), 1.0 / n);
        float g = sqrt(pow(qx, 2.0 * n - 2.0) + pow(qy, 2.0 * n - 2.0)) / pow(f, n - 1.0);
        return (f - 1.0) * a / max(g, 0.001);
    }
    return min(max(dx, dy), 0.0) + length(max(float2(dx, dy), 0.0)) - a;
}

half4 main(float2 p) {
    float2 c = rect.xy + rect.zw * 0.5;
    float2 q = c + (p - c) / max(magnification, 0.01);
    float d = -continuous_distance(p);
    if (band <= 0.0 || d >= band || d < 0.0) {
        return backdrop.eval(q);
    }
    float2 e = float2(0.5, 0.0);
    float2 n = float2(continuous_distance(p + e.xy) - continuous_distance(p - e.xy),
                      continuous_distance(p + e.yx) - continuous_distance(p - e.yx));
    float ln = length(n);
    n = ln > 0.0001 ? n / ln : float2(0.0);
    float t = d / band;
    float shift = distortion * band * (1.0 - t) * (1.0 - t);
    float2 s = q - n * shift;
    float split = chromatic * band * (1.0 - t);
    half4 mid = backdrop.eval(s);
    half r = backdrop.eval(s + n * split).r;
    half b = backdrop.eval(s - n * split).b;
    return half4(r, mid.g, b, mid.a);
}
"#;

thread_local! {
    static EFFECT: RefCell<Option<Option<RuntimeEffect>>> = const { RefCell::new(None) };
}

fn effect() -> Option<RuntimeEffect> {
    EFFECT.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            let made = match RuntimeEffect::make_for_shader(REFRACTION_SKSL, None) {
                Ok(e) => Some(e),
                Err(err) => {
                    tracing::error!("glass refraction shader: {err}");
                    None
                }
            };
            *slot = Some(made);
        }
        slot.clone().flatten()
    })
}

/// The refraction (and lens magnification) image filter of `glass` on a layer in `rect`,
/// applied after `input` (the blur). `None` when the glass does not bend: the caller uses
/// `input` alone.
pub fn refraction_filter(
    rect: Rect,
    glass: &Glass,
    input: Option<ImageFilter>,
) -> Option<ImageFilter> {
    let band = glass.depth.max(0.0) * rect.width.min(rect.height) / 2.0;
    let magnify = glass.magnification.is_finite() && (glass.magnification - 1.0).abs() > 1e-3;
    if (band <= 0.0 || glass.distortion <= 0.0) && !magnify {
        return input;
    }
    let Some(effect) = effect() else {
        return input;
    };
    let mut b = RuntimeShaderBuilder::new(effect);
    let radius = rect.width.min(rect.height) / 2.0;
    let ok = b
        .set_uniform_float("rect", &[rect.x, rect.y, rect.width, rect.height])
        .and_then(|_| b.set_uniform_float("radius", &[radius]))
        .and_then(|_| b.set_uniform_float("smoothing", &[crate::shape::AMALFI_SMOOTHING]))
        .and_then(|_| b.set_uniform_float("band", &[band]))
        .and_then(|_| b.set_uniform_float("distortion", &[glass.distortion.max(0.0)]))
        .and_then(|_| b.set_uniform_float("chromatic", &[glass.chromatic_aberration.max(0.0)]))
        .and_then(|_| b.set_uniform_float("magnification", &[glass.magnification.max(0.01)]));
    if ok.is_err() {
        return input;
    }
    let keep = input.clone();
    skia_safe::image_filters::runtime_shader(&b, "backdrop", input).or(keep)
}

/// The specular highlight on a glass rim: a thin light along the top edge fading down the
/// sides, clipped to the outline (the caller has clipped already).
pub fn paint_specular(canvas: &Canvas, rect: Rect, outline: &Path) {
    let mut p = Paint::default();
    p.set_anti_alias(true);
    p.set_style(PaintStyle::Stroke);
    p.set_stroke_width(1.5);
    p.set_blend_mode(BlendMode::Plus);
    let colors = [
        Color::rgba(255, 255, 255, 46).to_sk4f(),
        Color::TRANSPARENT.to_sk4f(),
    ];
    let positions = [0.0, 1.0];
    let gradient = Gradient::new(
        Colors::new(&colors, Some(&positions), skia_safe::TileMode::Clamp, None),
        Interpolation::default(),
    );
    let shader = shaders::linear_gradient(
        (
            skia_safe::Point::new(rect.x, rect.y),
            skia_safe::Point::new(rect.x, rect.y + rect.height.min(24.0)),
        ),
        &gradient,
        None,
    );
    if let Some(s) = shader {
        p.set_shader(s);
    }
    canvas.save();
    canvas.clip_path(outline, ClipOp::Intersect, true);
    canvas.draw_path(outline, &p);
    canvas.restore();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shader_compiles() {
        assert!(effect().is_some());
    }

    #[test]
    fn flat_glass_has_no_filter() {
        let g = Glass::default();
        assert!(refraction_filter(Rect::new(0.0, 0.0, 10.0, 10.0), &g, None).is_none());
        let bent = Glass {
            depth: 0.3,
            distortion: 0.3,
            ..Glass::default()
        };
        assert!(refraction_filter(Rect::new(0.0, 0.0, 40.0, 20.0), &bent, None).is_some());
    }
}
