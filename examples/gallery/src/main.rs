//! The gallery: every Vetro component in one window, for the dev VM and screenshots.

use vetro_lib::amalfi::button::{PushButton, PushVariant};
use vetro_lib::amalfi::foundation::ControlSize;
use vetro_lib::core::{Alignment, Prop, Signal, Stack, Ui, Value, WindowKind};
use vetro_lib::geom::Size;

fn build(ui: &mut Ui) {
    let win = ui.create_window(WindowKind::Normal, Size::new(720.0, 480.0));
    ui.set_title(win, "Vetro Gallery");
    let column = ui.create(Stack::column(16.0).aligned(Alignment::LEADING));
    let _ = ui.set(column, Prop::Padding, Value::from(24.0));
    let _ = ui.set(column, Prop::MaxWidth, Value::from(f32::INFINITY));
    let _ = ui.set(column, Prop::MaxHeight, Value::from(f32::INFINITY));
    let _ = ui.set(column, Prop::FrameAlignment, Value::Int(0));
    let bg = ui.theme_of(column).colors.surface_window;
    let _ = ui.set(column, Prop::Background, Value::Color(bg));
    for size in [
        ControlSize::Mini,
        ControlSize::Small,
        ControlSize::Regular,
        ControlSize::Large,
    ] {
        let row = ui.create(Stack::row(12.0));
        for variant in [
            PushVariant::Primary,
            PushVariant::Secondary,
            PushVariant::Destructive,
            PushVariant::Borderless,
            PushVariant::Glass,
        ] {
            let b = ui.create(PushButton::new("Button").variant(variant).size(size));
            ui.connect(b, Signal::Activated, move |_, _, _| {
                println!("activated {variant:?} {size:?}");
            });
            ui.add_child(row, b);
        }
        ui.add_child(column, row);
    }
    let quit = ui.create(PushButton::new("Quit").variant(PushVariant::Primary));
    ui.connect(quit, Signal::Activated, |ui, _, _| {
        ui.request(vetro_lib::core::Request::Quit)
    });
    ui.add_child(column, quit);
    ui.set_content(win, column);
    ui.show_window(win);
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    if let Err(e) = vetro_lib::platform::run("dev.silica.VetroGallery", build) {
        eprintln!("vetro-gallery: {e}");
        std::process::exit(1);
    }
}
