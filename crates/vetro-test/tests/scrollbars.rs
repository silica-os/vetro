//! Kit parity of `Scrollbars`: bars, the corner and the scroll area.

use vetro_lib::amalfi::scrollbars::{
    BarLook, Hold, ScrollView, Scrollbar, ScrollbarCorner, ScrollbarStyle, scroll_view,
};
use vetro_lib::core::{Axis, LayoutCx, PaintCx, Prop, Proposal, Value, Widget};
use vetro_lib::geom::{Point, Rect, Size};
use vetro_lib::shape::Shape;
use vetro_lib::style::{Border, Style};
use vetro_test::{KitMaster, Scene, Tolerance};

/// Thin bars: edges and 4 pt thumbs.
const BAR: Tolerance = Tolerance {
    mean: 3.0,
    max: 120,
    mask_text: true,
};

fn check(scene: &mut Scene, name: &str, devs: &[&str], tol: Tolerance) -> Option<String> {
    let report = scene.auto_geometry(devs);
    if !report.is_empty() {
        return Some(format!("{name}:\n    {}", report.join("\n    ")));
    }
    scene.pixels(tol).err().map(|e| format!("{name}: {e}"))
}

/// The clipped corners of the area antialias a little differently (a few pixels at 131).
const AREA: Tolerance = Tolerance {
    mean: 3.0,
    max: 140,
    mask_text: true,
};

#[test]
fn bars_and_the_corner_match_the_kit() {
    let mut names = KitMaster::names("Scrollbars/Legacy");
    names.extend(KitMaster::names("Scrollbars/Overlay"));
    names.extend(KitMaster::names("Scrollbars/Corner"));
    assert_eq!(names.len(), 26, "{} masters", names.len());
    let mut failures = Vec::new();
    for name in &names {
        let parts: Vec<&str> = name.split('/').collect();
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        if parts.get(1) == Some(&"Corner") {
            scene.mount(|ui| ui.create(ScrollbarCorner));
        } else {
            let [_, style, axis, state, _app] = parts[..] else {
                continue;
            };
            let vertical = axis == "Vertical";
            let legacy = style == "Legacy";
            let length = match (legacy, vertical) {
                (true, true) => 164.0,
                (true, false) => 248.0,
                (false, true) => 168.0,
                (false, false) => 252.0,
            };
            let mut bar = Scrollbar::new(length);
            bar.look = BarLook {
                axis: if vertical {
                    Axis::Vertical
                } else {
                    Axis::Horizontal
                },
                style: if legacy {
                    ScrollbarStyle::Legacy
                } else {
                    ScrollbarStyle::Overlay
                },
                expansion: if state == "Thin" { 0.0 } else { 1.0 },
                hovered: state == "Hover",
                dragging: state == "Dragging",
                thumb: Some(if vertical { (16.0, 64.0) } else { (20.0, 88.0) }),
            };
            scene.mount(|ui| ui.create(bar));
            if state == "Disabled" {
                scene.set_state("Disabled");
            }
        }
        if let Some(f) = check(&mut scene, name, &[], BAR) {
            failures.push(f);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The demo content of the area masters: a window-coloured frame with eight lines of text.
struct Frame {
    lines: Vec<Rect>,
}

impl Widget for Frame {
    fn kind(&self) -> &'static str {
        "Frame"
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        Size::new(264.0, 180.0)
    }

    fn place(&mut self, cx: &mut LayoutCx<'_>, size: Size) {
        for c in cx.children() {
            let _ = cx.measure(c, Proposal::fixed(size));
            cx.place(c, Rect::from_size(size));
        }
    }

    fn paint(&mut self, cx: &mut PaintCx<'_>) {
        let c = cx.theme().colors;
        let b = cx.bounds();
        for name in ["Clip", "Surface"] {
            cx.rec
                .layer(name, b, Shape::rounded(12.0), Style::fill(c.surface_raised));
        }
        for r in &self.lines {
            cx.rec.layer(
                "Text line",
                Rect::new(b.x + r.x, b.y + r.y, r.width, r.height),
                Shape::rounded(3.0),
                Style::fill(c.line_border),
            );
        }
    }

    fn paint_over(&mut self, cx: &mut PaintCx<'_>) {
        let c = cx.theme().colors;
        cx.rec.layer(
            "Edge",
            cx.bounds(),
            Shape::rounded(12.0),
            Style::default().with_border(Border::inside(c.line_border, 1.0)),
        );
    }

    fn hit(&self, _p: Point, _size: Size) -> bool {
        false
    }
}

/// The content of the scroll view: nothing to see, only a size.
struct Blank(Size);

impl Widget for Blank {
    fn kind(&self) -> &'static str {
        "Blank"
    }

    fn measure(&mut self, _cx: &mut LayoutCx<'_>, _p: Proposal) -> Size {
        self.0
    }

    fn hit(&self, _p: Point, _size: Size) -> bool {
        false
    }
}

#[test]
fn scroll_areas_match_the_kit() {
    let names = KitMaster::names("Scrollbars/Area");
    assert_eq!(names.len(), 8, "{names:?}");
    let mut failures = Vec::new();
    for name in &names {
        let parts: Vec<&str> = name.split('/').collect();
        let [_, _, style, state, _app] = parts[..] else {
            continue;
        };
        let Ok(mut scene) = Scene::load(name) else {
            continue;
        };
        let legacy = style == "Legacy";
        let lines: Vec<Rect> = scene
            .master
            .root
            .children
            .iter()
            .filter(|c| c.name == "Text line")
            .map(|c| c.frame)
            .collect();
        let hold = match (legacy, state) {
            (true, "Hover") => Hold::Thin,
            (false, "Thin") => Hold::Thin,
            (false, "Expanded") => Hold::Expanded,
            _ => Hold::None,
        };
        // The content and the offset that put the thumbs where the kit's nested bars have
        // them: (16, 64) down and (20, 88) across.
        let (content, offset) = if legacy {
            (Size::new(700.0, 420.0), Point::new(56.0, 41.0))
        } else {
            (Size::new(756.0, 472.0), Point::new(60.0, 45.0))
        };
        let sv = scene.mount_sized(Size::new(264.0, 180.0), |ui| {
            let frame = ui.create(Frame { lines });
            let blank = ui.create(Blank(content));
            let v = scroll_view(
                ui,
                if legacy {
                    ScrollbarStyle::Legacy
                } else {
                    ScrollbarStyle::Overlay
                },
                blank,
            );
            ui.with_widget::<ScrollView, _>(v, |s| s.hold = hold);
            ui.add_child(frame, v);
            let _ = ui.set(v, Prop::ScrollOffset, Value::Point(offset));
            // The area's corners clip the bars (the kit's Clip layer).
            let _ = ui.set(frame, Prop::ClipsChildren, Value::Bool(true));
            let _ = ui.set(frame, Prop::CornerRadius, Value::from(12.0));
            frame
        });
        let _ = sv;
        scene.refresh();
        if let Some(f) = check(&mut scene, name, &[], AREA) {
            failures.push(f);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
