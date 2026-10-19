//! Headless frame-time budgets: a window of 500 controls; the first frame (layout and full
//! paint), a hover change (one control repaints), a 10 MB document typing. Prints the times
//! and exits non-zero when a budget is exceeded (release builds only are meaningful).

use std::time::{Duration, Instant};

use vetro_lib::amalfi::button::{PushButton, PushVariant};
use vetro_lib::amalfi::toggles::Toggle;
use vetro_lib::core::layout::Alignment;
use vetro_lib::core::{Event, Grid, Mods, Prop, Ui, Value, WindowKind};
use vetro_lib::geom::{Point, Size};
use vetro_lib::text::TextStyle;
use vetro_lib::text::view::TextView;

fn main() {
    let mut failures = Vec::new();
    let mut ui = Ui::new_headless();
    let win = ui.create_window(WindowKind::Normal, Size::new(1600.0, 1000.0));
    let grid = ui.create(Grid {
        columns: 20,
        column_spacing: 8.0,
        row_spacing: 8.0,
        alignment: Alignment::TOP_LEADING,
    });
    let mut first = None;
    for i in 0..500 {
        let v = if i % 2 == 0 {
            ui.create(PushButton::new("Button").variant(PushVariant::Secondary))
        } else {
            ui.create(Toggle::checkbox().label("Check"))
        };
        first.get_or_insert(v);
        ui.add_child(grid, v);
    }
    let _ = ui.set(grid, Prop::Padding, Value::from(16.0));
    ui.set_content(win, grid);
    ui.show_window(win);
    let t = Instant::now();
    let img = ui.snapshot(win, 1.0);
    let first_frame = t.elapsed();
    println!("first frame (500 controls, layout + full raster paint): {first_frame:?}");
    if img.is_none() || first_frame > Duration::from_millis(400) {
        failures.push("first frame over 400 ms");
    }
    // Hover one control: only it is recorded again; the damage is its bounds.
    let Some(b) = first else { return };
    let center = ui.tree.window_rect(b).center();
    let t = Instant::now();
    ui.pointer_move(win, center, Mods::empty());
    let damage = ui.prepare_frame(win);
    let hover = t.elapsed();
    println!(
        "hover change: {hover:?}, damage {damage:?}, control {:?}",
        ui.tree.window_rect(b)
    );
    let t = Instant::now();
    for _ in 0..30 {
        ui.advance(1.0 / 60.0);
        let _ = ui.prepare_frame(win);
    }
    println!("hover fade, per frame: {:?}", t.elapsed() / 30);
    if hover > Duration::from_millis(8) {
        failures.push("hover over 8 ms");
    }
    let _ = Event::Activate;
    let _ = Point::ZERO;
    // Typing in a 10 MB document.
    let mut ui = Ui::new_headless();
    let w = ui.create_window(WindowKind::Normal, Size::new(800.0, 600.0));
    let text = "The quick brown fox jumps over the lazy dog, again and again, every day! ok\n"
        .repeat(130_000);
    let t = Instant::now();
    let v = ui.create(TextView::new(&text, TextStyle::default()));
    let _ = ui.set(v, Prop::MaxWidth, Value::from(f32::INFINITY));
    let _ = ui.set(v, Prop::MaxHeight, Value::from(f32::INFINITY));
    ui.set_content(w, v);
    ui.show_window(w);
    ui.prepare_frame(w);
    let open = t.elapsed();
    ui.set_focus(v, false);
    let t = Instant::now();
    for _ in 0..50 {
        ui.text_input(w, "a");
        ui.prepare_frame(w);
    }
    let per_key = t.elapsed() / 50;
    println!("10 MB document: open {open:?}, typing {per_key:?} per key");
    if per_key > Duration::from_millis(8) {
        failures.push("typing over 8 ms");
    }
    if failures.is_empty() {
        println!("budgets met");
    } else {
        eprintln!("budgets exceeded: {}", failures.join(", "));
        std::process::exit(1);
    }
}
